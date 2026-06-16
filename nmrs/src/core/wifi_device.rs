//! Per-Wi-Fi-device enumeration and control.
//!
//! NetworkManager's global `WirelessEnabled` flag toggles every Wi-Fi radio
//! at once. To disable one specific Wi-Fi device while leaving the others
//! online, we set `Device.Autoconnect = false` and then call
//! `Device.Disconnect()` — the kernel hands the radio back to NM but no
//! connection will be re-activated until autoconnect is re-enabled.

use log::{debug, warn};
use zbus::Connection;

use crate::Result;
use crate::api::models::{ConnectionError, WifiDevice};
use crate::core::connection::{disconnect_wifi_and_wait, get_device_by_interface};
use crate::dbus::{NMAccessPointProxy, NMDeviceProxy, NMProxy, NMWirelessProxy};
use crate::types::constants::device_type;
use crate::util::utils::decode_ssid_or_hidden;

/// Lists every managed Wi-Fi device with current MAC, state, and active AP info.
pub(crate) async fn list_wifi_devices(conn: &Connection) -> Result<Vec<WifiDevice>> {
    let nm = NMProxy::new(conn).await?;
    let paths = nm.get_devices().await?;

    let mut out = Vec::new();
    for p in paths {
        let dev = NMDeviceProxy::builder(conn)
            .path(p.clone())?
            .build()
            .await?;
        if dev.device_type().await? != device_type::WIFI {
            continue;
        }

        let interface = dev.interface().await.unwrap_or_default();
        let hw_address = dev
            .hw_address()
            .await
            .unwrap_or_else(|_| String::from("00:00:00:00:00:00"));
        let permanent_hw_address = dev.perm_hw_address().await.ok();
        let driver = dev.driver().await.ok();
        let state = dev.state().await?.into();
        let managed = dev.managed().await.unwrap_or(false);
        let autoconnect = dev.autoconnect().await.unwrap_or(true);

        let wifi = NMWirelessProxy::builder(conn)
            .path(p.clone())?
            .build()
            .await?;
        let active_ap_path = wifi.active_access_point().await.ok();
        let (is_active, active_ssid, active_frequency_mhz) = match active_ap_path {
            Some(ap_path) if ap_path.as_str() != "/" => {
                match NMAccessPointProxy::builder(conn)
                    .path(ap_path)?
                    .build()
                    .await
                {
                    Ok(ap) => {
                        let active_ssid = ap
                            .ssid()
                            .await
                            .ok()
                            .map(|bytes| decode_ssid_or_hidden(&bytes).into_owned());
                        let active_frequency_mhz = ap.frequency().await.ok();
                        (true, active_ssid, active_frequency_mhz)
                    }
                    Err(_) => (true, None, None),
                }
            }
            _ => (false, None, None),
        };

        out.push(WifiDevice {
            path: p,
            interface,
            hw_address,
            permanent_hw_address,
            driver,
            state,
            managed,
            autoconnect,
            is_active,
            active_ssid,
            active_frequency_mhz,
        });
    }

    Ok(out)
}

/// Disable or re-enable a single Wi-Fi device.
///
/// `enabled = false` clears `Device.Autoconnect` and disconnects the device.
/// `enabled = true` re-enables autoconnect; NM will activate any saved
/// connection on its own.
///
/// This is independent of NetworkManager's global `WirelessEnabled` killswitch
/// (controlled via [`crate::NetworkManager::set_wireless_enabled`]).
pub(crate) async fn set_wifi_enabled_for_interface(
    conn: &Connection,
    interface: &str,
    enabled: bool,
) -> Result<()> {
    let path = match get_device_by_interface(conn, interface).await {
        Ok(p) => p,
        Err(ConnectionError::NotFound) => {
            return Err(ConnectionError::WifiInterfaceNotFound {
                interface: interface.to_string(),
            });
        }
        Err(e) => return Err(e),
    };

    let dev = NMDeviceProxy::builder(conn)
        .path(path.clone())?
        .build()
        .await?;
    if dev.device_type().await? != device_type::WIFI {
        return Err(ConnectionError::NotAWifiDevice {
            interface: interface.to_string(),
        });
    }

    debug!("setting Autoconnect={} for {}", enabled, interface);
    if let Err(e) = dev.set_autoconnect(enabled).await {
        warn!("failed to set autoconnect on {}: {}", interface, e);
        return Err(ConnectionError::DbusOperation {
            context: format!("failed to set Autoconnect on {}", interface),
            source: e,
        });
    }

    if !enabled {
        disconnect_wifi_and_wait(conn, &path, None).await?;
    }

    Ok(())
}
