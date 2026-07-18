//! VLAN (802.1Q) connection builder.
//!
//! This module provides functions to create VLAN connection settings
//! for NetworkManager.

use std::collections::HashMap;
use zvariant::Value;

use crate::ConnectionOptions;
use crate::api::models::{ConnectionError, VlanConfig};

/// Builds a VLAN connection settings dictionary for NetworkManager.
///
/// Creates all necessary settings sections for a VLAN connection including
/// the connection metadata, VLAN-specific settings, and IP configuration.
///
/// # Arguments
///
/// * `config` - VLAN configuration
/// * `opts` - Connection options (autoconnect, priority, etc.)
///
/// # Errors
///
/// Returns `ConnectionError::InvalidVlanId` if the VLAN ID is out of range.
/// Returns `ConnectionError::InvalidInput` if the parent interface is empty.
///
/// # Examples
///
/// ```rust
/// use nmrs::builders::build_vlan_connection;
/// use nmrs::{VlanConfig, ConnectionOptions};
///
/// let config = VlanConfig::new("eth0", 100)
///     .with_connection_name("Office VLAN");
/// let opts = ConnectionOptions::new(true);
///
/// let settings = build_vlan_connection(&config, &opts).unwrap();
/// ```
pub fn build_vlan_connection(
    config: &VlanConfig,
    opts: &ConnectionOptions,
) -> Result<HashMap<&'static str, HashMap<&'static str, Value<'static>>>, ConnectionError> {
    config.validate()?;

    let mut conn: HashMap<&'static str, HashMap<&'static str, Value<'static>>> = HashMap::new();

    // Connection section
    conn.insert("connection", connection_section(config, opts));

    // VLAN section
    conn.insert("vlan", vlan_section(config));

    if let Some(mtu) = config.mtu {
        let mut wired = HashMap::new();
        wired.insert("mtu", Value::from(mtu));
        conn.insert("802-3-ethernet", wired);
    }

    // IPv4 section (auto by default)
    let mut ipv4 = HashMap::new();
    ipv4.insert("method", Value::from("auto"));
    conn.insert("ipv4", ipv4);

    // IPv6 section (auto by default)
    let mut ipv6 = HashMap::new();
    ipv6.insert("method", Value::from("auto"));
    conn.insert("ipv6", ipv6);

    Ok(conn)
}

fn connection_section(
    config: &VlanConfig,
    opts: &ConnectionOptions,
) -> HashMap<&'static str, Value<'static>> {
    let mut s = HashMap::new();

    s.insert("type", Value::from("vlan"));
    s.insert("id", Value::from(config.effective_connection_name()));
    s.insert("uuid", Value::from(uuid::Uuid::new_v4().to_string()));
    s.insert("autoconnect", Value::from(opts.autoconnect));
    s.insert(
        "interface-name",
        Value::from(config.effective_interface_name()),
    );

    if let Some(p) = opts.autoconnect_priority {
        s.insert("autoconnect-priority", Value::from(p));
    }

    if let Some(r) = opts.autoconnect_retries {
        s.insert("autoconnect-retries", Value::from(r));
    }

    s
}

fn vlan_section(config: &VlanConfig) -> HashMap<&'static str, Value<'static>> {
    let mut s = HashMap::new();

    s.insert("parent", Value::from(config.parent.clone()));
    s.insert("id", Value::from(u32::from(config.id)));

    if let Some(flags) = config.flags {
        s.insert("flags", Value::from(flags));
    }

    if let Some(ref map) = config.ingress_priority_map {
        s.insert("ingress-priority-map", Value::from(map.clone()));
    }

    if let Some(ref map) = config.egress_priority_map {
        s.insert("egress-priority-map", Value::from(map.clone()));
    }

    s
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_opts() -> ConnectionOptions {
        ConnectionOptions {
            autoconnect: true,
            autoconnect_priority: Some(10),
            autoconnect_retries: Some(3),
        }
    }

    #[test]
    fn builds_basic_vlan_connection() {
        let config = VlanConfig::new("eth0", 100);
        let opts = test_opts();

        let conn = build_vlan_connection(&config, &opts).unwrap();
        let connection = conn.get("connection").unwrap();
        assert_eq!(connection.get("type"), Some(&Value::from("vlan")));
        assert_eq!(connection.get("id"), Some(&Value::from("VLAN 100 on eth0")));
        assert_eq!(
            connection.get("interface-name"),
            Some(&Value::from("eth0.100"))
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
        assert_eq!(conn["vlan"].get("parent"), Some(&Value::from("eth0")));
        assert_eq!(conn["vlan"].get("id"), Some(&Value::from(100u32)));
        assert_eq!(conn["ipv4"].get("method"), Some(&Value::from("auto")));
        assert_eq!(conn["ipv6"].get("method"), Some(&Value::from("auto")));
    }

    #[test]
    fn connection_section_has_correct_type() {
        let config = VlanConfig::new("eth0", 100);
        let opts = test_opts();

        let conn = build_vlan_connection(&config, &opts).unwrap();
        let connection = conn.get("connection").unwrap();

        assert_eq!(connection.get("type"), Some(&Value::from("vlan")));
    }

    #[test]
    fn vlan_section_has_parent_and_id() {
        let config = VlanConfig::new("enp3s0", 200);
        let opts = test_opts();

        let conn = build_vlan_connection(&config, &opts).unwrap();
        let vlan = conn.get("vlan").unwrap();

        assert_eq!(vlan.get("parent"), Some(&Value::from("enp3s0")));
        assert_eq!(vlan.get("id"), Some(&Value::from(200u32)));
    }

    #[test]
    fn uses_default_interface_name() {
        let config = VlanConfig::new("eth0", 100);
        let opts = test_opts();

        let conn = build_vlan_connection(&config, &opts).unwrap();
        let connection = conn.get("connection").unwrap();

        assert_eq!(
            connection.get("interface-name"),
            Some(&Value::from("eth0.100"))
        );
    }

    #[test]
    fn uses_custom_interface_name() {
        let config = VlanConfig::new("eth0", 100).with_interface_name("office-vlan");
        let opts = test_opts();

        let conn = build_vlan_connection(&config, &opts).unwrap();
        let connection = conn.get("connection").unwrap();

        assert_eq!(
            connection.get("interface-name"),
            Some(&Value::from("office-vlan"))
        );
    }

    #[test]
    fn uses_default_connection_name() {
        let config = VlanConfig::new("eth0", 100);
        let opts = test_opts();

        let conn = build_vlan_connection(&config, &opts).unwrap();
        let connection = conn.get("connection").unwrap();

        assert_eq!(connection.get("id"), Some(&Value::from("VLAN 100 on eth0")));
    }

    #[test]
    fn uses_custom_connection_name() {
        let config = VlanConfig::new("eth0", 100).with_connection_name("Office Network");
        let opts = test_opts();

        let conn = build_vlan_connection(&config, &opts).unwrap();
        let connection = conn.get("connection").unwrap();

        assert_eq!(connection.get("id"), Some(&Value::from("Office Network")));
    }

    #[test]
    fn includes_vlan_flags() {
        let config = VlanConfig::new("eth0", 100).with_flags(0x5);
        let opts = test_opts();

        let conn = build_vlan_connection(&config, &opts).unwrap();
        let vlan = conn.get("vlan").unwrap();

        assert_eq!(vlan.get("flags"), Some(&Value::from(0x5u32)));
    }

    #[test]
    fn serializes_mtu_in_wired_setting() {
        let config = VlanConfig::new("eth0", 100).with_mtu(1496);

        let conn = build_vlan_connection(&config, &test_opts()).unwrap();
        let wired = conn
            .get("802-3-ethernet")
            .expect("MTU requires an 802-3-ethernet setting");

        assert_eq!(wired.get("mtu"), Some(&Value::from(1496u32)));
        assert_eq!(wired["mtu"].value_signature().to_string(), "u");
        assert!(!conn["vlan"].contains_key("mtu"));
    }

    #[test]
    fn omits_wired_setting_without_mtu() {
        let conn = build_vlan_connection(&VlanConfig::new("eth0", 100), &test_opts()).unwrap();

        assert!(!conn.contains_key("802-3-ethernet"));
    }

    #[test]
    fn serializes_priority_maps_as_string_arrays() {
        let config = VlanConfig::new("eth0", 100)
            .with_ingress_priority_map(vec!["0:0", "7:3"])
            .with_egress_priority_map(vec!["0:1", "4:7"]);

        let conn = build_vlan_connection(&config, &test_opts()).unwrap();
        let vlan = conn.get("vlan").unwrap();
        let ingress = vlan.get("ingress-priority-map").unwrap();
        let egress = vlan.get("egress-priority-map").unwrap();

        assert_eq!(ingress.value_signature().to_string(), "as");
        assert_eq!(egress.value_signature().to_string(), "as");
        assert_eq!(
            ingress,
            &Value::from(vec!["0:0".to_string(), "7:3".to_string()])
        );
        assert_eq!(
            egress,
            &Value::from(vec!["0:1".to_string(), "4:7".to_string()])
        );
    }

    #[test]
    fn propagates_vlan_model_validation_errors() {
        let config = VlanConfig::new("eth0", 0);
        let opts = test_opts();

        let result = build_vlan_connection(&config, &opts);
        assert!(matches!(
            result.unwrap_err(),
            ConnectionError::InvalidVlanId { id: 0 }
        ));
    }

    #[test]
    fn uuid_is_unique() {
        let config = VlanConfig::new("eth0", 100);
        let opts = test_opts();

        let conn1 = build_vlan_connection(&config, &opts).unwrap();
        let conn2 = build_vlan_connection(&config, &opts).unwrap();

        let Value::Str(uuid1) = &conn1["connection"]["uuid"] else {
            panic!("conn1 UUID must be a string");
        };
        let Value::Str(uuid2) = &conn2["connection"]["uuid"] else {
            panic!("conn2 UUID must be a string");
        };
        let uuid1 = uuid::Uuid::parse_str(uuid1.as_str()).expect("conn1 UUID must be valid");
        let uuid2 = uuid::Uuid::parse_str(uuid2.as_str()).expect("conn2 UUID must be valid");

        assert_ne!(uuid1, uuid2, "UUIDs should be unique");
    }
}
