use std::collections::HashMap;
use std::sync::Arc;

use tokio::sync::{Mutex, watch};
use zbus::Connection;
use zvariant::OwnedValue;

use crate::Result;
use crate::api::models::access_point::AccessPoint;
use crate::api::models::{
    AirplaneModeState, Device, Network, NetworkInfo, RadioState, SavedConnection,
    SavedConnectionBrief, SettingsPatch, WifiDevice, WifiSecurity,
};
use crate::api::wifi_scope::WifiScope;
use crate::core::airplane;
use crate::core::bluetooth::connect_bluetooth;
use crate::core::connection::{
    connect, connect_to_bssid, connect_wired, disconnect, forget_by_name_and_type,
    get_device_by_interface, is_connected,
};
use crate::core::connection_settings::{
    get_saved_connection_path, get_saved_connection_uuid, has_saved_connection,
};
use crate::core::device::{
    is_connecting, list_bluetooth_devices, list_devices, wait_for_wifi_ready,
};
use crate::core::saved_connection as saved_profiles;
use crate::core::scan::{current_network, list_access_points, list_networks, scan_networks};
use crate::core::vpn::{
    active_vpn_connections, connect_vpn, connect_vpn_by_id, connect_vpn_by_uuid, disconnect_vpn,
    disconnect_vpn_by_uuid, get_vpn_info, list_vpn_connections,
};
use crate::core::wifi_device::{list_wifi_devices, set_wifi_enabled_for_interface};
use crate::models::{
    BluetoothDevice, BluetoothIdentity, VpnConfig, VpnConfiguration, VpnConnection,
    VpnConnectionInfo,
};
use crate::monitoring::device as device_monitor;
use crate::monitoring::info::show_details;
use crate::monitoring::network as network_monitor;
use crate::monitoring::wifi::{current_connection_info, current_ssid};
use crate::types::constants::device_type;

/// High-level interface to NetworkManager over D-Bus.
///
/// This is the main entry point for managing network connections on Linux systems.
/// It provides a safe, async Rust API over NetworkManager's D-Bus interface.
///
/// # Creating an Instance
///
/// ```no_run
/// use nmrs::NetworkManager;
///
/// # async fn example() -> nmrs::Result<()> {
/// let nm = NetworkManager::new().await?;
/// # Ok(())
/// # }
/// ```
///
/// # Capabilities
///
/// - **Device Management**: List devices, enable/disable WiFi
/// - **Network Scanning**: Discover available WiFi networks
/// - **Connection Management**: Connect to WiFi, Ethernet networks
/// - **Profile Management**: Save, retrieve, and delete connection profiles
/// - **Real-Time Monitoring**: Subscribe to network and device state changes
///
/// # Examples
///
/// ## Basic WiFi Connection
///
/// ```no_run
/// use nmrs::{NetworkManager, WifiSecurity};
///
/// # async fn example() -> nmrs::Result<()> {
/// let nm = NetworkManager::new().await?;
///
/// // Scan and list networks (None = all Wi-Fi devices)
/// let networks = nm.list_networks(None).await?;
/// for net in &networks {
///     println!("{}: {}%", net.ssid, net.strength.unwrap_or(0));
/// }
///
/// // Connect to a network on the first Wi-Fi device
/// nm.connect("MyNetwork", None, WifiSecurity::WpaPsk {
///     psk: "password".into()
/// }).await?;
/// # Ok(())
/// # }
/// ```
///
/// ## Device Management
///
/// ```no_run
/// use nmrs::NetworkManager;
///
/// # async fn example() -> nmrs::Result<()> {
/// let nm = NetworkManager::new().await?;
///
/// // List all network devices
/// let devices = nm.list_devices().await?;
///
/// // Control WiFi
/// nm.set_wireless_enabled(false).await?;  // Disable WiFi
/// nm.set_wireless_enabled(true).await?;   // Enable WiFi
///
/// // Check airplane mode
/// let state = nm.airplane_mode_state().await?;
/// println!("Airplane mode: {}", state.is_airplane_mode());
/// # Ok(())
/// # }
/// ```
///
/// ## Connection Profiles
///
/// ```no_run
/// use nmrs::NetworkManager;
///
/// # async fn example() -> nmrs::Result<()> {
/// let nm = NetworkManager::new().await?;
///
/// // Check for saved connection
/// if nm.has_saved_connection("MyNetwork").await? {
///     println!("Connection profile exists");
///     
///     // Delete it
///     nm.forget("MyNetwork").await?;
/// }
/// # Ok(())
/// # }
/// ```
///
/// # Thread Safety
///
/// `NetworkManager` is `Clone` and can be safely shared across async tasks.
/// Each clone shares the same underlying D-Bus connection.
///
/// # Concurrency
///
/// All connection-mutating operations (`connect`, `connect_to_bssid`,
/// `connect_wired`, `connect_bluetooth`, `connect_vpn*`, `disconnect`)
/// are serialized by an internal mutex. If one task is already inside a
/// connect call, a second call will wait for the first to finish before
/// proceeding.
///
/// For callers that want to fail fast instead of waiting, use the
/// `try_connect` / `try_connect_to_bssid` family: these use
/// [`try_lock`](tokio::sync::Mutex::try_lock) on the mutex (returning
/// [`ConnectionInProgress`](crate::ConnectionError::ConnectionInProgress)
/// if another task holds it), then check [`is_connecting`](Self::is_connecting)
/// before proceeding. Plain `connect` / `disconnect` use
/// [`lock`](tokio::sync::Mutex::lock) and will wait until the mutex is
/// free, which may take up to the configured connection timeout.
///
/// The mutex is shared across all clones of a `NetworkManager` instance.
/// Operations through a [`WifiScope`](crate::WifiScope) obtained from
/// [`wifi()`](Self::wifi) share the same mutex as the parent.
#[derive(Debug, Clone)]
pub struct NetworkManager {
    conn: Connection,
    timeout_config: crate::api::models::TimeoutConfig,
    connect_guard: Arc<Mutex<()>>,
}

impl NetworkManager {
    /// Creates a new `NetworkManager` connected to the system D-Bus with default timeout configuration.
    ///
    /// Uses default timeouts of 30 seconds for connection and 10 seconds for disconnection.
    /// To customize timeouts, use [`with_config()`](Self::with_config) instead.
    pub async fn new() -> Result<Self> {
        let conn = Connection::system().await?;
        Ok(Self {
            conn,
            timeout_config: crate::api::models::TimeoutConfig::default(),
            connect_guard: Arc::new(Mutex::new(())),
        })
    }

    /// Creates a new `NetworkManager` with custom timeout configuration.
    ///
    /// This allows you to customize how long NetworkManager will wait for
    /// various operations to complete before timing out.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use nmrs::{NetworkManager, TimeoutConfig};
    /// use std::time::Duration;
    ///
    /// # async fn example() -> nmrs::Result<()> {
    /// // Configure longer timeouts for slow networks
    /// let config = TimeoutConfig::new()
    ///     .with_connection_timeout(Duration::from_secs(60))
    ///     .with_disconnect_timeout(Duration::from_secs(20));
    ///
    /// let nm = NetworkManager::with_config(config).await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn with_config(timeout_config: crate::api::models::TimeoutConfig) -> Result<Self> {
        let conn = Connection::system().await?;
        Ok(Self {
            conn,
            timeout_config,
            connect_guard: Arc::new(Mutex::new(())),
        })
    }

    /// Returns the current timeout configuration.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use nmrs::NetworkManager;
    ///
    /// # async fn example() -> nmrs::Result<()> {
    /// let nm = NetworkManager::new().await?;
    /// let config = nm.timeout_config();
    /// println!("Connection timeout: {:?}", config.connection_timeout);
    /// # Ok(())
    /// # }
    /// ```
    #[must_use]
    pub fn timeout_config(&self) -> crate::api::models::TimeoutConfig {
        self.timeout_config
    }

    /// List all network devices managed by NetworkManager.
    pub async fn list_devices(&self) -> Result<Vec<Device>> {
        list_devices(&self.conn).await
    }

    /// List all bluetooth devices.
    pub async fn list_bluetooth_devices(&self) -> Result<Vec<BluetoothDevice>> {
        list_bluetooth_devices(&self.conn).await
    }

    /// Lists all network devices managed by NetworkManager.
    pub async fn list_wireless_devices(&self) -> Result<Vec<Device>> {
        let devices = list_devices(&self.conn).await?;
        Ok(devices.into_iter().filter(|d| d.is_wireless()).collect())
    }

    /// List all wired (Ethernet) devices.
    pub async fn list_wired_devices(&self) -> Result<Vec<Device>> {
        let devices = list_devices(&self.conn).await?;
        Ok(devices.into_iter().filter(|d| d.is_wired()).collect())
    }

    /// Lists all visible Wi-Fi networks.
    ///
    /// Networks sharing an SSID on the same device are grouped, keeping the
    /// strongest AP as the representative. Each returned [`Network`] carries
    /// `best_bssid`, `bssids`, and `security_features` from the underlying APs.
    ///
    /// Pass `interface = Some("wlan0")` to scope to a single Wi-Fi device,
    /// or `None` to enumerate across every Wi-Fi device.
    ///
    /// **3.0 break:** added the `interface` parameter. For old behavior,
    /// pass `None`.
    pub async fn list_networks(&self, interface: Option<&str>) -> Result<Vec<Network>> {
        list_networks(&self.conn, interface).await
    }

    /// Lists every managed Wi-Fi device on the system.
    ///
    /// Each [`WifiDevice`] includes its interface name, MAC, current state,
    /// and the SSID/frequency of any active connection.
    pub async fn list_wifi_devices(&self) -> Result<Vec<WifiDevice>> {
        list_wifi_devices(&self.conn).await
    }

    /// Look up a single Wi-Fi device by interface name.
    ///
    /// Returns
    /// [`WifiInterfaceNotFound`](crate::ConnectionError::WifiInterfaceNotFound)
    /// if no device matches, or
    /// [`NotAWifiDevice`](crate::ConnectionError::NotAWifiDevice) if the
    /// interface exists but isn't a Wi-Fi device.
    pub async fn wifi_device_by_interface(&self, name: &str) -> Result<WifiDevice> {
        let all = list_wifi_devices(&self.conn).await?;
        all.into_iter()
            .find(|d| d.interface == name)
            .ok_or_else(|| crate::ConnectionError::WifiInterfaceNotFound {
                interface: name.to_string(),
            })
    }

    /// Build a [`WifiScope`] pinned to the given interface.
    ///
    /// All operations on the returned scope target only that one Wi-Fi
    /// device. Useful on multi-radio systems (laptops with USB dongles,
    /// docks with a second wireless adapter).
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use nmrs::{NetworkManager, WifiSecurity};
    ///
    /// # async fn example() -> nmrs::Result<()> {
    /// let nm = NetworkManager::new().await?;
    /// nm.wifi("wlan1").connect(
    ///     "Guest",
    ///     WifiSecurity::WpaPsk { psk: "guestpass".into() },
    /// ).await?;
    /// # Ok(())
    /// # }
    /// ```
    #[must_use]
    pub fn wifi(&self, interface: impl Into<String>) -> WifiScope {
        WifiScope {
            conn: self.conn.clone(),
            interface: interface.into(),
            timeout_config: self.timeout_config,
            connect_guard: self.connect_guard.clone(),
        }
    }

    /// Lists all visible access points, one entry per BSSID.
    ///
    /// Unlike [`list_networks`](Self::list_networks), this preserves
    /// duplicate BSSIDs for the same SSID and includes per-AP details
    /// like BSSID, exact frequency, bitrate, and device state.
    ///
    /// Pass `interface` to restrict to a single wireless device (e.g.
    /// `Some("wlan0")`), or `None` for all devices.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use nmrs::NetworkManager;
    ///
    /// # async fn example() -> nmrs::Result<()> {
    /// let nm = NetworkManager::new().await?;
    /// let mut aps = nm.list_access_points(None).await?;
    /// aps.sort_by(|a, b| b.strength.cmp(&a.strength));
    /// for ap in &aps {
    ///     println!("{:>3}%  {:<20} {}  {} MHz",
    ///         ap.strength, ap.ssid, ap.bssid, ap.frequency_mhz);
    /// }
    /// # Ok(())
    /// # }
    /// ```
    pub async fn list_access_points(&self, interface: Option<&str>) -> Result<Vec<AccessPoint>> {
        list_access_points(&self.conn, interface).await
    }

    /// Connects to a specific access point by SSID and optional BSSID.
    ///
    /// If `bssid` is `Some`, the connection targets that specific AP rather
    /// than the strongest match for the SSID. If `None`, behaves identically
    /// to [`connect`](Self::connect).
    ///
    /// **3.0 break:** added the `interface` parameter (3rd argument). Pass
    /// `None` for the previous behavior of using the first available Wi-Fi
    /// device, or `Some("wlan1")` to pin the connection to a specific
    /// interface. For an ergonomic per-interface API, see
    /// [`wifi`](Self::wifi).
    ///
    /// # Errors
    ///
    /// Returns [`ApBssidNotFound`](crate::ConnectionError::ApBssidNotFound) if
    /// no AP matching both the SSID and BSSID is visible.
    /// Returns [`InvalidBssid`](crate::ConnectionError::InvalidBssid) if the
    /// BSSID format is invalid.
    /// Returns
    /// [`WifiInterfaceNotFound`](crate::ConnectionError::WifiInterfaceNotFound)
    /// or [`NotAWifiDevice`](crate::ConnectionError::NotAWifiDevice) if the
    /// supplied interface name is bad.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use nmrs::{NetworkManager, WifiSecurity};
    ///
    /// # async fn example() -> nmrs::Result<()> {
    /// let nm = NetworkManager::new().await?;
    /// nm.connect_to_bssid(
    ///     "HomeWiFi",
    ///     Some("AA:BB:CC:DD:EE:FF"),
    ///     None,
    ///     WifiSecurity::WpaPsk { psk: "password".into() },
    /// ).await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn connect_to_bssid(
        &self,
        ssid: &str,
        bssid: Option<&str>,
        interface: Option<&str>,
        creds: WifiSecurity,
    ) -> Result<()> {
        let _guard = self.connect_guard.lock().await;
        connect_to_bssid(
            &self.conn,
            ssid,
            bssid,
            creds,
            interface,
            Some(self.timeout_config),
        )
        .await
    }

    /// Connects to a Wi-Fi network with the given credentials.
    ///
    /// **3.0 break:** added the `interface` parameter (3rd argument). Pass
    /// `None` for the previous behavior of using the first available Wi-Fi
    /// device, or `Some("wlan1")` to pin the connection to a specific
    /// interface.
    ///
    /// Concurrent calls on the same [`NetworkManager`] instance are
    /// serialized: if another task is already connecting, this call waits
    /// for it to finish (up to the configured timeout). Use
    /// [`try_connect`](Self::try_connect) to fail immediately instead.
    ///
    /// # Errors
    ///
    /// Returns `ConnectionError::NotFound` if the network is not visible,
    /// `ConnectionError::AuthFailed` if authentication fails, or other
    /// variants for specific failure reasons.
    pub async fn connect(
        &self,
        ssid: &str,
        interface: Option<&str>,
        creds: WifiSecurity,
    ) -> Result<()> {
        let _guard = self.connect_guard.lock().await;
        connect(
            &self.conn,
            ssid,
            creds,
            interface,
            Some(self.timeout_config),
        )
        .await
    }

    /// Connects to a wired (Ethernet) device.
    ///
    /// Finds the first available wired device and either activates an existing
    /// saved connection or creates a new one. The connection will activate
    /// when a cable is plugged in.
    ///
    /// # Errors
    ///
    /// Returns `ConnectionError::NoWiredDevice` if no wired device is found.
    pub async fn connect_wired(&self) -> Result<()> {
        let _guard = self.connect_guard.lock().await;
        connect_wired(&self.conn, Some(self.timeout_config)).await
    }

    /// Connects to a bluetooth device using the provided identity.
    ///
    /// # Example
    ///
    /// ```no_run
    /// use nmrs::{NetworkManager, models::BluetoothIdentity, models::BluetoothNetworkRole};
    ///
    /// # async fn example() -> nmrs::Result<()> {
    ///    let nm = NetworkManager::new().await?;
    ///
    ///    let identity = BluetoothIdentity::new(
    ///         "C8:1F:E8:F0:51:57".into(),
    ///         BluetoothNetworkRole::PanU,
    ///     )?;
    ///
    ///    nm.connect_bluetooth("My Phone", &identity).await?;
    ///    Ok(())
    /// }
    ///
    /// ```
    pub async fn connect_bluetooth(&self, name: &str, identity: &BluetoothIdentity) -> Result<()> {
        let _guard = self.connect_guard.lock().await;
        connect_bluetooth(&self.conn, name, identity, Some(self.timeout_config)).await
    }

    /// Connects to a VPN using the provided configuration.
    ///
    /// Supports WireGuard and OpenVPN connections. The function checks for an
    /// existing saved VPN connection by name. If found, it activates the saved
    /// connection. If not found, it creates a new VPN connection with the provided
    /// configuration.
    ///
    /// # Examples
    ///
    /// ## WireGuard
    ///
    /// ```rust
    /// use nmrs::{NetworkManager, WireGuardConfig, WireGuardPeer};
    ///
    /// # async fn example() -> nmrs::Result<()> {
    /// let nm = NetworkManager::new().await?;
    ///
    /// let peer = WireGuardPeer::new(
    ///     "peer_public_key",
    ///     "vpn.example.com:51820",
    ///     vec!["0.0.0.0/0".into()],
    /// ).with_persistent_keepalive(25);
    ///
    /// let config = WireGuardConfig::new(
    ///     "MyVPN",
    ///     "vpn.example.com:51820",
    ///     "your_private_key",
    ///     "10.0.0.2/24",
    ///     vec![peer],
    /// ).with_dns(vec!["1.1.1.1".into()]);
    ///
    /// nm.connect_vpn(config).await?;
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// ## OpenVPN
    ///
    /// ```rust
    /// use nmrs::{NetworkManager, OpenVpnConfig, OpenVpnAuthType};
    ///
    /// # async fn example() -> nmrs::Result<()> {
    /// let nm = NetworkManager::new().await?;
    ///
    /// let config = OpenVpnConfig::new("CorpVPN", "vpn.example.com", 1194, false)
    ///     .with_auth_type(OpenVpnAuthType::PasswordTls)
    ///     .with_username("user")
    ///     .with_password("secret")
    ///     .with_ca_cert("/etc/openvpn/ca.crt")
    ///     .with_client_cert("/etc/openvpn/client.crt")
    ///     .with_client_key("/etc/openvpn/client.key");
    ///
    /// nm.connect_vpn(config).await?;
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - NetworkManager is not running or accessible
    /// - The configuration is invalid or incomplete
    /// - The VPN connection fails to activate
    pub async fn connect_vpn<C>(&self, config: C) -> Result<()>
    where
        C: VpnConfig + Into<VpnConfiguration>,
    {
        let _guard = self.connect_guard.lock().await;
        connect_vpn(&self.conn, config.into(), Some(self.timeout_config)).await
    }

    /// Imports a `.ovpn` file and activates the OpenVPN connection.
    ///
    /// Parses the file, persists any inline certificates, builds the
    /// connection profile, and activates it through NetworkManager.
    ///
    /// # Arguments
    ///
    /// * `path` — Path to the `.ovpn` configuration file
    /// * `username` — Optional username for password-based authentication
    /// * `password` — Optional password for password-based authentication
    ///
    /// # Example
    ///
    /// ```no_run
    /// use nmrs::NetworkManager;
    ///
    /// # async fn example() -> nmrs::Result<()> {
    /// let nm = NetworkManager::new().await?;
    /// nm.import_ovpn("corp.ovpn", Some("user"), Some("secret")).await?;
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The file cannot be read or parsed
    /// - Inline certificate storage fails
    /// - The configuration is incomplete (e.g. TLS auth without certs)
    /// - The VPN connection fails to activate
    pub async fn import_ovpn(
        &self,
        path: impl AsRef<std::path::Path>,
        username: Option<&str>,
        password: Option<&str>,
    ) -> Result<()> {
        use crate::builders::OpenVpnBuilder;

        let mut builder = OpenVpnBuilder::from_ovpn_file(path)?;
        if let Some(u) = username {
            builder = builder.username(u);
        }
        if let Some(p) = password {
            builder = builder.password(p);
        }
        let config = builder.build()?;
        let _guard = self.connect_guard.lock().await;
        connect_vpn(&self.conn, config.into(), Some(self.timeout_config)).await
    }

    /// Disconnects from an active VPN connection by name.
    ///
    /// Searches through active connections for a VPN matching the given name.
    /// If found, deactivates the connection. If not found or already disconnected,
    /// returns success.
    ///
    /// # Example
    ///
    /// ```no_run
    /// use nmrs::NetworkManager;
    ///
    /// # async fn example() -> nmrs::Result<()> {
    /// let nm = NetworkManager::new().await?;
    /// nm.disconnect_vpn("MyVPN").await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn disconnect_vpn(&self, name: &str) -> Result<()> {
        let _guard = self.connect_guard.lock().await;
        disconnect_vpn(&self.conn, name).await
    }

    /// Lists all saved VPN connections.
    ///
    /// Returns a list of all VPN connection profiles saved in NetworkManager,
    /// including their name, type, and current state. Returns WireGuard and
    /// OpenVPN connections.
    ///
    /// # Example
    ///
    /// ```no_run
    /// use nmrs::NetworkManager;
    ///
    /// # async fn example() -> nmrs::Result<()> {
    /// let nm = NetworkManager::new().await?;
    /// let vpns = nm.list_vpn_connections().await?;
    ///
    /// for vpn in vpns {
    ///     println!("{}: {:?}", vpn.name, vpn.vpn_type);
    /// }
    /// # Ok(())
    /// # }
    /// ```
    pub async fn list_vpn_connections(&self) -> Result<Vec<VpnConnection>> {
        list_vpn_connections(&self.conn).await
    }

    /// Only active VPNs (subset of `list_vpn_connections` with `active = true`).
    pub async fn active_vpn_connections(&self) -> Result<Vec<VpnConnection>> {
        active_vpn_connections(&self.conn).await
    }

    /// Activate a saved VPN by UUID.
    ///
    /// # Example
    ///
    /// ```no_run
    /// use nmrs::NetworkManager;
    ///
    /// # async fn example() -> nmrs::Result<()> {
    /// let nm = NetworkManager::new().await?;
    /// nm.connect_vpn_by_uuid("2c3f1234-abcd-5678-ef01-234567890abc").await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn connect_vpn_by_uuid(&self, uuid: &str) -> Result<()> {
        let _guard = self.connect_guard.lock().await;
        connect_vpn_by_uuid(&self.conn, uuid, Some(self.timeout_config)).await
    }

    /// Activate a saved VPN by connection display name.
    ///
    /// Fails with [`VpnIdAmbiguous`](crate::ConnectionError::VpnIdAmbiguous)
    /// if multiple VPNs share the same name.
    pub async fn connect_vpn_by_id(&self, id: &str) -> Result<()> {
        let _guard = self.connect_guard.lock().await;
        connect_vpn_by_id(&self.conn, id, Some(self.timeout_config)).await
    }

    /// Disconnect a VPN by UUID.
    pub async fn disconnect_vpn_by_uuid(&self, uuid: &str) -> Result<()> {
        let _guard = self.connect_guard.lock().await;
        disconnect_vpn_by_uuid(&self.conn, uuid).await
    }

    /// Forgets (deletes) a saved VPN connection by name.
    ///
    /// Searches through saved connections for a VPN matching the given name.
    /// If found, deletes the connection profile. If currently connected, the
    /// VPN will be disconnected first before deletion.
    ///
    /// # Example
    ///
    /// ```no_run
    /// use nmrs::NetworkManager;
    ///
    /// # async fn example() -> nmrs::Result<()> {
    /// let nm = NetworkManager::new().await?;
    /// nm.forget_vpn("MyVPN").await?;
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// # Errors
    ///
    /// Returns an error only if the operation fails unexpectedly.
    /// Returns `Ok(())` if no matching VPN connection is found.
    pub async fn forget_vpn(&self, name: &str) -> Result<()> {
        let _guard = self.connect_guard.lock().await;
        crate::core::vpn::forget_vpn(&self.conn, name).await
    }

    /// Gets detailed information about an active VPN connection.
    ///
    /// Retrieves comprehensive information about a VPN connection, including
    /// IP configuration, DNS servers, gateway, interface, and connection state.
    /// The VPN must be actively connected to retrieve this information.
    ///
    /// # Example
    ///
    /// ```no_run
    /// use nmrs::NetworkManager;
    ///
    /// # async fn example() -> nmrs::Result<()> {
    /// let nm = NetworkManager::new().await?;
    /// let info = nm.get_vpn_info("MyVPN").await?;
    ///
    /// println!("VPN: {}", info.name);
    /// println!("Interface: {:?}", info.interface);
    /// println!("IP Address: {:?}", info.ip4_address);
    /// println!("DNS Servers: {:?}", info.dns_servers);
    /// println!("State: {:?}", info.state);
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// # Errors
    ///
    /// Returns `ConnectionError::NoVpnConnection` if the VPN is not found
    /// or not currently active.
    pub async fn get_vpn_info(&self, name: &str) -> Result<VpnConnectionInfo> {
        get_vpn_info(&self.conn, name).await
    }

    /// Returns the combined software/hardware state of the Wi-Fi radio.
    ///
    /// See [`RadioState`] for the distinction between `enabled` (software)
    /// and `hardware_enabled` (rfkill). The `present` flag reflects whether
    /// NetworkManager has a Wi-Fi device object; if device enumeration fails,
    /// `present` defaults to `true` so callers are not misled into thinking
    /// Wi-Fi is absent.
    pub async fn wifi_state(&self) -> Result<RadioState> {
        let present_types = airplane::fetch_present_device_types(&self.conn).await;
        airplane::wifi_state(&self.conn, present_types.as_ref()).await
    }

    /// Returns the combined software/hardware state of the WWAN radio.
    ///
    /// The `present` flag reflects whether NetworkManager has a modem device
    /// object; if device enumeration fails, `present` defaults to `true`.
    pub async fn wwan_state(&self) -> Result<RadioState> {
        let present_types = airplane::fetch_present_device_types(&self.conn).await;
        airplane::wwan_state(&self.conn, present_types.as_ref()).await
    }

    /// Returns the combined software/hardware state of the Bluetooth radio.
    ///
    /// Reads power state from all BlueZ adapters and cross-references rfkill.
    /// If BlueZ is not running or no adapters exist, returns a [`RadioState`]
    /// with `present = false` so callers can ignore Bluetooth on hosts that
    /// don't have it.
    pub async fn bluetooth_radio_state(&self) -> Result<RadioState> {
        airplane::bluetooth_radio_state(&self.conn).await
    }

    /// Returns the aggregated airplane-mode state across all radios.
    ///
    /// Fans out to Wi-Fi, WWAN, and Bluetooth concurrently and returns
    /// an [`AirplaneModeState`] snapshot. Radios that are not actually
    /// present on the host (no Wi-Fi card, no modem, no BlueZ) are reported
    /// with `present = false` and are ignored by
    /// [`AirplaneModeState::is_airplane_mode`] /
    /// [`AirplaneModeState::any_hardware_killed`].
    pub async fn airplane_mode_state(&self) -> Result<AirplaneModeState> {
        airplane::airplane_mode_state(&self.conn).await
    }

    /// Enables or disables the Wi-Fi radio (software toggle).
    ///
    /// This replaces the deprecated [`set_wifi_enabled`](Self::set_wifi_enabled).
    /// If the radio is hardware-killed, NM accepts the write but the radio
    /// remains off until hardware is unkilled.
    ///
    /// Note: NetworkManager implements this by writing rfkill soft blocks,
    /// which most distributions persist across reboots via `rfkill-restore`
    /// or systemd. A wifi disabled this way will remain disabled until it
    /// is explicitly re-enabled.
    pub async fn set_wireless_enabled(&self, enabled: bool) -> Result<()> {
        airplane::set_wireless_enabled(&self.conn, enabled).await
    }

    /// Enables or disables the WWAN (mobile broadband) radio.
    ///
    /// Writes the `WwanEnabled` property on NetworkManager.
    pub async fn set_wwan_enabled(&self, enabled: bool) -> Result<()> {
        airplane::set_wwan_enabled(&self.conn, enabled).await
    }

    /// Enables or disables the Bluetooth radio by toggling all BlueZ adapters.
    ///
    /// Returns [`BluezUnavailable`](crate::ConnectionError::BluezUnavailable) if BlueZ is not running
    /// or no adapters exist, or [`BluetoothToggleFailed`](crate::ConnectionError::BluetoothToggleFailed)
    /// if any adapter could not be toggled or did not reach the requested power state.
    ///
    /// Uses kernel rfkill (`rfkill block/unblock bluetooth`) as the primary
    /// mechanism, then also toggles BlueZ adapter `Powered` properties.
    pub async fn set_bluetooth_radio_enabled(&self, enabled: bool) -> Result<()> {
        airplane::set_bluetooth_radio_enabled(&self.conn, enabled).await
    }

    /// Flips all three radios in one call.
    ///
    /// **`enabled = true` means airplane mode is on, i.e. radios are off.**
    ///
    /// Wi-Fi and WWAN are toggled via NetworkManager properties. Bluetooth
    /// is toggled via kernel rfkill plus BlueZ adapter `Powered`, ensuring
    /// the soft-block state is visible to other components that read rfkill
    /// to determine airplane-mode status.
    ///
    /// Does not fail fast: attempts all three toggles concurrently and
    /// returns the first error at the end, if any. A missing Bluetooth
    /// stack (BlueZ not running or no adapters) is treated as a successful
    /// no-op rather than as an error so that flipping airplane mode on a
    /// wifi-only host still succeeds and leaves the toggle in the expected
    /// state. Bluetooth adapter toggle/settle failures are treated as
    /// non-fatal for this aggregate operation when at least one Wi-Fi or
    /// WWAN device is present and succeeds; such failures are logged while
    /// Wi-Fi/WWAN success still yields `Ok(())`. If no Wi-Fi or WWAN device
    /// is detected, the aggregate call can still return
    /// [`BluetoothToggleFailed`](crate::ConnectionError::BluetoothToggleFailed)
    /// for Bluetooth toggle/settle failures.
    pub async fn set_airplane_mode(&self, enabled: bool) -> Result<()> {
        airplane::set_airplane_mode(&self.conn, enabled).await
    }

    /// Current connectivity state as NM sees it (single property read).
    ///
    /// # Example
    ///
    /// ```no_run
    /// use nmrs::NetworkManager;
    ///
    /// # async fn example() -> nmrs::Result<()> {
    /// let nm = NetworkManager::new().await?;
    /// let state = nm.connectivity().await?;
    /// println!("{state:?}");
    /// # Ok(())
    /// # }
    /// ```
    pub async fn connectivity(&self) -> Result<crate::ConnectivityState> {
        crate::core::connectivity::connectivity(&self.conn).await
    }

    /// Forces NM to re-check connectivity by probing the configured URI.
    ///
    /// Returns the new state once the check completes.
    ///
    /// # Errors
    ///
    /// Returns [`ConnectivityCheckDisabled`](crate::ConnectionError::ConnectivityCheckDisabled)
    /// if NM's connectivity checks are turned off.
    pub async fn check_connectivity(&self) -> Result<crate::ConnectivityState> {
        crate::core::connectivity::check_connectivity(&self.conn).await
    }

    /// Full connectivity report including check URI and captive-portal URL.
    ///
    /// # Example
    ///
    /// ```no_run
    /// use nmrs::NetworkManager;
    ///
    /// # async fn example() -> nmrs::Result<()> {
    /// let nm = NetworkManager::new().await?;
    /// let report = nm.connectivity_report().await?;
    /// println!("{:?} portal={:?}", report.state, report.captive_portal_url);
    /// # Ok(())
    /// # }
    /// ```
    pub async fn connectivity_report(&self) -> Result<crate::ConnectivityReport> {
        crate::core::connectivity::connectivity_report(&self.conn).await
    }

    /// Captive-portal URL detected by NM, if state is `Portal`.
    ///
    /// Returns `None` if NM is not in `Portal` state or if this NM version
    /// does not expose the URL.
    pub async fn captive_portal_url(&self) -> Result<Option<String>> {
        let report = crate::core::connectivity::connectivity_report(&self.conn).await?;
        Ok(report.captive_portal_url)
    }

    /// Disable or re-enable a single Wi-Fi interface.
    ///
    /// Sets `Device.Autoconnect = enabled` and, when disabling, calls
    /// `Device.Disconnect()`. This is independent of the global wireless
    /// killswitch ([`set_wireless_enabled`](Self::set_wireless_enabled)) and
    /// safe to use on multi-radio systems.
    ///
    /// # Errors
    ///
    /// Returns
    /// [`WifiInterfaceNotFound`](crate::ConnectionError::WifiInterfaceNotFound)
    /// if no device with that name exists, or
    /// [`NotAWifiDevice`](crate::ConnectionError::NotAWifiDevice) if the
    /// interface isn't a Wi-Fi device.
    pub async fn set_wifi_enabled(&self, interface: &str, enabled: bool) -> Result<()> {
        set_wifi_enabled_for_interface(&self.conn, interface, enabled).await
    }

    /// Waits for a Wi-Fi device to become ready (disconnected or activated).
    pub async fn wait_for_wifi_ready(&self) -> Result<()> {
        wait_for_wifi_ready(&self.conn).await
    }

    /// Triggers a Wi-Fi scan.
    ///
    /// **3.0 break:** added the `interface` parameter. Pass `None` to scan
    /// every Wi-Fi device, or `Some("wlan0")` to scan one. See
    /// [`wifi`](Self::wifi) for an ergonomic per-interface API.
    pub async fn scan_networks(&self, interface: Option<&str>) -> Result<()> {
        scan_networks(&self.conn, interface).await
    }

    /// Returns whether any network device is currently in a transitional state.
    ///
    /// A device is considered "connecting" when its state is one of:
    /// Prepare, Config, NeedAuth, IpConfig, IpCheck, Secondaries, or Deactivating.
    ///
    /// This is a point-in-time snapshot and does **not** hold any lock.
    /// If you need an atomic "check then connect" operation, use
    /// [`try_connect`](Self::try_connect) instead, which checks under the
    /// internal connection mutex and returns
    /// [`ConnectionInProgress`](crate::ConnectionError::ConnectionInProgress)
    /// when another operation is in flight.
    ///
    /// # Example
    ///
    /// ```no_run
    /// use nmrs::NetworkManager;
    ///
    /// # async fn example() -> nmrs::Result<()> {
    /// let nm = NetworkManager::new().await?;
    ///
    /// if nm.is_connecting().await? {
    ///     eprintln!("A connection operation is already in progress");
    /// }
    /// # Ok(())
    /// # }
    /// ```
    pub async fn is_connecting(&self) -> Result<bool> {
        is_connecting(&self.conn).await
    }

    /// Atomically checks that no device is currently connecting, then
    /// connects to the given Wi-Fi network.
    ///
    /// This is the race-free alternative to calling
    /// [`is_connecting`](Self::is_connecting) followed by
    /// [`connect`](Self::connect). Unlike [`connect`](Self::connect), this
    /// does not wait on the connection mutex: it returns
    /// [`ConnectionInProgress`](crate::ConnectionError::ConnectionInProgress)
    /// if another task holds the mutex or any device is in a transitional
    /// state.
    ///
    /// # Example
    ///
    /// ```no_run
    /// use nmrs::{NetworkManager, WifiSecurity, ConnectionError};
    ///
    /// # async fn example() -> nmrs::Result<()> {
    /// let nm = NetworkManager::new().await?;
    ///
    /// match nm.try_connect("MyNetwork", None, WifiSecurity::WpaPsk {
    ///     psk: "password".into(),
    /// }).await {
    ///     Ok(()) => println!("Connected!"),
    ///     Err(ConnectionError::ConnectionInProgress) => {
    ///         eprintln!("Another connection is already in progress");
    ///     }
    ///     Err(e) => eprintln!("Error: {e}"),
    /// }
    /// # Ok(())
    /// # }
    /// ```
    pub async fn try_connect(
        &self,
        ssid: &str,
        interface: Option<&str>,
        creds: WifiSecurity,
    ) -> Result<()> {
        let _guard = self
            .connect_guard
            .try_lock()
            .map_err(|_| crate::ConnectionError::ConnectionInProgress)?;
        if is_connecting(&self.conn).await? {
            return Err(crate::ConnectionError::ConnectionInProgress);
        }
        connect(
            &self.conn,
            ssid,
            creds,
            interface,
            Some(self.timeout_config),
        )
        .await
    }

    /// Atomically checks that no device is currently connecting, then
    /// connects to a specific access point by SSID and optional BSSID.
    ///
    /// Like [`try_connect`](Self::try_connect) but targets a specific BSSID.
    /// Returns
    /// [`ConnectionInProgress`](crate::ConnectionError::ConnectionInProgress)
    /// if any device is already in a transitional state.
    pub async fn try_connect_to_bssid(
        &self,
        ssid: &str,
        bssid: Option<&str>,
        interface: Option<&str>,
        creds: WifiSecurity,
    ) -> Result<()> {
        let _guard = self
            .connect_guard
            .try_lock()
            .map_err(|_| crate::ConnectionError::ConnectionInProgress)?;
        if is_connecting(&self.conn).await? {
            return Err(crate::ConnectionError::ConnectionInProgress);
        }
        connect_to_bssid(
            &self.conn,
            ssid,
            bssid,
            creds,
            interface,
            Some(self.timeout_config),
        )
        .await
    }

    /// Check if a network is connected
    pub async fn is_connected(&self, ssid: &str) -> Result<bool> {
        is_connected(&self.conn, ssid).await
    }

    /// Disconnects from the current Wi-Fi network.
    ///
    /// If currently connected to a Wi-Fi network, this deactivates the
    /// active connection on the targeted device and waits for it to reach
    /// the disconnected state.
    ///
    /// **3.0 break:** added the `interface` parameter. Pass `None` for the
    /// previous behavior (first Wi-Fi device), or `Some("wlan1")` to target
    /// a specific interface.
    ///
    /// Returns `Ok(())` if disconnected successfully or if no active
    /// connection exists.
    ///
    /// # Example
    ///
    /// ```no_run
    /// use nmrs::NetworkManager;
    ///
    /// # async fn example() -> nmrs::Result<()> {
    /// let nm = NetworkManager::new().await?;
    /// nm.disconnect(None).await?;
    /// nm.disconnect(Some("wlan1")).await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn disconnect(&self, interface: Option<&str>) -> Result<()> {
        let _guard = self.connect_guard.lock().await;
        disconnect(&self.conn, interface, Some(self.timeout_config)).await
    }

    /// Returns the full `Network` object for the currently connected WiFi network.
    ///
    /// This provides detailed information about the active connection including
    /// signal strength, frequency, security type, and BSSID.
    ///
    /// # Example
    ///
    /// ```no_run
    /// use nmrs::NetworkManager;
    ///
    /// # async fn example() -> nmrs::Result<()> {
    /// let nm = NetworkManager::new().await?;
    /// if let Some(network) = nm.current_network().await? {
    ///     println!("Connected to: {} ({}%)", network.ssid, network.strength.unwrap_or(0));
    /// } else {
    ///     println!("Not connected");
    /// }
    /// # Ok(())
    /// # }
    /// ```
    pub async fn current_network(&self) -> Result<Option<Network>> {
        current_network(&self.conn).await
    }

    /// Lists all saved connection profiles with decoded [`SavedConnection`] summaries.
    ///
    /// Secrets are not included; use a [secret agent](crate::agent) with
    /// `GetSecrets` for passwords and keys.
    ///
    /// For a lighter call that only resolves `uuid`, `id`, and `type`, see
    /// [`Self::list_saved_connections_brief`].
    ///
    /// # Example
    ///
    /// ```no_run
    /// use nmrs::NetworkManager;
    ///
    /// # async fn example() -> nmrs::Result<()> {
    /// let nm = NetworkManager::new().await?;
    /// for c in nm.list_saved_connections().await? {
    ///     println!("{}  {}  {}", c.id, c.connection_type, c.uuid);
    /// }
    /// # Ok(())
    /// # }
    /// ```
    pub async fn list_saved_connections(&self) -> Result<Vec<SavedConnection>> {
        saved_profiles::list_saved_connections(&self.conn).await
    }

    /// Lists saved profiles with only `connection.uuid`, `id`, and `type` (still one
    /// `GetSettings` per profile, but skips building [`SettingsSummary`](crate::SettingsSummary)).
    pub async fn list_saved_connections_brief(&self) -> Result<Vec<SavedConnectionBrief>> {
        saved_profiles::list_saved_connections_brief(&self.conn).await
    }

    /// Returns the human-visible names (`connection.id`) of all saved profiles.
    ///
    /// Convenience over `list_saved_connections().map(|v| v.into_iter().map(|c| c.id).collect())`.
    pub async fn list_saved_connection_ids(&self) -> Result<Vec<String>> {
        Ok(saved_profiles::list_saved_connections_brief(&self.conn)
            .await?
            .into_iter()
            .map(|c| c.id)
            .collect())
    }

    /// Loads one saved profile by UUID with full [`SavedConnection`] decode.
    ///
    /// # Errors
    ///
    /// [`SavedConnectionNotFound`](crate::ConnectionError::SavedConnectionNotFound) if
    /// the UUID does not exist.
    pub async fn get_saved_connection(&self, uuid: &str) -> Result<SavedConnection> {
        saved_profiles::get_saved_connection(&self.conn, uuid).await
    }

    /// Raw `GetSettings` map for advanced consumers.
    pub async fn get_saved_connection_raw(
        &self,
        uuid: &str,
    ) -> Result<HashMap<String, HashMap<String, OwnedValue>>> {
        saved_profiles::get_saved_connection_raw(&self.conn, uuid).await
    }

    /// Deletes a saved profile by UUID (`Settings.Connection.Delete`).
    pub async fn delete_saved_connection(&self, uuid: &str) -> Result<()> {
        saved_profiles::delete_saved_connection(&self.conn, uuid).await
    }

    /// Merges a [`SettingsPatch`] into an existing profile.
    ///
    /// This loads the current settings, applies the patch, then writes the full
    /// settings map back with NetworkManager's `Update` / `UpdateUnsaved`.
    ///
    /// `uuid` is the profile's `connection.uuid` (see [`SavedConnection::uuid`]), **not**
    /// the Wi-Fi SSID from a scan [`Network`]. Use [`Self::get_saved_connection_uuid`] or
    /// [`Self::list_saved_connections`] to resolve the UUID from a profile name / SSID.
    ///
    /// # Example
    ///
    /// ```no_run
    /// use nmrs::{NetworkManager, SettingsPatch};
    ///
    /// # async fn example() -> nmrs::Result<()> {
    /// let nm = NetworkManager::new().await?;
    ///
    /// if let Some(uuid) = nm.get_saved_connection_uuid("HomeWiFi").await? {
    ///     let mut patch = SettingsPatch::default();
    ///     patch.autoconnect = Some(false);
    ///     nm.update_saved_connection(&uuid, patch).await?;
    /// }
    /// # Ok(())
    /// # }
    /// ```
    pub async fn update_saved_connection(&self, uuid: &str, patch: SettingsPatch) -> Result<()> {
        saved_profiles::update_saved_connection(&self.conn, uuid, &patch).await
    }

    /// Calls `ReloadConnections` so NM re-reads profiles from disk.
    pub async fn reload_saved_connections(&self) -> Result<()> {
        saved_profiles::reload_saved_connections(&self.conn).await
    }

    /// Finds a device by its interface name (e.g., "wlan0", "eth0").
    ///
    /// Returns the D-Bus object path of the device if found.
    ///
    /// # Example
    ///
    /// ```no_run
    /// use nmrs::NetworkManager;
    ///
    /// # async fn example() -> nmrs::Result<()> {
    /// let nm = NetworkManager::new().await?;
    /// let device_path = nm.get_device_by_interface("wlan0").await?;
    /// println!("Device path: {}", device_path.as_str());
    /// # Ok(())
    /// # }
    /// ```
    pub async fn get_device_by_interface(&self, name: &str) -> Result<zvariant::OwnedObjectPath> {
        get_device_by_interface(&self.conn, name).await
    }

    /// Returns the SSID of the currently connected network, if any.
    #[must_use]
    pub async fn current_ssid(&self) -> Option<String> {
        current_ssid(&self.conn).await
    }

    /// Returns the SSID and frequency of the current connection, if any.
    #[must_use]
    pub async fn current_connection_info(&self) -> Option<(String, Option<u32>)> {
        current_connection_info(&self.conn).await
    }

    /// Returns detailed information about a specific network.
    pub async fn show_details(&self, net: &Network) -> Result<NetworkInfo> {
        show_details(&self.conn, net).await
    }

    /// Returns whether a saved connection exists for the given SSID.
    pub async fn has_saved_connection(&self, ssid: &str) -> Result<bool> {
        has_saved_connection(&self.conn, ssid).await
    }

    /// Returns the D-Bus object path of a saved connection for the given SSID.
    pub async fn get_saved_connection_path(
        &self,
        ssid: &str,
    ) -> Result<Option<zvariant::OwnedObjectPath>> {
        get_saved_connection_path(&self.conn, ssid).await
    }

    /// Returns the profile UUID for a saved connection whose `connection.id` matches `name`.
    ///
    /// For Wi-Fi profiles, `name` is usually the SSID. Use the returned UUID with
    /// [`Self::update_saved_connection`] or [`Self::delete_saved_connection`].
    ///
    /// # Example
    ///
    /// ```no_run
    /// use nmrs::NetworkManager;
    ///
    /// # async fn example() -> nmrs::Result<()> {
    /// let nm = NetworkManager::new().await?;
    ///
    /// if let Some(uuid) = nm.get_saved_connection_uuid("HomeWiFi").await? {
    ///     println!("Profile UUID: {uuid}");
    /// }
    /// # Ok(())
    /// # }
    /// ```
    pub async fn get_saved_connection_uuid(&self, name: &str) -> Result<Option<String>> {
        get_saved_connection_uuid(&self.conn, name).await
    }

    /// Forgets (deletes) a saved WiFi connection for the given SSID.
    ///
    /// If currently connected to this network, disconnects first, then deletes
    /// all saved connection profiles matching the SSID.
    ///
    /// # Returns
    ///
    /// Returns `Ok(())` if one or more connections were deleted successfully,
    /// or if no matching connections were found.
    pub async fn forget(&self, ssid: &str) -> Result<()> {
        let _guard = self.connect_guard.lock().await;
        forget_by_name_and_type(
            &self.conn,
            ssid,
            Some(device_type::WIFI),
            Some(self.timeout_config),
        )
        .await
    }

    /// Forgets (deletes) a saved Bluetooth connection.
    ///
    /// If currently connected to this device, it will disconnect first before
    /// deleting the connection profile. Can match by connection name or bdaddr.
    ///
    /// # Arguments
    ///
    /// * `name` - Connection name or bdaddr to forget
    ///
    /// # Returns
    ///
    /// Returns `Ok(())` if the connection was deleted successfully.
    /// Returns `NoSavedConnection` if no matching connection was found.
    pub async fn forget_bluetooth(&self, name: &str) -> Result<()> {
        let _guard = self.connect_guard.lock().await;
        forget_by_name_and_type(
            &self.conn,
            name,
            Some(device_type::BLUETOOTH),
            Some(self.timeout_config),
        )
        .await
    }
    ///
    /// Subscribes to D-Bus signals for access point additions, removals, and
    /// signal strength changes on all Wi-Fi devices. Invokes the callback
    /// whenever the network list or signal data changes, enabling live UI
    /// updates without polling.
    ///
    /// This function runs indefinitely until an error occurs. Run it in a
    /// background task.
    ///
    /// # Example
    ///
    /// ```ignore
    /// # use nmrs::NetworkManager;
    /// # async fn example() -> nmrs::Result<()> {
    /// let nm = NetworkManager::new().await?;
    ///
    /// let nm_clone = nm.clone();
    /// tokio::spawn(async move {
    ///     nm_clone.monitor_network_changes(|| {
    ///         println!("Networks changed!");
    ///     }).await
    /// });
    /// # Ok(())
    /// # }
    /// ```
    pub async fn monitor_network_changes<F>(&self, callback: F) -> Result<()>
    where
        F: Fn() + Send + 'static,
    {
        let (_tx, rx) = watch::channel(());
        network_monitor::monitor_network_changes(&self.conn, rx, callback).await
    }

    /// Monitors device state changes in real-time.
    ///
    /// Subscribes to D-Bus signals for device state changes on all network
    /// devices (both wired and wireless). Invokes the callback whenever a
    /// device state changes (e.g., cable plugged in, device activated),
    /// enabling live UI updates without polling.
    ///
    /// This function runs indefinitely until an error occurs. Run it in a
    /// background task.
    ///
    /// # Example
    ///
    /// ```ignore
    /// # use nmrs::NetworkManager;
    /// # async fn example() -> nmrs::Result<()> {
    /// let nm = NetworkManager::new().await?;
    ///
    /// let nm_clone = nm.clone();
    /// tokio::spawn(async move {
    ///     nm_clone.monitor_device_changes(|| {
    ///         println!("Device state changed!");
    ///     }).await
    /// });
    /// # Ok(())
    /// # }
    /// ```
    pub async fn monitor_device_changes<F>(&self, callback: F) -> Result<()>
    where
        F: Fn() + Send + 'static,
    {
        let (_tx, rx) = watch::channel(());
        device_monitor::monitor_device_changes(&self.conn, rx, callback).await
    }
}
