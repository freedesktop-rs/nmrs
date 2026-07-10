//! Network information and detailed network status.
//!
//! Provides functions to retrieve detailed information about WiFi networks,
//! including security capabilities, signal strength, and connection details.

use log::debug;
use zbus::Connection;

use crate::Result;
use crate::api::models::{ConnectionError, Network, NetworkInfo};
use crate::dbus::{NMAccessPointProxy, NMDeviceProxy, NMProxy, NMWirelessProxy};
use crate::try_log;
use crate::types::constants::{device_type, rate, security_flags};
use crate::util::utils::{
    bars_from_strength, channel_from_freq, decode_ssid_or_empty, for_each_access_point,
    get_ip_addresses_from_active_connection, mode_to_string, strength_or_zero,
};

/// Returns detailed information about a WiFi network.
///
/// Queries the access point for comprehensive details including:
/// - BSSID (MAC address)
/// - Signal strength and visual bars
/// - Frequency and channel
/// - Wi-Fi mode (infrastructure, adhoc, etc.)
/// - Connection speed (actual if connected, max otherwise)
/// - Security capabilities (WEP, WPA, WPA2, PSK, 802.1X)
/// - Current connection status
pub(crate) async fn show_details(conn: &Connection, net: &Network) -> Result<NetworkInfo> {
    let active_ssid = current_ssid(conn).await;
    let is_connected_outer = active_ssid.as_deref() == Some(&net.ssid);
    let target_ssid_outer = net.ssid.clone();
    let target_strength = net.strength;

    // Get IP addresses if connected
    let (ip4_address, ip6_address) = if is_connected_outer {
        // Find the WiFi device and get its active connection
        let nm = NMProxy::new(conn).await?;
        let devices = nm.get_devices().await?;

        let mut ip_addrs = (None, None);
        for dev_path in devices {
            let dev = match async {
                NMDeviceProxy::builder(conn)
                    .path(dev_path.clone())
                    .ok()?
                    .build()
                    .await
                    .ok()
            }
            .await
            {
                Some(d) => d,
                None => continue,
            };

            if dev.device_type().await.ok() == Some(device_type::WIFI)
                && let Ok(active_conn_path) = dev.active_connection().await
                && active_conn_path.as_str() != "/"
            {
                ip_addrs = get_ip_addresses_from_active_connection(conn, &active_conn_path).await;
                break;
            }
        }
        ip_addrs
    } else {
        (None, None)
    };

    let results = for_each_access_point(conn, |_dev, _active_ap, _ap_path, ap, _on_device| {
        let target_ssid = target_ssid_outer.clone();
        let is_connected = is_connected_outer;
        Box::pin(async move {
            let ssid_bytes = ap.ssid().await?;
            if decode_ssid_or_empty(&ssid_bytes) != target_ssid {
                return Ok(None);
            }

            let strength = strength_or_zero(target_strength);
            let bssid = ap.hw_address().await?;
            let flags = ap.flags().await?;
            let wpa_flags = ap.wpa_flags().await?;
            let rsn_flags = ap.rsn_flags().await?;
            let freq = match ap.frequency().await {
                Ok(f) => Some(f),
                Err(e) => {
                    debug!("Failed to get frequency for AP: {}", e);
                    None
                }
            };
            let max_br = match ap.max_bitrate().await {
                Ok(br) => Some(br),
                Err(e) => {
                    debug!("Failed to get max bitrate for AP: {}", e);
                    None
                }
            };
            let mode_raw = match ap.mode().await {
                Ok(m) => Some(m),
                Err(e) => {
                    debug!("Failed to get mode for AP: {}", e);
                    None
                }
            };

            let wep = (flags & security_flags::WEP) != 0 && wpa_flags == 0 && rsn_flags == 0;
            let wpa1 = wpa_flags != 0;
            let wpa2_or_3 = rsn_flags != 0;
            let psk = ((wpa_flags | rsn_flags) & security_flags::PSK) != 0;
            let eap = ((wpa_flags | rsn_flags) & security_flags::EAP) != 0;

            let mut parts = Vec::new();
            if wep {
                parts.push("WEP");
            }
            if wpa1 {
                parts.push("WPA");
            }
            if wpa2_or_3 {
                parts.push("WPA2/WPA3");
            }
            if psk {
                parts.push("PSK");
            }
            if eap {
                parts.push("802.1X");
            }

            let security = if parts.is_empty() {
                "Open".to_string()
            } else {
                parts.join(" + ")
            };

            let status = if is_connected {
                "Connected".to_string()
            } else {
                "Disconnected".to_string()
            };

            let channel = freq.and_then(channel_from_freq);
            let rate_mbps = max_br.map(|kbit| kbit / rate::KBIT_TO_MBPS);
            let bars = bars_from_strength(strength).to_string();
            let mode = mode_raw
                .map(mode_to_string)
                .unwrap_or("Unknown")
                .to_string();

            Ok(Some(NetworkInfo {
                ssid: target_ssid,
                bssid,
                strength,
                freq,
                channel,
                mode,
                rate_mbps,
                bars,
                security,
                status,
                ip4_address: None,
                ip6_address: None,
            }))
        })
    })
    .await?;

    let mut info = results
        .into_iter()
        .next()
        .ok_or(ConnectionError::NotFound)?;

    // Set IP addresses
    info.ip4_address = ip4_address;
    info.ip6_address = ip6_address;

    Ok(info)
}

/// Returns the SSID of the currently connected Wi-Fi network.
///
/// Checks all Wi-Fi devices for an active access point and returns
/// its SSID. Returns `None` if not connected to any Wi-Fi network.
///
/// Uses the `try_log!` macro to gracefully handle errors without
/// propagating them, since this is often used in non-critical contexts.
pub(crate) async fn current_ssid(conn: &Connection) -> Option<String> {
    let nm = try_log!(NMProxy::new(conn).await, "Failed to create NM proxy");
    let devices = try_log!(nm.get_devices().await, "Failed to get devices");

    for dp in devices {
        let dev_builder = try_log!(
            NMDeviceProxy::builder(conn).path(dp.clone()),
            "Failed to create device proxy builder"
        );
        let dev = try_log!(dev_builder.build().await, "Failed to build device proxy");

        let dev_type = try_log!(dev.device_type().await, "Failed to get device type");
        if dev_type != device_type::WIFI {
            continue;
        }

        let wifi_builder = try_log!(
            NMWirelessProxy::builder(conn).path(dp.clone()),
            "Failed to create wireless proxy builder"
        );
        let wifi = try_log!(wifi_builder.build().await, "Failed to build wireless proxy");

        if let Ok(active_ap) = wifi.active_access_point().await
            && active_ap.as_str() != "/"
        {
            let ap_builder = try_log!(
                NMAccessPointProxy::builder(conn).path(active_ap),
                "Failed to create access point proxy builder"
            );
            let ap = try_log!(
                ap_builder.build().await,
                "Failed to build access point proxy"
            );
            let ssid_bytes = try_log!(ap.ssid().await, "Failed to get SSID bytes");
            let ssid = decode_ssid_or_empty(&ssid_bytes);
            return Some(ssid.to_string());
        }
    }
    None
}
