//! Bluetooth connection management module.
//!
//! This module provides functions to create and manage Bluetooth network connections
//! using NetworkManager's D-Bus API. It includes builders for Bluetooth PAN (Personal Area
//! Network) connections and DUN (Dial-Up Networking) connections.
//!
//! # Usage
//!
//! Most users should use the high-level [`NetworkManager`](crate::NetworkManager) API
//! instead of calling these builders directly. These are exposed for advanced use cases
//! where you need fine-grained control over connection settings.
//!
//! # Example
//!
//! ```rust
//! use nmrs::builders::build_bluetooth_connection;
//! use nmrs::models::{BluetoothIdentity, BluetoothNetworkRole};
//!
//! let bt_settings = BluetoothIdentity::new(
//!    "00:1A:7D:DA:71:13".into(),
//!    BluetoothNetworkRole::PanU,
//! ).unwrap();
//! ```

use std::collections::HashMap;
use zvariant::Value;

use crate::{
    ConnectionOptions,
    models::{BluetoothIdentity, BluetoothNetworkRole},
};

/// Builds the `connection` section with type, id, uuid, and autoconnect settings.
#[must_use]
pub fn base_connection_section(
    name: &str,
    opts: &ConnectionOptions,
) -> HashMap<&'static str, Value<'static>> {
    let mut s = HashMap::new();
    s.insert("type", Value::from("bluetooth"));
    s.insert("id", Value::from(name.to_string()));
    s.insert("uuid", Value::from(uuid::Uuid::new_v4().to_string()));
    s.insert("autoconnect", Value::from(opts.autoconnect));

    if let Some(p) = opts.autoconnect_priority {
        s.insert("autoconnect-priority", Value::from(p));
    }

    if let Some(r) = opts.autoconnect_retries {
        s.insert("autoconnect-retries", Value::from(r));
    }

    s
}

/// Builds a Bluetooth connection settings dictionary.
fn bluetooth_section(settings: &BluetoothIdentity) -> HashMap<&'static str, Value<'static>> {
    let mut s = HashMap::new();
    s.insert("bdaddr", Value::from(settings.bdaddr.clone()));
    let bt_type = match settings.bt_device_type {
        BluetoothNetworkRole::PanU => "panu",
        BluetoothNetworkRole::Dun => "dun",
    };
    s.insert("type", Value::from(bt_type));
    s
}

#[must_use]
pub fn build_bluetooth_connection(
    name: &str,
    settings: &BluetoothIdentity,
    opts: &ConnectionOptions,
) -> HashMap<&'static str, HashMap<&'static str, Value<'static>>> {
    let mut conn: HashMap<&'static str, HashMap<&'static str, Value<'static>>> = HashMap::new();

    // Base connections
    conn.insert("connection", base_connection_section(name, opts));
    conn.insert("bluetooth", bluetooth_section(settings));

    let mut ipv4 = HashMap::new();
    ipv4.insert("method", Value::from("auto"));
    conn.insert("ipv4", ipv4);

    let mut ipv6 = HashMap::new();
    ipv6.insert("method", Value::from("auto"));
    conn.insert("ipv6", ipv6);

    conn
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_opts() -> ConnectionOptions {
        ConnectionOptions {
            autoconnect: true,
            autoconnect_priority: Some(10),
            autoconnect_retries: Some(3),
        }
    }

    fn create_test_identity_panu() -> BluetoothIdentity {
        BluetoothIdentity::new("00:1A:7D:DA:71:13".into(), BluetoothNetworkRole::PanU).unwrap()
    }

    fn create_test_identity_dun() -> BluetoothIdentity {
        BluetoothIdentity::new("C8:1F:E8:F0:51:57".into(), BluetoothNetworkRole::Dun).unwrap()
    }

    #[test]
    fn test_base_connection_section() {
        let opts = create_test_opts();
        let section = base_connection_section("TestBluetooth", &opts);

        // Check required fields
        assert!(section.contains_key("type"));
        assert!(section.contains_key("id"));
        assert!(section.contains_key("uuid"));
        assert!(section.contains_key("autoconnect"));

        assert_eq!(section.get("type"), Some(&Value::from("bluetooth")));
        assert_eq!(section.get("id"), Some(&Value::from("TestBluetooth")));
        assert_eq!(section.get("autoconnect"), Some(&Value::from(true)));
        assert_eq!(
            section.get("autoconnect-priority"),
            Some(&Value::from(10i32))
        );
        assert_eq!(section.get("autoconnect-retries"), Some(&Value::from(3i32)));
    }

    #[test]
    fn test_base_connection_section_without_optional_fields() {
        let opts = ConnectionOptions {
            autoconnect: false,
            autoconnect_priority: None,
            autoconnect_retries: None,
        };
        let section = base_connection_section("MinimalBT", &opts);

        assert!(section.contains_key("type"));
        assert!(section.contains_key("id"));
        assert!(section.contains_key("uuid"));
        assert!(section.contains_key("autoconnect"));

        // Optional fields should not be present
        assert!(!section.contains_key("autoconnect-priority"));
        assert!(!section.contains_key("autoconnect-retries"));
    }

    #[test]
    fn test_bluetooth_section_panu() {
        let identity = create_test_identity_panu();
        let section = bluetooth_section(&identity);

        assert!(section.contains_key("bdaddr"));
        assert!(section.contains_key("type"));

        assert_eq!(
            section.get("bdaddr"),
            Some(&Value::from("00:1A:7D:DA:71:13"))
        );
        assert_eq!(section.get("type"), Some(&Value::from("panu")));
    }

    #[test]
    fn test_bluetooth_section_dun() {
        let identity = create_test_identity_dun();
        let section = bluetooth_section(&identity);

        assert!(section.contains_key("bdaddr"));
        assert!(section.contains_key("type"));

        assert_eq!(
            section.get("bdaddr"),
            Some(&Value::from("C8:1F:E8:F0:51:57"))
        );
        assert_eq!(section.get("type"), Some(&Value::from("dun")));
    }

    #[test]
    fn test_build_bluetooth_connection_panu() {
        let identity = create_test_identity_panu();
        let opts = create_test_opts();
        let conn = build_bluetooth_connection("MyPhone", &identity, &opts);

        // Check main sections
        assert!(conn.contains_key("connection"));
        assert!(conn.contains_key("bluetooth"));
        assert!(conn.contains_key("ipv4"));
        assert!(conn.contains_key("ipv6"));

        let connection_section = conn.get("connection").unwrap();
        assert_eq!(connection_section.get("id"), Some(&Value::from("MyPhone")));

        let bt_section = conn.get("bluetooth").unwrap();
        assert_eq!(
            bt_section.get("bdaddr"),
            Some(&Value::from("00:1A:7D:DA:71:13"))
        );
        assert_eq!(bt_section.get("type"), Some(&Value::from("panu")));

        let ipv4_section = conn.get("ipv4").unwrap();
        assert_eq!(ipv4_section.get("method"), Some(&Value::from("auto")));

        let ipv6_section = conn.get("ipv6").unwrap();
        assert_eq!(ipv6_section.get("method"), Some(&Value::from("auto")));
    }

    #[test]
    fn test_build_bluetooth_connection_dun() {
        let identity = create_test_identity_dun();
        let opts = ConnectionOptions {
            autoconnect: false,
            autoconnect_priority: None,
            autoconnect_retries: None,
        };
        let conn = build_bluetooth_connection("MobileHotspot", &identity, &opts);

        assert!(conn.contains_key("connection"));
        assert!(conn.contains_key("bluetooth"));
        assert!(conn.contains_key("ipv4"));
        assert!(conn.contains_key("ipv6"));

        let bt_section = conn.get("bluetooth").unwrap();
        assert_eq!(
            conn["connection"].get("autoconnect"),
            Some(&Value::from(false))
        );
        assert_eq!(bt_section.get("type"), Some(&Value::from("dun")));
        assert_eq!(
            bt_section.get("bdaddr"),
            Some(&Value::from("C8:1F:E8:F0:51:57"))
        );
    }

    #[test]
    fn test_uuid_is_unique() {
        let identity = create_test_identity_panu();
        let opts = create_test_opts();

        let conn1 = build_bluetooth_connection("BT1", &identity, &opts);
        let conn2 = build_bluetooth_connection("BT2", &identity, &opts);

        let Value::Str(uuid1) = &conn1["connection"]["uuid"] else {
            panic!("conn1 UUID must be a string");
        };
        let Value::Str(uuid2) = &conn2["connection"]["uuid"] else {
            panic!("conn2 UUID must be a string");
        };
        let uuid1 = uuid::Uuid::parse_str(uuid1.as_str()).expect("conn1 must contain a valid UUID");
        let uuid2 = uuid::Uuid::parse_str(uuid2.as_str()).expect("conn2 must contain a valid UUID");

        assert_ne!(uuid1, uuid2, "UUIDs should be unique");
    }

    #[test]
    fn test_bdaddr_format_preserved() {
        let identity =
            BluetoothIdentity::new("AA:BB:CC:DD:EE:FF".into(), BluetoothNetworkRole::PanU).unwrap();
        let opts = create_test_opts();
        let conn = build_bluetooth_connection("Test", &identity, &opts);

        let bt_section = conn.get("bluetooth").unwrap();
        assert_eq!(
            bt_section.get("bdaddr"),
            Some(&Value::from("AA:BB:CC:DD:EE:FF"))
        );
    }
}
