//! Bond device and profile operations.

use std::collections::HashMap;

use zbus::Connection;
use zvariant::{Dict, OwnedValue, Value};

use crate::Result;
use crate::api::builders::{build_bond_connection, build_bond_port_connection};
use crate::api::models::{
    BondConfig, BondDevice, BondMode, ConnectionError, ConnectionOptions, DeviceType, RawSettings,
};
use crate::core::controller::{controller_devices, ensure_connection_type, save_profile};
use crate::core::saved_connection::edit_saved_connection;

/// Saves a bond controller profile and returns its UUID.
pub(crate) async fn create_bond(
    conn: &Connection,
    config: &BondConfig,
    opts: &ConnectionOptions,
) -> Result<String> {
    let settings = build_bond_connection(config, opts)?;
    save_profile(conn, settings).await
}

/// Saves a port profile attaching `port_interface` to `bond_interface` and returns its UUID.
pub(crate) async fn add_bond_port(
    conn: &Connection,
    bond_interface: &str,
    port_interface: &str,
    opts: &ConnectionOptions,
) -> Result<String> {
    let settings = build_bond_port_connection(bond_interface, port_interface, opts)?;
    save_profile(conn, settings).await
}

/// Changes the bonding mode of a saved bond profile, keeping its other options.
pub(crate) async fn set_bond_mode(conn: &Connection, uuid: &str, mode: BondMode) -> Result<()> {
    edit_saved_connection(conn, uuid, |settings| set_mode(settings, uuid, mode)).await
}

/// Sets `bond.options["mode"]` in a profile's settings, keeping every other option.
fn set_mode(settings: &mut RawSettings, uuid: &str, mode: BondMode) -> Result<()> {
    ensure_connection_type(settings, uuid, "bond")?;

    let bond = settings.entry("bond".to_string()).or_default();
    let mut options = match bond.remove("options") {
        Some(value) => HashMap::<String, String>::try_from(value)
            .map_err(|_| ConnectionError::MalformedSavedConnection(uuid.to_string()))?,
        None => HashMap::new(),
    };
    options.insert("mode".to_string(), mode.as_str().to_string());

    let options = OwnedValue::try_from(Value::Dict(Dict::from(options)))
        .map_err(|e| ConnectionError::Dbus(e.into()))?;
    bond.insert("options".to_string(), options);
    Ok(())
}

/// Lists bond devices with their MAC address, state, and port interfaces.
pub(crate) async fn list_bond_devices(conn: &Connection) -> Result<Vec<BondDevice>> {
    Ok(controller_devices(conn, DeviceType::Bond.to_code())
        .await?
        .into_iter()
        .map(|device| BondDevice {
            path: device.path.to_string(),
            interface: device.interface,
            hw_address: device.hw_address,
            state: device.state,
            ports: device.ports,
        })
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::controller::tests::settings_of_type;

    fn bond_settings(options: Option<&[(&str, &str)]>) -> RawSettings {
        let mut settings = settings_of_type("bond");
        if let Some(options) = options {
            let map: HashMap<String, String> = options
                .iter()
                .map(|(k, v)| ((*k).to_string(), (*v).to_string()))
                .collect();
            let value = OwnedValue::try_from(Value::Dict(Dict::from(map))).expect("a{ss} value");
            settings.insert(
                "bond".to_string(),
                HashMap::from([("options".to_string(), value)]),
            );
        }
        settings
    }

    fn options_after(
        mut settings: RawSettings,
        mode: BondMode,
    ) -> (String, HashMap<String, String>) {
        set_mode(&mut settings, "u", mode).expect("set mode");
        let value = settings["bond"]["options"]
            .try_clone()
            .expect("clone options");
        let signature = value.value_signature().to_string();
        let map = HashMap::<String, String>::try_from(value).expect("a{ss} options");
        (signature, map)
    }

    #[test]
    fn set_mode_replaces_mode_and_keeps_other_options() {
        let settings = bond_settings(Some(&[
            ("mode", "active-backup"),
            ("miimon", "100"),
            ("primary", "eth0"),
        ]));
        let (signature, options) = options_after(settings, BondMode::BalanceAlb);
        assert_eq!(signature, "a{ss}");
        assert_eq!(
            options,
            HashMap::from([
                ("mode".into(), "balance-alb".into()),
                ("miimon".into(), "100".into()),
                ("primary".into(), "eth0".into()),
            ])
        );
    }

    #[test]
    fn set_mode_without_options_writes_mode() {
        let (_, options) = options_after(bond_settings(None), BondMode::Ieee8023ad);
        assert_eq!(options, HashMap::from([("mode".into(), "802.3ad".into())]));
    }

    #[test]
    fn set_mode_rejects_non_bond_profile() {
        let mut settings = settings_of_type("802-3-ethernet");
        let err = set_mode(&mut settings, "u", BondMode::BalanceRr).expect_err("must reject");
        assert!(matches!(
            err,
            ConnectionError::InvalidInput { ref field, .. } if field == "uuid"
        ));
        assert!(!settings.contains_key("bond"));
    }

    #[test]
    fn set_mode_rejects_malformed_options() {
        let mut settings = bond_settings(None);
        settings.insert(
            "bond".to_string(),
            HashMap::from([("options".to_string(), OwnedValue::from(42u32))]),
        );
        let err = set_mode(&mut settings, "u", BondMode::BalanceRr).expect_err("must reject");
        assert!(matches!(err, ConnectionError::MalformedSavedConnection(ref u) if u == "u"));
    }
}
