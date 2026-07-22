use thiserror::Error;

use crate::core::ovpn_parser::error::OvpnParseError;

use super::connection_state::ConnectionStateReason;
use super::state_reason::StateReason;

/// Errors that can occur during network operations.
///
/// This enum provides specific error types for different failure modes,
/// making it easy to handle errors appropriately in your application.
///
/// # Examples
///
/// ## Basic Error Handling
///
/// ```no_run
/// use nmrs::{NetworkManager, WifiSecurity, ConnectionError, Passphrase};
///
/// # async fn example() -> nmrs::Result<()> {
/// let nm = NetworkManager::new().await?;
///
/// match nm.connect("MyNetwork", None, WifiSecurity::WpaPsk {
///     psk: Passphrase::new("password".to_string())
/// }).await {
///     Ok(_) => println!("Connected!"),
///     Err(ConnectionError::AuthFailed) => {
///         eprintln!("Wrong password");
///     }
///     Err(ConnectionError::NotFound) => {
///         eprintln!("Network not in range");
///     }
///     Err(ConnectionError::Timeout) => {
///         eprintln!("Connection timed out");
///     }
///     Err(e) => eprintln!("Error: {}", e),
/// }
/// # Ok(())
/// # }
/// ```
///
/// ## Retry Logic
///
/// ```no_run
/// use nmrs::{NetworkManager, WifiSecurity, ConnectionError};
///
/// # async fn example() -> nmrs::Result<()> {
/// let nm = NetworkManager::new().await?;
///
/// for attempt in 1..=3 {
///     match nm.connect("MyNetwork", None, WifiSecurity::Open).await {
///         Ok(_) => {
///             println!("Connected on attempt {}", attempt);
///             break;
///         }
///         Err(ConnectionError::Timeout) if attempt < 3 => {
///             println!("Timeout, retrying...");
///             continue;
///         }
///         Err(e) => return Err(e),
///     }
/// }
/// # Ok(())
/// # }
/// ```
#[non_exhaustive]
#[derive(Debug, Error)]
pub enum ConnectionError {
    /// A D-Bus communication error occurred.
    #[error("D-Bus error: {0}")]
    Dbus(#[from] zbus::Error),

    /// The requested network was not found during scan.
    #[error("network not found")]
    NotFound,

    /// Authentication with the access point failed (wrong password, rejected credentials).
    #[error("authentication failed")]
    AuthFailed,

    /// The supplicant (wpa_supplicant) encountered a configuration error.
    #[error("supplicant configuration failed")]
    SupplicantConfigFailed,

    /// The supplicant timed out during authentication.
    #[error("supplicant timeout")]
    SupplicantTimeout,

    /// DHCP failed to obtain an IP address.
    #[error("DHCP failed")]
    DhcpFailed,

    /// The connection timed out waiting for activation.
    #[error("connection timeout")]
    Timeout,

    /// The connection is stuck in an unexpected state.
    #[error("connection stuck in state: {0}")]
    Stuck(String),

    /// No Wi-Fi device was found on the system.
    #[error("no Wi-Fi device found")]
    NoWifiDevice,

    /// No wired (ethernet) device was found on the system.
    #[error("no wired device was found")]
    NoWiredDevice,

    /// Wi-Fi device did not become ready in time.
    #[error("Wi-Fi device not ready")]
    WifiNotReady,

    /// No saved connection exists for the requested network.
    #[error("no saved connection for network")]
    NoSavedConnection,

    /// No saved profile with the given UUID.
    #[error("saved connection '{0}' not found")]
    SavedConnectionNotFound(String),

    /// Saved profile settings are missing required keys or are inconsistent.
    #[error("saved connection malformed: {0}")]
    MalformedSavedConnection(String),

    /// A public builder was missing a required field.
    #[error("incomplete builder: {0}")]
    IncompleteBuilder(String),

    /// NM's connectivity checks are disabled; `check_connectivity` cannot run.
    #[error("connectivity checks are disabled in NetworkManager")]
    ConnectivityCheckDisabled,

    /// An empty password was provided for the requested network.
    #[error("no password was provided")]
    MissingPassword,

    /// A general connection failure with a device state reason code.
    #[error("connection failed: {0}")]
    DeviceFailed(StateReason),

    /// A connection activation failure with a connection state reason.
    #[error("connection activation failed: {0}")]
    ActivationFailed(ConnectionStateReason),

    /// Invalid UTF-8 encountered in SSID.
    #[error("invalid UTF-8 in SSID: {0}")]
    InvalidUtf8(#[from] std::str::Utf8Error),

    /// No VPN connection found.
    #[error("no VPN connection found")]
    NoVpnConnection,

    /// VPN connection not found by UUID or name.
    #[error("VPN connection '{0}' not found")]
    VpnNotFound(String),

    /// Multiple VPN connections share the same display name.
    #[error("multiple VPN connections named '{0}', use UUID")]
    VpnIdAmbiguous(String),

    /// Invalid IP address or CIDR notation.
    #[error("invalid address: {0}")]
    InvalidAddress(String),

    /// Invalid VPN peer configuration.
    #[error("invalid peer configuration: {0}")]
    InvalidPeers(String),

    /// Invalid WireGuard private key format.
    #[error("invalid WireGuard private key: {0}")]
    InvalidPrivateKey(String),

    /// Invalid WireGuard public key format.
    #[error("invalid WireGuard public key: {0}")]
    InvalidPublicKey(String),

    /// Invalid VPN gateway format (should be `host:port`).
    #[error("invalid VPN gateway: {0}")]
    InvalidGateway(String),

    /// VPN connection failed.
    #[error("VPN connection failed: {0}")]
    VpnFailed(String),

    /// Bluetooth device not found.
    #[error("Bluetooth device not found")]
    NoBluetoothDevice,

    /// A D-Bus operation failed, with context about what was being attempted.
    #[error("{context}: {source}")]
    DbusOperation {
        /// Human-readable description of the operation that failed.
        context: String,
        /// The underlying `zbus` error.
        #[source]
        source: zbus::Error,
    },

    /// Secret agent registration with NetworkManager failed.
    #[error("secret agent registration failed: {context}")]
    AgentRegistration {
        /// What went wrong during registration.
        context: String,
    },

    /// Operation requires a registered secret agent but none is active.
    #[error("secret agent not registered")]
    AgentNotRegistered,

    /// A secret agent registration conflicts with another registered agent.
    #[error("secret agent registration conflict")]
    AgentAlreadyRegistered,

    /// An error occurred while parsing a configuration.
    #[error("error while parsing a configuration: {0}")]
    ParseError(OvpnParseError),

    /// Access point with the given SSID and BSSID was not found.
    #[error("access point for SSID '{ssid}' with BSSID '{bssid}' not found")]
    ApBssidNotFound {
        /// SSID that was searched for.
        ssid: String,
        /// BSSID that was searched for.
        bssid: String,
    },

    /// Invalid BSSID format.
    #[error("invalid BSSID format: '{0}' (expected XX:XX:XX:XX:XX:XX)")]
    InvalidBssid(String),

    /// Interface exists but is not a Wi-Fi device.
    #[error("interface '{interface}' is not a Wi-Fi device")]
    NotAWifiDevice {
        /// The interface name that was checked.
        interface: String,
    },

    /// No Wi-Fi device with the given interface name.
    #[error("no Wi-Fi device named '{interface}'")]
    WifiInterfaceNotFound {
        /// The interface name that was searched for.
        interface: String,
    },

    /// A radio is hardware-disabled via rfkill.
    #[error("radio is hardware-disabled (rfkill)")]
    HardwareRadioKilled,

    /// The BlueZ Bluetooth stack is unavailable (not running or no adapters).
    #[error("bluetooth stack unavailable: {0}")]
    BluezUnavailable(String),

    /// Bluetooth adapters exist but toggling them failed.
    #[error("bluetooth toggle failed: {0}")]
    BluetoothToggleFailed(String),

    /// A connection operation is already in progress.
    ///
    /// Returned by [`try_connect`](crate::NetworkManager::try_connect) and
    /// related `try_*` methods when another task is already connecting.
    #[error("a connection operation is already in progress")]
    ConnectionInProgress,

    /// Invalid VLAN ID (must be 1-4094).
    #[error("invalid VLAN ID {id}: must be between 1 and 4094")]
    InvalidVlanId {
        /// The invalid VLAN ID that was provided.
        id: u16,
    },

    /// Invalid input for a configuration field.
    #[error("invalid {field}: {reason}")]
    InvalidInput {
        /// The field that was invalid.
        field: String,
        /// Why the input was invalid.
        reason: String,
    },
}
