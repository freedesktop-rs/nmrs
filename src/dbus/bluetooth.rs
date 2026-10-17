//! Bluetooth Device Proxy
//!
//! This module provides D-Bus proxy interfaces for interacting with Bluetooth
//! devices through NetworkManager and BlueZ.

use zbus::Result;
use zbus::proxy;

/// Proxy for Bluetooth devices
///
/// Provides access to Bluetooth-specific properties and methods through
/// NetworkManager's D-Bus interface.
///
/// # Example
///
/// ```ignore
/// use nmrs::dbus::NMBluetoothProxy;
/// use zbus::Connection;
///
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// let conn = Connection::system().await?;
/// let proxy = NMBluetoothProxy::builder(&conn)
///     .path("/org/freedesktop/NetworkManager/Devices/1")?
///     .build()
///     .await?;
///
/// let bdaddr = proxy.bd_address().await?;
/// println!("Bluetooth address: {}", bdaddr);
/// # Ok(())
/// # }
/// ```
#[proxy(
    interface = "org.freedesktop.NetworkManager.Device.Bluetooth",
    default_service = "org.freedesktop.NetworkManager"
)]
pub trait NMBluetooth {
    /// Bluetooth MAC address of the device.
    ///
    /// Returns the BD_ADDR (Bluetooth Device Address) in the format
    /// "XX:XX:XX:XX:XX:XX" where each XX is a hexadecimal value.
    #[zbus(property)]
    fn hw_address(&self) -> Result<String>;

    /// Bluetooth capabilities of the device (either DUN or NAP).
    ///
    /// Returns a bitmask where:
    /// - 0x01 = DUN (Dial-Up Networking)
    /// - 0x02 = NAP (Network Access Point)
    ///
    /// A device may support multiple capabilities.
    #[zbus(property)]
    fn bt_capabilities(&self) -> Result<u32>;
}

/// Extension trait for Bluetooth device information via BlueZ.
///
/// Provides convenient methods to access Bluetooth-specific properties
/// that are otherwise not exposed by NetworkManager. This interfaces directly
/// with BlueZ, the Linux Bluetooth stack.
///
/// # Example
///
/// ```ignore
/// use nmrs::dbus::BluezDeviceExtProxy;
/// use zbus::Connection;
///
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// let conn = Connection::system().await?;
/// let proxy = BluezDeviceExtProxy::builder(&conn)
///     .path("/org/bluez/hci0/dev_00_1A_7D_DA_71_13")?
///     .build()
///     .await?;
///
/// let name = proxy.name().await?;
/// let alias = proxy.alias().await?;
/// println!("Device: {} ({})", alias, name);
/// # Ok(())
/// # }
/// ```
#[proxy(interface = "org.bluez.Device1", default_service = "org.bluez")]
pub trait BluezDeviceExt {
    /// Returns the name of the Bluetooth device.
    ///
    /// This is typically the manufacturer-assigned name of the device.
    #[zbus(property)]
    fn name(&self) -> Result<String>;

    /// Returns the alias of the Bluetooth device.
    ///
    /// This is typically a user-friendly name that can be customized.
    /// If no alias is set, this usually returns the same value as `name()`.
    #[zbus(property)]
    fn alias(&self) -> Result<String>;
}
