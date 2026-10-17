//! NetworkManager Active Connection proxy.

use zbus::{Result, proxy};
use zvariant::OwnedObjectPath;

/// Proxy for active connection interface.
///
/// Provides access to the state of an active (in-progress or established)
/// network connection. Use this to monitor connection activation progress
/// and detect failures with specific reason codes.
///
/// # Signals
///
/// The `StateChanged` signal is emitted when the connection activation state
/// changes. Use `receive_activation_state_changed()` to get a stream of state changes:
///
/// ```ignore
/// let mut stream = active_conn_proxy.receive_activation_state_changed().await?;
/// while let Some(signal) = stream.next().await {
///     let args = signal.args()?;
///     match args.state {
///         2 => println!("Connection activated!"),
///         4 => println!("Connection failed: reason {}", args.reason),
///         _ => {}
///     }
/// }
/// ```
#[proxy(
    interface = "org.freedesktop.NetworkManager.Connection.Active",
    default_service = "org.freedesktop.NetworkManager"
)]
pub trait NMActiveConnection {
    /// Current state of the active connection.
    ///
    /// Values:
    /// - 0: Unknown
    /// - 1: Activating
    /// - 2: Activated
    /// - 3: Deactivating
    /// - 4: Deactivated
    #[zbus(property)]
    fn state(&self) -> Result<u32>;

    /// Path to the connection settings used for this connection.
    #[zbus(property)]
    fn connection(&self) -> Result<OwnedObjectPath>;

    /// Path to the specific object (e.g., access point) used for this connection.
    #[zbus(property)]
    fn specific_object(&self) -> Result<OwnedObjectPath>;

    /// Connection identifier (usually the SSID for Wi-Fi).
    #[zbus(property)]
    fn id(&self) -> Result<String>;

    /// Connection UUID.
    #[zbus(property)]
    fn uuid(&self) -> Result<String>;

    /// Paths to devices using this connection.
    #[zbus(property)]
    fn devices(&self) -> Result<Vec<OwnedObjectPath>>;

    /// Path to the IPv4 configuration object.
    /// Returns "/" if no IPv4 configuration is available.
    #[zbus(property)]
    fn ip4_config(&self) -> Result<OwnedObjectPath>;

    /// Path to the IPv6 configuration object.
    /// Returns "/" if no IPv6 configuration is available.
    #[zbus(property)]
    fn ip6_config(&self) -> Result<OwnedObjectPath>;

    /// Signal emitted when the connection activation state changes.
    ///
    /// The method is named `activation_state_changed` to avoid conflicts with
    /// the `state` property's change stream. Use `receive_activation_state_changed()`
    /// to subscribe to this signal.
    ///
    /// Arguments:
    /// - `state`: The new connection state (see `ActiveConnectionState`)
    /// - `reason`: The reason for the state change (see `ConnectionStateReason`)
    #[zbus(signal, name = "StateChanged")]
    fn activation_state_changed(&self, state: u32, reason: u32);
}
