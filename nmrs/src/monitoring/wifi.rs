//! WiFi connection monitoring and current connection status.
//!
//! Provides functions to retrieve information about currently connected
//! WiFi networks and their connection state.

use zbus::Connection;

use crate::dbus::{NMAccessPointProxy, NMDeviceProxy, NMProxy, NMWirelessProxy};
use crate::monitoring::transport::ActiveTransport;
use crate::try_log;
use crate::types::constants::device_type;
use crate::util::utils::decode_ssid_or_empty;

pub(crate) struct Wifi;

impl ActiveTransport for Wifi {
    type Output = String;

    async fn current(conn: &Connection) -> Option<Self::Output> {
        current_ssid(conn).await
    }
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

/// Returns the SSID and frequency of the current Wi-Fi connection.
///
/// Similar to `current_ssid` but also returns the operating frequency
/// in MHz, useful for determining if connected to 2.4GHz or 5GHz band.
pub(crate) async fn current_connection_info(conn: &Connection) -> Option<(String, Option<u32>)> {
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
            let frequency = ap.frequency().await.ok();
            return Some((ssid.to_string(), frequency));
        }
    }
    None
}
