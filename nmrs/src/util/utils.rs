//! Utility functions for Wi-Fi data conversion and display.
//!
//! Provides helpers for converting between Wi-Fi data representations:
//! frequency to channel, signal strength to visual bars, SSID bytes to strings.

use log::{trace, warn};
use std::borrow::Cow;
use std::collections::HashMap;
use std::str;
use zbus::{Connection, Proxy};
use zvariant::{OwnedObjectPath, OwnedValue};

use crate::Result;
use crate::api::models::ConnectionStateReason;
use crate::dbus::{
    NMAccessPointProxy, NMActiveConnectionProxy, NMDeviceProxy, NMProxy, NMWirelessProxy,
};
use crate::types::constants::{device_type, frequency, signal_strength, wifi_mode};

/// Converts a Wi-Fi frequency in MHz to a channel number.
///
/// Supports 2.4GHz (channels 1-14), 5GHz, and 6GHz bands.
/// Returns `None` for frequencies outside known Wi-Fi bands.
pub(crate) fn channel_from_freq(mhz: u32) -> Option<u16> {
    match mhz {
        frequency::BAND_2_4_START..=frequency::BAND_2_4_END => {
            Some(((mhz - frequency::BAND_2_4_START) / frequency::CHANNEL_SPACING + 1) as u16)
        }
        frequency::BAND_2_4_CH14 => Some(14),
        frequency::BAND_5_START..=frequency::BAND_5_END => {
            Some(((mhz - 5000) / frequency::CHANNEL_SPACING) as u16)
        }
        frequency::BAND_6_START..=frequency::BAND_6_END => {
            Some(((mhz - frequency::BAND_6_START) / frequency::CHANNEL_SPACING + 1) as u16)
        }
        _ => None,
    }
}

/// Converts signal strength (0-100) to a visual bar representation.
///
/// Returns a 4-character string using Unicode block characters:
/// - 0-24%:   `▂___` (1 bar)
/// - 25-49%:  `▂▄__` (2 bars)
/// - 50-74%:  `▂▄▆_` (3 bars)
/// - 75-100%: `▂▄▆█` (4 bars)
pub(crate) fn bars_from_strength(s: u8) -> &'static str {
    match s {
        0..=signal_strength::BAR_1_MAX => "▂___",
        signal_strength::BAR_2_MIN..=signal_strength::BAR_2_MAX => "▂▄__",
        signal_strength::BAR_3_MIN..=signal_strength::BAR_3_MAX => "▂▄▆_",
        _ => "▂▄▆█",
    }
}

/// Converts a Wi-Fi mode code to a human-readable string.
///
/// Mode codes: 1 = Ad-hoc, 2 = Infrastructure, 3 = Access Point.
pub(crate) fn mode_to_string(m: u32) -> &'static str {
    match m {
        wifi_mode::ADHOC => "Adhoc",
        wifi_mode::INFRA => "Infra",
        wifi_mode::AP => "AP",
        _ => "Unknown",
    }
}

/// Decode SSID bytes, defaulting to `<Hidden Network>` if empty or invalid UTF-8.
/// This is safer than unwrap_or and logs the error.
pub(crate) fn decode_ssid_or_hidden(bytes: &[u8]) -> Cow<'static, str> {
    if bytes.is_empty() {
        return Cow::Borrowed("<Hidden Network>");
    }

    match str::from_utf8(bytes) {
        Ok(s) => Cow::Owned(s.to_owned()),
        Err(e) => {
            warn!("Invalid UTF-8 in SSID during comparison: {e}");
            Cow::Borrowed("<Hidden Network>")
        }
    }
}

/// Decode SSID bytes for comparison purposes, defaulting to empty string if invalid.
pub(crate) fn decode_ssid_or_empty(bytes: &[u8]) -> Cow<'static, str> {
    if bytes.is_empty() {
        return Cow::Borrowed("");
    }

    match str::from_utf8(bytes) {
        Ok(s) => Cow::Owned(s.to_owned()),
        Err(e) => {
            warn!("Invalid UTF-8 in SSID during comparison: {e}");
            Cow::Borrowed("")
        }
    }
}

/// Safely get signal strength with a default value.
/// This is safer than unwrap_or(0) as it makes the default explicit.
pub(crate) fn strength_or_zero(strength: Option<u8>) -> u8 {
    strength.unwrap_or(0)
}

/// This helper iterates through all WiFi access points and calls the provided async function.
///
/// Loops through devices, filters for WiFi, and invokes `func` for each access point.
///
/// For each Wi-Fi device, queries the active access point and, when connected, the interface
/// name and IP addresses once per device (not per AP).
///
/// The function is awaited immediately in the loop to avoid lifetime issues.
///
/// The `+ Send` bound on the returned future lets callers await this helper (and everything
/// that calls it, like `list_networks` and `current_network_info`) inside `tokio::spawn`
/// on a multi-threaded runtime. `NMAccessPointProxy` is already `Send + Sync` and nothing
/// captured in practice holds a non-`Send` value across an `.await`, so the bound is
/// always satisfiable at the call sites in this crate.
pub(crate) async fn for_each_access_point<F, T>(conn: &Connection, mut func: F) -> Result<Vec<T>>
where
    F: for<'a> FnMut(
        &'a NMDeviceProxy<'a>,
        &'a OwnedObjectPath,
        OwnedObjectPath,
        &'a NMAccessPointProxy<'a>,
        (String, Option<String>, Option<String>),
    ) -> std::pin::Pin<
        Box<dyn std::future::Future<Output = Result<Option<T>>> + Send + 'a>,
    >,
{
    let nm = NMProxy::new(conn).await?;
    let devices = nm.get_devices().await?;

    let mut results = Vec::new();

    for dp in devices {
        let d_proxy = NMDeviceProxy::builder(conn)
            .path(dp.clone())?
            .build()
            .await?;

        if d_proxy.device_type().await? != device_type::WIFI {
            continue;
        }

        let wifi = NMWirelessProxy::builder(conn)
            .path(dp.clone())?
            .build()
            .await?;

        let active_ap = wifi.active_access_point().await?;
        let on_device = if active_ap.as_str() != "/" {
            match d_proxy.active_connection().await {
                Ok(ac) if ac.as_str() != "/" => {
                    let (ip4, ip6) = get_ip_addresses_from_active_connection(conn, &ac).await;
                    let iface = d_proxy.interface().await.unwrap_or_default();
                    (iface, ip4, ip6)
                }
                _ => (String::new(), None, None),
            }
        } else {
            (String::new(), None, None)
        };

        for ap_path in wifi.access_points().await? {
            let ap = NMAccessPointProxy::builder(conn)
                .path(ap_path.clone())?
                .build()
                .await?;
            if let Some(result) =
                func(&d_proxy, &active_ap, ap_path, &ap, on_device.clone()).await?
            {
                results.push(result);
            }
        }
    }

    Ok(results)
}

/// Helper to create a NetworkManager D-Bus proxy for a given path and interface.
///
/// Returns a zbus Proxy instance for the specified path and interface.
pub(crate) async fn nm_proxy<'a, P>(
    conn: &'a Connection,
    path: P,
    interface: &'a str,
) -> Result<zbus::Proxy<'a>>
where
    P: TryInto<OwnedObjectPath>,
    P::Error: Into<zbus::Error>,
{
    let owned_path = path.try_into().map_err(Into::into)?;
    Ok(zbus::proxy::Builder::new(conn)
        .destination("org.freedesktop.NetworkManager")?
        .path(owned_path)?
        .interface(interface)?
        .build()
        .await?)
}

/// Helper to create a Settings proxy.
///
/// Creates a proxy for the NetworkManager Settings interface at the standard path.
/// This is used to list, add, and manage saved connection profiles.
pub(crate) async fn settings_proxy(conn: &Connection) -> Result<zbus::Proxy<'_>> {
    nm_proxy(
        conn,
        "/org/freedesktop/NetworkManager/Settings",
        "org.freedesktop.NetworkManager.Settings",
    )
    .await
}

/// Helper to create a Settings.Connection proxy for a specific connection.
///
/// Creates a proxy for a specific saved connection object.
/// This is used to get/update connection settings or delete the connection.
pub(crate) async fn connection_settings_proxy<'a, P>(
    conn: &'a Connection,
    connection_path: P,
) -> Result<zbus::Proxy<'a>>
where
    P: TryInto<OwnedObjectPath>,
    P::Error: Into<zbus::Error>,
{
    nm_proxy(
        conn,
        connection_path,
        "org.freedesktop.NetworkManager.Settings.Connection",
    )
    .await
}

/// Attempts to extract the actual state reason from an active connection.
///
/// NetworkManager only provides reason codes via StateChanged signals, not as
/// a queryable property. This helper attempts to query the connection state
/// to verify it exists, but cannot extract the reason for its current state.
/// Returns Unknown if extraction fails, with appropriate logging.
pub(crate) async fn extract_connection_state_reason(
    conn: &Connection,
    active_conn_path: &OwnedObjectPath,
) -> ConnectionStateReason {
    match NMActiveConnectionProxy::builder(conn).path(active_conn_path.clone()) {
        Ok(builder) => match builder.build().await {
            Ok(ac) => match ac.state().await {
                Ok(state) => {
                    trace!(
                        "Active connection state: {}, but reason not available as property",
                        state
                    );
                    ConnectionStateReason::Unknown
                }
                Err(e) => {
                    warn!("Failed to query active connection state: {}", e);
                    ConnectionStateReason::Unknown
                }
            },
            Err(e) => {
                warn!("Failed to build active connection proxy: {}", e);
                ConnectionStateReason::Unknown
            }
        },
        Err(e) => {
            warn!("Failed to create active connection proxy builder: {}", e);
            ConnectionStateReason::Unknown
        }
    }
}

/// Constructs a BlueZ D-Bus object path from a Bluetooth device address.
///
/// Uses the given adapter name (e.g. `"hci0"`) or defaults to `"hci0"`
/// when `None` is provided.
///
/// # Example
///
/// ```ignore
/// bluez_device_path("00:1A:7D:DA:71:13", None)
/// // => "/org/bluez/hci0/dev_00_1A_7D_DA_71_13"
///
/// bluez_device_path("00:1A:7D:DA:71:13", Some("hci1"))
/// // => "/org/bluez/hci1/dev_00_1A_7D_DA_71_13"
/// ```
pub(crate) async fn bluez_device_path(bdaddr: &str, adapter: Option<&str>) -> String {
    let default_adapter = get_adapter().await;
    let default_adapter = match default_adapter {
        Ok(val) => val.unwrap(),
        Err(err) => format!("Failed to get adapter with error: {}", err),
    };
    let adapter = adapter.unwrap_or(&default_adapter);
    format!("/org/bluez/{adapter}/dev_{}", bdaddr.replace(':', "_"))
}

// Helper to get the device adapter without assuming hci0
async fn get_adapter() -> zbus::Result<Option<String>> {
    let conn = Connection::system().await?;
    let proxy = Proxy::new(
        &conn,
        "org.bluez",
        "/",
        "org.freedesktop.DBus.ObjectManager",
    )
    .await?;
    let objects: HashMap<OwnedObjectPath, HashMap<String, HashMap<String, OwnedValue>>> =
        proxy.call("GetManagedObjects", &()).await?;
    let adapter = objects
        .iter()
        .filter(|(_, interfaces)| interfaces.contains_key("org.bluez.Adapter1"))
        .find_map(|(path, _)| path.split("/").last().map(str::to_string));
    Ok(adapter)
}

/// Macro to convert Result to Option with error logging.
/// Usage: `try_log!(result, "context message")?`
#[macro_export]
macro_rules! try_log {
    ($result:expr, $context:expr) => {
        match $result {
            Ok(value) => value,
            Err(e) => {
                log::warn!("{}: {:?}", $context, e);
                return None;
            }
        }
    };
}

/// Helper to extract IP address from AddressData property.
async fn extract_ip_address(
    conn: &Connection,
    config_path: OwnedObjectPath,
    interface: &str,
) -> Option<String> {
    let proxy = nm_proxy(conn, config_path, interface).await.ok()?;
    let addr_array: Vec<HashMap<String, zvariant::Value>> =
        proxy.get_property("AddressData").await.ok()?;

    addr_array.first().and_then(|addr_map| {
        let address = match addr_map.get("address")? {
            zvariant::Value::Str(s) => s.as_str().to_string(),
            _ => return None,
        };
        let prefix = match addr_map.get("prefix")? {
            zvariant::Value::U32(p) => *p,
            _ => return None,
        };
        Some(format!("{}/{}", address, prefix))
    })
}

/// Extracts IPv4 and IPv6 addresses from an active connection.
///
/// Returns a tuple of (ipv4_address, ipv6_address) where each is an Option<String>
/// in CIDR notation (e.g., "192.168.1.100/24" or "2001:db8::1/64").
///
/// Returns (None, None) if the connection has no IP configuration.
pub(crate) async fn get_ip_addresses_from_active_connection(
    conn: &Connection,
    active_conn_path: &OwnedObjectPath,
) -> (Option<String>, Option<String>) {
    let ac_proxy = match async {
        NMActiveConnectionProxy::builder(conn)
            .path(active_conn_path.clone())
            .ok()?
            .build()
            .await
            .ok()
    }
    .await
    {
        Some(proxy) => proxy,
        None => return (None, None),
    };

    // Get IPv4 address
    let ip4_address = match ac_proxy.ip4_config().await {
        Ok(path) if path.as_str() != "/" => {
            extract_ip_address(conn, path, "org.freedesktop.NetworkManager.IP4Config").await
        }
        _ => None,
    };

    // Get IPv6 address
    let ip6_address = match ac_proxy.ip6_config().await {
        Ok(path) if path.as_str() != "/" => {
            extract_ip_address(conn, path, "org.freedesktop.NetworkManager.IP6Config").await
        }
        _ => None,
    };

    (ip4_address, ip6_address)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_channel_from_freq_2_4ghz() {
        assert_eq!(channel_from_freq(2412), Some(1));
        assert_eq!(channel_from_freq(2437), Some(6));
        assert_eq!(channel_from_freq(2472), Some(13));
        assert_eq!(channel_from_freq(2484), Some(14));
    }

    #[test]
    fn test_channel_from_freq_5ghz() {
        assert_eq!(channel_from_freq(5180), Some(36));
        assert_eq!(channel_from_freq(5220), Some(44));
        assert_eq!(channel_from_freq(5500), Some(100));
    }

    #[test]
    fn test_channel_from_freq_6ghz() {
        assert_eq!(channel_from_freq(5955), Some(1));
        assert_eq!(channel_from_freq(6115), Some(33));
    }

    #[test]
    fn test_channel_from_freq_invalid() {
        assert_eq!(channel_from_freq(1000), None);
        assert_eq!(channel_from_freq(9999), None);
    }

    #[test]
    fn test_bars_from_strength() {
        assert_eq!(bars_from_strength(0), "▂___");
        assert_eq!(bars_from_strength(24), "▂___");
        assert_eq!(bars_from_strength(25), "▂▄__");
        assert_eq!(bars_from_strength(49), "▂▄__");
        assert_eq!(bars_from_strength(50), "▂▄▆_");
        assert_eq!(bars_from_strength(74), "▂▄▆_");
        assert_eq!(bars_from_strength(75), "▂▄▆█");
        assert_eq!(bars_from_strength(100), "▂▄▆█");
    }

    #[test]
    fn test_mode_to_string() {
        assert_eq!(mode_to_string(1), "Adhoc");
        assert_eq!(mode_to_string(2), "Infra");
        assert_eq!(mode_to_string(3), "AP");
        assert_eq!(mode_to_string(99), "Unknown");
    }

    #[test]
    fn test_decode_ssid_or_hidden() {
        assert_eq!(decode_ssid_or_hidden(b"MyNetwork"), "MyNetwork");
        assert_eq!(decode_ssid_or_hidden(b""), "<Hidden Network>");
        assert_eq!(decode_ssid_or_hidden(b"Test_SSID-123"), "Test_SSID-123");
    }

    #[test]
    fn test_decode_ssid_or_empty() {
        assert_eq!(decode_ssid_or_empty(b"MyNetwork"), "MyNetwork");
        assert_eq!(decode_ssid_or_empty(b""), "");
        // Test with valid UTF-8
        assert_eq!(decode_ssid_or_empty("café".as_bytes()), "café");
    }

    #[test]
    fn test_strength_or_zero() {
        assert_eq!(strength_or_zero(Some(75)), 75);
        assert_eq!(strength_or_zero(Some(0)), 0);
        assert_eq!(strength_or_zero(Some(100)), 100);
        assert_eq!(strength_or_zero(None), 0);
    }

    #[tokio::test]
    async fn test_bluez_device_path() {
        assert_eq!(
            bluez_device_path("00:1A:7D:DA:71:13", None).await,
            "/org/bluez/hci0/dev_00_1A_7D_DA_71_13"
        );
        assert_eq!(
            bluez_device_path("00:1A:7D:DA:71:13", Some("hci1")).await,
            "/org/bluez/hci1/dev_00_1A_7D_DA_71_13"
        )
    }
}
