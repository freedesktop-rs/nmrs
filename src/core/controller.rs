//! Helpers shared by bond and team (controller/port) operations.

use std::collections::HashMap;

use zbus::Connection;
use zbus::proxy::CacheProperties;
use zvariant::{OwnedObjectPath, Value};

use crate::Result;
use crate::api::models::{ConnectionError, DeviceState, FromSetting, RawSettings};
use crate::core::custom_connection::add_connection;
use crate::dbus::{NMDeviceProxy, NMProxy};

/// Saves builder output with `Settings.AddConnection` and returns the profile UUID.
pub(crate) async fn save_profile(
    conn: &Connection,
    settings: HashMap<&'static str, HashMap<&'static str, Value<'static>>>,
) -> Result<String> {
    let uuid = settings
        .get("connection")
        .and_then(|c| c.get("uuid"))
        .and_then(|v| String::try_from(v.clone()).ok())
        .ok_or_else(|| ConnectionError::InvalidInput {
            field: "connection.uuid".into(),
            reason: "settings dictionary is missing connection.uuid".into(),
        })?;
    add_connection(conn, settings).await?;
    Ok(uuid)
}

/// Rejects a saved profile whose `connection.type` is not `expected`.
pub(crate) fn ensure_connection_type(
    settings: &RawSettings,
    uuid: &str,
    expected: &str,
) -> Result<()> {
    let actual = settings
        .get("connection")
        .and_then(|section| section.get("type"))
        .and_then(String::from_setting);
    if actual.as_deref() == Some(expected) {
        Ok(())
    } else {
        Err(ConnectionError::InvalidInput {
            field: "uuid".into(),
            reason: format!("connection {uuid} is not a {expected} profile"),
        })
    }
}

/// The fields every controller device type (bond, team) shares.
pub(crate) struct ControllerDevice {
    pub(crate) path: OwnedObjectPath,
    pub(crate) interface: String,
    pub(crate) hw_address: String,
    pub(crate) state: DeviceState,
    pub(crate) ports: Vec<String>,
}

/// Lists the devices whose NetworkManager device type is `type_code`.
pub(crate) async fn controller_devices(
    conn: &Connection,
    type_code: u32,
) -> Result<Vec<ControllerDevice>> {
    let nm = NMProxy::new(conn).await?;
    let paths = nm
        .get_devices()
        .await
        .map_err(|e| ConnectionError::DbusOperation {
            context: "failed to get device paths from NetworkManager".to_string(),
            source: e,
        })?;

    let mut devices = Vec::new();
    for path in paths {
        let device = NMDeviceProxy::builder(conn)
            .path(path.clone())?
            .build()
            .await?;

        if device.device_type().await? != type_code {
            continue;
        }

        let interface = device
            .interface()
            .await
            .map_err(|e| ConnectionError::DbusOperation {
                context: format!("failed to get interface name for device {}", path.as_str()),
                source: e,
            })?;
        // Some virtual devices omit HwAddress; keep the row usable.
        let hw_address = device
            .hw_address()
            .await
            .unwrap_or_else(|_| String::from("00:00:00:00:00:00"));
        let state = device.state().await?.into();
        let ports = port_interfaces(conn, &path, &device).await?;

        devices.push(ControllerDevice {
            path,
            interface,
            hw_address,
            state,
            ports,
        });
    }

    Ok(devices)
}

/// Resolves the interface names of a controller's port devices (`Device.Ports`).
///
/// Errors are propagated: a port that cannot be read is not silently dropped.
async fn port_interfaces(
    conn: &Connection,
    controller: &OwnedObjectPath,
    device: &NMDeviceProxy<'_>,
) -> Result<Vec<String>> {
    let port_paths = device
        .ports()
        .await
        .map_err(|e| ConnectionError::DbusOperation {
            context: format!("failed to get ports for device {}", controller.as_str()),
            source: e,
        })?;

    let mut ports = Vec::with_capacity(port_paths.len());
    for port_path in port_paths {
        let port = NMDeviceProxy::builder(conn)
            .path(port_path.clone())?
            .cache_properties(CacheProperties::No)
            .build()
            .await?;
        let name = port
            .interface()
            .await
            .map_err(|e| ConnectionError::DbusOperation {
                context: format!(
                    "failed to get interface name for port device {}",
                    port_path.as_str()
                ),
                source: e,
            })?;
        ports.push(name);
    }
    Ok(ports)
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use zvariant::{OwnedValue, Str};

    /// Raw settings holding only `connection.type`.
    pub(crate) fn settings_of_type(ty: &str) -> RawSettings {
        HashMap::from([(
            "connection".to_string(),
            HashMap::from([("type".to_string(), OwnedValue::from(Str::from(ty)))]),
        )])
    }

    #[test]
    fn ensure_connection_type_accepts_matching_type() {
        ensure_connection_type(&settings_of_type("bond"), "u", "bond").expect("bond is a bond");
    }

    #[test]
    fn ensure_connection_type_rejects_other_or_missing_type() {
        for settings in [settings_of_type("802-3-ethernet"), HashMap::new()] {
            let err = ensure_connection_type(&settings, "u", "team").expect_err("not a team");
            assert!(matches!(
                err,
                ConnectionError::InvalidInput { ref field, ref reason }
                    if field == "uuid" && reason == "connection u is not a team profile"
            ));
        }
    }
}
