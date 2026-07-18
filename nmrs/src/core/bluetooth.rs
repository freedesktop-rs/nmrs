//! Core Bluetooth connection management logic.
//!
//! This module contains the internal implementation details for managing
//! Bluetooth devices and connections.
//!
//! Similar to other device types, it handles scanning, connecting, and monitoring
//! Bluetooth devices using NetworkManager's D-Bus API.

use log::{debug, trace};
use zbus::Connection;
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
use crate::util::utils::bluez_device_path;
use crate::util::validation::validate_bluetooth_address;
use crate::{
    Result,
    dbus::NMProxy,
    models::{BluetoothIdentity, TimeoutConfig},
};

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

    let bluez_path = bluez_device_path(bdaddr, adapter);

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

    let specific_object = OwnedObjectPath::try_from(bluez_device_path(
        &settings.bdaddr,
        settings.adapter.as_deref(),
    ))
    .map_err(|e| ConnectionError::InvalidAddress(format!("Invalid BlueZ path: {e}")))?;

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
