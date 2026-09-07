//! NetworkManager Device proxy.

use zbus::{Result, proxy};
use zvariant::OwnedObjectPath;

/// Proxy for NetworkManager device interface.
///
/// Provides access to device properties like interface name, type, state,
/// and the reason for state transitions.
///
/// # Signals
///
/// The `StateChanged` signal is emitted whenever the device state changes.
/// Use `receive_device_state_changed()` to get a stream of state change events:
///
/// ```ignore
/// let mut stream = device_proxy.receive_device_state_changed().await?;
/// while let Some(signal) = stream.next().await {
///     let args = signal.args()?;
///     println!("New state: {}, Old state: {}, Reason: {}",
///              args.new_state, args.old_state, args.reason);
/// }
/// ```
#[proxy(
    interface = "org.freedesktop.NetworkManager.Device",
    default_service = "org.freedesktop.NetworkManager"
)]
pub trait NMDevice {
    /// The network interface name (e.g., "wlan0").
    #[zbus(property)]
    fn interface(&self) -> Result<String>;

    /// Device type as a numeric code (2 = Wi-Fi).
    #[zbus(property)]
    fn device_type(&self) -> Result<u32>;

    /// Current device state (100 = activated, 120 = failed).
    #[zbus(property)]
    fn state(&self) -> Result<u32>;

    /// Whether NetworkManager manages this device.
    #[zbus(property)]
    fn managed(&self) -> Result<bool>;

    /// Set whether NetworkManager manages this device until the daemon restarts.
    #[zbus(property)]
    fn set_managed(&self, value: bool) -> Result<()>;

    /// The kernel driver in use.
    #[zbus(property)]
    fn driver(&self) -> Result<String>;

    /// Current state and reason code for the last state change.
    #[zbus(property)]
    fn state_reason(&self) -> Result<(u32, u32)>;

    /// Hardware (MAC) address of the device.
    #[zbus(property)]
    fn hw_address(&self) -> Result<String>;

    /// Permanent hardware (MAC) address of the device.
    /// Note: This property may not be available on all device types or systems.
    #[zbus(property, name = "PermHwAddress")]
    fn perm_hw_address(&self) -> Result<String>;

    /// Path to the active connection object for this device.
    /// Returns "/" if the device is not connected.
    #[zbus(property)]
    fn active_connection(&self) -> Result<OwnedObjectPath>;

    /// Whether NM automatically activates known connections on this device.
    #[zbus(property)]
    fn autoconnect(&self) -> Result<bool>;

    /// Set the per-device autoconnect flag.
    #[zbus(property)]
    fn set_autoconnect(&self, value: bool) -> Result<()>;

    /// Disconnect the active connection on this device, if any.
    fn disconnect(&self) -> Result<()>;

    /// Signal emitted when device state changes.
    ///
    /// The method is named `device_state_changed` to avoid conflicts with the
    /// `state` property's change stream. Use `receive_device_state_changed()`
    /// to subscribe to this signal.
    ///
    /// Arguments:
    /// - `new_state`: The new device state code
    /// - `old_state`: The previous device state code
    /// - `reason`: The reason code for the state change
    #[zbus(signal, name = "StateChanged")]
    fn device_state_changed(&self, new_state: u32, old_state: u32, reason: u32);
}
