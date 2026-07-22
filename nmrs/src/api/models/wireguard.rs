#![allow(deprecated)]

use crate::Passphrase;

use super::error::ConnectionError;
use super::vpn::{VpnConfig, VpnKind};
use uuid::Uuid;

/// WireGuard configuration for establishing a VPN connection.
///
/// Stores the necessary information to configure and connect to a VPN.
///
/// # Fields
///
/// - `name`: Unique identifier for the connection
/// - `gateway`: VPN gateway endpoint (e.g., "vpn.example.com:51820")
/// - `private_key`: Client's WireGuard private key
/// - `address`: Client's IP address with CIDR notation (e.g., "10.0.0.2/24")
/// - `peers`: List of WireGuard peers to connect to
/// - `dns`: Optional DNS servers to use (e.g., ["1.1.1.1", "8.8.8.8"])
/// - `mtu`: Optional Maximum Transmission Unit
/// - `uuid`: Optional UUID for the connection (auto-generated if not provided)
///
/// # Example
///
/// ```rust
/// use nmrs::{WireGuardConfig, WireGuardPeer};
///
/// let peer = WireGuardPeer::new(
///     "server_public_key",
///     "vpn.home.com:51820",
///     vec!["0.0.0.0/0".into()],
/// ).with_persistent_keepalive(25);
///
/// let config = WireGuardConfig::new(
///     "HomeVPN",
///     "vpn.home.com:51820",
///     "aBcDeFgHiJkLmNoPqRsTuVwXyZ0123456789=".to_string(),
///     "10.0.0.2/24",
///     vec![peer],
/// ).with_dns(vec!["1.1.1.1".into()]);
/// ```
#[non_exhaustive]
#[derive(Debug, Clone)]
pub struct WireGuardConfig {
    /// Unique name for the connection profile.
    pub name: String,
    /// VPN gateway endpoint (e.g., "vpn.example.com:51820").
    pub gateway: String,
    /// Client's WireGuard private key (base64 encoded).
    pub private_key: Passphrase,
    /// Client's IP address with CIDR notation (e.g., "10.0.0.2/24").
    pub address: String,
    /// List of WireGuard peers to connect to.
    pub peers: Vec<WireGuardPeer>,
    /// Optional DNS servers to use when connected.
    pub dns: Option<Vec<String>>,
    /// Optional Maximum Transmission Unit size.
    pub mtu: Option<u32>,
    /// Optional UUID for the connection (auto-generated if not provided).
    pub uuid: Option<Uuid>,
}

impl WireGuardConfig {
    /// Creates new `WireGuardConfig` with the required fields.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use nmrs::{WireGuardConfig, WireGuardPeer};
    ///
    /// let peer = WireGuardPeer::new(
    ///     "server_public_key",
    ///     "vpn.example.com:51820",
    ///     vec!["0.0.0.0/0".into()],
    /// );
    ///
    /// let config = WireGuardConfig::new(
    ///     "MyVPN",
    ///     "vpn.example.com:51820",
    ///     "client_private_key".to_string(),
    ///     "10.0.0.2/24",
    ///     vec![peer],
    /// );
    /// ```
    pub fn new(
        name: impl Into<String>,
        gateway: impl Into<String>,
        private_key: impl Into<Passphrase>,
        address: impl Into<String>,
        peers: Vec<WireGuardPeer>,
    ) -> Self {
        Self {
            name: name.into(),
            gateway: gateway.into(),
            private_key: private_key.into(),
            address: address.into(),
            peers,
            dns: None,
            mtu: None,
            uuid: None,
        }
    }

    /// Sets the DNS servers to use when connected.
    #[must_use]
    pub fn with_dns(mut self, dns: Vec<String>) -> Self {
        self.dns = Some(dns);
        self
    }

    /// Sets the MTU (Maximum Transmission Unit) size.
    #[must_use]
    pub fn with_mtu(mut self, mtu: u32) -> Self {
        self.mtu = Some(mtu);
        self
    }

    /// Sets the UUID for the connection.
    #[must_use]
    pub fn with_uuid(mut self, uuid: Uuid) -> Self {
        self.uuid = Some(uuid);
        self
    }
}

impl super::vpn::sealed::Sealed for WireGuardConfig {}

impl VpnConfig for WireGuardConfig {
    fn vpn_kind(&self) -> VpnKind {
        VpnKind::WireGuard
    }

    fn name(&self) -> &str {
        &self.name
    }

    fn dns(&self) -> Option<&[String]> {
        self.dns.as_deref()
    }

    fn mtu(&self) -> Option<u32> {
        self.mtu
    }

    fn uuid(&self) -> Option<Uuid> {
        self.uuid
    }
}

impl From<WireGuardConfig> for VpnCredentials {
    fn from(config: WireGuardConfig) -> Self {
        Self {
            vpn_type: VpnKind::WireGuard,
            name: config.name,
            gateway: config.gateway,
            private_key: config.private_key,
            address: config.address,
            peers: config.peers,
            dns: config.dns,
            mtu: config.mtu,
            uuid: config.uuid,
        }
    }
}

impl From<VpnCredentials> for WireGuardConfig {
    fn from(config: VpnCredentials) -> Self {
        Self {
            name: config.name,
            gateway: config.gateway,
            private_key: config.private_key,
            address: config.address,
            peers: config.peers,
            dns: config.dns,
            mtu: config.mtu,
            uuid: config.uuid,
        }
    }
}

/// Legacy VPN credentials for establishing a VPN connection.
///
/// Prefer [`WireGuardConfig`] for new WireGuard connections.
#[deprecated(note = "Use WireGuardConfig instead.")]
#[non_exhaustive]
#[derive(Debug, Clone)]
pub struct VpnCredentials {
    /// The type of VPN (currently only WireGuard).
    pub vpn_type: VpnKind,
    /// Unique name for the connection profile.
    pub name: String,
    /// VPN gateway endpoint (e.g., "vpn.example.com:51820").
    pub gateway: String,
    /// Client's WireGuard private key (base64 encoded).
    pub private_key: Passphrase,
    /// Client's IP address with CIDR notation (e.g., "10.0.0.2/24").
    pub address: String,
    /// List of WireGuard peers to connect to.
    pub peers: Vec<WireGuardPeer>,
    /// Optional DNS servers to use when connected.
    pub dns: Option<Vec<String>>,
    /// Optional Maximum Transmission Unit size.
    pub mtu: Option<u32>,
    /// Optional UUID for the connection (auto-generated if not provided).
    pub uuid: Option<Uuid>,
}

impl VpnCredentials {
    /// Creates new `VpnCredentials` with the required fields.
    ///
    /// Prefer [`WireGuardConfig::new`] for new code.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use nmrs::{VpnCredentials, VpnKind, WireGuardPeer};
    ///
    /// let peer = WireGuardPeer::new(
    ///     "server_public_key",
    ///     "vpn.example.com:51820",
    ///     vec!["0.0.0.0/0".into()],
    /// );
    ///
    /// let creds = VpnCredentials::new(
    ///     VpnKind::WireGuard,
    ///     "MyVPN",
    ///     "vpn.example.com:51820",
    ///     "client_private_key".to_string(),
    ///     "10.0.0.2/24",
    ///     vec![peer],
    /// );
    /// ```
    pub fn new(
        vpn_type: VpnKind,
        name: impl Into<String>,
        gateway: impl Into<String>,
        private_key: impl Into<Passphrase>,
        address: impl Into<String>,
        peers: Vec<WireGuardPeer>,
    ) -> Self {
        Self {
            vpn_type,
            name: name.into(),
            gateway: gateway.into(),
            private_key: private_key.into(),
            address: address.into(),
            peers,
            dns: None,
            mtu: None,
            uuid: None,
        }
    }

    /// Creates a new `VpnCredentials` builder.
    #[must_use]
    pub fn builder() -> VpnCredentialsBuilder {
        VpnCredentialsBuilder::default()
    }

    /// Sets the DNS servers to use when connected.
    #[must_use]
    pub fn with_dns(mut self, dns: Vec<String>) -> Self {
        self.dns = Some(dns);
        self
    }

    /// Sets the MTU (Maximum Transmission Unit) size.
    #[must_use]
    pub fn with_mtu(mut self, mtu: u32) -> Self {
        self.mtu = Some(mtu);
        self
    }

    /// Sets the UUID for the connection.
    #[must_use]
    pub fn with_uuid(mut self, uuid: Uuid) -> Self {
        self.uuid = Some(uuid);
        self
    }
}

impl super::vpn::sealed::Sealed for VpnCredentials {}

impl VpnConfig for VpnCredentials {
    fn vpn_kind(&self) -> VpnKind {
        self.vpn_type
    }

    fn name(&self) -> &str {
        &self.name
    }

    fn dns(&self) -> Option<&[String]> {
        self.dns.as_deref()
    }

    fn mtu(&self) -> Option<u32> {
        self.mtu
    }

    fn uuid(&self) -> Option<Uuid> {
        self.uuid
    }
}

/// Builder for constructing `VpnCredentials` with a fluent API.
///
/// This builder provides a more ergonomic way to create VPN credentials,
/// making the code more readable and less error-prone compared to the
/// traditional constructor with many positional parameters.
///
/// # Examples
///
/// ## Basic WireGuard VPN
///
/// ```rust
/// use nmrs::{VpnCredentials, WireGuardPeer};
///
/// let peer = WireGuardPeer::new(
///     "HIgo9xNzJMWLKAShlKl6/bUT1VI9Q0SDBXGtLXkPFXc=",
///     "vpn.example.com:51820",
///     vec!["0.0.0.0/0".into()],
/// );
///
/// let creds = VpnCredentials::builder()
///     .name("HomeVPN")
///     .wireguard()
///     .gateway("vpn.example.com:51820")
///     .private_key("YBk6X3pP8KjKz7+HFWzVHNqL3qTZq8hX9VxFQJ4zVmM=".to_string())
///     .address("10.0.0.2/24")
///     .add_peer(peer)
///     .build()
///     .expect("all required fields set");
/// ```
///
/// ## With Optional DNS and MTU
///
/// ```rust
/// use nmrs::{VpnCredentials, WireGuardPeer};
///
/// let peer = WireGuardPeer::new(
///     "server_public_key",
///     "vpn.example.com:51820",
///     vec!["0.0.0.0/0".into()],
/// ).with_persistent_keepalive(25);
///
/// let creds = VpnCredentials::builder()
///     .name("CorpVPN")
///     .wireguard()
///     .gateway("vpn.corp.com:51820")
///     .private_key("private_key_here".to_string())
///     .address("10.8.0.2/24")
///     .add_peer(peer)
///     .with_dns(vec!["1.1.1.1".into(), "8.8.8.8".into()])
///     .with_mtu(1420)
///     .build()
///     .expect("all required fields set");
/// ```
#[non_exhaustive]
#[derive(Debug, Default)]
pub struct VpnCredentialsBuilder {
    vpn_type: Option<VpnKind>,
    name: Option<String>,
    gateway: Option<String>,
    private_key: Option<Passphrase>,
    address: Option<String>,
    peers: Vec<WireGuardPeer>,
    dns: Option<Vec<String>>,
    mtu: Option<u32>,
    uuid: Option<Uuid>,
}

impl VpnCredentialsBuilder {
    /// Sets the VPN type to WireGuard.
    ///
    /// Currently, WireGuard is the only supported VPN type.
    #[must_use]
    pub fn wireguard(mut self) -> Self {
        self.vpn_type = Some(VpnKind::WireGuard);
        self
    }

    /// Sets the VPN kind.
    ///
    /// For most use cases, prefer using [`wireguard()`](Self::wireguard) instead.
    #[must_use]
    pub fn vpn_kind(mut self, vpn_kind: VpnKind) -> Self {
        self.vpn_type = Some(vpn_kind);
        self
    }

    /// Sets the connection name.
    ///
    /// This is the unique identifier for the VPN connection profile.
    #[must_use]
    pub fn name(mut self, name: impl Into<String>) -> Self {
        self.name = Some(name.into());
        self
    }

    /// Sets the VPN gateway endpoint.
    ///
    /// Should be in "host:port" format (e.g., "vpn.example.com:51820").
    #[must_use]
    pub fn gateway(mut self, gateway: impl Into<String>) -> Self {
        self.gateway = Some(gateway.into());
        self
    }

    /// Sets the client's WireGuard private key.
    ///
    /// The private key should be base64 encoded.
    #[must_use]
    pub fn private_key(mut self, private_key: impl Into<Passphrase>) -> Self {
        self.private_key = Some(private_key.into());
        self
    }

    /// Sets the client's IP address with CIDR notation.
    ///
    /// # Examples
    ///
    /// - "10.0.0.2/24" for a /24 subnet
    /// - "192.168.1.10/32" for a single IP
    #[must_use]
    pub fn address(mut self, address: impl Into<String>) -> Self {
        self.address = Some(address.into());
        self
    }

    /// Adds a WireGuard peer to the connection.
    ///
    /// Multiple peers can be added by calling this method multiple times.
    #[must_use]
    pub fn add_peer(mut self, peer: WireGuardPeer) -> Self {
        self.peers.push(peer);
        self
    }

    /// Sets all WireGuard peers at once.
    ///
    /// This replaces any previously added peers.
    #[must_use]
    pub fn peers(mut self, peers: Vec<WireGuardPeer>) -> Self {
        self.peers = peers;
        self
    }

    /// Sets the DNS servers to use when connected.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use nmrs::VpnCredentials;
    ///
    /// let builder = VpnCredentials::builder()
    ///     .with_dns(vec!["1.1.1.1".into(), "8.8.8.8".into()]);
    /// ```
    #[must_use]
    pub fn with_dns(mut self, dns: Vec<String>) -> Self {
        self.dns = Some(dns);
        self
    }

    /// Sets the MTU (Maximum Transmission Unit) size.
    ///
    /// Typical values are 1420 for WireGuard over standard networks.
    #[must_use]
    pub fn with_mtu(mut self, mtu: u32) -> Self {
        self.mtu = Some(mtu);
        self
    }

    /// Sets a specific UUID for the connection.
    ///
    /// If not set, NetworkManager will generate one automatically.
    #[must_use]
    pub fn with_uuid(mut self, uuid: Uuid) -> Self {
        self.uuid = Some(uuid);
        self
    }

    /// Builds the `VpnCredentials` from the configured values.
    ///
    /// # Errors
    ///
    /// Returns [`ConnectionError::IncompleteBuilder`](crate::ConnectionError::IncompleteBuilder)
    /// if a required string field is missing, or
    /// [`ConnectionError::InvalidPeers`](crate::ConnectionError::InvalidPeers) if no peers
    /// were added.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use nmrs::{VpnCredentials, WireGuardPeer};
    ///
    /// let peer = WireGuardPeer::new(
    ///     "public_key",
    ///     "vpn.example.com:51820",
    ///     vec!["0.0.0.0/0".into()],
    /// );
    ///
    /// let creds = VpnCredentials::builder()
    ///     .name("MyVPN")
    ///     .wireguard()
    ///     .gateway("vpn.example.com:51820")
    ///     .private_key("private_key".to_string())
    ///     .address("10.0.0.2/24")
    ///     .add_peer(peer)
    ///     .build()
    ///     .expect("all required fields set");
    /// ```
    #[must_use = "the built credentials must be passed to the VPN connect API"]
    pub fn build(self) -> Result<VpnCredentials, ConnectionError> {
        let vpn_type = self.vpn_type.ok_or_else(|| {
            ConnectionError::IncompleteBuilder("VPN type is required (use .wireguard())".into())
        })?;
        let name = self.name.ok_or_else(|| {
            ConnectionError::IncompleteBuilder("connection name is required (use .name())".into())
        })?;
        let gateway = self.gateway.ok_or_else(|| {
            ConnectionError::IncompleteBuilder("gateway is required (use .gateway())".into())
        })?;
        let private_key = self.private_key.ok_or_else(|| {
            ConnectionError::IncompleteBuilder(
                "private key is required (use .private_key())".into(),
            )
        })?;
        let address = self.address.ok_or_else(|| {
            ConnectionError::IncompleteBuilder("address is required (use .address())".into())
        })?;
        if self.peers.is_empty() {
            return Err(ConnectionError::InvalidPeers(
                "at least one peer is required (use .add_peer())".into(),
            ));
        }
        Ok(VpnCredentials {
            vpn_type,
            name,
            gateway,
            private_key,
            address,
            peers: self.peers,
            dns: self.dns,
            mtu: self.mtu,
            uuid: self.uuid,
        })
    }
}

/// WireGuard peer configuration.
///
/// Represents a single WireGuard peer (server) to connect to.
///
/// # Fields
///
/// - `public_key`: The peer's WireGuard public key
/// - `gateway`: Peer endpoint in "host:port" format (e.g., "vpn.example.com:51820")
/// - `allowed_ips`: List of IP ranges allowed through this peer (e.g., ["0.0.0.0/0"])
/// - `preshared_key`: Optional pre-shared key for additional security
/// - `persistent_keepalive`: Optional keepalive interval in seconds (e.g., 25)
///
/// # Example
///
/// ```rust
/// use nmrs::WireGuardPeer;
///
/// let peer = WireGuardPeer::new(
///     "aBcDeFgHiJkLmNoPqRsTuVwXyZ0123456789=",
///     "vpn.example.com:51820",
///     vec!["0.0.0.0/0".into(), "::/0".into()],
/// );
/// ```
#[non_exhaustive]
#[derive(Debug, Clone)]
pub struct WireGuardPeer {
    /// The peer's WireGuard public key (base64 encoded).
    pub public_key: String,
    /// Peer endpoint in "host:port" format.
    pub gateway: String,
    /// IP ranges to route through this peer (e.g., ["0.0.0.0/0"]).
    pub allowed_ips: Vec<String>,
    /// Optional pre-shared key for additional security.
    pub preshared_key: Option<Passphrase>,
    /// Optional keepalive interval in seconds (e.g., 25).
    pub persistent_keepalive: Option<u32>,
}

impl WireGuardPeer {
    /// Creates a new `WireGuardPeer` with the required fields.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use nmrs::WireGuardPeer;
    ///
    /// let peer = WireGuardPeer::new(
    ///     "aBcDeFgHiJkLmNoPqRsTuVwXyZ0123456789=",
    ///     "vpn.example.com:51820",
    ///     vec!["0.0.0.0/0".into()],
    /// );
    /// ```
    pub fn new(
        public_key: impl Into<String>,
        gateway: impl Into<String>,
        allowed_ips: Vec<String>,
    ) -> Self {
        Self {
            public_key: public_key.into(),
            gateway: gateway.into(),
            allowed_ips,
            preshared_key: None,
            persistent_keepalive: None,
        }
    }

    /// Sets the pre-shared key for additional security.
    #[must_use]
    pub fn with_preshared_key(mut self, psk: impl Into<Passphrase>) -> Self {
        self.preshared_key = Some(psk.into());
        self
    }

    /// Sets the persistent keepalive interval in seconds.
    #[must_use]
    pub fn with_persistent_keepalive(mut self, interval: u32) -> Self {
        self.persistent_keepalive = Some(interval);
        self
    }
}
