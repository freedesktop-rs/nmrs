//! Network device enumeration and control.
//!
//! Provides functions for listing network devices, checking Wi-Fi state,
//! and enabling/disabling Wi-Fi. Uses D-Bus signals for efficient state
//! monitoring instead of polling.

use log::{debug, trace, warn};
use zbus::Connection;

use crate::Result;
use crate::api::models::{
    BluetoothDevice, ConnectionError, Device, DeviceIdentity, DeviceState, WiredDevice,
};
use crate::core::bluetooth::populate_bluez_info;
use crate::core::connection::get_device_by_interface;
use crate::core::state_wait::wait_for_wifi_device_ready;
use crate::dbus::{
    NMAccessPointProxy, NMActiveConnectionProxy, NMBluetoothProxy, NMDeviceProxy, NMProxy,
    NMWiredProxy, NMWirelessProxy,
};
use crate::types::constants::device_type;
use crate::types::device_type_registry;
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
                trace!(
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
                trace!(
                    "Failed to get 'managed' property for device {}: {}",
                    interface, e
                );
                None
            }
        };
        let driver = match d_proxy.driver().await {
            Ok(d) => Some(d),
            Err(e) => {
                trace!("Failed to get driver for device {}: {}", interface, e);
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
                                trace!("Failed to build active AP proxy for {}: {}", interface, e);
                                None
                            }
                        }
                    }
                    Ok(_) => None,
                    Err(e) => {
                        trace!("Failed to get active AP for {}: {}", interface, e);
                        None
                    }
                },
                Err(e) => {
                    trace!("Failed to build wireless proxy for {}: {}", interface, e);
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

        let speed_mbps = if device_type_registry::is_wired(raw_type) {
            async {
                let wired = NMWiredProxy::builder(conn).path(p.clone())?.build().await?;
                wired.speed().await
            }
            .await
            .ok()
        } else {
            None
        };

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
            speed_mbps,
        });
    }
    Ok(devices)
}

/// Lists wired Ethernet devices with Ethernet-specific details.
pub(crate) async fn list_wired_device_details(conn: &Connection) -> Result<Vec<WiredDevice>> {
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

        if !device_type_registry::is_wired(d_proxy.device_type().await?) {
            continue;
        }

        let interface = d_proxy
            .interface()
            .await
            .map_err(|e| ConnectionError::DbusOperation {
                context: format!("failed to get interface name for device {}", p.as_str()),
                source: e,
            })?;
        // Some virtual or unusual devices omit HwAddress; keep the row usable.
        let hw_address = d_proxy
            .hw_address()
            .await
            .unwrap_or_else(|_| String::from("00:00:00:00:00:00"));
        let permanent_hw_address = d_proxy
            .perm_hw_address()
            .await
            .ok()
            .filter(|addr| !addr.is_empty());
        let state = d_proxy.state().await?.into();

        let speed_mbps = async {
            let wired = NMWiredProxy::builder(conn).path(p.clone())?.build().await?;
            wired.speed().await
        }
        .await
        .ok();

        let active_conn_path = d_proxy.active_connection().await.ok();
        let active_connection_id = match active_conn_path.as_ref() {
            Some(path) if path.as_str() != "/" => {
                async {
                    let active = NMActiveConnectionProxy::builder(conn)
                        .path(path.clone())
                        .ok()?
                        .build()
                        .await
                        .ok()?;
                    active.id().await.ok()
                }
                .await
            }
            _ => None,
        };

        let (ip4_address, ip6_address) = match active_conn_path.as_ref() {
            Some(path) if path.as_str() != "/" => {
                get_ip_addresses_from_active_connection(conn, path).await
            }
            _ => (None, None),
        };

        devices.push(WiredDevice {
            path: p.to_string(),
            interface,
            hw_address,
            permanent_hw_address,
            speed_mbps,
            active_connection_id,
            state,
            ip4_address,
            ip6_address,
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
    let mut pending_wifi_device = None;
    let mut found_wifi_device = false;

    // Prefer a ready device. An unmanaged radio can appear before a usable one.
    // A managed radio can temporarily be Unavailable while rfkill is lifted,
    // so keep it as a wait candidate.
    for dev_path in devices {
        let dev = NMDeviceProxy::builder(conn)
            .path(dev_path.clone())?
            .build()
            .await?;

        if dev.device_type().await? != device_type::WIFI {
            continue;
        }

        found_wifi_device = true;

        debug!("Found Wi-Fi device, checking whether it is ready");

        let current_state = dev.state().await?;
        let state = DeviceState::from(current_state);

        if state == DeviceState::Disconnected || state == DeviceState::Activated {
            debug!("Wi-Fi device already ready");
            return Ok(());
        }

        if state != DeviceState::Unmanaged && pending_wifi_device.is_none() {
            pending_wifi_device = Some(dev_path);
        }
    }

    if let Some(dev_path) = pending_wifi_device {
        let dev = NMDeviceProxy::builder(conn).path(dev_path)?.build().await?;
        return wait_for_wifi_device_ready(&dev).await;
    }

    if found_wifi_device {
        Err(ConnectionError::WifiNotReady)
    } else {
        Err(ConnectionError::NoWifiDevice)
    }
}
