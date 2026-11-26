//! Bond connection builders.
//!
//! [`BondBuilder`] creates the bond itself; [`BondPortBuilder`] creates a
//! separate port profile for an interface, like
//! `nmcli connection add type bond-slave`. Save both with
//! [`NetworkManager::add_connection`](crate::NetworkManager::add_connection)
//! and activate them separately.

use std::collections::{BTreeMap, HashMap};
use std::fmt;
use uuid::Uuid;
use zvariant::Value;

use super::bridge_builder::{check_interface_name, invalid, set_id};
use super::connection_builder::ConnectionBuilder;
use crate::api::models::{ConnectionError, ConnectionOptions};

/// Bonding mode, written as `mode` in `bond.options`.
///
/// # Example
///
/// ```rust
/// use nmrs::builders::BondMode;
///
/// assert_eq!(BondMode::ActiveBackup.as_str(), "active-backup");
/// assert_eq!(BondMode::Ieee8023ad.to_string(), "802.3ad");
/// ```
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BondMode {
    /// Round-robin transmission across ports (`balance-rr`).
    BalanceRr,
    /// One active port, others take over on failure (`active-backup`).
    ActiveBackup,
    /// Transmit policy based on a hash of packet headers (`balance-xor`).
    BalanceXor,
    /// Transmit on all ports (`broadcast`).
    Broadcast,
    /// IEEE 802.3ad dynamic link aggregation, LACP (`802.3ad`).
    Ieee8023ad,
    /// Adaptive transmit load balancing (`balance-tlb`).
    BalanceTlb,
    /// Adaptive load balancing, transmit and receive (`balance-alb`).
    BalanceAlb,
}

impl BondMode {
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::BalanceRr => "balance-rr",
            Self::ActiveBackup => "active-backup",
            Self::BalanceXor => "balance-xor",
            Self::Broadcast => "broadcast",
            Self::Ieee8023ad => "802.3ad",
            Self::BalanceTlb => "balance-tlb",
            Self::BalanceAlb => "balance-alb",
        }
    }
}

impl fmt::Display for BondMode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Builder for bond (link aggregation) connections.
///
/// Bonding options are written to `bond.options` as given; NetworkManager
/// validates them when the profile is saved. Unset options use
/// NetworkManager's defaults (mode `balance-rr`). The bond gets
/// `ipv4`/`ipv6` `auto`.
///
/// # Example
///
/// ```rust
/// use nmrs::builders::{BondBuilder, BondMode};
///
/// let settings = BondBuilder::new("bond0")
///     .mode(BondMode::ActiveBackup)
///     .miimon(100)
///     .autoconnect(false)
///     .build()
///     .expect("valid bond");
/// ```
#[non_exhaustive]
pub struct BondBuilder {
    inner: ConnectionBuilder,
    interface_name: String,
    options: BTreeMap<String, String>,
}

impl BondBuilder {
    /// Creates a bond builder for `interface_name` (e.g. `"bond0"`, max 15
    /// bytes). The connection name defaults to the interface name.
    #[must_use]
    pub fn new(interface_name: impl Into<String>) -> Self {
        let interface_name = interface_name.into();
        Self {
            inner: ConnectionBuilder::new("bond", &interface_name),
            interface_name,
            options: BTreeMap::new(),
        }
    }

    /// Sets a human-readable connection name.
    #[must_use]
    pub fn connection_name(mut self, name: impl Into<String>) -> Self {
        self.inner = set_id(self.inner, name.into());
        self
    }

    /// Sets the bonding mode (`mode`).
    #[must_use]
    pub fn mode(self, mode: BondMode) -> Self {
        self.option("mode", mode.as_str())
    }

    /// Sets the MII link monitoring interval in milliseconds (`miimon`).
    #[must_use]
    pub fn miimon(self, interval_ms: u32) -> Self {
        self.option("miimon", interval_ms.to_string())
    }

    /// Sets the preferred port interface (`primary`).
    #[must_use]
    pub fn primary(self, interface_name: impl Into<String>) -> Self {
        self.option("primary", interface_name)
    }

    /// Sets a bonding option, e.g. `("lacp_rate", "fast")`. A later call
    /// with the same key, including [`mode`](Self::mode), replaces it.
    #[must_use]
    pub fn option(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.options.insert(key.into(), value.into());
        self
    }

    /// Sets a specific UUID for the connection. Random by default.
    #[must_use]
    pub fn uuid(mut self, uuid: Uuid) -> Self {
        self.inner = self.inner.uuid(uuid);
        self
    }

    /// Applies connection options.
    #[must_use]
    pub fn options(mut self, opts: &ConnectionOptions) -> Self {
        self.inner = self.inner.options(opts);
        self
    }

    /// Enables or disables automatic connection.
    ///
    /// NetworkManager activates an autoconnect bond as soon as it is saved.
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

    /// Builds the bond connection settings.
    ///
    /// # Errors
    ///
    /// Returns `ConnectionError::InvalidInput` if the interface name is not a
    /// valid kernel interface name (1–15 bytes, no `/`, `:`, or whitespace,
    /// not `.` or `..`).
    #[must_use = "the connection settings must be passed to NetworkManager"]
    pub fn build(
        self,
    ) -> Result<HashMap<&'static str, HashMap<&'static str, Value<'static>>>, ConnectionError> {
        check_interface_name(&self.interface_name)?;

        let mut bond = HashMap::new();
        if !self.options.is_empty() {
            let options: HashMap<String, String> = self.options.into_iter().collect();
            bond.insert("options", Value::from(options));
        }

        Ok(self
            .inner
            .interface_name(self.interface_name)
            .with_section("bond", bond)
            .ipv4_auto()
            .ipv6_auto()
            .build())
    }
}

/// Builder for a bond port: a separate wired profile attached to a bond.
///
/// Mirrors `nmcli connection add type bond-slave ifname eth0 master bond0`.
/// The result is an `802-3-ethernet` profile with `connection.master` set to
/// the bond and `connection.slave-type = "bond"`, and no `ipv4`/`ipv6`
/// sections: NetworkManager forbids IP configuration on ports. Saving it
/// leaves any existing profile for the interface untouched; activating it
/// takes the interface off whatever it was using.
///
/// The legacy `master`/`slave-type` keys are used because NetworkManager
/// accepts them on every version; 1.46+ exposes them as `controller` and
/// `port-type` too. No `bond-port` section is written: it only exists on
/// NetworkManager 1.34+, which adds it to the saved profile itself.
///
/// # Example
///
/// ```rust
/// use nmrs::builders::BondPortBuilder;
///
/// let settings = BondPortBuilder::new("bond0", "eth0")
///     .autoconnect(false)
///     .build()
///     .expect("valid port");
/// assert!(!settings.contains_key("ipv4"));
/// ```
#[non_exhaustive]
pub struct BondPortBuilder {
    inner: ConnectionBuilder,
    bond: String,
    interface_name: String,
}

impl BondPortBuilder {
    /// Creates a port builder attaching `interface_name` to `bond`.
    ///
    /// `bond` is the bond interface name or its profile UUID. The
    /// connection name defaults to `bond-port-<interface_name>`.
    #[must_use]
    pub fn new(bond: impl Into<String>, interface_name: impl Into<String>) -> Self {
        let interface_name = interface_name.into();
        Self {
            inner: ConnectionBuilder::new("802-3-ethernet", format!("bond-port-{interface_name}")),
            bond: bond.into(),
            interface_name,
        }
    }

    /// Sets a human-readable connection name.
    #[must_use]
    pub fn connection_name(mut self, name: impl Into<String>) -> Self {
        self.inner = set_id(self.inner, name.into());
        self
    }

    /// Sets a specific UUID for the connection. Random by default.
    #[must_use]
    pub fn uuid(mut self, uuid: Uuid) -> Self {
        self.inner = self.inner.uuid(uuid);
        self
    }

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

    /// Builds the bond port connection settings.
    ///
    /// # Errors
    ///
    /// Returns `ConnectionError::InvalidInput` if `bond` is empty or the
    /// interface name is not a valid kernel interface name (1–15 bytes, no
    /// `/`, `:`, or whitespace, not `.` or `..`).
    #[must_use = "the connection settings must be passed to NetworkManager"]
    pub fn build(
        self,
    ) -> Result<HashMap<&'static str, HashMap<&'static str, Value<'static>>>, ConnectionError> {
        if self.bond.is_empty() {
            return Err(invalid("bond", "must not be empty"));
        }
        check_interface_name(&self.interface_name)?;

        let bond = self.bond;
        Ok(self
            .inner
            .interface_name(self.interface_name)
            .update_section("connection", |s| {
                s.insert("master", Value::from(bond));
                s.insert("slave-type", Value::from("bond"));
            })
            .with_section("802-3-ethernet", HashMap::new())
            .build())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn invalid_field(
        result: Result<
            HashMap<&'static str, HashMap<&'static str, Value<'static>>>,
            ConnectionError,
        >,
    ) -> String {
        match result {
            Err(ConnectionError::InvalidInput { field, .. }) => field,
            other => panic!("expected InvalidInput, got {other:?}"),
        }
    }

    fn bond_options(
        conn: &HashMap<&'static str, HashMap<&'static str, Value<'static>>>,
    ) -> HashMap<String, String> {
        HashMap::<String, String>::try_from(conn["bond"]["options"].clone())
            .expect("bond.options must be a{ss}")
    }

    #[test]
    fn mode_names_match_networkmanager() {
        for (mode, name) in [
            (BondMode::BalanceRr, "balance-rr"),
            (BondMode::ActiveBackup, "active-backup"),
            (BondMode::BalanceXor, "balance-xor"),
            (BondMode::Broadcast, "broadcast"),
            (BondMode::Ieee8023ad, "802.3ad"),
            (BondMode::BalanceTlb, "balance-tlb"),
            (BondMode::BalanceAlb, "balance-alb"),
        ] {
            assert_eq!(mode.as_str(), name);
            assert_eq!(mode.to_string(), name);
        }
    }

    #[test]
    fn builds_basic_bond_connection() {
        let conn = BondBuilder::new("bond0")
            .options(&ConnectionOptions {
                autoconnect: true,
                autoconnect_priority: Some(10),
                autoconnect_retries: Some(3),
            })
            .build()
            .unwrap();
        let connection = &conn["connection"];

        assert_eq!(connection.get("type"), Some(&Value::from("bond")));
        assert_eq!(connection.get("id"), Some(&Value::from("bond0")));
        assert_eq!(
            connection.get("interface-name"),
            Some(&Value::from("bond0"))
        );
        assert_eq!(connection.get("autoconnect"), Some(&Value::from(true)));
        assert_eq!(
            connection.get("autoconnect-priority"),
            Some(&Value::from(10i32))
        );
        assert_eq!(
            connection.get("autoconnect-retries"),
            Some(&Value::from(3i32))
        );
        assert!(conn["bond"].is_empty());
        assert_eq!(conn["ipv4"].get("method"), Some(&Value::from("auto")));
        assert_eq!(conn["ipv6"].get("method"), Some(&Value::from("auto")));
    }

    #[test]
    fn serializes_bond_options_as_string_dict() {
        let conn = BondBuilder::new("bond0")
            .mode(BondMode::ActiveBackup)
            .miimon(100)
            .primary("eth0")
            .option("updelay", "200")
            .build()
            .unwrap();

        assert_eq!(
            conn["bond"]["options"].value_signature().to_string(),
            "a{ss}"
        );
        let options = bond_options(&conn);
        assert_eq!(options.len(), 4);
        assert_eq!(options["mode"], "active-backup");
        assert_eq!(options["miimon"], "100");
        assert_eq!(options["primary"], "eth0");
        assert_eq!(options["updelay"], "200");
    }

    #[test]
    fn passes_option_combinations_through_to_networkmanager() {
        let conn = BondBuilder::new("bond0")
            .mode(BondMode::BalanceRr)
            .primary("eth0")
            .build()
            .unwrap();
        let options = bond_options(&conn);
        assert_eq!(options["mode"], "balance-rr");
        assert_eq!(options["primary"], "eth0");
    }

    #[test]
    fn later_mode_setting_wins() {
        let conn = BondBuilder::new("bond0")
            .option("mode", "balance-xor")
            .mode(BondMode::Ieee8023ad)
            .build()
            .unwrap();
        assert_eq!(bond_options(&conn)["mode"], "802.3ad");

        let conn = BondBuilder::new("bond0")
            .mode(BondMode::Ieee8023ad)
            .option("mode", "balance-xor")
            .build()
            .unwrap();
        assert_eq!(bond_options(&conn)["mode"], "balance-xor");
    }

    #[test]
    fn uses_custom_connection_name_and_uuid() {
        let uuid = Uuid::new_v4();
        let conn = BondBuilder::new("bond0")
            .connection_name("Uplink")
            .uuid(uuid)
            .build()
            .unwrap();

        assert_eq!(conn["connection"].get("id"), Some(&Value::from("Uplink")));
        assert_eq!(
            conn["connection"].get("uuid"),
            Some(&Value::from(uuid.to_string()))
        );
    }

    #[test]
    fn uuid_is_unique() {
        let a = BondBuilder::new("bond0").build().unwrap();
        let b = BondBuilder::new("bond0").build().unwrap();
        assert_ne!(a["connection"]["uuid"], b["connection"]["uuid"]);
    }

    #[test]
    fn rejects_bad_interface_name() {
        for name in [
            "",
            "0123456789abcdef",
            ".",
            "..",
            "bond 0",
            "bond/0",
            "bond:0",
        ] {
            assert_eq!(
                invalid_field(BondBuilder::new(name).build()),
                "interface_name"
            );
        }
    }

    #[test]
    fn builds_port_profile_without_ip() {
        let conn = BondPortBuilder::new("bond0", "eth0")
            .autoconnect(false)
            .build()
            .unwrap();
        let connection = &conn["connection"];

        assert_eq!(connection.get("type"), Some(&Value::from("802-3-ethernet")));
        assert_eq!(connection.get("id"), Some(&Value::from("bond-port-eth0")));
        assert_eq!(connection.get("interface-name"), Some(&Value::from("eth0")));
        assert_eq!(connection.get("master"), Some(&Value::from("bond0")));
        assert_eq!(connection.get("slave-type"), Some(&Value::from("bond")));
        assert!(!connection.contains_key("controller"));
        assert!(!connection.contains_key("port-type"));
        assert_eq!(connection.get("autoconnect"), Some(&Value::from(false)));
        assert!(conn.contains_key("802-3-ethernet"));
        assert!(!conn.contains_key("bond-port"));
        assert!(!conn.contains_key("ipv4"));
        assert!(!conn.contains_key("ipv6"));
    }

    #[test]
    fn port_accepts_bond_profile_uuid() {
        let bond_uuid = Uuid::new_v4().to_string();
        let conn = BondPortBuilder::new(bond_uuid.clone(), "eth0")
            .build()
            .unwrap();
        assert_eq!(
            conn["connection"].get("master"),
            Some(&Value::from(bond_uuid))
        );
    }

    #[test]
    fn port_uses_custom_connection_name_and_uuid() {
        let uuid = Uuid::new_v4();
        let conn = BondPortBuilder::new("bond0", "eth0")
            .connection_name("uplink")
            .uuid(uuid)
            .build()
            .unwrap();
        assert_eq!(conn["connection"].get("id"), Some(&Value::from("uplink")));
        assert_eq!(
            conn["connection"].get("uuid"),
            Some(&Value::from(uuid.to_string()))
        );
    }

    #[test]
    fn port_rejects_invalid_input() {
        for (field, builder) in [
            ("bond", BondPortBuilder::new("", "eth0")),
            ("interface_name", BondPortBuilder::new("bond0", "")),
            (
                "interface_name",
                BondPortBuilder::new("bond0", "0123456789abcdef"),
            ),
            ("interface_name", BondPortBuilder::new("bond0", "eth 0")),
        ] {
            assert_eq!(invalid_field(builder.build()), field);
        }
    }
}
