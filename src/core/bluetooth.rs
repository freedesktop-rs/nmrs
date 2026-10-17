//! Core Bluetooth connection management logic.
//!
//! This module contains the internal implementation details for managing
//! Bluetooth devices and connections.
//!
//! Similar to other device types, it handles scanning, connecting, and monitoring
//! Bluetooth devices using NetworkManager's D-Bus API.

use log::{debug, trace};
use zbus::Connection;
use zbus::fdo::{ManagedObjects, ObjectManagerProxy};
use zvariant::OwnedObjectPath;
// use futures_timer::Delay;

use crate::ConnectionError;
use crate::builders::bluetooth;
use crate::core::connection_settings::get_saved_connection_path;
use crate::core::state_wait::{wait_for_connection_activation, wait_for_device_disconnect};
use crate::dbus::{BluezDeviceExtProxy, NMDeviceProxy};
use crate::monitoring::bluetooth::Bluetooth;
use crate::monitoring::transport::ActiveTransport;
use crate::types::constants::device_state;
use crate::types::constants::device_type;
use crate::util::validation::validate_bluetooth_address;
use crate::{
    Result,
    dbus::NMProxy,
    models::{BluetoothIdentity, TimeoutConfig},
};

const BLUEZ_DEVICE_INTERFACE: &str = "org.bluez.Device1";

fn bluez_device_path_for_adapter(bdaddr: &str, adapter: &str) -> Result<OwnedObjectPath> {
    OwnedObjectPath::try_from(format!(
        "/org/bluez/{adapter}/dev_{}",
        bdaddr.replace(':', "_")
    ))
    .map_err(|error| ConnectionError::InvalidAddress(format!("Invalid BlueZ device path: {error}")))
}

fn find_bluez_device_path(objects: &ManagedObjects, bdaddr: &str) -> Option<OwnedObjectPath> {
    objects.iter().find_map(|(path, interfaces)| {
        let properties = interfaces.get(BLUEZ_DEVICE_INTERFACE)?;
        let address = <&str>::try_from(properties.get("Address")?).ok()?;
        address.eq_ignore_ascii_case(bdaddr).then(|| path.clone())
    })
}

async fn bluez_managed_objects(conn: &Connection) -> Result<ManagedObjects> {
    let manager = ObjectManagerProxy::builder(conn)
        .destination("org.bluez")
        .map_err(|error| ConnectionError::BluezUnavailable(error.to_string()))?
        .path("/")
        .map_err(|error| ConnectionError::BluezUnavailable(error.to_string()))?
        .build()
        .await
        .map_err(|error| {
            ConnectionError::BluezUnavailable(format!("failed to connect to BlueZ: {error}"))
        })?;

    manager.get_managed_objects().await.map_err(|error| {
        ConnectionError::BluezUnavailable(format!("failed to enumerate BlueZ objects: {error}"))
    })
}

pub(crate) async fn resolve_bluez_device_path(
    conn: &Connection,
    bdaddr: &str,
    adapter: Option<&str>,
) -> Result<OwnedObjectPath> {
    validate_bluetooth_address(bdaddr)?;

    if let Some(adapter) = adapter {
        return bluez_device_path_for_adapter(bdaddr, adapter);
    }

    let objects = bluez_managed_objects(conn).await?;
    find_bluez_device_path(&objects, bdaddr).ok_or(ConnectionError::NoBluetoothDevice)
}

/// Populated Bluetooth device information via BlueZ.
///
/// Given a Bluetooth device address (BDADDR), this function queries BlueZ
/// over D-Bus to retrieve the device's name and alias. It constructs the
/// appropriate D-Bus object path based on the BDADDR format.
///
/// If the given address is not a valid bluetooth device address,
/// the function will return error.
///
/// NetworkManager does not expose Bluetooth device names/aliases directly,
/// hence this additional step is necessary to obtain user-friendly
/// identifiers for Bluetooth devices. (See `BluezDeviceExtProxy` for details.)
pub(crate) async fn populate_bluez_info(
    conn: &Connection,
    bdaddr: &str,
    adapter: Option<&str>,
) -> Result<(Option<String>, Option<String>)> {
    validate_bluetooth_address(bdaddr)?;

    let bluez_path = match resolve_bluez_device_path(conn, bdaddr, adapter).await {
        Ok(path) => path,
        Err(
            error @ (ConnectionError::NoBluetoothDevice | ConnectionError::BluezUnavailable(_)),
        ) => {
            trace!("Could not resolve BlueZ metadata path for {bdaddr}: {error}");
            return Ok((None, None));
        }
        Err(error) => return Err(error),
    };

    match BluezDeviceExtProxy::builder(conn)
        .path(bluez_path)?
        .build()
        .await
    {
        Ok(proxy) => {
            let name = proxy.name().await.ok();
            let alias = proxy.alias().await.ok();
            Ok((name, alias))
        }
        Err(_) => Ok((None, None)),
    }
}

pub(crate) async fn find_bluetooth_device(
    conn: &Connection,
    nm: &NMProxy<'_>,
) -> Result<OwnedObjectPath> {
    let devices = nm.get_devices().await?;

    for dp in devices {
        let dev = NMDeviceProxy::builder(conn)
            .path(dp.clone())?
            .build()
            .await?;
        if dev.device_type().await? == device_type::BLUETOOTH {
            return Ok(dp);
        }
    }
    Err(ConnectionError::NoBluetoothDevice)
}

/// Connects to a Bluetooth device using NetworkManager.
///
/// This function establishes a Bluetooth network connection. The flow:
/// 1. Check if already connected to this device
/// 2. Find the Bluetooth hardware adapter
/// 3. Check for an existing saved connection
/// 4. Either activate the saved connection or create a new one
/// 5. Wait for the connection to reach the activated state
///
/// **Important:** The Bluetooth device must already be paired via BlueZ
/// (using `bluetoothctl` or similar) before NetworkManager can connect to it.
///
/// # Arguments
///
/// * `conn` - D-Bus connection
/// * `name` - Connection name/identifier
/// * `settings` - Bluetooth device settings (bdaddr and type)
///
/// # Example
///
/// ```no_run
/// use nmrs::models::{BluetoothIdentity, BluetoothNetworkRole};
///
/// let settings = BluetoothIdentity::new(
///     "C8:1F:E8:F0:51:57".into(),
///     BluetoothNetworkRole::PanU,
/// ).unwrap();
/// // connect_bluetooth(&conn, "My Phone", &settings).await?;
/// ```
pub(crate) async fn connect_bluetooth(
    conn: &Connection,
    name: &str,
    settings: &BluetoothIdentity,
    timeout_config: Option<TimeoutConfig>,
) -> Result<()> {
    debug!(
        "Connecting to '{}' (Bluetooth) | bdaddr={} type={:?}",
        name, settings.bdaddr, settings.bt_device_type
    );

    let nm = NMProxy::new(conn).await?;

    // Check if already connected to this device
    if let Some(active) = Bluetooth::current(conn).await {
        debug!("Currently connected to Bluetooth device: {active}");
        if active == settings.bdaddr {
            debug!("Already connected to {active}, skipping connect()");
            return Ok(());
        }
    } else {
        trace!("Not currently connected to any Bluetooth device");
    }

    // Find the Bluetooth hardware adapter
    // Note: Unlike WiFi, Bluetooth connections in NetworkManager don't require
    // specifying a specific device. We use "/" to let NetworkManager auto-select.
    let bt_device = find_bluetooth_device(conn, &nm).await?;
    trace!("Using auto-select device path for Bluetooth connection");

    // Check for saved connection
    let saved = get_saved_connection_path(conn, name).await?;

    let specific_object =
        resolve_bluez_device_path(conn, &settings.bdaddr, settings.adapter.as_deref()).await?;

    match saved {
        Some(saved_path) => {
            debug!(
                "Activating saved Bluetooth connection: {}",
                saved_path.as_str()
            );
            let active_conn = nm
                .activate_connection(saved_path, bt_device.clone(), specific_object)
                .await?;

            let timeout = timeout_config.map(|c| c.connection_timeout);
            crate::core::state_wait::wait_for_connection_activation(conn, &active_conn, timeout)
                .await?;
        }
        None => {
            debug!("No saved connection found, creating new Bluetooth connection");
            let opts = crate::api::models::ConnectionOptions {
                autoconnect: false, // Bluetooth typically doesn't auto-connect
                autoconnect_priority: None,
                autoconnect_retries: None,
            };

            let connection_settings = bluetooth::build_bluetooth_connection(name, settings, &opts);

            trace!(
                "Creating Bluetooth connection with settings: {:#?}",
                connection_settings
            );

            let (_, active_conn) = nm
                .add_and_activate_connection(
                    connection_settings,
                    bt_device.clone(),
                    specific_object,
                )
                .await?;

            let timeout = timeout_config.map(|c| c.connection_timeout);
            wait_for_connection_activation(conn, &active_conn, timeout).await?;
        }
    }

    log::info!("Successfully connected to Bluetooth device '{name}'");
    Ok(())
}

/// Disconnects a Bluetooth device and waits for it to reach disconnected state.
///
/// Calls the Disconnect method on the device and waits for the `StateChanged`
/// signal to indicate the device has reached Disconnected or Unavailable state.
pub(crate) async fn disconnect_bluetooth_and_wait(
    conn: &Connection,
    dev_path: &OwnedObjectPath,
    timeout_config: Option<TimeoutConfig>,
) -> Result<()> {
    let dev = NMDeviceProxy::builder(conn)
        .path(dev_path.clone())?
        .build()
        .await?;

    // Check if already disconnected
    let current_state = dev.state().await?;
    if current_state == device_state::DISCONNECTED || current_state == device_state::UNAVAILABLE {
        debug!("Bluetooth device already disconnected");
        return Ok(());
    }

    let raw: zbus::proxy::Proxy = zbus::proxy::Builder::new(conn)
        .destination("org.freedesktop.NetworkManager")?
        .path(dev_path.clone())?
        .interface("org.freedesktop.NetworkManager.Device")?
        .build()
        .await?;

    trace!("Sending disconnect request to Bluetooth device");
    raw.call_method("Disconnect", &()).await?;

    // Wait for disconnect using signal-based monitoring
    let timeout = timeout_config.map(|c| c.disconnect_timeout);
    wait_for_device_disconnect(&dev, timeout).await?;

    // Brief stabilization delay
    // Delay::new(timeouts::stabilization_delay()).await;

    Ok(())
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use zbus::names::OwnedInterfaceName;
    use zvariant::{OwnedValue, Str};

    use super::*;

    fn managed_device(
        path: &str,
        address: &str,
    ) -> (
        OwnedObjectPath,
        HashMap<OwnedInterfaceName, HashMap<String, OwnedValue>>,
    ) {
        let properties =
            HashMap::from([("Address".to_string(), OwnedValue::from(Str::from(address)))]);
        let interfaces = HashMap::from([(
            OwnedInterfaceName::try_from(BLUEZ_DEVICE_INTERFACE).expect("valid interface name"),
            properties,
        )]);
        (
            OwnedObjectPath::try_from(path).expect("valid object path"),
            interfaces,
        )
    }

    #[test]
    fn formats_path_for_explicit_adapter() {
        let path =
            bluez_device_path_for_adapter("00:1A:7D:DA:71:13", "hci1").expect("valid BlueZ path");

        assert_eq!(path.as_str(), "/org/bluez/hci1/dev_00_1A_7D_DA_71_13");
    }

    #[test]
    fn finds_device_path_on_matching_adapter_case_insensitively() {
        let objects = HashMap::from([
            managed_device("/org/bluez/hci0/dev_AA_BB_CC_DD_EE_FF", "AA:BB:CC:DD:EE:FF"),
            managed_device("/org/bluez/hci1/dev_00_1A_7D_DA_71_13", "00:1A:7D:DA:71:13"),
        ]);

        let path =
            find_bluez_device_path(&objects, "00:1a:7d:da:71:13").expect("matching BlueZ device");

        assert_eq!(path.as_str(), "/org/bluez/hci1/dev_00_1A_7D_DA_71_13");
    }

    #[test]
    fn returns_none_when_bluez_device_is_absent() {
        let objects = HashMap::from([managed_device(
            "/org/bluez/hci0/dev_AA_BB_CC_DD_EE_FF",
            "AA:BB:CC:DD:EE:FF",
        )]);

        assert!(find_bluez_device_path(&objects, "00:1A:7D:DA:71:13").is_none());
    }
}
