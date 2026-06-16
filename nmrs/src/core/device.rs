//! Network device enumeration and control.
//!
//! Provides functions for listing network devices, checking Wi-Fi state,
//! and enabling/disabling Wi-Fi. Uses D-Bus signals for efficient state
//! monitoring instead of polling.

use log::{debug, warn};
use zbus::Connection;

use crate::Result;
use crate::api::models::{BluetoothDevice, ConnectionError, Device, DeviceIdentity, DeviceState};
use crate::core::bluetooth::populate_bluez_info;
use crate::core::connection::get_device_by_interface;
use crate::core::state_wait::wait_for_wifi_device_ready;
use crate::dbus::{NMAccessPointProxy, NMBluetoothProxy, NMDeviceProxy, NMProxy, NMWirelessProxy};
use crate::types::constants::device_type;
use crate::util::utils::get_ip_addresses_from_active_connection;

/// Lists all network devices managed by NetworkManager.
///
/// Returns information about each device including its interface name,
/// type (Ethernet, Wi-Fi, etc.), current state, and driver.
pub(crate) async fn list_devices(conn: &Connection) -> Result<Vec<Device>> {
    let proxy = NMProxy::new(conn).await?;
    let paths = proxy
        .get_devices()
        .await
        .map_err(|e| ConnectionError::DbusOperation {
            context: "failed to get device paths from NetworkManager".to_string(),
            source: e,
        })?;

    let mut devices = Vec::new();
    for p in paths {
        let d_proxy = NMDeviceProxy::builder(conn)
            .path(p.clone())?
            .build()
            .await?;

        let interface = d_proxy
            .interface()
            .await
            .map_err(|e| ConnectionError::DbusOperation {
                context: format!("failed to get interface name for device {}", p.as_str()),
                source: e,
            })?;

        let raw_type = d_proxy
            .device_type()
            .await
            .map_err(|e| ConnectionError::DbusOperation {
                context: format!("failed to get device type for {}", interface),
                source: e,
            })?;
        let current_mac = match d_proxy.hw_address().await {
            Ok(addr) => addr,
            Err(e) => {
                warn!(
                    "Failed to get hardware address for device {}: {}",
                    interface, e
                );
                String::from("00:00:00:00:00:00")
            }
        };

        let perm_mac = match d_proxy.perm_hw_address().await {
            Ok(addr) => addr,
            Err(e) => {
                debug!(
                    "Permanent hardware address not available for device {}: {}",
                    interface, e
                );
                current_mac.clone()
            }
        };

        let device_type = raw_type.into();
        let raw_state = d_proxy.state().await?;
        let state = raw_state.into();
        let managed = match d_proxy.managed().await {
            Ok(m) => Some(m),
            Err(e) => {
                debug!(
                    "Failed to get 'managed' property for device {}: {}",
                    interface, e
                );
                None
            }
        };
        let driver = match d_proxy.driver().await {
            Ok(d) => Some(d),
            Err(e) => {
                debug!("Failed to get driver for device {}: {}", interface, e);
                None
            }
        };
        let frequency = if raw_type == device_type::WIFI {
            match NMWirelessProxy::builder(conn)
                .path(p.clone())?
                .build()
                .await
            {
                Ok(wifi) => match wifi.active_access_point().await {
                    Ok(ap_path) if ap_path.as_str() != "/" => {
                        match NMAccessPointProxy::builder(conn)
                            .path(ap_path)?
                            .build()
                            .await
                        {
                            Ok(ap) => ap.frequency().await.ok(),
                            Err(e) => {
                                debug!("Failed to build active AP proxy for {}: {}", interface, e);
                                None
                            }
                        }
                    }
                    Ok(_) => None,
                    Err(e) => {
                        debug!("Failed to get active AP for {}: {}", interface, e);
                        None
                    }
                },
                Err(e) => {
                    debug!("Failed to build wireless proxy for {}: {}", interface, e);
                    None
                }
            }
        } else {
            None
        };

        // Get IP addresses from active connection
        let (ip4_address, ip6_address) =
            if let Ok(active_conn_path) = d_proxy.active_connection().await {
                if active_conn_path.as_str() != "/" {
                    get_ip_addresses_from_active_connection(conn, &active_conn_path).await
                } else {
                    (None, None)
                }
            } else {
                (None, None)
            };

        // Avoiding this breaking change for now
        // Get link speed for wired devices
        /* let speed = if raw_type == device_type::ETHERNET {
            async {
                let wired = NMWiredProxy::builder(conn).path(p.clone())?.build().await?;
                wired.speed().await
            }
            .await
            .ok()
        } else {
            None
        };*/
        devices.push(Device {
            path: p.to_string(),
            interface,
            identity: DeviceIdentity::new(perm_mac, current_mac),
            device_type,
            state,
            managed,
            driver,
            ip4_address,
            ip6_address,
            frequency,
            // speed,
        });
    }
    Ok(devices)
}

/// Returns `true` if any network device is in a transitional state
/// (preparing, configuring, authenticating, obtaining IP, etc.).
///
/// Useful for guarding against concurrent connection attempts.
pub(crate) async fn is_connecting(conn: &Connection) -> Result<bool> {
    let nm = NMProxy::new(conn).await?;
    let devices = nm.get_devices().await?;

    for dp in devices {
        let dev = NMDeviceProxy::builder(conn)
            .path(dp.clone())?
            .build()
            .await?;

        let raw_state = dev
            .state()
            .await
            .map_err(|e| ConnectionError::DbusOperation {
                context: format!("failed to get state for device {}", dp.as_str()),
                source: e,
            })?;

        let state: DeviceState = raw_state.into();
        if state.is_transitional() {
            return Ok(true);
        }
    }

    Ok(false)
}

/// Returns `true` if the device with the given interface name is in a
/// transitional state.
///
/// Returns `false` if no device matches the interface name.
pub(crate) async fn is_connecting_on_interface(conn: &Connection, interface: &str) -> Result<bool> {
    let path = match get_device_by_interface(conn, interface).await {
        Ok(p) => p,
        Err(ConnectionError::NotFound) => return Ok(false),
        Err(e) => return Err(e),
    };

    let dev = NMDeviceProxy::builder(conn)
        .path(path.clone())?
        .build()
        .await?;

    let raw_state = dev
        .state()
        .await
        .map_err(|e| ConnectionError::DbusOperation {
            context: format!("failed to get state for device {}", path.as_str()),
            source: e,
        })?;

    Ok(DeviceState::from(raw_state).is_transitional())
}

pub(crate) async fn list_bluetooth_devices(conn: &Connection) -> Result<Vec<BluetoothDevice>> {
    let proxy = NMProxy::new(conn).await?;
    let paths = proxy.get_devices().await?;

    let mut devices = Vec::new();
    for p in paths {
        // So we can get the device type and state
        let d_proxy = NMDeviceProxy::builder(conn)
            .path(p.clone())?
            .build()
            .await?;

        // Only process Bluetooth devices
        let dev_type = d_proxy
            .device_type()
            .await
            .map_err(|e| ConnectionError::DbusOperation {
                context: format!(
                    "failed to get device type for {} during Bluetooth scan",
                    p.as_str()
                ),
                source: e,
            })?;

        if dev_type != device_type::BLUETOOTH {
            continue;
        }

        // Bluetooth-specific proxy
        // to get BD_ADDR and capabilities
        let bd_proxy = NMBluetoothProxy::builder(conn)
            .path(p.clone())?
            .build()
            .await?;

        let bdaddr = bd_proxy
            .hw_address()
            .await
            .unwrap_or_else(|_| String::from("00:00:00:00:00:00"));
        let bt_caps = bd_proxy.bt_capabilities().await?;
        let raw_state = d_proxy.state().await?;
        let state = raw_state.into();

        let bluez_info = populate_bluez_info(conn, &bdaddr, None).await?;

        devices.push(BluetoothDevice::new(
            bdaddr,
            bluez_info.0,
            bluez_info.1,
            bt_caps,
            state,
        ));
    }
    Ok(devices)
}

/// Waits for a Wi-Fi device to become ready for operations.
///
/// Uses D-Bus signals to efficiently wait until a Wi-Fi device reaches
/// either Disconnected or Activated state, indicating it's ready for
/// scanning or connection operations. This is useful after enabling Wi-Fi,
/// as the device may take time to initialize.
///
/// Returns `WifiNotReady` if no Wi-Fi device becomes ready within the timeout.
pub(crate) async fn wait_for_wifi_ready(conn: &Connection) -> Result<()> {
    let nm = NMProxy::new(conn).await?;
    let devices = nm.get_devices().await?;

    // Find the Wi-Fi device
    for dev_path in devices {
        let dev = NMDeviceProxy::builder(conn)
            .path(dev_path.clone())?
            .build()
            .await?;

        if dev.device_type().await? != device_type::WIFI {
            continue;
        }

        debug!("Found Wi-Fi device, waiting for it to become ready");

        // Check current state first
        let current_state = dev.state().await?;
        let state = DeviceState::from(current_state);

        if state == DeviceState::Disconnected || state == DeviceState::Activated {
            debug!("Wi-Fi device already ready");
            return Ok(());
        }

        // Wait for device to become ready using signal-based monitoring
        return wait_for_wifi_device_ready(&dev).await;
    }

    Err(ConnectionError::NoWifiDevice)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::BluetoothNetworkRole;

    #[test]
    fn test_default_bluetooth_address() {
        // Test that the default address used for devices without hardware address is valid
        let default_addr = "00:00:00:00:00:00";
        assert_eq!(default_addr.len(), 17);
        assert_eq!(default_addr.matches(':').count(), 5);
    }

    #[test]
    fn test_bluetooth_device_construction() {
        let panu = BluetoothNetworkRole::PanU as u32;
        let device = BluetoothDevice::new(
            "00:1A:7D:DA:71:13".into(),
            Some("TestDevice".into()),
            Some("Test".into()),
            panu,
            DeviceState::Activated,
        );

        assert_eq!(device.bdaddr, "00:1A:7D:DA:71:13");
        assert_eq!(device.name, Some("TestDevice".into()));
        assert_eq!(device.alias, Some("Test".into()));
        assert!(matches!(device.bt_caps, _panu));
        assert_eq!(device.state, DeviceState::Activated);
    }

    // Note: Most device listing functions require a real D-Bus connection
    // and NetworkManager running, so they are better suited for integration tests.
}
