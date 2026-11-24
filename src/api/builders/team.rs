//! Team controller and port builders.

use std::collections::HashMap;
use zvariant::Value;

use super::connection_builder::ConnectionBuilder;
use super::port::{build_port_connection, with_wired_mtu};
use crate::ConnectionOptions;
use crate::api::models::{ConnectionError, TeamConfig};

/// Builds a team controller profile.
///
/// Writes `team.runner`, never `team.config` (NM ignores individual team
/// properties when it is present). `802-3-ethernet` only when an MTU is set.
///
/// # Errors
///
/// Returns `ConnectionError::InvalidInput` if [`TeamConfig::validate`] fails.
///
/// # Examples
///
/// ```rust
/// use nmrs::builders::build_team_connection;
/// use nmrs::{ConnectionOptions, TeamConfig, TeamRunner};
///
/// let config = TeamConfig::new("team0", TeamRunner::ActiveBackup);
/// let opts = ConnectionOptions::new(true);
///
/// let settings = build_team_connection(&config, &opts).unwrap();
/// ```
pub fn build_team_connection(
    config: &TeamConfig,
    opts: &ConnectionOptions,
) -> Result<HashMap<&'static str, HashMap<&'static str, Value<'static>>>, ConnectionError> {
    config.validate()?;

    let builder = ConnectionBuilder::new("team", config.effective_connection_name())
        .interface_name(config.interface_name.clone())
        .options(opts)
        .with_section("team", team_section(config))
        .ipv4_auto()
        .ipv6_auto();
    Ok(with_wired_mtu(builder, config.mtu).build())
}

/// Builds a port profile attaching `port_interface` to a team
/// (`controller`/`port-type`, no IP configuration).
///
/// # Errors
///
/// Returns `ConnectionError::InvalidInput` if either interface name is
/// invalid or both are equal.
pub fn build_team_port_connection(
    team_interface: &str,
    port_interface: &str,
    opts: &ConnectionOptions,
) -> Result<HashMap<&'static str, HashMap<&'static str, Value<'static>>>, ConnectionError> {
    build_port_connection("team", team_interface, port_interface, opts)
}

fn team_section(config: &TeamConfig) -> HashMap<&'static str, Value<'static>> {
    HashMap::from([("runner", Value::from(config.runner.as_str()))])
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::api::models::TeamRunner;

    use crate::api::builders::port::tests::test_opts;

    #[test]
    fn builds_basic_team_connection() {
        let config = TeamConfig::new("team0", TeamRunner::ActiveBackup);
        let conn = build_team_connection(&config, &test_opts()).unwrap();

        let c = &conn["connection"];
        assert_eq!(c["type"], Value::from("team"));
        assert_eq!(c["id"], Value::from("Team team0"));
        assert_eq!(c["interface-name"], Value::from("team0"));
        assert_eq!(c["autoconnect"], Value::from(true));
        assert_eq!(c["autoconnect-priority"], Value::from(10i32));
        assert_eq!(c["autoconnect-retries"], Value::from(3i32));
        assert_eq!(conn["team"]["runner"], Value::from("activebackup"));
        assert_eq!(conn["team"]["runner"].value_signature().to_string(), "s");
        assert_eq!(conn["ipv4"]["method"], Value::from("auto"));
        assert_eq!(conn["ipv6"]["method"], Value::from("auto"));
    }

    #[test]
    fn uses_custom_connection_name() {
        let config = TeamConfig::new("team0", TeamRunner::Lacp).with_connection_name("Uplink");
        let conn = build_team_connection(&config, &test_opts()).unwrap();

        assert_eq!(conn["connection"]["id"], Value::from("Uplink"));
    }

    #[test]
    fn does_not_write_team_config() {
        let conn = build_team_connection(&TeamConfig::new("team0", TeamRunner::Lacp), &test_opts())
            .unwrap();

        assert!(!conn["team"].contains_key("config"));
        assert_eq!(conn["team"].len(), 1);
    }

    #[test]
    fn serializes_mtu_in_wired_setting() {
        let config = TeamConfig::new("team0", TeamRunner::Lacp).with_mtu(9000);
        let conn = build_team_connection(&config, &test_opts()).unwrap();
        let wired = conn
            .get("802-3-ethernet")
            .expect("MTU requires an 802-3-ethernet setting");

        assert_eq!(wired["mtu"], Value::from(9000u32));
        assert_eq!(wired["mtu"].value_signature().to_string(), "u");
        assert!(!conn["team"].contains_key("mtu"));
    }

    #[test]
    fn omits_wired_setting_without_mtu() {
        let conn = build_team_connection(&TeamConfig::new("team0", TeamRunner::Lacp), &test_opts())
            .unwrap();

        assert!(!conn.contains_key("802-3-ethernet"));
    }

    #[test]
    fn propagates_model_validation_errors() {
        let err = build_team_connection(&TeamConfig::new("a/b", TeamRunner::Lacp), &test_opts())
            .unwrap_err();
        assert!(matches!(
            err,
            ConnectionError::InvalidInput { ref field, .. } if field == "interface_name"
        ));
    }

    #[test]
    fn port_profile_attaches_to_team() {
        let conn = build_team_port_connection("team0", "eth0", &test_opts()).unwrap();

        assert_eq!(conn["connection"]["controller"], Value::from("team0"));
        assert_eq!(conn["connection"]["port-type"], Value::from("team"));
        assert_eq!(conn["connection"]["id"], Value::from("team-port-eth0"));
    }
}
