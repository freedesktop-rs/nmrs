//! Team device and profile operations.

use zbus::Connection;
use zbus::proxy::CacheProperties;
use zvariant::{OwnedValue, Str};

use crate::Result;
use crate::api::builders::{build_team_connection, build_team_port_connection};
use crate::api::models::{
    ConnectionOptions, DeviceType, RawSettings, TeamConfig, TeamDevice, TeamRunner,
};
use crate::core::controller::{controller_devices, ensure_connection_type, save_profile};
use crate::core::saved_connection::edit_saved_connection;
use crate::dbus::NMTeamProxy;

/// Saves a team controller profile and returns its UUID.
pub(crate) async fn create_team(
    conn: &Connection,
    config: &TeamConfig,
    opts: &ConnectionOptions,
) -> Result<String> {
    let settings = build_team_connection(config, opts)?;
    save_profile(conn, settings).await
}

/// Saves a port profile attaching `port_interface` to `team_interface` and returns its UUID.
pub(crate) async fn add_team_port(
    conn: &Connection,
    team_interface: &str,
    port_interface: &str,
    opts: &ConnectionOptions,
) -> Result<String> {
    let settings = build_team_port_connection(team_interface, port_interface, opts)?;
    save_profile(conn, settings).await
}

/// Changes the runner of a saved team profile, dropping any custom `team.config`.
pub(crate) async fn set_team_runner(
    conn: &Connection,
    uuid: &str,
    runner: TeamRunner,
) -> Result<()> {
    edit_saved_connection(conn, uuid, |settings| set_runner(settings, uuid, runner)).await
}

/// Sets `team.runner` and removes `team.config`.
///
/// `GetSettings` always returns a synthesized `team.config`, which makes NM
/// ignore `runner`. Keeping it would silently no-op. Custom JSON is dropped,
/// as with `nmcli ... team.runner`.
fn set_runner(settings: &mut RawSettings, uuid: &str, runner: TeamRunner) -> Result<()> {
    ensure_connection_type(settings, uuid, "team")?;

    let team = settings.entry("team".to_string()).or_default();
    team.remove("config");
    team.insert(
        "runner".to_string(),
        OwnedValue::from(Str::from(runner.as_str())),
    );
    Ok(())
}

/// Lists team devices with their MAC address, state, ports, and applied config.
pub(crate) async fn list_team_devices(conn: &Connection) -> Result<Vec<TeamDevice>> {
    let mut teams = Vec::new();
    for device in controller_devices(conn, DeviceType::Team.to_code()).await? {
        let team = NMTeamProxy::builder(conn)
            .path(device.path.clone())?
            .cache_properties(CacheProperties::No)
            .build()
            .await?;
        let config = team.config().await.ok().filter(|c| !c.is_empty());

        teams.push(TeamDevice {
            path: device.path.to_string(),
            interface: device.interface,
            hw_address: device.hw_address,
            state: device.state,
            ports: device.ports,
            config,
        });
    }
    Ok(teams)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::api::models::{ConnectionError, FromSetting};
    use crate::core::controller::tests::settings_of_type;
    use std::collections::HashMap;

    #[test]
    fn set_runner_writes_runner_and_drops_config() {
        let mut settings = settings_of_type("team");
        settings.insert(
            "team".to_string(),
            HashMap::from([
                (
                    "config".to_string(),
                    OwnedValue::from(Str::from(r#"{"runner":{"name":"activebackup"}}"#)),
                ),
                (
                    "runner".to_string(),
                    OwnedValue::from(Str::from("activebackup")),
                ),
            ]),
        );

        set_runner(&mut settings, "u", TeamRunner::Lacp).expect("set runner");

        let team = &settings["team"];
        assert!(!team.contains_key("config"));
        assert_eq!(
            String::from_setting(&team["runner"]).as_deref(),
            Some("lacp")
        );
    }

    #[test]
    fn set_runner_rejects_non_team_profile() {
        let mut settings = settings_of_type("bond");
        let err = set_runner(&mut settings, "u", TeamRunner::Lacp).expect_err("must reject");
        assert!(matches!(
            err,
            ConnectionError::InvalidInput { ref field, .. } if field == "uuid"
        ));
        assert!(!settings.contains_key("team"));
    }
}
