//! Core connection builder for NetworkManager settings.
//!
//! This module provides a flexible builder API for constructing NetworkManager
//! connection settings dictionaries. The `ConnectionBuilder` handles common
//! sections like connection metadata, IPv4/IPv6 configuration, and allows
//! type-specific builders to add their own sections.
//!
//! # Design Philosophy
//!
//! The builder follows a "base + specialization" pattern:
//! - `ConnectionBuilder` handles common sections (connection, ipv4, ipv6)
//! - Type-specific builders (WifiConnectionBuilder, VpnBuilder, etc.) add
//!   connection-type-specific sections and provide ergonomic APIs
//!
//! # Example
//!
//! ```rust
//! use nmrs::builders::ConnectionBuilder;
//! use std::net::Ipv4Addr;
//!
//! let settings = ConnectionBuilder::new("802-3-ethernet", "eth0")
//!     .autoconnect(true)
//!     .ipv4_auto()
//!     .ipv6_auto()
//!     .build();
//! ```

use std::collections::HashMap;
use std::net::{Ipv4Addr, Ipv6Addr};
use uuid::Uuid;
use zbus::Variant;

use crate::api::models::ConnectionOptions;

/// IP address configuration with CIDR prefix.
#[derive(Debug, Clone)]
pub struct IpConfig {
    pub address: String,
    pub prefix: u32,
}

impl IpConfig {
    /// Creates a new IP configuration.
    #[must_use]
    pub fn new(address: impl Into<String>, prefix: u32) -> Self {
        Self {
            address: address.into(),
            prefix,
        }
    }
}

/// Route configuration for static routing.
#[derive(Debug, Clone)]
pub struct Route {
    pub dest: String,
    pub prefix: u32,
    pub next_hop: Option<String>,
    pub metric: Option<u32>,
}

impl Route {
    /// Creates a new route configuration.
    #[must_use]
    pub fn new(dest: impl Into<String>, prefix: u32) -> Self {
        Self {
            dest: dest.into(),
            prefix,
            next_hop: None,
            metric: None,
        }
    }

    /// Sets the next hop gateway for this route.
    #[must_use]
    pub fn next_hop(mut self, gateway: impl Into<String>) -> Self {
        self.next_hop = Some(gateway.into());
        self
    }

    /// Sets the metric (priority) for this route.
    #[must_use]
    pub fn metric(mut self, metric: u32) -> Self {
        self.metric = Some(metric);
        self
    }
}

/// Core connection settings builder.
///
/// This builder constructs the base NetworkManager connection settings dictionary
/// that all connection types share. Type-specific builders wrap this to add
/// their own sections.
///
/// # Sections Managed
///
/// - `connection`: Metadata (type, id, uuid, autoconnect settings)
/// - `ipv4`: IPv4 configuration (auto/manual/disabled/etc)
/// - `ipv6`: IPv6 configuration (auto/manual/ignore/etc)
///
/// # Usage Pattern
///
/// This builder is typically wrapped by type-specific builders like
/// `WifiConnectionBuilder` or `EthernetConnectionBuilder`. However, it can
/// be used directly for advanced use cases:
///
/// ```rust
/// use nmrs::builders::ConnectionBuilder;
///
/// let settings = ConnectionBuilder::new("802-11-wireless", "MyNetwork")
///     .autoconnect(true)
///     .autoconnect_priority(10)
///     .ipv4_auto()
///     .ipv6_auto()
///     .build();
/// ```
pub struct ConnectionBuilder {
    settings: HashMap<&'static str, HashMap<&'static str, Variant<'static>>>,
}

impl ConnectionBuilder {
    /// Creates a new connection builder with the specified type and ID.
    ///
    /// # Arguments
    ///
    /// * `connection_type` - NetworkManager connection type (e.g., "802-11-wireless",
    ///   "802-3-ethernet", "wireguard", "bridge", "bond", "vlan")
    /// * `id` - Human-readable connection identifier
    ///
    /// # Example
    ///
    /// ```rust
    /// use nmrs::builders::ConnectionBuilder;
    ///
    /// let builder = ConnectionBuilder::new("802-11-wireless", "HomeNetwork");
    /// ```
    #[must_use]
    pub fn new(connection_type: &str, id: impl Into<String>) -> Self {
        let mut settings = HashMap::new();
        let mut connection = HashMap::new();

        connection.insert("type", Variant::from(connection_type.to_string()));
        connection.insert("id", Variant::from(id.into()));
        connection.insert("uuid", Variant::from(Uuid::new_v4().to_string()));

        settings.insert("connection", connection);

        Self { settings }
    }

    /// Sets a specific UUID for the connection.
    ///
    /// By default, a random UUID is generated. Use this to specify a deterministic
    /// UUID for testing or when recreating existing connections.
    #[must_use]
    pub fn uuid(mut self, uuid: Uuid) -> Self {
        if let Some(conn) = self.settings.get_mut("connection") {
            conn.insert("uuid", Variant::from(uuid.to_string()));
        }
        self
    }

    /// Sets the network interface name for this connection.
    ///
    /// This restricts the connection to a specific interface (e.g., "wlan0", "eth0").
    #[must_use]
    pub fn interface_name(mut self, name: impl Into<String>) -> Self {
        if let Some(conn) = self.settings.get_mut("connection") {
            conn.insert("interface-name", Variant::from(name.into()));
        }
        self
    }

    /// Enables or disables automatic connection on boot/availability.
    #[must_use]
    pub fn autoconnect(mut self, enabled: bool) -> Self {
        if let Some(conn) = self.settings.get_mut("connection") {
            conn.insert("autoconnect", Variant::from(enabled));
        }
        self
    }

    /// Sets the autoconnect priority (higher values are preferred).
    ///
    /// When multiple connections are available, NetworkManager connects to the
    /// one with the highest priority. Default is 0.
    #[must_use]
    pub fn autoconnect_priority(mut self, priority: i32) -> Self {
        if let Some(conn) = self.settings.get_mut("connection") {
            conn.insert("autoconnect-priority", Variant::from(priority));
        }
        self
    }

    /// Sets the number of autoconnect retry attempts.
    ///
    /// After this many failed attempts, the connection won't auto-retry.
    /// Default is -1 (unlimited retries).
    #[must_use]
    pub fn autoconnect_retries(mut self, retries: i32) -> Self {
        if let Some(conn) = self.settings.get_mut("connection") {
            conn.insert("autoconnect-retries", Variant::from(retries));
        }
        self
    }

    /// Applies multiple connection options at once.
    ///
    /// This is a convenience method to apply all fields from `ConnectionOptions`.
    #[must_use]
    pub fn options(mut self, opts: &ConnectionOptions) -> Self {
        if let Some(conn) = self.settings.get_mut("connection") {
            conn.insert("autoconnect", Variant::from(opts.autoconnect));

            if let Some(priority) = opts.autoconnect_priority {
                conn.insert("autoconnect-priority", Variant::from(priority));
            }

            if let Some(retries) = opts.autoconnect_retries {
                conn.insert("autoconnect-retries", Variant::from(retries));
            }
        }
        self
    }

    /// Configures IPv4 to use automatic configuration (DHCP).
    #[must_use]
    pub fn ipv4_auto(mut self) -> Self {
        let mut ipv4 = HashMap::new();
        ipv4.insert("method", Variant::from("auto"));
        self.settings.insert("ipv4", ipv4);
        self
    }

    /// Configures IPv4 with manual (static) addresses.
    ///
    /// # Example
    ///
    /// ```rust
    /// use nmrs::builders::{ConnectionBuilder, IpConfig};
    ///
    /// let settings = ConnectionBuilder::new("802-3-ethernet", "eth0")
    ///     .ipv4_manual(vec![
    ///         IpConfig::new("192.168.1.100", 24),
    ///     ])
    ///     .build();
    /// ```
    #[must_use]
    pub fn ipv4_manual(mut self, addresses: Vec<IpConfig>) -> Self {
        let mut ipv4 = HashMap::new();
        ipv4.insert("method", Variant::from("manual"));

        // Convert to address-data format (array of dictionaries)
        let address_data: Vec<HashMap<String, Variant<'static>>> = addresses
            .into_iter()
            .map(|config| {
                let mut addr_dict = HashMap::new();
                addr_dict.insert("address".to_string(), Variant::from(config.address));
                addr_dict.insert("prefix".to_string(), Variant::from(config.prefix));
                addr_dict
            })
            .collect();

        ipv4.insert("address-data", Variant::from(address_data));
        self.settings.insert("ipv4", ipv4);
        self
    }

    /// Disables IPv4 for this connection.
    #[must_use]
    pub fn ipv4_disabled(mut self) -> Self {
        let mut ipv4 = HashMap::new();
        ipv4.insert("method", Variant::from("disabled"));
        self.settings.insert("ipv4", ipv4);
        self
    }

    /// Configures IPv4 to use link-local addressing (169.254.x.x).
    #[must_use]
    pub fn ipv4_link_local(mut self) -> Self {
        let mut ipv4 = HashMap::new();
        ipv4.insert("method", Variant::from("link-local"));
        self.settings.insert("ipv4", ipv4);
        self
    }

    /// Configures IPv4 for internet connection sharing.
    ///
    /// The connection will provide DHCP and NAT for other devices.
    #[must_use]
    pub fn ipv4_shared(mut self) -> Self {
        let mut ipv4 = HashMap::new();
        ipv4.insert("method", Variant::from("shared"));
        self.settings.insert("ipv4", ipv4);
        self
    }

    /// Configures IPv4 for internet connection sharing
    /// with a custom IP range
    ///
    /// The connection will provide DHCP and NAT for other devices.
    #[must_use]
    pub fn ipv4_shared_with_ip_range(mut self, addresses: Vec<IpConfig>) -> Self {
        let mut ipv4 = HashMap::new();
        ipv4.insert("method", Variant::from("shared"));

        let address_data: Vec<HashMap<String, Variant<'static>>> = addresses
            .into_iter()
            .map(|config| {
                let mut addr_dict = HashMap::new();
                addr_dict.insert("address".to_string(), Variant::from(config.address));
                addr_dict.insert("prefix".to_string(), Variant::from(config.prefix));
                addr_dict
            })
            .collect();
        ipv4.insert("address-data", Variant::from(address_data));

        self.settings.insert("ipv4", ipv4);
        self
    }

    /// Sets IPv4 DNS servers.
    ///
    /// NetworkManager's legacy `ipv4.dns` property is an array of `in_addr_t`,
    /// i.e. each `u32`'s in-memory bytes are the address octets in order
    /// (network byte order). `u32::from(Ipv4Addr)` yields the *host*-order
    /// integer instead, which NetworkManager reads back with the octets
    /// reversed on little-endian machines, so build the integer from the raw
    /// octets in native byte order.
    #[must_use]
    pub fn ipv4_dns(mut self, servers: Vec<Ipv4Addr>) -> Self {
        let dns_u32: Vec<u32> = servers
            .into_iter()
            .map(|server| u32::from_ne_bytes(server.octets()))
            .collect();

        if let Some(ipv4) = self.settings.get_mut("ipv4") {
            ipv4.insert("dns", Variant::from(dns_u32));
        }
        self
    }

    /// Sets the IPv4 gateway.
    #[must_use]
    pub fn ipv4_gateway(mut self, gateway: Ipv4Addr) -> Self {
        if let Some(ipv4) = self.settings.get_mut("ipv4") {
            ipv4.insert("gateway", Variant::from(gateway.to_string()));
        }
        self
    }

    /// Adds IPv4 static routes.
    #[must_use]
    pub fn ipv4_routes(mut self, routes: Vec<Route>) -> Self {
        let route_data: Vec<HashMap<String, Variant<'static>>> = routes
            .into_iter()
            .map(|route| {
                let mut route_dict = HashMap::new();
                route_dict.insert("dest".to_string(), Variant::from(route.dest));
                route_dict.insert("prefix".to_string(), Variant::from(route.prefix));

                if let Some(next_hop) = route.next_hop {
                    route_dict.insert("next-hop".to_string(), Variant::from(next_hop));
                }

                if let Some(metric) = route.metric {
                    route_dict.insert("metric".to_string(), Variant::from(metric));
                }

                route_dict
            })
            .collect();

        if let Some(ipv4) = self.settings.get_mut("ipv4") {
            ipv4.insert("route-data", Variant::from(route_data));
        }
        self
    }

    /// Configures IPv6 to use automatic configuration (SLAAC/DHCPv6).
    #[must_use]
    pub fn ipv6_auto(mut self) -> Self {
        let mut ipv6 = HashMap::new();
        ipv6.insert("method", Variant::from("auto"));
        self.settings.insert("ipv6", ipv6);
        self
    }

    /// Configures IPv6 with manual (static) addresses.
    #[must_use]
    pub fn ipv6_manual(mut self, addresses: Vec<IpConfig>) -> Self {
        let mut ipv6 = HashMap::new();
        ipv6.insert("method", Variant::from("manual"));

        let address_data: Vec<HashMap<String, Variant<'static>>> = addresses
            .into_iter()
            .map(|config| {
                let mut addr_dict = HashMap::new();
                addr_dict.insert("address".to_string(), Variant::from(config.address));
                addr_dict.insert("prefix".to_string(), Variant::from(config.prefix));
                addr_dict
            })
            .collect();

        ipv6.insert("address-data", Variant::from(address_data));
        self.settings.insert("ipv6", ipv6);
        self
    }

    /// Disables IPv6 for this connection.
    #[must_use]
    pub fn ipv6_ignore(mut self) -> Self {
        let mut ipv6 = HashMap::new();
        ipv6.insert("method", Variant::from("ignore"));
        self.settings.insert("ipv6", ipv6);
        self
    }

    /// Configures IPv6 to use link-local addressing only.
    #[must_use]
    pub fn ipv6_link_local(mut self) -> Self {
        let mut ipv6 = HashMap::new();
        ipv6.insert("method", Variant::from("link-local"));
        self.settings.insert("ipv6", ipv6);
        self
    }

    /// Sets IPv6 DNS servers.
    #[must_use]
    pub fn ipv6_dns(mut self, servers: Vec<Ipv6Addr>) -> Self {
        let dns_bytes: Vec<Vec<u8>> = servers
            .into_iter()
            .map(|server| server.octets().to_vec())
            .collect();

        if let Some(ipv6) = self.settings.get_mut("ipv6") {
            ipv6.insert("dns", Variant::from(dns_bytes));
        }
        self
    }

    /// Sets the IPv6 gateway.
    #[must_use]
    pub fn ipv6_gateway(mut self, gateway: Ipv6Addr) -> Self {
        if let Some(ipv6) = self.settings.get_mut("ipv6") {
            ipv6.insert("gateway", Variant::from(gateway.to_string()));
        }
        self
    }

    /// Adds IPv6 static routes.
    #[must_use]
    pub fn ipv6_routes(mut self, routes: Vec<Route>) -> Self {
        let route_data: Vec<HashMap<String, Variant<'static>>> = routes
            .into_iter()
            .map(|route| {
                let mut route_dict = HashMap::new();
                route_dict.insert("dest".to_string(), Variant::from(route.dest));
                route_dict.insert("prefix".to_string(), Variant::from(route.prefix));

                if let Some(next_hop) = route.next_hop {
                    route_dict.insert("next-hop".to_string(), Variant::from(next_hop));
                }

                if let Some(metric) = route.metric {
                    route_dict.insert("metric".to_string(), Variant::from(metric));
                }

                route_dict
            })
            .collect();

        if let Some(ipv6) = self.settings.get_mut("ipv6") {
            ipv6.insert("route-data", Variant::from(route_data));
        }
        self
    }

    /// Adds or replaces a complete settings section.
    ///
    /// This is useful for type-specific settings that don't have dedicated
    /// builder methods. For example, adding "802-11-wireless" or "wireguard"
    /// sections.
    ///
    /// # Example
    ///
    /// ```rust
    /// use nmrs::builders::ConnectionBuilder;
    /// use std::collections::HashMap;
    /// use zbus::Variant;
    ///
    /// let mut bridge_section = HashMap::new();
    /// bridge_section.insert("stp", Variant::from(true));
    ///
    /// let settings = ConnectionBuilder::new("bridge", "br0")
    ///     .with_section("bridge", bridge_section)
    ///     .build();
    /// ```
    #[must_use]
    pub fn with_section(
        mut self,
        name: &'static str,
        section: HashMap<&'static str, Variant<'static>>,
    ) -> Self {
        self.settings.insert(name, section);
        self
    }

    pub(crate) fn without_section(mut self, name: &'static str) -> Self {
        self.settings.remove(name);
        self
    }

    /// Updates an existing section using a closure.
    ///
    /// This allows modifying a section after it's been created, which is useful
    /// when a builder method creates a base section and you need to add extra fields.
    ///
    /// # Example
    ///
    /// ```rust
    /// use nmrs::builders::ConnectionBuilder;
    /// use zbus::Variant;
    ///
    /// let settings = ConnectionBuilder::new("802-3-ethernet", "eth0")
    ///     .ipv4_auto()
    ///     .update_section("ipv4", |ipv4| {
    ///         ipv4.insert("may-fail", Variant::from(false));
    ///     })
    ///     .build();
    /// ```
    #[must_use]
    pub fn update_section<F>(mut self, name: &'static str, f: F) -> Self
    where
        F: FnOnce(&mut HashMap<&'static str, Variant<'static>>),
    {
        if let Some(section) = self.settings.get_mut(name) {
            f(section);
        }
        self
    }

    /// Builds and returns the final settings dictionary.
    ///
    /// This consumes the builder and returns the complete settings structure
    /// ready to be passed to NetworkManager's D-Bus API.
    #[must_use]
    pub fn build(self) -> HashMap<&'static str, HashMap<&'static str, Variant<'static>>> {
        self.settings
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn address_data(address: &str, prefix: u32) -> Variant<'static> {
        let mut entry = HashMap::new();
        entry.insert("address".to_string(), Variant::from(address.to_string()));
        entry.insert("prefix".to_string(), Variant::from(prefix));
        Variant::from(vec![entry])
    }

    fn route_data(
        dest: &str,
        prefix: u32,
        next_hop: Option<&str>,
        metric: Option<u32>,
    ) -> Variant<'static> {
        let mut entry = HashMap::new();
        entry.insert("dest".to_string(), Variant::from(dest.to_string()));
        entry.insert("prefix".to_string(), Variant::from(prefix));
        if let Some(next_hop) = next_hop {
            entry.insert("next-hop".to_string(), Variant::from(next_hop.to_string()));
        }
        if let Some(metric) = metric {
            entry.insert("metric".to_string(), Variant::from(metric));
        }
        Variant::from(vec![entry])
    }

    #[test]
    fn creates_basic_connection() {
        let settings = ConnectionBuilder::new("802-11-wireless", "TestNetwork").build();

        assert!(settings.contains_key("connection"));
        let conn = settings.get("connection").unwrap();
        assert_eq!(conn.get("type"), Some(&Variant::from("802-11-wireless")));
        assert_eq!(conn.get("id"), Some(&Variant::from("TestNetwork")));
        assert!(conn.contains_key("uuid"));
    }

    #[test]
    fn sets_custom_uuid() {
        let test_uuid = Uuid::new_v4();
        let settings = ConnectionBuilder::new("802-3-ethernet", "eth0")
            .uuid(test_uuid)
            .build();

        let conn = settings.get("connection").unwrap();
        assert_eq!(
            conn.get("uuid"),
            Some(&Variant::from(test_uuid.to_string()))
        );
    }

    #[test]
    fn sets_interface_name() {
        let settings = ConnectionBuilder::new("802-3-ethernet", "MyConnection")
            .interface_name("eth0")
            .build();

        let conn = settings.get("connection").unwrap();
        assert_eq!(conn.get("interface-name"), Some(&Variant::from("eth0")));
    }

    #[test]
    fn configures_autoconnect() {
        let settings = ConnectionBuilder::new("802-11-wireless", "test")
            .autoconnect(false)
            .autoconnect_priority(10)
            .autoconnect_retries(3)
            .build();

        let conn = settings.get("connection").unwrap();
        assert_eq!(conn.get("autoconnect"), Some(&Variant::from(false)));
        assert_eq!(
            conn.get("autoconnect-priority"),
            Some(&Variant::from(10i32))
        );
        assert_eq!(conn.get("autoconnect-retries"), Some(&Variant::from(3i32)));
    }

    #[test]
    fn applies_connection_options() {
        let opts = ConnectionOptions {
            autoconnect: true,
            autoconnect_priority: Some(5),
            autoconnect_retries: Some(2),
        };

        let settings = ConnectionBuilder::new("802-3-ethernet", "eth0")
            .options(&opts)
            .build();

        let conn = settings.get("connection").unwrap();
        assert_eq!(conn.get("autoconnect"), Some(&Variant::from(true)));
        assert_eq!(conn.get("autoconnect-priority"), Some(&Variant::from(5i32)));
        assert_eq!(conn.get("autoconnect-retries"), Some(&Variant::from(2i32)));
    }

    #[test]
    fn omits_unset_optional_connection_options() {
        let settings = ConnectionBuilder::new("802-3-ethernet", "eth0")
            .options(&ConnectionOptions::new(false))
            .build();

        let conn = settings.get("connection").unwrap();
        assert_eq!(conn.get("autoconnect"), Some(&Variant::from(false)));
        assert!(!conn.contains_key("autoconnect-priority"));
        assert!(!conn.contains_key("autoconnect-retries"));
    }

    #[test]
    fn configures_ipv4_auto() {
        let settings = ConnectionBuilder::new("802-3-ethernet", "eth0")
            .ipv4_auto()
            .build();

        let ipv4 = settings.get("ipv4").unwrap();
        assert_eq!(ipv4.get("method"), Some(&Variant::from("auto")));
    }

    #[test]
    fn configures_ipv4_manual() {
        let settings = ConnectionBuilder::new("802-3-ethernet", "eth0")
            .ipv4_manual(vec![IpConfig::new("192.168.1.100", 24)])
            .build();

        let ipv4 = settings.get("ipv4").unwrap();
        assert_eq!(ipv4.get("method"), Some(&Variant::from("manual")));
        let addresses = ipv4.get("address-data").unwrap();
        assert_eq!(addresses.value_signature().to_string(), "aa{sv}");
        assert_eq!(addresses, &address_data("192.168.1.100", 24));
    }

    #[test]
    fn configures_ipv4_disabled() {
        let settings = ConnectionBuilder::new("802-3-ethernet", "eth0")
            .ipv4_disabled()
            .build();

        let ipv4 = settings.get("ipv4").unwrap();
        assert_eq!(ipv4.get("method"), Some(&Variant::from("disabled")));
    }

    #[test]
    fn configures_ipv4_link_local() {
        let link_local = ConnectionBuilder::new("802-3-ethernet", "local")
            .ipv4_link_local()
            .build();
        assert_eq!(
            link_local["ipv4"].get("method"),
            Some(&Variant::from("link-local"))
        );
    }

    #[test]
    fn configures_ipv4_shared() {
        let shared = ConnectionBuilder::new("802-3-ethernet", "shared")
            .ipv4_shared()
            .build();
        assert_eq!(shared["ipv4"].get("method"), Some(&Variant::from("shared")));
    }

    #[test]
    fn configures_ipv4_shared_with_ip_range() {
        let settings = ConnectionBuilder::new("802-3-ethernet", "shared")
            .ipv4_shared_with_ip_range(vec![IpConfig::new("192.168.1.100", 24)])
            .build();
        let shared = settings.get("ipv4").unwrap();
        assert_eq!(shared.get("method"), Some(&Variant::from("shared")));
        let addresses = shared.get("address-data").unwrap();
        assert_eq!(addresses.value_signature().to_string(), "aa{sv}");
        assert_eq!(addresses, &address_data("192.168.1.100", 24));
    }

    /// Decodes NetworkManager's `ipv4.dns` payload back into addresses.
    ///
    /// Each entry is an `in_addr_t`, so the address octets are the `u32`'s
    /// in-memory bytes. Deliberately expressed as the inverse of the contract
    /// rather than by reusing the builder's own conversion.
    fn decode_ipv4_dns(value: &Variant<'_>) -> Vec<Ipv4Addr> {
        <Vec<u32>>::try_from(value.try_clone().unwrap())
            .unwrap()
            .into_iter()
            .map(|raw| Ipv4Addr::from(raw.to_ne_bytes()))
            .collect()
    }

    #[test]
    fn configures_ipv4_dns() {
        // Non-palindromic on purpose: `1.1.1.1`-style addresses are byte-order
        // agnostic and hid the octet reversal in pop-os/cosmic-settings #2108.
        let dns: Vec<Ipv4Addr> = vec![
            "10.2.0.1".parse().unwrap(),
            "192.168.10.53".parse().unwrap(),
        ];
        let settings = ConnectionBuilder::new("802-3-ethernet", "eth0")
            .ipv4_auto()
            .ipv4_dns(dns.clone())
            .build();

        let ipv4 = settings.get("ipv4").unwrap();
        let value = ipv4.get("dns").unwrap();
        assert_eq!(value.value_signature().to_string(), "au");
        assert_eq!(decode_ipv4_dns(value), dns);
    }

    /// Regression test for pop-os/cosmic-settings #2108: `10.2.0.1` reached NetworkManager as
    /// `1.0.2.10` because the builder sent the host-order integer.
    #[test]
    #[cfg(target_endian = "little")]
    fn ipv4_dns_uses_network_byte_order() {
        let settings = ConnectionBuilder::new("802-3-ethernet", "eth0")
            .ipv4_auto()
            .ipv4_dns(vec!["10.2.0.1".parse().unwrap()])
            .build();

        let raw = <Vec<u32>>::try_from(settings["ipv4"].get("dns").unwrap().try_clone().unwrap())
            .unwrap();

        assert_eq!(raw, vec![0x0100_020A]);
        assert_ne!(raw, vec![u32::from(Ipv4Addr::new(10, 2, 0, 1))]);
    }

    #[test]
    fn configures_ipv4_gateway_and_routes() {
        let settings = ConnectionBuilder::new("802-3-ethernet", "eth0")
            .ipv4_manual(vec![IpConfig::new("192.168.1.100", 24)])
            .ipv4_gateway("192.168.1.1".parse().unwrap())
            .ipv4_routes(vec![
                Route::new("10.0.0.0", 8)
                    .next_hop("192.168.1.254")
                    .metric(25),
            ])
            .build();

        let ipv4 = settings.get("ipv4").unwrap();
        assert_eq!(ipv4.get("gateway"), Some(&Variant::from("192.168.1.1")));
        let routes = ipv4.get("route-data").unwrap();
        assert_eq!(routes.value_signature().to_string(), "aa{sv}");
        assert_eq!(
            routes,
            &route_data("10.0.0.0", 8, Some("192.168.1.254"), Some(25))
        );
    }

    #[test]
    fn configures_ipv6_auto() {
        let settings = ConnectionBuilder::new("802-3-ethernet", "eth0")
            .ipv6_auto()
            .build();

        let ipv6 = settings.get("ipv6").unwrap();
        assert_eq!(ipv6.get("method"), Some(&Variant::from("auto")));
    }

    #[test]
    fn configures_ipv6_manual() {
        let settings = ConnectionBuilder::new("802-3-ethernet", "eth0")
            .ipv6_manual(vec![IpConfig::new("2001:db8::10", 64)])
            .build();

        let ipv6 = settings.get("ipv6").unwrap();
        assert_eq!(ipv6.get("method"), Some(&Variant::from("manual")));
        let addresses = ipv6.get("address-data").unwrap();
        assert_eq!(addresses.value_signature().to_string(), "aa{sv}");
        assert_eq!(addresses, &address_data("2001:db8::10", 64));
    }

    #[test]
    fn configures_ipv6_ignore() {
        let settings = ConnectionBuilder::new("802-3-ethernet", "eth0")
            .ipv6_ignore()
            .build();

        let ipv6 = settings.get("ipv6").unwrap();
        assert_eq!(ipv6.get("method"), Some(&Variant::from("ignore")));
    }

    #[test]
    fn configures_ipv6_link_local() {
        let settings = ConnectionBuilder::new("802-3-ethernet", "eth0")
            .ipv6_link_local()
            .build();

        assert_eq!(
            settings["ipv6"].get("method"),
            Some(&Variant::from("link-local"))
        );
    }

    #[test]
    fn configures_ipv6_dns_gateway_and_routes() {
        let dns: Vec<Ipv6Addr> = vec![
            "2001:4860:4860::8888".parse().unwrap(),
            "2606:4700:4700::1111".parse().unwrap(),
        ];
        let settings = ConnectionBuilder::new("802-3-ethernet", "eth0")
            .ipv6_manual(vec![IpConfig::new("2001:db8::10", 64)])
            .ipv6_dns(dns.clone())
            .ipv6_gateway("2001:db8::1".parse().unwrap())
            .ipv6_routes(vec![
                Route::new("2001:db8:1::", 64)
                    .next_hop("2001:db8::2")
                    .metric(50),
            ])
            .build();

        let ipv6 = settings.get("ipv6").unwrap();
        let dns_value = ipv6.get("dns").unwrap();
        assert_eq!(dns_value.value_signature().to_string(), "aay");
        assert_eq!(
            dns_value,
            &Variant::from(
                dns.into_iter()
                    .map(|server| server.octets().to_vec())
                    .collect::<Vec<_>>()
            )
        );
        assert_eq!(ipv6.get("gateway"), Some(&Variant::from("2001:db8::1")));
        let routes = ipv6.get("route-data").unwrap();
        assert_eq!(routes.value_signature().to_string(), "aa{sv}");
        assert_eq!(
            routes,
            &route_data("2001:db8:1::", 64, Some("2001:db8::2"), Some(50))
        );
    }

    #[test]
    fn adds_custom_section() {
        let mut bridge = HashMap::new();
        bridge.insert("stp", Variant::from(true));

        let settings = ConnectionBuilder::new("bridge", "br0")
            .with_section("bridge", bridge)
            .build();

        assert!(settings.contains_key("bridge"));
        let bridge_section = settings.get("bridge").unwrap();
        assert_eq!(bridge_section.get("stp"), Some(&Variant::from(true)));
    }

    #[test]
    fn updates_existing_section() {
        let settings = ConnectionBuilder::new("802-3-ethernet", "eth0")
            .ipv4_auto()
            .update_section("ipv4", |ipv4| {
                ipv4.insert("may-fail", Variant::from(false));
            })
            .build();

        let ipv4 = settings.get("ipv4").unwrap();
        assert_eq!(ipv4.get("may-fail"), Some(&Variant::from(false)));
    }

    #[test]
    fn configures_complete_static_ipv4() {
        let settings = ConnectionBuilder::new("802-3-ethernet", "eth0")
            .ipv4_manual(vec![IpConfig::new("192.168.1.100", 24)])
            .ipv4_gateway("192.168.1.1".parse().unwrap())
            .ipv4_dns(vec!["192.168.1.53".parse().unwrap()])
            .build();

        let ipv4 = settings.get("ipv4").unwrap();
        assert_eq!(ipv4.get("method"), Some(&Variant::from("manual")));
        assert_eq!(
            ipv4.get("address-data"),
            Some(&address_data("192.168.1.100", 24))
        );
        assert_eq!(ipv4.get("gateway"), Some(&Variant::from("192.168.1.1")));
        assert_eq!(
            decode_ipv4_dns(ipv4.get("dns").unwrap()),
            vec![Ipv4Addr::new(192, 168, 1, 53)]
        );
    }
}
