//! A Rust library for managing network connections via NetworkManager.
//!
//! This crate provides a high-level async API for NetworkManager over D-Bus,
//! enabling easy management of WiFi, Ethernet, and VPN connections on Linux.
//!
//! # Quick Start
//!
//! ## WiFi Connection
//!
//! ```rust
//! use nmrs::{NetworkManager, Passphrase, WifiSecurity};
//!
//! # async fn example() -> nmrs::Result<()> {
//! let nm = NetworkManager::new().await?;
//!
//! // List visible networks (None = all Wi-Fi devices)
//! let networks = nm.list_networks(None).await?;
//! for net in &networks {
//!     println!("{} - Signal: {}%", net.ssid, net.strength.unwrap_or(0));
//! }
//!
//! // Connect to a network on the first Wi-Fi device
//! nm.connect("MyNetwork", None, WifiSecurity::WpaPsk {
//!     psk: Passphrase::new("password123".to_string())
//! }).await?;
//!
//! // Check current connection
//! if let Some(ssid) = nm.current_ssid().await {
//!     println!("Connected to: {}", ssid);
//! }
//! # Ok(())
//! # }
//! ```
//!
//! ## VPN Connection (WireGuard)
//!
//! ```rust
//! use nmrs::{NetworkManager, WireGuardConfig, WireGuardPeer};
//!
//! # async fn example() -> nmrs::Result<()> {
//! let nm = NetworkManager::new().await?;
//!
//! let peer = WireGuardPeer::new(
//!     "peer_public_key",
//!     "vpn.example.com:51820",
//!     vec!["0.0.0.0/0".into()],
//! ).with_persistent_keepalive(25);
//!
//! let config = WireGuardConfig::new(
//!     "MyVPN",
//!     "vpn.example.com:51820",
//!     "your_private_key".to_string(),
//!     "10.0.0.2/24",
//!     vec![peer],
//! ).with_dns(vec!["1.1.1.1".into(), "8.8.8.8".into()]);
//!
//! nm.connect_vpn(config).await?;
//! # Ok(())
//! # }
//! ```
//!
//! ## VPN Connection (OpenVPN)
//!
//! ```rust
//! use nmrs::{NetworkManager, OpenVpnConfig, OpenVpnAuthType, Passphrase};
//!
//! # async fn example() -> nmrs::Result<()> {
//! let nm = NetworkManager::new().await?;
//!
//! let config = OpenVpnConfig::new("CorpVPN", "vpn.example.com", 1194, false)
//!     .with_auth_type(OpenVpnAuthType::PasswordTls)
//!     .with_username("user")
//!     .with_password(Passphrase::new("secret".to_string()))
//!     .with_ca_cert("/etc/openvpn/ca.crt")
//!     .with_client_cert("/etc/openvpn/client.crt")
//!     .with_client_key("/etc/openvpn/client.key");
//!
//! nm.connect_vpn(config).await?;
//!
//! // Or import an .ovpn file directly:
//! nm.import_ovpn("corp.ovpn", Some("user"), Some(Passphrase::new("secret".to_string()))).await?;
//!
//! // List VPN connections
//! let vpns = nm.list_vpn_connections().await?;
//! for vpn in vpns {
//!     println!("{}: {:?} - {:?}", vpn.name, vpn.vpn_type, vpn.state);
//! }
//!
//! nm.disconnect_vpn("CorpVPN").await?;
//! # Ok(())
//! # }
//! ```
//!
//! # Core Concepts
//!
//! ## NetworkManager
//!
//! The main entry point is [`NetworkManager`], which provides methods for:
//! - Listing and managing network devices
//! - Scanning for available Wi-Fi networks
//! - Connecting to networks (Wi-Fi, Ethernet, Bluetooth PAN, VPN)
//! - Managing saved connection profiles
//! - Real-time monitoring of network and device changes
//! - Querying connectivity state and captive-portal URLs
//! - Toggling Wi-Fi/WWAN/Bluetooth radios and airplane mode
//!
//! ## Models
//!
//! The [`models`] module contains all types, enums, and errors. The most
//! commonly used items are also re-exported at the crate root:
//!
//! - [`Device`] / [`DeviceType`] / [`DeviceState`] — network devices and their state
//! - [`Network`] / [`AccessPoint`] / [`NetworkInfo`] — discovered Wi-Fi data
//! - [`WifiDevice`] — per-Wi-Fi-device summary
//! - [`WifiSecurity`] / [`EapOptions`] / [`EapMethod`] / [`Phase2`] — Wi-Fi security
//! - [`ConnectionOptions`] / [`TimeoutConfig`] — connection knobs
//! - [`WireGuardConfig`] / [`WireGuardPeer`] — WireGuard configuration
//! - [`OpenVpnConfig`] / [`OpenVpnAuthType`] / [`OpenVpnProxy`] — OpenVPN configuration
//! - [`VpnConfig`] / [`VpnConfiguration`] — generic VPN dispatch trait/enum
//! - [`VpnConnection`] / [`VpnConnectionInfo`] / [`VpnDetails`] / [`VpnType`] / [`VpnKind`] — saved or active VPN data
//! - [`SavedConnection`] / [`SavedConnectionBrief`] / [`SettingsSummary`] / [`SettingsPatch`] — saved profile management
//! - [`AirplaneModeState`] / [`RadioState`] — radio/rfkill state
//! - [`BluetoothDevice`] / [`BluetoothIdentity`] / [`BluetoothNetworkRole`] — Bluetooth networking
//! - [`ConnectivityState`] / [`ConnectivityReport`] — internet connectivity
//! - [`ConnectionError`] / [`StateReason`] / [`ConnectionStateReason`] — errors
//!
//! [`VpnCredentials`] is still re-exported but is **deprecated**; new code
//! should use [`WireGuardConfig`] together with [`NetworkManager::connect_vpn`].
//!
//! ## Connection Builders
//!
//! The [`builders`] module provides both fluent builder types
//! ([`builders::ConnectionBuilder`], [`builders::WifiConnectionBuilder`],
//! [`builders::WireGuardBuilder`], [`builders::OpenVpnBuilder`]) and
//! free functions (`build_wifi_connection`, `build_ethernet_connection`,
//! `build_wireguard_connection`, `build_openvpn_connection`,
//! `build_bluetooth_connection`, `build_vlan_connection`) for constructing
//! NetworkManager settings dictionaries. Most callers should reach for the
//! higher-level [`NetworkManager`] API; these builders are exposed for
//! advanced use cases that need to assemble the raw settings dictionary
//! before calling a D-Bus method directly via
//! [`add_connection`](crate::NetworkManager::add_connection),
//! [`add_and_activate_connection`](crate::NetworkManager::add_and_activate_connection),
//! [`dbus_connection`](crate::NetworkManager::dbus_connection), or [`raw`](crate::raw).
//!
//! ## Raw D-Bus Access
//!
//! The [`raw`] module re-exports [`zbus`] and [`zvariant`] so builder output
//! types stay compatible with the connection returned by
//! [`NetworkManager::dbus_connection`](crate::NetworkManager::dbus_connection).
//!
//! ## Secret Agent
//!
//! The [`agent`] module lets a consumer register a NetworkManager **secret
//! agent** to handle interactive credential prompts (Wi-Fi passwords, VPN
//! tokens, 802.1X passwords) over D-Bus. See the module docs for the
//! three-stream model and a full example.
//!
//! # Examples
//!
//! ## Connecting to Different Network Types
//!
//! ```rust
//! use nmrs::{NetworkManager, WifiSecurity, EapOptions, EapMethod, Passphrase, Phase2};
//!
//! # async fn example() -> nmrs::Result<()> {
//! let nm = NetworkManager::new().await?;
//!
//! // Open network
//! nm.connect("OpenWiFi", None, WifiSecurity::Open).await?;
//!
//! // WPA-PSK (password-protected)
//! nm.connect("HomeWiFi", None, WifiSecurity::WpaPsk {
//!     psk: Passphrase::new("my_password".to_string())
//! }).await?;
//!
//! // WPA-EAP (Enterprise)
//! let eap_opts = EapOptions::new("user@company.com", Passphrase::new("password".to_string()))
//!     .with_domain_suffix_match("company.com")
//!     .with_system_ca_certs(true)
//!     .with_method(EapMethod::Peap)
//!     .with_phase2(Phase2::Mschapv2);
//!
//! nm.connect("CorpWiFi", None, WifiSecurity::WpaEap {
//!     opts: eap_opts
//! }).await?;
//!
//! // Ethernet (auto-connects when cable is plugged in)
//! nm.connect_wired().await?;
//! # Ok(())
//! # }
//! ```
//!
//! ## Error Handling
//!
//! All operations return [`Result<T>`], which is an alias for `Result<T, ConnectionError>`.
//! The [`ConnectionError`] type provides specific variants for different failure modes:
//!
//! ```rust
//! use nmrs::{NetworkManager, WifiSecurity, ConnectionError, Passphrase};
//!
//! # async fn example() -> nmrs::Result<()> {
//! let nm = NetworkManager::new().await?;
//!
//! match nm.connect("MyNetwork", None, WifiSecurity::WpaPsk {
//!     psk: Passphrase::new("wrong_password".to_string())
//! }).await {
//!     Ok(_) => println!("Connected successfully"),
//!     Err(ConnectionError::AuthFailed) => {
//!         eprintln!("Wrong password!");
//!     }
//!     Err(ConnectionError::NotFound) => {
//!         eprintln!("Network not found or out of range");
//!     }
//!     Err(ConnectionError::Timeout) => {
//!         eprintln!("Connection timed out");
//!     }
//!     Err(ConnectionError::DhcpFailed) => {
//!         eprintln!("Failed to obtain IP address");
//!     }
//!     Err(e) => eprintln!("Error: {}", e),
//! }
//! # Ok(())
//! # }
//! ```
//!
//! ## Device Management
//!
//! ```rust
//! use nmrs::NetworkManager;
//!
//! # async fn example() -> nmrs::Result<()> {
//! let nm = NetworkManager::new().await?;
//!
//! // List all devices
//! let devices = nm.list_devices().await?;
//! for device in devices {
//!     println!("{}: {} ({})",
//!         device.interface,
//!         device.device_type,
//!         device.state
//!     );
//! }
//!
//! // Enable/disable WiFi
//! nm.set_wireless_enabled(false).await?;
//! nm.set_wireless_enabled(true).await?;
//! # Ok(())
//! # }
//! ```
//!
//! ## Real-Time Monitoring
//!
//! Monitor network and device changes in real-time using D-Bus signals:
//!
//! ```rust
//! use nmrs::NetworkManager;
//!
//! # async fn example() -> nmrs::Result<()> {
//! let nm = NetworkManager::new().await?;
//!
//! // Monitor network changes (new networks, signal changes, etc.)
//! let net_handle = nm.monitor_network_changes(|| {
//!     println!("Networks changed! Refresh your UI.");
//! }).await?;
//!
//! // Monitor device state changes (cable plugged in, device activated, etc.)
//! let dev_handle = nm.monitor_device_changes(|| {
//!     println!("Device state changed!");
//! }).await?;
//!
//! // Shut down cleanly when done:
//! net_handle.stop().await?;
//! dev_handle.stop().await?;
//! # Ok(())
//! # }
//! ```
//!
//! # Architecture
//!
//! This crate uses D-Bus signals for efficient state monitoring instead of polling.
//! When connecting to a network, it subscribes to NetworkManager's `StateChanged`
//! signals to detect connection success or failure immediately. This provides:
//!
//! - **Faster response times** - Immediate notification vs polling delay
//! - **Lower CPU usage** - No spinning loops
//! - **Better error messages** - Specific failure reasons from NetworkManager
//!
//! # Logging
//!
//! This crate uses the [`log`](https://docs.rs/log) facade. To see log output,
//! add a logging implementation like `env_logger`:
//!
//! ```no_run,ignore
//! env_logger::init();
//! ```
//!
//! # Feature Flags
//!
//! This crate currently has no optional features. All functionality is enabled by default.
//!
//! # Platform Support
//!
//! This crate is Linux-only and requires:
//! - NetworkManager running and accessible via D-Bus
//! - Appropriate permissions to manage network connections

// Internal modules (not exposed in public API)
mod api;
mod core;
mod dbus;
mod monitoring;
mod types;
mod util;

/// NetworkManager secret agent for credential prompting over D-Bus.
///
/// See the [module documentation](agent) for the three-stream model,
/// lifecycle, and a full example.
pub mod agent;

/// Low-level D-Bus dependencies used by [`builders`](crate::builders).
///
/// Re-exports [`zbus`] and [`zvariant`] so advanced callers can construct
/// proxies against [`NetworkManager::dbus_connection`](crate::NetworkManager::dbus_connection)
/// without pinning their own potentially incompatible versions.
pub mod raw {
    pub use zbus;
    pub use zvariant;
}

// ============================================================================
// Public API
// ============================================================================

/// Connection builders for Wi-Fi, Ethernet, Bluetooth, VLAN, and VPN connections.
///
/// This module provides two complementary APIs for constructing NetworkManager
/// settings dictionaries:
///
/// - **Fluent builder types** — [`ConnectionBuilder`](builders::ConnectionBuilder),
///   [`WifiConnectionBuilder`](builders::WifiConnectionBuilder),
///   [`WireGuardBuilder`](builders::WireGuardBuilder), and
///   [`OpenVpnBuilder`](builders::OpenVpnBuilder), which support method
///   chaining and validation at `.build()`.
/// - **Free functions** — `build_wifi_connection`, `build_ethernet_connection`,
///   `build_wireguard_connection`, `build_openvpn_connection`,
///   `build_bluetooth_connection`, and `build_vlan_connection`, which are
///   handy for one-shot construction.
///
/// Most callers should prefer [`NetworkManager`](crate::NetworkManager)'s
/// high-level methods such as [`connect`](crate::NetworkManager::connect)
/// and [`connect_vpn`](crate::NetworkManager::connect_vpn). Use these
/// builders only when you need to feed a raw settings dictionary to
/// NetworkManager via [`add_connection`](crate::NetworkManager::add_connection),
/// [`add_and_activate_connection`](crate::NetworkManager::add_and_activate_connection),
/// [`dbus_connection`](crate::NetworkManager::dbus_connection), or [`raw`](crate::raw).
///
/// # Example
///
/// ```rust
/// use nmrs::builders::build_wifi_connection;
/// use nmrs::{ConnectionOptions, WifiSecurity};
///
/// let opts = ConnectionOptions::new(true);
/// let settings = build_wifi_connection("MyNetwork", &WifiSecurity::Open, &opts);
/// // `settings` can be passed straight to NetworkManager via D-Bus.
/// ```
pub mod builders {
    pub use crate::api::builders::*;
}

/// Types, enums, and errors for NetworkManager operations.
///
/// This module re-exports every public data type used by the crate.
/// The same types are also re-exported at the crate root for convenience
/// (so `nmrs::Device` and `nmrs::models::Device` refer to the same type),
/// with the exceptions of [`NetworkManager`](crate::NetworkManager) and
/// [`WifiScope`](crate::WifiScope), which live only at the crate root.
///
/// # Core Data Types
/// - [`Device`] — Network device representation
/// - [`Network`] — Wi-Fi network representation (SSID-grouped)
/// - [`AccessPoint`] — Per-BSSID access point details
/// - [`NetworkInfo`] — Detailed network information returned by `show_details`
/// - [`WifiDevice`] — Wi-Fi-specific device summary
/// - [`WiredDevice`] — Ethernet-specific device summary
/// - [`ActiveConnection`] — Typed active connection summary
/// - [`NetworkSnapshot`] — Point-in-time applet state snapshot
/// - [`NetworkEvent`] / [`SettingsChange`] — Refresh-oriented monitoring events
/// - [`BluetoothDevice`] — Discovered Bluetooth peer
/// - [`SavedConnection`] / [`SavedConnectionBrief`] — Saved profile snapshots
/// - [`SettingsSummary`] / [`SettingsPatch`] — Decoded NM settings & update patches
/// - [`VpnConnection`] / [`VpnConnectionInfo`] / [`VpnDetails`] — Active or saved VPN data
///
/// # Configuration
/// - [`WifiSecurity`] — Wi-Fi security types (Open, WPA-PSK, WPA-EAP)
/// - [`EapOptions`] — Enterprise authentication options
/// - [`ConnectionOptions`] — Connection settings (autoconnect, priority, retries)
/// - [`TimeoutConfig`] — Timeout configuration for connection operations
/// - [`WireGuardConfig`] / [`WireGuardPeer`] — WireGuard tunnel configuration
/// - [`OpenVpnConfig`] — OpenVPN plugin configuration
/// - [`VlanConfig`] — VLAN tagging configuration
/// - [`BluetoothIdentity`] — Bluetooth target (bdaddr + role)
/// - [`VpnConfig`] / [`VpnConfiguration`] — Trait & enum used by `connect_vpn`
///
/// # Enums
/// - [`DeviceType`] — Device types (Ethernet, Wi-Fi, Bluetooth, etc.)
/// - [`DeviceState`] — Device states (Disconnected, Activated, etc.)
/// - [`ActiveConnectionState`] — State of an active connection
/// - [`ConnectivityState`] — NM-reported internet connectivity
/// - [`RadioState`] / [`AirplaneModeState`] — Radio/rfkill state
/// - [`ApMode`] — Access point operating mode
/// - [`BluetoothNetworkRole`] — PAN-U / NAP / DUN roles
/// - [`EapMethod`] — EAP authentication methods
/// - [`Phase2`] — Phase 2 authentication for EAP
/// - [`OpenVpnAuthType`] / [`OpenVpnConnectionType`] / [`OpenVpnCompression`] — OpenVPN auth/transport options
/// - [`OpenVpnProxy`] — OpenVPN HTTP/SOCKS proxy configuration
/// - [`VpnKind`] / [`VpnType`] — Plugin vs. kernel WireGuard, plus protocol-specific metadata
/// - [`VpnSecretFlags`] — NM secret flags for VPN credentials
/// - [`WifiKeyMgmt`] / [`WifiSecuritySummary`] / [`SecurityFeatures`] — Decoded Wi-Fi security info
/// - [`ConnectType`] — How a `connect_vpn` call resolved (saved vs. new)
///
/// # Errors
/// - [`ConnectionError`] — Comprehensive error type for all operations
/// - [`StateReason`] — Device state change reasons
/// - [`ConnectionStateReason`] — Connection state change reasons
///
/// # Helper Functions
/// - [`reason_to_error`] — Convert a device state reason to a [`ConnectionError`]
/// - [`connection_state_reason_to_error`] — Convert an active-connection state reason to a [`ConnectionError`]
pub mod models {
    pub use crate::api::models::*;
}

// Re-export commonly used types at crate root for convenience
#[allow(deprecated)]
pub use api::models::{
    AccessPoint, ActiveConnection, ActiveConnectionState, ActiveOtherConnection,
    ActiveVpnConnection, ActiveWifiConnection, ActiveWiredConnection, AirplaneModeState, ApMode,
    AppletNetworkSummary, BluetoothDevice, BluetoothIdentity, BluetoothNetworkRole, ConnectType,
    ConnectionError, ConnectionOptions, ConnectionStateReason, ConnectivityReport,
    ConnectivityState, Device, DeviceState, DeviceType, EapMethod, EapOptions, MonitorHandle,
    Network, NetworkEvent, NetworkEventStream, NetworkInfo, NetworkSnapshot, OpenVpnAuthType,
    OpenVpnCompression, OpenVpnConfig, OpenVpnConnectionType, OpenVpnProxy, Passphrase, Phase2,
    RadioState, SavedConnection, SavedConnectionBrief, SavedVpnSummary, SecurityFeatures,
    SettingsChange, SettingsEventStream, SettingsPatch, SettingsSummary, StateReason,
    TimeoutConfig, VlanConfig, VpnConfig, VpnConfiguration, VpnConnection, VpnConnectionInfo,
    VpnCredentials, VpnDetails, VpnKind, VpnRoute, VpnSecretFlags, VpnType, WifiDevice,
    WifiKeyMgmt, WifiNetworkGroup, WifiSecurity, WifiSecuritySummary, WireGuardConfig,
    WireGuardPeer, WiredDevice, connection_state_reason_to_error, reason_to_error,
};
pub use api::network_manager::NetworkManager;
pub use api::wifi_scope::WifiScope;

/// A specialized `Result` type for network operations.
///
/// This is an alias for `Result<T, ConnectionError>` and is used throughout
/// the crate for all fallible operations.
///
/// # Examples
///
/// ```rust
/// use nmrs::Result;
///
/// async fn connect_to_wifi() -> Result<()> {
///     // Your code here
///     Ok(())
/// }
/// ```
pub type Result<T> = std::result::Result<T, ConnectionError>;
