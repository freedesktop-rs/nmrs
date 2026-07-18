//! Bluetooth device monitoring and current connection status.
//!
//! Provides functions to retrieve information about currently connected
//! Bluetooth devices and their connection state.

use async_trait::async_trait;
use zbus::Connection;

use crate::dbus::{NMBluetoothProxy, NMDeviceProxy, NMProxy};
use crate::monitoring::transport::ActiveTransport;
use crate::try_log;
use crate::types::constants::{device_state, device_type};

pub(crate) struct Bluetooth;

#[async_trait]
impl ActiveTransport for Bluetooth {
    type Output = String;

    async fn current(conn: &Connection) -> Option<Self::Output> {
        current_bluetooth_bdaddr(conn).await
    }
}

/// Returns the Bluetooth MAC address (bdaddr) of the currently connected Bluetooth device.
///
/// Checks all Bluetooth devices for an active connection and returns
/// the MAC address. Returns `None` if not connected to any Bluetooth device.
///
/// Uses the `try_log!` macro to gracefully handle errors without
/// propagating them, since this is often used in non-critical contexts.
///
/// # Example
///
/// ```ignore
/// use nmrs::monitoring::bluetooth::current_bluetooth_bdaddr;
/// use zbus::Connection;
///
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// let conn = Connection::system().await?;
/// if let Some(bdaddr) = current_bluetooth_bdaddr(&conn).await {
///     println!("Connected to Bluetooth device: {}", bdaddr);
/// } else {
///     println!("No Bluetooth device connected");
/// }
/// # Ok(())
/// # }
/// ```
pub(crate) async fn current_bluetooth_bdaddr(conn: &Connection) -> Option<String> {
    let nm = try_log!(NMProxy::new(conn).await, "Failed to create NM proxy");
    let devices = try_log!(nm.get_devices().await, "Failed to get devices");

    for dp in devices {
        let dev_builder = try_log!(
            NMDeviceProxy::builder(conn).path(dp.clone()),
            "Failed to create device proxy builder"
        );
        let dev = try_log!(dev_builder.build().await, "Failed to build device proxy");

        let dev_type = try_log!(dev.device_type().await, "Failed to get device type");
        if dev_type != device_type::BLUETOOTH {
            continue;
        }

        // Check if device is in an active/connected state
        let state = try_log!(dev.state().await, "Failed to get device state");
        // State 100 = Activated (connected)
        if state != device_state::ACTIVATED {
            continue;
        }

        // Get the Bluetooth MAC address from the Bluetooth-specific interface
        let bt_builder = try_log!(
            NMBluetoothProxy::builder(conn).path(dp.clone()),
            "Failed to create Bluetooth proxy builder"
        );
        let bt = try_log!(bt_builder.build().await, "Failed to build Bluetooth proxy");

        if let Ok(bdaddr) = bt.hw_address().await {
            return Some(bdaddr);
        }
    }
    None
}
