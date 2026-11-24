//! Port profiles shared by bridge, bond, and team builders.

use std::collections::HashMap;
use zvariant::Value;

use super::connection_builder::ConnectionBuilder;
use crate::ConnectionOptions;
use crate::api::models::ConnectionError;
use crate::util::validation::validate_interface_name;

/// Makes `builder` a port of `controller`: `802-3-ethernet` with `controller`
/// and `port-type`. No IP sections (NM
/// forbids them on ports); NM adds the `<type>-port` section itself.
pub(super) fn attach_port(
    builder: ConnectionBuilder,
    port_type: &'static str,
    controller: String,
    port: String,
) -> ConnectionBuilder {
    builder
        .interface_name(port)
        .update_section("connection", |c| {
            c.insert("controller", Value::from(controller));
            c.insert("port-type", Value::from(port_type));
        })
        .with_section("802-3-ethernet", HashMap::new())
}

/// Default port profile id, e.g. `bond-port-eth0`.
pub(super) fn port_id(port_type: &str, port: &str) -> String {
    format!("{port_type}-port-{port}")
}

/// Rejects a port with the same name as its controller.
pub(super) fn ensure_distinct(
    controller: &str,
    port: &str,
    field: &str,
) -> Result<(), ConnectionError> {
    if controller == port {
        return Err(ConnectionError::InvalidInput {
            field: field.to_string(),
            reason: "port interface cannot be the same as the controller interface".to_string(),
        });
    }
    Ok(())
}

/// Builds a bond or team port profile attaching `port` to `controller`.
pub(crate) fn build_port_connection(
    port_type: &'static str,
    controller: &str,
    port: &str,
    opts: &ConnectionOptions,
) -> Result<HashMap<&'static str, HashMap<&'static str, Value<'static>>>, ConnectionError> {
    validate_interface_name(controller, "controller_interface")?;
    validate_interface_name(port, "port_interface")?;
    ensure_distinct(controller, port, "port_interface")?;

    let builder = ConnectionBuilder::new("802-3-ethernet", port_id(port_type, port)).options(opts);
    Ok(attach_port(builder, port_type, controller.to_string(), port.to_string()).build())
}

/// Adds an `802-3-ethernet` section carrying only the MTU, when one is set.
pub(super) fn with_wired_mtu(builder: ConnectionBuilder, mtu: Option<u32>) -> ConnectionBuilder {
    match mtu {
        Some(mtu) => {
            builder.with_section("802-3-ethernet", HashMap::from([("mtu", Value::from(mtu))]))
        }
        None => builder,
    }
}

#[cfg(test)]
pub(super) mod tests {
    use super::*;

    pub(in crate::api::builders) fn test_opts() -> ConnectionOptions {
        ConnectionOptions {
            autoconnect: true,
            autoconnect_priority: Some(10),
            autoconnect_retries: Some(3),
        }
    }

    #[test]
    fn builds_port_profile_for_each_port_type() {
        for port_type in ["bond", "team"] {
            let conn = build_port_connection(port_type, "ctl0", "eth0", &test_opts()).unwrap();
            let c = &conn["connection"];

            assert_eq!(c["type"], Value::from("802-3-ethernet"));
            assert_eq!(c["id"], Value::from(format!("{port_type}-port-eth0")));
            assert_eq!(c["interface-name"], Value::from("eth0"));
            assert_eq!(c["controller"], Value::from("ctl0"));
            assert_eq!(c["port-type"], Value::from(port_type));
            assert_eq!(c["autoconnect"], Value::from(true));
            assert_eq!(c["autoconnect-priority"], Value::from(10i32));
            assert_eq!(c["autoconnect-retries"], Value::from(3i32));
            assert!(!c.contains_key("master") && !c.contains_key("slave-type"));
            // No IP or `*-port` sections.
            assert_eq!(conn.len(), 2);
            assert!(conn["802-3-ethernet"].is_empty());
        }
    }

    #[test]
    fn omits_optional_keys_when_unset() {
        let conn =
            build_port_connection("bond", "bond0", "eth0", &ConnectionOptions::new(false)).unwrap();
        let c = &conn["connection"];

        assert_eq!(c["autoconnect"], Value::from(false));
        assert!(!c.contains_key("autoconnect-priority"));
        assert!(!c.contains_key("autoconnect-retries"));
    }

    #[test]
    fn rejects_invalid_names_with_field() {
        let err = build_port_connection("bond", "a/b", "eth0", &test_opts()).unwrap_err();
        assert!(matches!(
            err,
            ConnectionError::InvalidInput { ref field, .. } if field == "controller_interface"
        ));

        let err = build_port_connection("bond", "bond0", "", &test_opts()).unwrap_err();
        assert!(matches!(
            err,
            ConnectionError::InvalidInput { ref field, .. } if field == "port_interface"
        ));
    }

    #[test]
    fn rejects_port_equal_to_controller() {
        let err = build_port_connection("team", "eth0", "eth0", &test_opts()).unwrap_err();
        assert!(matches!(
            err,
            ConnectionError::InvalidInput { ref field, .. } if field == "port_interface"
        ));
    }
}
