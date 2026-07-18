//! Wi-Fi network scanning and enumeration.
//!
//! Provides functions to trigger Wi-Fi scans and list visible networks
//! with their properties (SSID, signal strength, security type).

use std::collections::HashMap;
use zbus::Connection;

use crate::Result;
use crate::api::models::access_point::{AccessPoint, ApMode, decode_security};
use crate::api::models::{ConnectionError, DeviceState, Network};
use crate::core::connection_settings::has_saved_connection;
use crate::dbus::{NMAccessPointProxy, NMDeviceProxy, NMProxy, NMWirelessProxy};
use crate::monitoring::info::current_ssid;
use crate::types::constants::{device_type, security_flags, wifi_mode};
use crate::util::utils::{
    decode_ssid_or_empty, decode_ssid_or_hidden, get_ip_addresses_from_active_connection,
};

/// Triggers a Wi-Fi scan.
///
/// When `interface` is `None`, scans on every Wi-Fi device.
/// When `Some`, scans only the matching device.
/// The scan runs asynchronously; call [`list_networks`] after a delay.
pub(crate) async fn scan_networks(conn: &Connection, interface: Option<&str>) -> Result<()> {
    let nm = NMProxy::new(conn).await?;
    let devices = nm.get_devices().await?;

    let mut scanned_any = false;
    for dp in devices {
        let d_proxy = NMDeviceProxy::builder(conn)
            .path(dp.clone())?
            .build()
            .await?;

        let dev_type = d_proxy
            .device_type()
            .await
            .map_err(|e| ConnectionError::DbusOperation {
                context: format!(
                    "failed to get device type for {} during Wi-Fi scan",
                    dp.as_str()
                ),
                source: e,
            })?;

        if dev_type != device_type::WIFI {
            continue;
        }

        if let Some(want) = interface {
            let iface = d_proxy.interface().await.unwrap_or_default();
            if iface != want {
                continue;
            }
        } else {
            let state = DeviceState::from(d_proxy.state().await?);
            if matches!(state, DeviceState::Unmanaged | DeviceState::Unavailable) {
                continue;
            }
        }

        let wifi = NMWirelessProxy::builder(conn)
            .path(dp.clone())?
            .build()
            .await?;

        let opts = std::collections::HashMap::new();
        wifi.request_scan(opts)
            .await
            .map_err(|e| ConnectionError::DbusOperation {
                context: format!("failed to request Wi-Fi scan on device {}", dp.as_str()),
                source: e,
            })?;
        scanned_any = true;
    }

    if let Some(want) = interface
        && !scanned_any
    {
        return Err(ConnectionError::WifiInterfaceNotFound {
            interface: want.to_string(),
        });
    }

    Ok(())
}

/// Lists all visible access points, one entry per BSSID.
///
/// When `interface` is `Some`, only APs from that wireless device are returned.
/// When `None`, APs from all wireless devices are returned.
///
/// The returned list is ordered per-device then NM's native order (no explicit
/// strength sort — consumers can sort as needed).
pub(crate) async fn list_access_points(
    conn: &Connection,
    interface: Option<&str>,
) -> Result<Vec<AccessPoint>> {
    let nm = NMProxy::new(conn).await?;
    let devices = nm.get_devices().await?;

    let mut results = Vec::new();

    for dp in devices {
        let dev = NMDeviceProxy::builder(conn)
            .path(dp.clone())?
            .build()
            .await?;

        if dev.device_type().await? != device_type::WIFI {
            continue;
        }

        let iface = dev.interface().await.unwrap_or_default();

        if let Some(target) = interface
            && iface != target
        {
            continue;
        }

        let raw_state = dev.state().await?;
        let device_state: DeviceState = raw_state.into();

        let wifi = NMWirelessProxy::builder(conn)
            .path(dp.clone())?
            .build()
            .await?;

        let active_ap = wifi.active_access_point().await?;
        let is_active_ap = |path: &zvariant::OwnedObjectPath| -> bool {
            active_ap.as_str() != "/" && &active_ap == path
        };

        for ap_path in wifi.access_points().await? {
            let ap = NMAccessPointProxy::builder(conn)
                .path(ap_path.clone())?
                .build()
                .await?;

            let ssid_bytes = ap.ssid().await?;
            let ssid = decode_ssid_or_hidden(&ssid_bytes);
            let bssid = ap.hw_address().await?;
            let flags = ap.flags().await?;
            let wpa = ap.wpa_flags().await?;
            let rsn = ap.rsn_flags().await?;
            let frequency_mhz = ap.frequency().await?;
            let max_bitrate_kbps = ap.max_bitrate().await.unwrap_or(0);
            let strength = ap.strength().await?;
            let mode_raw = ap.mode().await.unwrap_or(0);
            let last_seen_raw = ap.last_seen().await.unwrap_or(-1);
            let last_seen_secs = if last_seen_raw < 0 {
                None
            } else {
                Some(i64::from(last_seen_raw))
            };

            results.push(AccessPoint {
                path: ap_path.clone(),
                device_path: dp.clone(),
                interface: iface.clone(),
                ssid: ssid.to_string(),
                ssid_bytes: ssid_bytes.clone(),
                bssid,
                frequency_mhz,
                max_bitrate_kbps,
                strength,
                mode: ApMode::from(mode_raw),
                security: decode_security(flags, wpa, rsn),
                last_seen_secs,
                is_active: is_active_ap(&ap_path),
                device_state: device_state.clone(),
            });
        }
    }

    Ok(results)
}

/// Lists all visible Wi-Fi networks.
///
/// Enumerates access points from all Wi-Fi devices and returns a deduplicated
/// list of networks. Networks are keyed by (SSID, device interface) and groups
/// APs by SSID, picking the strongest signal as the representative.
///
/// Each returned [`Network`] carries the `best_bssid`, `bssids` list, and
/// `security_features` from the underlying access points.
pub(crate) async fn list_networks(
    conn: &Connection,
    interface: Option<&str>,
) -> Result<Vec<Network>> {
    let aps = list_access_points(conn, interface).await?;

    let mut groups: HashMap<(String, String), Network> = HashMap::new();

    for ap in &aps {
        let key = (ap.interface.clone(), ap.ssid.clone());
        let sec_flags = ap.security;
        let secured = !sec_flags.is_open();
        let is_psk = sec_flags.psk;
        let is_eap = sec_flags.eap || sec_flags.eap_suite_b_192;
        let is_hotspot = ap.mode == ApMode::Ap;

        let (ip4_address, ip6_address) = if ap.is_active {
            active_ip_addresses(conn, &ap.device_path).await
        } else {
            (None, None)
        };

        let net = Network {
            // A scan result is always associated with the interface that
            // discovered its access point, regardless of connection state.
            device: ap.interface.clone(),
            ssid: ap.ssid.clone(),
            bssid: Some(ap.bssid.clone()),
            strength: Some(ap.strength),
            frequency: Some(ap.frequency_mhz),
            secured,
            is_psk,
            is_eap,
            is_hotspot,
            ip4_address,
            ip6_address,
            best_bssid: ap.bssid.clone(),
            bssids: vec![ap.bssid.clone()],
            is_active: ap.is_active,
            known: false,
            security_features: sec_flags,
        };

        groups
            .entry(key)
            .and_modify(|n| n.merge_ap(&net))
            .or_insert(net);
    }

    // Populate `known` by checking saved connections
    for net in groups.values_mut() {
        net.known = has_saved_connection(conn, &net.ssid).await.unwrap_or(false);
    }

    Ok(groups.into_values().collect())
}

/// Helper to get IP addresses from the active connection on a device.
async fn active_ip_addresses(
    conn: &Connection,
    device_path: &zvariant::OwnedObjectPath,
) -> (Option<String>, Option<String>) {
    let builder = match NMDeviceProxy::builder(conn).path(device_path.clone()) {
        Ok(b) => b,
        Err(_) => return (None, None),
    };
    let dev = match builder.build().await {
        Ok(d) => d,
        Err(_) => return (None, None),
    };

    match dev.active_connection().await {
        Ok(ac) if ac.as_str() != "/" => get_ip_addresses_from_active_connection(conn, &ac).await,
        _ => (None, None),
    }
}

/// Returns the full Network object for the currently connected WiFi network.
///
/// Returns `None` if not connected to any WiFi network.
pub(crate) async fn current_network(conn: &Connection) -> Result<Option<Network>> {
    // Get current SSID
    let current_ssid = match current_ssid(conn).await {
        Some(ssid) => ssid,
        None => return Ok(None),
    };

    // Find the WiFi device and active access point
    let nm = NMProxy::new(conn).await?;
    let devices = nm.get_devices().await?;

    for dev_path in devices {
        let dev = NMDeviceProxy::builder(conn)
            .path(dev_path.clone())?
            .build()
            .await?;

        if dev.device_type().await? != device_type::WIFI {
            continue;
        }

        let wifi = NMWirelessProxy::builder(conn)
            .path(dev_path.clone())?
            .build()
            .await?;

        let ap_path = wifi.active_access_point().await?;
        if ap_path.as_str() == "/" {
            continue;
        }

        let ap = NMAccessPointProxy::builder(conn)
            .path(ap_path)?
            .build()
            .await?;

        let ssid_bytes = ap.ssid().await?;
        let ssid = decode_ssid_or_empty(&ssid_bytes);

        if ssid != current_ssid {
            continue;
        }

        // Found the active AP, build Network object
        let strength = ap.strength().await?;
        let bssid = ap.hw_address().await?;
        let flags = ap.flags().await?;
        let wpa = ap.wpa_flags().await?;
        let rsn = ap.rsn_flags().await?;
        let frequency = ap.frequency().await?;

        let secured = (flags & security_flags::WEP) != 0 || wpa != 0 || rsn != 0;
        let is_psk = (wpa & security_flags::PSK) != 0 || (rsn & security_flags::PSK) != 0;
        let is_eap = (wpa & security_flags::EAP) != 0 || (rsn & security_flags::EAP) != 0;
        let is_hotspot = ap.mode().await.unwrap_or(0) == wifi_mode::AP;

        let interface = dev.interface().await.unwrap_or_default();

        // Get IP addresses from active connection
        let (ip4_address, ip6_address) = if let Ok(active_conn_path) = dev.active_connection().await
        {
            if active_conn_path.as_str() != "/" {
                get_ip_addresses_from_active_connection(conn, &active_conn_path).await
            } else {
                (None, None)
            }
        } else {
            (None, None)
        };

        let sec_features = decode_security(flags, wpa, rsn);

        return Ok(Some(Network {
            device: interface,
            ssid: ssid.to_string(),
            bssid: Some(bssid.clone()),
            strength: Some(strength),
            frequency: Some(frequency),
            secured,
            is_psk,
            is_eap,
            is_hotspot,
            ip4_address,
            ip6_address,
            best_bssid: bssid.clone(),
            bssids: vec![bssid],
            is_active: true,
            known: true,
            security_features: sec_features,
        }));
    }

    Ok(None)
}
