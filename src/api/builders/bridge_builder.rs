//! Bridge connection builders.
//!
//! [`BridgeBuilder`] creates the bridge itself; [`BridgePortBuilder`] creates
//! a separate port profile for an interface, like
//! `nmcli connection add type bridge-slave`. Save both with
//! [`NetworkManager::add_connection`](crate::NetworkManager::add_connection)
//! and activate them separately.

use std::collections::HashMap;
use uuid::Uuid;
use zvariant::Value;

use super::connection_builder::ConnectionBuilder;
use crate::api::models::{ConnectionError, ConnectionOptions};

/// Linux `IFNAMSIZ` minus the trailing NUL.
const MAX_INTERFACE_NAME_LEN: usize = 15;

/// Builder for bridge (virtual switch) connections.
///
/// Unset bridge options use NetworkManager's defaults. The bridge gets
/// `ipv4`/`ipv6` `auto`. Options are validated at [`build`](Self::build).
///
/// # Example
///
/// ```rust
/// use nmrs::builders::BridgeBuilder;
///
/// let settings = BridgeBuilder::new("br0")
///     .stp(true)
///     .priority(32768)
///     .autoconnect(false)
///     .build()
///     .expect("valid bridge");
/// ```
#[non_exhaustive]
pub struct BridgeBuilder {
    inner: ConnectionBuilder,
    interface_name: String,
    stp: Option<bool>,
    priority: Option<u16>,
    forward_delay: Option<u32>,
    hello_time: Option<u32>,
    max_age: Option<u32>,
    ageing_time: Option<u32>,
}

impl BridgeBuilder {
    /// Creates a bridge builder for `interface_name` (e.g. `"br0"`, max 15
    /// bytes). The connection name defaults to the interface name.
    #[must_use]
    pub fn new(interface_name: impl Into<String>) -> Self {
        let interface_name = interface_name.into();
        Self {
            inner: ConnectionBuilder::new("bridge", &interface_name),
            interface_name,
            stp: None,
            priority: None,
            forward_delay: None,
            hello_time: None,
            max_age: None,
            ageing_time: None,
        }
    }

    /// Sets a human-readable connection name.
    #[must_use]
    pub fn connection_name(mut self, name: impl Into<String>) -> Self {
        self.inner = set_id(self.inner, name.into());
        self
    }

    /// Enables or disables STP
    #[must_use]
    pub fn stp(mut self, stp: bool) -> Self {
        self.stp = Some(stp);
        self
    }

    /// Sets the STP bridge priority (`bridge.priority`).
    #[must_use]
    pub fn priority(mut self, priority: u16) -> Self {
        self.priority = Some(priority);
        self
    }

    /// Sets the STP forward delay in seconds (2-30; 0 allowed with STP disabled).
    #[must_use]
    pub fn forward_delay(mut self, seconds: u32) -> Self {
        self.forward_delay = Some(seconds);
        self
    }

    /// Sets the STP hello time in seconds (1-10; 0 allowed with STP disabled).
    #[must_use]
    pub fn hello_time(mut self, seconds: u32) -> Self {
        self.hello_time = Some(seconds);
        self
    }

    /// Sets the STP max age in seconds (6-40; 0 allowed with STP disabled).
    #[must_use]
    pub fn max_age(mut self, seconds: u32) -> Self {
        self.max_age = Some(seconds);
        self
    }

    /// Sets the MAC address ageing time in seconds (0-1000000).
    #[must_use]
    pub fn ageing_time(mut self, seconds: u32) -> Self {
        self.ageing_time = Some(seconds);
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
    /// NetworkManager activates an autoconnect bridge as soon as it is saved.
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

    /// Builds the bridge connection settings.
    ///
    /// # Errors
    ///
    /// Returns `ConnectionError::InvalidInput` if the interface name is not a
    /// valid kernel interface name (1–15 bytes, no `/`, `:`, or whitespace,
    /// not `.` or `..`), or an STP timer or ageing time is out of range.
    #[must_use = "the connection settings must be passed to NetworkManager"]
    pub fn build(
        self,
    ) -> Result<HashMap<&'static str, HashMap<&'static str, Value<'static>>>, ConnectionError> {
        check_interface_name(&self.interface_name)?;
        // NetworkManager accepts 0 for the STP timers only when STP is
        // disabled; STP defaults to enabled when unset.
        let zero_ok = self.stp == Some(false);
        check_range("forward_delay", self.forward_delay, 2, 30, zero_ok)?;
        check_range("hello_time", self.hello_time, 1, 10, zero_ok)?;
        check_range("max_age", self.max_age, 6, 40, zero_ok)?;
        check_range("ageing_time", self.ageing_time, 0, 1_000_000, false)?;

        let mut bridge = HashMap::new();
        if let Some(stp) = self.stp {
            bridge.insert("stp", Value::from(stp));
        }
        if let Some(v) = self.priority {
            bridge.insert("priority", Value::from(u32::from(v)));
        }
        for (key, value) in [
            ("forward-delay", self.forward_delay),
            ("hello-time", self.hello_time),
            ("max-age", self.max_age),
            ("ageing-time", self.ageing_time),
        ] {
            if let Some(v) = value {
                bridge.insert(key, Value::from(v));
            }
        }

        Ok(self
            .inner
            .interface_name(self.interface_name)
            .with_section("bridge", bridge)
            .ipv4_auto()
            .ipv6_auto()
            .build())
    }
}

/// Builder for a bridge port: a separate wired profile attached to a bridge.
///
/// Mirrors `nmcli connection add type bridge-slave ifname eth0 master br0`.
/// The result is an `802-3-ethernet` profile with `connection.master` set to
/// the bridge and `connection.slave-type = "bridge"`, and no `ipv4`/`ipv6`
/// sections: NetworkManager forbids IP configuration on ports. Saving it
/// leaves any existing profile for the interface untouched; activating it
/// takes the interface off whatever it was using.
///
/// The legacy `master`/`slave-type` keys are used because NetworkManager
/// accepts them on every version; 1.46+ exposes them as `controller` and
/// `port-type` too.
///
/// # Example
///
/// ```rust
/// use nmrs::builders::BridgePortBuilder;
///
/// let settings = BridgePortBuilder::new("br0", "eth0")
///     .autoconnect(false)
///     .build()
///     .expect("valid port");
/// assert!(!settings.contains_key("ipv4"));
/// ```
#[non_exhaustive]
pub struct BridgePortBuilder {
    inner: ConnectionBuilder,
    bridge: String,
    interface_name: String,
}

impl BridgePortBuilder {
    /// Creates a port builder attaching `interface_name` to `bridge`.
    ///
    /// `bridge` is the bridge interface name or its profile UUID. The
    /// connection name defaults to `bridge-port-<interface_name>`.
    #[must_use]
    pub fn new(bridge: impl Into<String>, interface_name: impl Into<String>) -> Self {
        let interface_name = interface_name.into();
        Self {
            inner: ConnectionBuilder::new(
                "802-3-ethernet",
                format!("bridge-port-{interface_name}"),
            ),
            bridge: bridge.into(),
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
    ///
    /// An autoconnect port can claim its interface whenever that interface
    /// is idle, including at boot.
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

    /// Builds the bridge port connection settings.
    ///
    /// # Errors
    ///
    /// Returns `ConnectionError::InvalidInput` if `bridge` is empty or the
    /// interface name is not a valid kernel interface name (1–15 bytes, no
    /// `/`, `:`, or whitespace, not `.` or `..`).
    #[must_use = "the connection settings must be passed to NetworkManager"]
    pub fn build(
        self,
    ) -> Result<HashMap<&'static str, HashMap<&'static str, Value<'static>>>, ConnectionError> {
        if self.bridge.is_empty() {
            return Err(invalid("bridge", "must not be empty"));
        }
        check_interface_name(&self.interface_name)?;

        let bridge = self.bridge;
        Ok(self
            .inner
            .interface_name(self.interface_name)
            .update_section("connection", |s| {
                s.insert("master", Value::from(bridge));
                s.insert("slave-type", Value::from("bridge"));
            })
            .with_section("802-3-ethernet", HashMap::new())
            .with_section("bridge-port", HashMap::new())
            .build())
    }
}

pub(super) fn set_id(inner: ConnectionBuilder, id: String) -> ConnectionBuilder {
    inner.update_section("connection", |s| {
        s.insert("id", Value::from(id));
    })
}

pub(super) fn check_interface_name(name: &str) -> Result<(), ConnectionError> {
    if name.is_empty() || name.len() > MAX_INTERFACE_NAME_LEN {
        return Err(invalid("interface_name", "must be between 1 and 15 bytes"));
    }
    if name == "." || name == ".." {
        return Err(invalid("interface_name", "must not be \".\" or \"..\""));
    }
    if name
        .bytes()
        .any(|b| b == b'/' || b == b':' || b.is_ascii_whitespace() || b == b'\x0b')
    {
        return Err(invalid(
            "interface_name",
            "must not contain '/', ':', or whitespace",
        ));
    }
    Ok(())
}

pub(super) fn invalid(field: &str, reason: &str) -> ConnectionError {
    ConnectionError::InvalidInput {
        field: field.to_string(),
        reason: reason.to_string(),
    }
}

fn check_range(
    field: &str,
    value: Option<u32>,
    min: u32,
    max: u32,
    zero_ok: bool,
) -> Result<(), ConnectionError> {
    match value {
        Some(0) if zero_ok => Ok(()),
        Some(v) if !(min..=max).contains(&v) => {
            let zero = if zero_ok { "0 or " } else { "" };
            Err(invalid(
                field,
                &format!("must be {zero}between {min} and {max}"),
            ))
        }
        _ => Ok(()),
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

    #[test]
    fn builds_basic_bridge_connection() {
        let conn = BridgeBuilder::new("br0")
            .options(&ConnectionOptions {
                autoconnect: true,
                autoconnect_priority: Some(10),
                autoconnect_retries: Some(3),
            })
            .build()
            .unwrap();
        let connection = &conn["connection"];

        assert_eq!(connection.get("type"), Some(&Value::from("bridge")));
        assert_eq!(connection.get("id"), Some(&Value::from("br0")));
        assert_eq!(connection.get("interface-name"), Some(&Value::from("br0")));
        assert_eq!(connection.get("autoconnect"), Some(&Value::from(true)));
        assert_eq!(
            connection.get("autoconnect-priority"),
            Some(&Value::from(10i32))
        );
        assert_eq!(
            connection.get("autoconnect-retries"),
            Some(&Value::from(3i32))
        );
        assert!(conn["bridge"].is_empty());
        assert_eq!(conn["ipv4"].get("method"), Some(&Value::from("auto")));
        assert_eq!(conn["ipv6"].get("method"), Some(&Value::from("auto")));
    }

    #[test]
    fn serializes_bridge_options() {
        let conn = BridgeBuilder::new("br0")
            .stp(true)
            .priority(100)
            .forward_delay(15)
            .hello_time(2)
            .max_age(20)
            .ageing_time(300)
            .build()
            .unwrap();
        let bridge = &conn["bridge"];

        assert_eq!(bridge.get("stp"), Some(&Value::from(true)));
        assert_eq!(bridge["stp"].value_signature().to_string(), "b");
        for (key, expected) in [
            ("priority", 100u32),
            ("forward-delay", 15),
            ("hello-time", 2),
            ("max-age", 20),
            ("ageing-time", 300),
        ] {
            assert_eq!(bridge.get(key), Some(&Value::from(expected)));
            assert_eq!(bridge[key].value_signature().to_string(), "u");
        }
    }

    #[test]
    fn uses_custom_connection_name_and_uuid() {
        let uuid = Uuid::new_v4();
        let conn = BridgeBuilder::new("br0")
            .connection_name("Lab bridge")
            .uuid(uuid)
            .build()
            .unwrap();

        assert_eq!(
            conn["connection"].get("id"),
            Some(&Value::from("Lab bridge"))
        );
        assert_eq!(
            conn["connection"].get("uuid"),
            Some(&Value::from(uuid.to_string()))
        );
    }

    #[test]
    fn uuid_is_unique() {
        let a = BridgeBuilder::new("br0").build().unwrap();
        let b = BridgeBuilder::new("br0").build().unwrap();
        assert_ne!(a["connection"]["uuid"], b["connection"]["uuid"]);
    }

    #[test]
    fn accepts_boundary_options() {
        BridgeBuilder::new("br0").ageing_time(0).build().unwrap();
        // With STP off, NetworkManager accepts 0 for the STP timers.
        BridgeBuilder::new("br0")
            .stp(false)
            .forward_delay(0)
            .hello_time(0)
            .max_age(0)
            .build()
            .unwrap();
    }

    #[test]
    fn rejects_bad_interface_name() {
        for name in [
            "",
            "0123456789abcdef",
            ".",
            "..",
            "br 0",
            "br\t0",
            "br/0",
            "br:0",
        ] {
            assert_eq!(
                invalid_field(BridgeBuilder::new(name).build()),
                "interface_name"
            );
        }
    }

    #[test]
    fn rejects_out_of_range_options() {
        let cases = [
            ("forward_delay", BridgeBuilder::new("br0").forward_delay(1)),
            ("hello_time", BridgeBuilder::new("br0").hello_time(11)),
            ("max_age", BridgeBuilder::new("br0").max_age(5)),
            (
                "ageing_time",
                BridgeBuilder::new("br0").ageing_time(1_000_001),
            ),
            // STP is enabled by default, so 0 is out of range.
            ("forward_delay", BridgeBuilder::new("br0").forward_delay(0)),
            (
                "hello_time",
                BridgeBuilder::new("br0").stp(true).hello_time(0),
            ),
            // 0 is the only out-of-range value STP-off allows.
            ("max_age", BridgeBuilder::new("br0").stp(false).max_age(5)),
        ];
        for (field, builder) in cases {
            assert_eq!(invalid_field(builder.build()), field);
        }
    }

    #[test]
    fn builds_port_profile_without_ip() {
        let conn = BridgePortBuilder::new("br0", "eth0")
            .autoconnect(false)
            .build()
            .unwrap();
        let connection = &conn["connection"];

        assert_eq!(connection.get("type"), Some(&Value::from("802-3-ethernet")));
        assert_eq!(connection.get("id"), Some(&Value::from("bridge-port-eth0")));
        assert_eq!(connection.get("interface-name"), Some(&Value::from("eth0")));
        assert_eq!(connection.get("master"), Some(&Value::from("br0")));
        assert_eq!(connection.get("slave-type"), Some(&Value::from("bridge")));
        assert_eq!(connection.get("autoconnect"), Some(&Value::from(false)));
        assert!(conn.contains_key("802-3-ethernet"));
        assert!(conn.contains_key("bridge-port"));
        assert!(!conn.contains_key("ipv4"));
        assert!(!conn.contains_key("ipv6"));
    }

    #[test]
    fn port_uses_custom_connection_name() {
        let conn = BridgePortBuilder::new("br0", "eth0")
            .connection_name("uplink")
            .build()
            .unwrap();
        assert_eq!(conn["connection"].get("id"), Some(&Value::from("uplink")));
    }

    #[test]
    fn port_rejects_invalid_input() {
        for (field, builder) in [
            ("bridge", BridgePortBuilder::new("", "eth0")),
            ("interface_name", BridgePortBuilder::new("br0", "")),
            (
                "interface_name",
                BridgePortBuilder::new("br0", "0123456789abcdef"),
            ),
            ("interface_name", BridgePortBuilder::new("br0", "eth 0")),
        ] {
            assert_eq!(invalid_field(builder.build()), field);
        }
    }
}
