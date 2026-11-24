//! Bond controller and port builders.

use std::collections::HashMap;
use zvariant::Value;

use super::connection_builder::ConnectionBuilder;
use super::port::{build_port_connection, with_wired_mtu};
use crate::ConnectionOptions;
use crate::api::models::{BondConfig, ConnectionError};

/// Builds a bond controller profile.
///
/// The mode and [`BondConfig::options`] go to `bond.options` (`a{ss}`).
/// `802-3-ethernet` only when an MTU is set.
///
/// # Errors
///
/// Returns `ConnectionError::InvalidInput` if [`BondConfig::validate`] fails.
///
/// # Examples
///
/// ```rust
/// use nmrs::builders::build_bond_connection;
/// use nmrs::{BondConfig, BondMode, ConnectionOptions};
///
/// let config = BondConfig::new("bond0", BondMode::ActiveBackup).with_miimon(100);
/// let opts = ConnectionOptions::new(true);
///
/// let settings = build_bond_connection(&config, &opts).unwrap();
/// ```
pub fn build_bond_connection(
    config: &BondConfig,
    opts: &ConnectionOptions,
) -> Result<HashMap<&'static str, HashMap<&'static str, Value<'static>>>, ConnectionError> {
    config.validate()?;

    let builder = ConnectionBuilder::new("bond", config.effective_connection_name())
        .interface_name(config.interface_name.clone())
        .options(opts)
        .with_section("bond", bond_section(config))
        .ipv4_auto()
        .ipv6_auto();
    Ok(with_wired_mtu(builder, config.mtu).build())
}

/// Builds a port profile attaching `port_interface` to a bond
/// (`controller`/`port-type`, no IP configuration).
///
/// # Errors
///
/// Returns `ConnectionError::InvalidInput` if either interface name is
/// invalid or both are equal.
pub fn build_bond_port_connection(
    bond_interface: &str,
    port_interface: &str,
    opts: &ConnectionOptions,
) -> Result<HashMap<&'static str, HashMap<&'static str, Value<'static>>>, ConnectionError> {
    build_port_connection("bond", bond_interface, port_interface, opts)
}

fn bond_section(config: &BondConfig) -> HashMap<&'static str, Value<'static>> {
    let mut options: HashMap<String, String> = config.options.clone().into_iter().collect();
    options.insert("mode".to_string(), config.mode.as_str().to_string());
    HashMap::from([("options", Value::from(options))])
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::api::models::BondMode;

    use crate::api::builders::port::tests::test_opts;

    fn bond_options(
        conn: &HashMap<&'static str, HashMap<&'static str, Value<'static>>>,
    ) -> HashMap<String, String> {
        HashMap::<String, String>::try_from(conn["bond"]["options"].clone())
            .expect("bond.options must be a{ss}")
    }

    #[test]
    fn builds_basic_bond_connection() {
        let config = BondConfig::new("bond0", BondMode::ActiveBackup);
        let conn = build_bond_connection(&config, &test_opts()).unwrap();

        let c = &conn["connection"];
        assert_eq!(c["type"], Value::from("bond"));
        assert_eq!(c["id"], Value::from("Bond bond0"));
        assert_eq!(c["interface-name"], Value::from("bond0"));
        assert_eq!(c["autoconnect"], Value::from(true));
        assert_eq!(c["autoconnect-priority"], Value::from(10i32));
        assert_eq!(c["autoconnect-retries"], Value::from(3i32));
        assert_eq!(conn["ipv4"]["method"], Value::from("auto"));
        assert_eq!(conn["ipv6"]["method"], Value::from("auto"));

        let options = bond_options(&conn);
        assert_eq!(options.len(), 1);
        assert_eq!(options["mode"], "active-backup");
    }

    #[test]
    fn uses_custom_connection_name() {
        let config = BondConfig::new("bond0", BondMode::BalanceRr).with_connection_name("Uplink");
        let conn = build_bond_connection(&config, &test_opts()).unwrap();

        assert_eq!(conn["connection"]["id"], Value::from("Uplink"));
    }

    #[test]
    fn bond_options_signature_is_string_dict() {
        let config = BondConfig::new("bond0", BondMode::Ieee8023ad).with_miimon(100);
        let conn = build_bond_connection(&config, &test_opts()).unwrap();

        assert_eq!(
            conn["bond"]["options"].value_signature().to_string(),
            "a{ss}"
        );
    }

    #[test]
    fn writes_mode_miimon_primary_and_extra_options() {
        let config = BondConfig::new("bond0", BondMode::ActiveBackup)
            .with_miimon(100)
            .with_primary("eth0")
            .with_option("updelay", "200");
        let conn = build_bond_connection(&config, &test_opts()).unwrap();
        let options = bond_options(&conn);

        assert_eq!(options.len(), 4);
        assert_eq!(options["mode"], "active-backup");
        assert_eq!(options["miimon"], "100");
        assert_eq!(options["primary"], "eth0");
        assert_eq!(options["updelay"], "200");
    }

    #[test]
    fn serializes_mtu_in_wired_setting() {
        let config = BondConfig::new("bond0", BondMode::BalanceRr).with_mtu(9000);
        let conn = build_bond_connection(&config, &test_opts()).unwrap();
        let wired = conn
            .get("802-3-ethernet")
            .expect("MTU requires an 802-3-ethernet setting");

        assert_eq!(wired["mtu"], Value::from(9000u32));
        assert_eq!(wired["mtu"].value_signature().to_string(), "u");
        assert!(!conn["bond"].contains_key("mtu"));
    }

    #[test]
    fn omits_wired_setting_without_mtu() {
        let conn =
            build_bond_connection(&BondConfig::new("bond0", BondMode::BalanceRr), &test_opts())
                .unwrap();

        assert!(!conn.contains_key("802-3-ethernet"));
    }

    #[test]
    fn propagates_model_validation_errors() {
        let err = build_bond_connection(&BondConfig::new("", BondMode::BalanceRr), &test_opts())
            .unwrap_err();
        assert!(matches!(
            err,
            ConnectionError::InvalidInput { ref field, .. } if field == "interface_name"
        ));

        let config = BondConfig::new("bond0", BondMode::BalanceRr).with_primary("eth0");
        let err = build_bond_connection(&config, &test_opts()).unwrap_err();
        assert!(matches!(
            err,
            ConnectionError::InvalidInput { ref field, .. } if field == "options.primary"
        ));

        let config = BondConfig::new("bond0", BondMode::BalanceRr).with_option("mode", "x");
        let err = build_bond_connection(&config, &test_opts()).unwrap_err();
        assert!(matches!(
            err,
            ConnectionError::InvalidInput { ref field, .. } if field == "options"
        ));
    }

    #[test]
    fn port_profile_attaches_to_bond() {
        let conn = build_bond_port_connection("bond0", "eth0", &test_opts()).unwrap();

        assert_eq!(conn["connection"]["controller"], Value::from("bond0"));
        assert_eq!(conn["connection"]["port-type"], Value::from("bond"));
        assert_eq!(conn["connection"]["id"], Value::from("bond-port-eth0"));
    }
}
