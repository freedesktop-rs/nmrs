//! WireGuard VPN connection builder with validation.
//!
//! Provides a type-safe builder API for constructing WireGuard VPN connections
//! with comprehensive validation of keys, addresses, and peer configurations.

use std::collections::HashMap;
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};
use uuid::Uuid;
use zvariant::Value;

use super::connection_builder::{ConnectionBuilder, IpConfig};
use crate::api::models::{ConnectionError, ConnectionOptions, WireGuardPeer};

/// Builder for WireGuard VPN connections.
///
/// This builder provides a fluent API for creating WireGuard VPN connection settings
/// with validation at build time.
///
/// # Example
///
/// ```rust
/// use nmrs::builders::WireGuardBuilder;
/// use nmrs::{WireGuardPeer, ConnectionOptions};
///
/// let peer = WireGuardPeer::new(
///     "HIgo9xNzJMWLKAShlKl6/bUT1VI9Q0SDBXGtLXkPFXc=",
///     "vpn.example.com:51820",
///     vec!["0.0.0.0/0".into()],
/// ).with_persistent_keepalive(25);
///
/// let settings = WireGuardBuilder::new("MyVPN")
///     .private_key("YBk6X3pP8KjKz7+HFWzVHNqL3qTZq8hX9VxFQJ4zVmM=")
///     .address("10.0.0.2/24")
///     .add_peer(peer)
///     .autoconnect(false)
///     .build()
///     .expect("Failed to build WireGuard connection");
/// ```
#[non_exhaustive]
pub struct WireGuardBuilder {
    inner: ConnectionBuilder,
    name: String,
    private_key: Option<String>,
    address: Option<String>,
    peers: Vec<WireGuardPeer>,
    dns: Option<Vec<String>>,
    mtu: Option<u32>,
    uuid: Option<Uuid>,
}

impl WireGuardBuilder {
    /// Creates a new WireGuard connection builder.
    ///
    /// # Arguments
    ///
    /// * `name` - Human-readable connection name
    #[must_use]
    pub fn new(name: impl Into<String>) -> Self {
        let name = name.into();
        let inner = ConnectionBuilder::new("wireguard", &name);

        Self {
            inner,
            name,
            private_key: None,
            address: None,
            peers: Vec::new(),
            dns: None,
            mtu: None,
            uuid: None,
        }
    }

    /// Sets the WireGuard private key.
    ///
    /// The key must be a valid base64-encoded 32-byte WireGuard key (44 characters).
    #[must_use]
    pub fn private_key(mut self, key: impl Into<String>) -> Self {
        self.private_key = Some(key.into());
        self
    }

    /// Sets the VPN interface IP address with CIDR notation.
    ///
    /// # Example
    ///
    /// ```rust
    /// # use nmrs::builders::WireGuardBuilder;
    /// let builder = WireGuardBuilder::new("MyVPN")
    ///     .address("10.0.0.2/24");
    /// ```
    #[must_use]
    pub fn address(mut self, address: impl Into<String>) -> Self {
        self.address = Some(address.into());
        self
    }

    /// Adds a WireGuard peer to the connection.
    ///
    /// At least one peer must be added before building.
    #[must_use]
    pub fn add_peer(mut self, peer: WireGuardPeer) -> Self {
        self.peers.push(peer);
        self
    }

    /// Adds multiple WireGuard peers at once.
    #[must_use]
    pub fn add_peers(mut self, peers: impl IntoIterator<Item = WireGuardPeer>) -> Self {
        self.peers.extend(peers);
        self
    }

    /// Sets DNS servers for the VPN connection.
    ///
    /// # Example
    ///
    /// ```rust
    /// # use nmrs::builders::WireGuardBuilder;
    /// let builder = WireGuardBuilder::new("MyVPN")
    ///     .dns(vec!["1.1.1.1".into(), "8.8.8.8".into()]);
    /// ```
    #[must_use]
    pub fn dns(mut self, servers: Vec<String>) -> Self {
        self.dns = Some(servers);
        self
    }

    /// Sets the MTU (Maximum Transmission Unit) for the WireGuard interface.
    ///
    /// Typical value is 1420 for WireGuard over IPv4.
    #[must_use]
    pub fn mtu(mut self, mtu: u32) -> Self {
        self.mtu = Some(mtu);
        self
    }

    /// Sets a specific UUID for the connection.
    ///
    /// If not set, a deterministic UUID will be generated based on the
    /// connection name.
    #[must_use]
    pub fn uuid(mut self, uuid: Uuid) -> Self {
        self.uuid = Some(uuid);
        self
    }

    // Delegation methods to inner ConnectionBuilder

    /// Applies connection options.
    #[must_use]
    pub fn options(mut self, opts: &ConnectionOptions) -> Self {
        self.inner = self.inner.options(opts);
        self
    }

    /// Enables or disables automatic connection.
    #[must_use]
    pub fn autoconnect(mut self, enabled: bool) -> Self {
        self.inner = self.inner.autoconnect(enabled);
        self
    }

    /// Sets autoconnect priority.
    #[must_use]
    pub fn autoconnect_priority(mut self, priority: i32) -> Self {
        self.inner = self.inner.autoconnect_priority(priority);
        self
    }

    /// Sets autoconnect retry limit.
    #[must_use]
    pub fn autoconnect_retries(mut self, retries: i32) -> Self {
        self.inner = self.inner.autoconnect_retries(retries);
        self
    }

    /// Builds the final WireGuard connection settings.
    ///
    /// This method validates all required fields and returns an error if
    /// any validation fails.
    ///
    /// # Errors
    ///
    /// - `ConnectionError::InvalidPrivateKey` if private key is missing or invalid
    /// - `ConnectionError::InvalidAddress` if address is missing or invalid
    /// - `ConnectionError::InvalidPeers` if no peers are configured or peer validation fails
    /// - `ConnectionError::InvalidGateway` if any peer gateway is invalid
    #[must_use = "the connection settings must be passed to NetworkManager"]
    pub fn build(
        mut self,
    ) -> Result<HashMap<&'static str, HashMap<&'static str, Value<'static>>>, ConnectionError> {
        // Validate required fields
        let private_key = self
            .private_key
            .ok_or_else(|| ConnectionError::InvalidPrivateKey("Private key not set".into()))?;

        let address = self
            .address
            .ok_or_else(|| ConnectionError::InvalidAddress("Address not set".into()))?;

        if self.peers.is_empty() {
            return Err(ConnectionError::InvalidPeers("No peers configured".into()));
        }

        // Validate private key
        validate_wireguard_key(&private_key, "Private key")?;

        // Validate address
        let (ip, prefix) = validate_address(&address)?;

        // Validate each peer
        for (i, peer) in self.peers.iter().enumerate() {
            validate_wireguard_key(&peer.public_key, &format!("Peer {} public key", i))?;
            validate_gateway(&peer.gateway)?;

            if peer.allowed_ips.is_empty() {
                return Err(ConnectionError::InvalidPeers(format!(
                    "Peer {} has no allowed IPs",
                    i
                )));
            }
        }

        // Generate interface name
        let interface_name = format!(
            "wg-{}",
            self.name
                .to_lowercase()
                .chars()
                .filter(|c| c.is_alphanumeric() || *c == '-')
                .take(10)
                .collect::<String>()
        );

        self.inner = self.inner.interface_name(&interface_name);

        // Set UUID (deterministic or provided)
        let uuid = self.uuid.unwrap_or_else(|| {
            // Generate deterministic UUID based on name
            Uuid::new_v5(&Uuid::NAMESPACE_DNS, format!("wg:{}", self.name).as_bytes())
        });

        self.inner = self.inner.uuid(uuid);

        // Build wireguard section
        let mut wireguard = HashMap::new();
        wireguard.insert("private-key", Value::from(private_key));

        // Build peers array
        let mut peers_array: Vec<HashMap<String, zvariant::Value<'static>>> = Vec::new();

        for peer in self.peers {
            let mut peer_dict: HashMap<String, zvariant::Value<'static>> = HashMap::new();

            peer_dict.insert("public-key".into(), Value::from(peer.public_key));
            peer_dict.insert("endpoint".into(), Value::from(peer.gateway));
            peer_dict.insert("allowed-ips".into(), Value::from(peer.allowed_ips));

            if let Some(psk) = peer.preshared_key {
                peer_dict.insert("preshared-key".into(), Value::from(psk));
            }

            if let Some(ka) = peer.persistent_keepalive {
                peer_dict.insert("persistent-keepalive".into(), Value::from(ka));
            }

            peers_array.push(peer_dict);
        }

        wireguard.insert("peers", Value::from(peers_array));

        if let Some(mtu) = self.mtu {
            wireguard.insert("mtu", Value::from(mtu));
        }

        self.inner = self.inner.with_section("wireguard", wireguard);

        match ip {
            IpAddr::V4(ip) => {
                self.inner = self
                    .inner
                    .ipv4_manual(vec![IpConfig::new(ip.to_string(), prefix)])
                    .ipv6_ignore();
            }
            IpAddr::V6(ip) => {
                self.inner = self
                    .inner
                    .ipv4_disabled()
                    .ipv6_manual(vec![IpConfig::new(ip.to_string(), prefix)]);
            }
        }

        if let Some(dns) = self.dns {
            let mut ipv4_dns = Vec::<Ipv4Addr>::new();
            let mut ipv6_dns = Vec::<Ipv6Addr>::new();
            for server in dns {
                match server.parse::<IpAddr>() {
                    Ok(IpAddr::V4(address)) => ipv4_dns.push(address),
                    Ok(IpAddr::V6(address)) => ipv6_dns.push(address),
                    Err(_) => {
                        return Err(ConnectionError::VpnFailed(format!(
                            "Invalid DNS server address: {server}"
                        )));
                    }
                }
            }
            if !ipv4_dns.is_empty() {
                self.inner = self.inner.ipv4_dns(ipv4_dns);
            }
            if !ipv6_dns.is_empty() {
                self.inner = self.inner.ipv6_dns(ipv6_dns);
            }
        }

        Ok(self.inner.build())
    }
}

// Validation functions (same as in vpn.rs)

fn validate_wireguard_key(key: &str, key_type: &str) -> Result<(), ConnectionError> {
    if key.trim().is_empty() {
        return Err(invalid_wireguard_key(
            key_type,
            format!("{} cannot be empty", key_type),
        ));
    }

    let len = key.trim().len();
    if !(40..=50).contains(&len) {
        return Err(invalid_wireguard_key(
            key_type,
            format!(
                "{} has invalid length: {} (expected ~44 characters)",
                key_type, len
            ),
        ));
    }

    let is_valid_base64 = key
        .trim()
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '+' || c == '/' || c == '=');

    if !is_valid_base64 {
        return Err(invalid_wireguard_key(
            key_type,
            format!("{} contains invalid base64 characters", key_type),
        ));
    }

    Ok(())
}

fn invalid_wireguard_key(key_type: &str, message: String) -> ConnectionError {
    if key_type.contains("public key") {
        ConnectionError::InvalidPublicKey(message)
    } else {
        ConnectionError::InvalidPrivateKey(message)
    }
}

fn validate_address(address: &str) -> Result<(IpAddr, u32), ConnectionError> {
    let (ip, prefix) = address.split_once('/').ok_or_else(|| {
        ConnectionError::InvalidAddress(format!(
            "missing CIDR prefix (e.g., '10.0.0.2/24'): {}",
            address
        ))
    })?;

    let ip = ip.trim().parse::<IpAddr>().map_err(|_| {
        ConnectionError::InvalidAddress(format!("invalid IP address: {}", ip.trim()))
    })?;

    let prefix: u32 = prefix
        .parse()
        .map_err(|_| ConnectionError::InvalidAddress(format!("invalid CIDR prefix: {}", prefix)))?;

    let max_prefix = if ip.is_ipv4() { 32 } else { 128 };
    if prefix > max_prefix {
        return Err(ConnectionError::InvalidAddress(format!(
            "CIDR prefix too large: {prefix} (max {max_prefix})"
        )));
    }

    Ok((ip, prefix))
}

fn validate_gateway(gateway: &str) -> Result<(), ConnectionError> {
    if gateway.trim().is_empty() {
        return Err(ConnectionError::InvalidGateway(
            "gateway cannot be empty".into(),
        ));
    }

    let (host, port_str) = gateway.rsplit_once(':').ok_or_else(|| {
        ConnectionError::InvalidGateway(format!("gateway must be in 'host:port' format: {gateway}"))
    })?;
    if host.trim().is_empty() {
        return Err(ConnectionError::InvalidGateway(
            "gateway host cannot be empty".into(),
        ));
    }
    if host.contains(':')
        && !(host.starts_with('[')
            && host.ends_with(']')
            && host[1..host.len() - 1].parse::<Ipv6Addr>().is_ok())
    {
        return Err(ConnectionError::InvalidGateway(format!(
            "IPv6 gateway must use '[address]:port' format: {gateway}"
        )));
    }

    let port: u16 = port_str
        .parse()
        .map_err(|_| ConnectionError::InvalidGateway(format!("invalid port number: {port_str}")))?;

    if port == 0 {
        return Err(ConnectionError::InvalidGateway("port cannot be 0".into()));
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    const PRIVATE_KEY: &str = "YBk6X3pP8KjKz7+HFWzVHNqL3qTZq8hX9VxFQJ4zVmM=";
    const PUBLIC_KEY: &str = "HIgo9xNzJMWLKAShlKl6/bUT1VI9Q0SDBXGtLXkPFXc=";

    fn address_data(address: &str, prefix: u32) -> Value<'static> {
        let mut entry = HashMap::new();
        entry.insert("address".to_string(), Value::from(address.to_string()));
        entry.insert("prefix".to_string(), Value::from(prefix));
        Value::from(vec![entry])
    }

    fn peer_string_array(peer: &zvariant::Dict<'_, '_>, key: &str) -> Vec<String> {
        let value = peer
            .iter()
            .find_map(|(candidate, value)| {
                matches!(candidate, Value::Str(candidate) if candidate.as_str() == key)
                    .then_some(value)
            })
            .unwrap_or_else(|| panic!("missing peer property {key}"));
        let Value::Value(value) = value else {
            panic!("peer property {key} must be stored as a variant");
        };
        let Value::Array(values) = value.as_ref() else {
            panic!("peer property {key} must be an array");
        };
        values
            .iter()
            .map(|value| match value {
                Value::Str(value) => value.as_str().to_string(),
                _ => panic!("peer property {key} entries must be strings"),
            })
            .collect()
    }

    fn create_test_peer() -> WireGuardPeer {
        WireGuardPeer {
            public_key: PUBLIC_KEY.into(),
            gateway: "vpn.example.com:51820".into(),
            allowed_ips: vec!["0.0.0.0/0".into()],
            preshared_key: None,
            persistent_keepalive: Some(25),
        }
    }

    fn build_test_connection(
        address: &str,
    ) -> HashMap<&'static str, HashMap<&'static str, Value<'static>>> {
        WireGuardBuilder::new("TestVPN")
            .private_key(PRIVATE_KEY)
            .address(address)
            .add_peer(create_test_peer())
            .autoconnect(false)
            .build()
            .expect("valid WireGuard settings")
    }

    #[test]
    fn builds_basic_wireguard_connection() {
        let settings = build_test_connection("10.0.0.2/24");

        let conn = settings.get("connection").unwrap();
        assert_eq!(conn.get("type"), Some(&Value::from("wireguard")));
        assert_eq!(conn.get("id"), Some(&Value::from("TestVPN")));
        assert_eq!(conn.get("interface-name"), Some(&Value::from("wg-testvpn")));
        assert_eq!(conn.get("autoconnect"), Some(&Value::from(false)));
        assert_eq!(
            conn.get("uuid"),
            Some(&Value::from(
                Uuid::new_v5(&Uuid::NAMESPACE_DNS, b"wg:TestVPN").to_string()
            ))
        );

        let ipv4 = settings.get("ipv4").unwrap();
        assert_eq!(ipv4.get("method"), Some(&Value::from("manual")));
        assert_eq!(
            ipv4.get("address-data"),
            Some(&address_data("10.0.0.2", 24))
        );
        assert_eq!(settings["ipv6"].get("method"), Some(&Value::from("ignore")));
    }

    #[test]
    fn serializes_complete_peer_payload() {
        let peer =
            create_test_peer().with_preshared_key("PSKABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklm=");
        let settings = WireGuardBuilder::new("TestVPN")
            .private_key(PRIVATE_KEY)
            .address("10.0.0.2/24")
            .add_peer(peer)
            .build()
            .unwrap();

        let wireguard = settings.get("wireguard").unwrap();
        assert_eq!(
            wireguard.get("private-key"),
            Some(&Value::from(PRIVATE_KEY))
        );
        let peers = wireguard.get("peers").unwrap();
        assert_eq!(peers.value_signature().to_string(), "aa{sv}");
        let Value::Array(peers) = peers else {
            panic!("wireguard.peers must be an array");
        };
        assert_eq!(peers.iter().count(), 1);
        let Value::Dict(peer) = peers.iter().next().unwrap() else {
            panic!("each wireguard peer must be a dictionary");
        };
        assert_eq!(
            peer.get::<Value, String>(&Value::from("public-key"))
                .unwrap()
                .as_deref(),
            Some(PUBLIC_KEY)
        );
        assert_eq!(
            peer.get::<Value, String>(&Value::from("endpoint"))
                .unwrap()
                .as_deref(),
            Some("vpn.example.com:51820")
        );
        assert_eq!(
            peer_string_array(peer, "allowed-ips"),
            vec!["0.0.0.0/0".to_string()]
        );
        assert_eq!(
            peer.get::<Value, String>(&Value::from("preshared-key"))
                .unwrap()
                .as_deref(),
            Some("PSKABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklm=")
        );
        assert_eq!(
            peer.get::<Value, u32>(&Value::from("persistent-keepalive"))
                .unwrap(),
            Some(25)
        );
    }

    #[test]
    fn requires_private_key() {
        let result = WireGuardBuilder::new("TestVPN")
            .address("10.0.0.2/24")
            .add_peer(create_test_peer())
            .build();

        assert!(matches!(
            result.unwrap_err(),
            ConnectionError::InvalidPrivateKey(message) if message == "Private key not set"
        ));
    }

    #[test]
    fn requires_address() {
        let result = WireGuardBuilder::new("TestVPN")
            .private_key(PRIVATE_KEY)
            .add_peer(create_test_peer())
            .build();

        assert!(matches!(
            result.unwrap_err(),
            ConnectionError::InvalidAddress(message) if message == "Address not set"
        ));
    }

    #[test]
    fn requires_at_least_one_peer() {
        let result = WireGuardBuilder::new("TestVPN")
            .private_key(PRIVATE_KEY)
            .address("10.0.0.2/24")
            .build();

        assert!(matches!(
            result.unwrap_err(),
            ConnectionError::InvalidPeers(message) if message == "No peers configured"
        ));
    }

    #[test]
    fn adds_dns_servers() {
        let settings = WireGuardBuilder::new("TestVPN")
            .private_key(PRIVATE_KEY)
            .address("10.0.0.2/24")
            .add_peer(create_test_peer())
            .dns(vec!["1.1.1.1".into(), "2001:4860:4860::8888".into()])
            .build()
            .expect("valid mixed-family DNS settings");

        let ipv4 = settings.get("ipv4").unwrap();
        assert_eq!(
            ipv4.get("dns"),
            Some(&Value::from(vec![u32::from(Ipv4Addr::new(1, 1, 1, 1))]))
        );
        assert_eq!(ipv4["dns"].value_signature().to_string(), "au");

        let ipv6 = settings.get("ipv6").unwrap();
        let expected_v6 = "2001:4860:4860::8888"
            .parse::<Ipv6Addr>()
            .unwrap()
            .octets()
            .to_vec();
        assert_eq!(ipv6.get("dns"), Some(&Value::from(vec![expected_v6])));
        assert_eq!(ipv6["dns"].value_signature().to_string(), "aay");
    }

    #[test]
    fn sets_mtu() {
        let settings = WireGuardBuilder::new("TestVPN")
            .private_key(PRIVATE_KEY)
            .address("10.0.0.2/24")
            .add_peer(create_test_peer())
            .mtu(1420)
            .build()
            .expect("Failed to build");

        let wireguard = settings.get("wireguard").unwrap();
        assert_eq!(wireguard.get("mtu"), Some(&Value::from(1420u32)));
        assert!(!settings["ipv4"].contains_key("mtu"));
    }

    #[test]
    fn supports_multiple_peers() {
        let peer1 = create_test_peer();
        let peer2 = WireGuardPeer {
            public_key: "xScVkH3fUGUVRvGLFcjkx+GGD7cf5eBVyN3Gh4FLjmI=".into(),
            gateway: "peer2.example.com:51821".into(),
            allowed_ips: vec!["192.168.0.0/16".into()],
            preshared_key: None,
            persistent_keepalive: None,
        };

        let settings = WireGuardBuilder::new("TestVPN")
            .private_key(PRIVATE_KEY)
            .address("10.0.0.2/24")
            .add_peers(vec![peer1, peer2])
            .build()
            .expect("Failed to build");

        let Value::Array(peers) = &settings["wireguard"]["peers"] else {
            panic!("wireguard.peers must be an array");
        };
        assert_eq!(peers.signature().to_string(), "aa{sv}");
        let peers = peers.iter().collect::<Vec<_>>();
        assert_eq!(peers.len(), 2);
        assert_eq!(peers[0].value_signature().to_string(), "a{sv}");
        assert_eq!(peers[1].value_signature().to_string(), "a{sv}");

        let Value::Dict(first) = peers[0] else {
            panic!("first wireguard peer must be a dictionary");
        };
        assert_eq!(
            first
                .get::<Value, String>(&Value::from("public-key"))
                .unwrap()
                .as_deref(),
            Some(PUBLIC_KEY)
        );
        assert_eq!(
            first
                .get::<Value, String>(&Value::from("endpoint"))
                .unwrap()
                .as_deref(),
            Some("vpn.example.com:51820")
        );
        assert_eq!(
            peer_string_array(first, "allowed-ips"),
            vec!["0.0.0.0/0".to_string()]
        );
        assert_eq!(
            first
                .get::<Value, u32>(&Value::from("persistent-keepalive"))
                .unwrap(),
            Some(25)
        );

        let Value::Dict(second) = peers[1] else {
            panic!("second wireguard peer must be a dictionary");
        };
        assert_eq!(
            second
                .get::<Value, String>(&Value::from("public-key"))
                .unwrap()
                .as_deref(),
            Some("xScVkH3fUGUVRvGLFcjkx+GGD7cf5eBVyN3Gh4FLjmI=")
        );
        assert_eq!(
            second
                .get::<Value, String>(&Value::from("endpoint"))
                .unwrap()
                .as_deref(),
            Some("peer2.example.com:51821")
        );
        assert_eq!(
            peer_string_array(second, "allowed-ips"),
            vec!["192.168.0.0/16".to_string()]
        );
        assert!(
            second
                .get::<Value, String>(&Value::from("preshared-key"))
                .unwrap()
                .is_none()
        );
        assert!(
            second
                .get::<Value, u32>(&Value::from("persistent-keepalive"))
                .unwrap()
                .is_none()
        );
    }

    #[test]
    fn serializes_ipv6_address_in_ipv6_section() {
        let settings = build_test_connection("fd00::2/64");

        assert_eq!(
            settings["ipv4"].get("method"),
            Some(&Value::from("disabled"))
        );
        assert!(!settings["ipv4"].contains_key("address-data"));
        assert_eq!(settings["ipv6"].get("method"), Some(&Value::from("manual")));
        assert_eq!(
            settings["ipv6"].get("address-data"),
            Some(&address_data("fd00::2", 64))
        );
    }

    #[test]
    fn rejects_invalid_ip_and_dns_addresses() {
        let invalid_address = WireGuardBuilder::new("TestVPN")
            .private_key(PRIVATE_KEY)
            .address("fd00::xyz/64")
            .add_peer(create_test_peer())
            .build();
        assert!(matches!(
            invalid_address.unwrap_err(),
            ConnectionError::InvalidAddress(message)
                if message == "invalid IP address: fd00::xyz"
        ));

        let invalid_dns = WireGuardBuilder::new("TestVPN")
            .private_key(PRIVATE_KEY)
            .address("10.0.0.2/24")
            .add_peer(create_test_peer())
            .dns(vec!["not-an-address".into()])
            .build();
        assert!(matches!(
            invalid_dns.unwrap_err(),
            ConnectionError::VpnFailed(message)
                if message == "Invalid DNS server address: not-an-address"
        ));
    }

    #[test]
    fn validates_gateway_host_and_ipv6_brackets() {
        for (gateway, expected) in [
            (":51820", "gateway host cannot be empty"),
            (
                "2001:db8::1:51820",
                "IPv6 gateway must use '[address]:port' format: 2001:db8::1:51820",
            ),
        ] {
            let mut peer = create_test_peer();
            peer.gateway = gateway.into();
            let result = WireGuardBuilder::new("TestVPN")
                .private_key(PRIVATE_KEY)
                .address("10.0.0.2/24")
                .add_peer(peer)
                .build();
            assert!(
                matches!(
                    result.unwrap_err(),
                    ConnectionError::InvalidGateway(message) if message == expected
                ),
                "gateway {gateway} should be rejected"
            );
        }

        let mut peer = create_test_peer();
        peer.public_key = "!".repeat(44);
        let result = WireGuardBuilder::new("TestVPN")
            .private_key(PRIVATE_KEY)
            .address("10.0.0.2/24")
            .add_peer(peer)
            .build();
        assert!(matches!(
            result.unwrap_err(),
            ConnectionError::InvalidPublicKey(message)
                if message == "Peer 0 public key contains invalid base64 characters"
        ));

        let mut peer = create_test_peer();
        peer.allowed_ips.clear();
        let result = WireGuardBuilder::new("TestVPN")
            .private_key(PRIVATE_KEY)
            .address("10.0.0.2/24")
            .add_peer(peer)
            .build();
        assert!(matches!(
            result.unwrap_err(),
            ConnectionError::InvalidPeers(message) if message == "Peer 0 has no allowed IPs"
        ));

        let mut peer = create_test_peer();
        peer.gateway = "[2001:db8::1]:51820".into();
        let settings = WireGuardBuilder::new("TestVPN")
            .private_key(PRIVATE_KEY)
            .address("fd00::2/64")
            .add_peer(peer)
            .build()
            .unwrap();
        assert_eq!(
            settings["ipv6"].get("address-data"),
            Some(&address_data("fd00::2", 64))
        );
    }
}
