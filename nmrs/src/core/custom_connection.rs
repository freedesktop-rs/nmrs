//! Submitting builder-produced connection settings to NetworkManager.
//!
//! Wraps the `AddConnection` and `AddAndActivateConnection` D-Bus methods so
//! callers can use [`builders`](crate::api::builders) output without writing
//! their own zbus proxies.

use std::collections::HashMap;

use log::trace;
use zbus::Connection;
use zvariant::{OwnedObjectPath, Value};

use crate::Result;
use crate::api::models::{ConnectionError, TimeoutConfig};
use crate::core::connection::{disconnect_wifi_and_wait, get_device_by_interface};
use crate::core::state_wait::wait_for_connection_activation;
use crate::dbus::{NMDeviceProxy, NMProxy};
use crate::types::constants::device_type;
use crate::util::utils::{bluez_device_path, settings_proxy};

fn connection_type_from_settings<'a>(
    settings: &'a HashMap<&str, HashMap<&str, Value<'_>>>,
) -> Result<&'a str> {
    settings
        .get("connection")
        .and_then(|section| section.get("type"))
        .and_then(|value| match value {
            Value::Str(conn_type) => Some(conn_type.as_str()),
            _ => None,
        })
        .ok_or_else(|| ConnectionError::InvalidInput {
            field: "connection.type".into(),
            reason: "settings dictionary is missing connection.type".into(),
        })
}

fn expected_device_type(conn_type: &str) -> Option<u32> {
    match conn_type {
        "802-11-wireless" => Some(device_type::WIFI),
        "802-3-ethernet" => Some(device_type::ETHERNET),
        "bluetooth" => Some(device_type::BLUETOOTH),
        _ => None,
    }
}

async fn find_first_device_by_type(conn: &Connection, raw_type: u32) -> Result<OwnedObjectPath> {
    let nm = NMProxy::new(conn).await?;
    let devices = nm.get_devices().await?;

    for dev_path in devices {
        let dev = NMDeviceProxy::builder(conn)
            .path(dev_path.clone())?
            .build()
            .await?;

        if dev.device_type().await? == raw_type {
            trace!(
                "Resolved device {} for connection type {}",
                dev.interface().await.unwrap_or_default(),
                raw_type
            );
            return Ok(dev_path);
        }
    }

    Err(ConnectionError::NotFound)
}

async fn resolve_device_path(
    conn: &Connection,
    settings: &HashMap<&str, HashMap<&str, Value<'_>>>,
    interface: Option<&str>,
) -> Result<OwnedObjectPath> {
    if let Some(iface) = interface {
        return get_device_by_interface(conn, iface).await;
    }

    let conn_type = connection_type_from_settings(settings)?;
    match expected_device_type(conn_type) {
        Some(raw_type) => find_first_device_by_type(conn, raw_type).await,
        None => Ok(OwnedObjectPath::default()),
    }
}

fn bluetooth_bdaddr_from_settings(
    settings: &HashMap<&str, HashMap<&str, Value<'_>>>,
) -> Result<String> {
    settings
        .get("bluetooth")
        .and_then(|section| section.get("bdaddr"))
        .and_then(|value| match value {
            Value::Str(bdaddr) => Some(bdaddr.as_str().to_string()),
            _ => None,
        })
        .ok_or_else(|| ConnectionError::InvalidInput {
            field: "bluetooth.bdaddr".into(),
            reason: "bluetooth settings are missing bdaddr".into(),
        })
}

fn resolve_specific_object(
    settings: &HashMap<&str, HashMap<&str, Value<'_>>>,
    specific_object: Option<&str>,
) -> Result<OwnedObjectPath> {
    if let Some(path) = specific_object {
        return OwnedObjectPath::try_from(path).map_err(|e| ConnectionError::InvalidInput {
            field: "specific_object".into(),
            reason: e.to_string(),
        });
    }

    if connection_type_from_settings(settings)? == "bluetooth" {
        let bdaddr = bluetooth_bdaddr_from_settings(settings)?;
        return OwnedObjectPath::try_from(bluez_device_path(&bdaddr, None)).map_err(|e| {
            ConnectionError::InvalidInput {
                field: "specific_object".into(),
                reason: e.to_string(),
            }
        });
    }

    Ok(OwnedObjectPath::default())
}

/// Saves a connection profile without activating it (`Settings.AddConnection`).
pub(crate) async fn add_connection(
    conn: &Connection,
    settings: HashMap<&str, HashMap<&str, Value<'_>>>,
) -> Result<OwnedObjectPath> {
    let settings_api = settings_proxy(conn).await?;
    let add_reply = settings_api
        .call_method("AddConnection", &(settings,))
        .await
        .map_err(|e| ConnectionError::DbusOperation {
            context: "failed to add connection".into(),
            source: e,
        })?;

    add_reply
        .body()
        .deserialize()
        .map_err(|e| ConnectionError::DbusOperation {
            context: "failed to decode AddConnection reply".into(),
            source: e,
        })
}

/// Creates and activates a connection in one step (`AddAndActivateConnection`).
pub(crate) async fn add_and_activate_connection(
    conn: &Connection,
    settings: HashMap<&str, HashMap<&str, Value<'_>>>,
    interface: Option<&str>,
    specific_object: Option<&str>,
    timeout_config: TimeoutConfig,
) -> Result<(OwnedObjectPath, OwnedObjectPath)> {
    let device = resolve_device_path(conn, &settings, interface).await?;
    let specific_object = resolve_specific_object(&settings, specific_object)?;

    if device.as_str() != "/" {
        disconnect_wifi_and_wait(conn, &device, Some(timeout_config)).await?;
    }

    let nm = NMProxy::new(conn).await?;
    let (profile_path, active_path) = nm
        .add_and_activate_connection(settings, device, specific_object)
        .await?;

    wait_for_connection_activation(conn, &active_path, Some(timeout_config.connection_timeout))
        .await?;

    Ok((profile_path, active_path))
}

#[cfg(test)]
mod tests {
    use super::*;
    use zvariant::Value;

    fn sample_wifi_settings() -> HashMap<&'static str, HashMap<&'static str, Value<'static>>> {
        let mut connection = HashMap::new();
        connection.insert("type", Value::from("802-11-wireless"));
        connection.insert("id", Value::from("Hotspot"));

        let mut wireless = HashMap::new();
        wireless.insert("mode", Value::from("ap"));

        let mut settings = HashMap::new();
        settings.insert("connection", connection);
        settings.insert("802-11-wireless", wireless);
        settings
    }

    fn sample_bluetooth_settings(
        bdaddr: Option<Value<'static>>,
    ) -> HashMap<&'static str, HashMap<&'static str, Value<'static>>> {
        let mut connection = HashMap::new();
        connection.insert("type", Value::from("bluetooth"));

        let mut bluetooth = HashMap::new();
        if let Some(bdaddr) = bdaddr {
            bluetooth.insert("bdaddr", bdaddr);
        }

        HashMap::from([("connection", connection), ("bluetooth", bluetooth)])
    }

    fn assert_invalid_input(error: ConnectionError, expected_field: &str, expected_reason: &str) {
        match error {
            ConnectionError::InvalidInput { field, reason } => {
                assert_eq!(field, expected_field);
                assert_eq!(reason, expected_reason);
            }
            other => panic!("expected InvalidInput, got {other:?}"),
        }
    }

    #[test]
    fn connection_type_from_settings_reads_type_field() {
        let settings = sample_wifi_settings();
        assert_eq!(
            connection_type_from_settings(&settings).unwrap(),
            "802-11-wireless"
        );
    }

    #[test]
    fn connection_type_from_settings_rejects_every_missing_or_wrong_type_shape() {
        let no_connection = HashMap::new();
        assert_invalid_input(
            connection_type_from_settings(&no_connection).unwrap_err(),
            "connection.type",
            "settings dictionary is missing connection.type",
        );

        let no_type = HashMap::from([("connection", HashMap::new())]);
        assert_invalid_input(
            connection_type_from_settings(&no_type).unwrap_err(),
            "connection.type",
            "settings dictionary is missing connection.type",
        );

        let wrong_type =
            HashMap::from([("connection", HashMap::from([("type", Value::from(42u32))]))]);
        assert_invalid_input(
            connection_type_from_settings(&wrong_type).unwrap_err(),
            "connection.type",
            "settings dictionary is missing connection.type",
        );
    }

    #[test]
    fn expected_device_type_maps_supported_and_virtual_connection_types() {
        assert_eq!(
            expected_device_type("802-11-wireless"),
            Some(device_type::WIFI)
        );
        assert_eq!(
            expected_device_type("802-3-ethernet"),
            Some(device_type::ETHERNET)
        );
        assert_eq!(
            expected_device_type("bluetooth"),
            Some(device_type::BLUETOOTH)
        );
        assert_eq!(expected_device_type("vpn"), None);
        assert_eq!(expected_device_type("wireguard"), None);
        assert_eq!(expected_device_type("unknown"), None);
    }

    #[test]
    fn bluetooth_bdaddr_from_settings_reads_address() {
        let settings = sample_bluetooth_settings(Some(Value::from("00:1A:7D:DA:71:13")));
        assert_eq!(
            bluetooth_bdaddr_from_settings(&settings).unwrap(),
            "00:1A:7D:DA:71:13"
        );
    }

    #[test]
    fn bluetooth_bdaddr_from_settings_rejects_missing_and_wrong_type_values() {
        let no_section = sample_wifi_settings();
        assert_invalid_input(
            bluetooth_bdaddr_from_settings(&no_section).unwrap_err(),
            "bluetooth.bdaddr",
            "bluetooth settings are missing bdaddr",
        );

        let no_address = sample_bluetooth_settings(None);
        assert_invalid_input(
            bluetooth_bdaddr_from_settings(&no_address).unwrap_err(),
            "bluetooth.bdaddr",
            "bluetooth settings are missing bdaddr",
        );

        let wrong_type = sample_bluetooth_settings(Some(Value::from(42u32)));
        assert_invalid_input(
            bluetooth_bdaddr_from_settings(&wrong_type).unwrap_err(),
            "bluetooth.bdaddr",
            "bluetooth settings are missing bdaddr",
        );
    }

    #[test]
    fn resolve_specific_object_defaults_to_root_path() {
        let settings = sample_wifi_settings();
        let path = resolve_specific_object(&settings, None).unwrap();
        assert_eq!(path.as_str(), "/");
    }

    #[test]
    fn resolve_specific_object_parses_explicit_path() {
        let settings = sample_wifi_settings();
        let path =
            resolve_specific_object(&settings, Some("/org/freedesktop/NetworkManager/Devices/3"))
                .unwrap();
        assert_eq!(path.as_str(), "/org/freedesktop/NetworkManager/Devices/3");
    }

    #[test]
    fn resolve_specific_object_rejects_invalid_explicit_path() {
        let settings = sample_wifi_settings();
        let error = resolve_specific_object(&settings, Some("not/an/object/path")).unwrap_err();

        match error {
            ConnectionError::InvalidInput { field, reason } => {
                assert_eq!(field, "specific_object");
                assert!(
                    !reason.is_empty(),
                    "zvariant should explain the invalid path"
                );
            }
            other => panic!("expected InvalidInput, got {other:?}"),
        }
    }

    #[test]
    fn resolve_specific_object_derives_bluez_device_path() {
        let settings = sample_bluetooth_settings(Some(Value::from("00:1A:7D:DA:71:13")));
        let path = resolve_specific_object(&settings, None).unwrap();

        assert_eq!(path.as_str(), "/org/bluez/hci0/dev_00_1A_7D_DA_71_13");
    }

    #[test]
    fn resolve_specific_object_requires_bluetooth_address() {
        let settings = sample_bluetooth_settings(None);
        assert_invalid_input(
            resolve_specific_object(&settings, None).unwrap_err(),
            "bluetooth.bdaddr",
            "bluetooth settings are missing bdaddr",
        );
    }
}
