//! Team controller configuration and device info.

use std::fmt;

use super::device::DeviceState;
use super::error::ConnectionError;
use crate::util::validation::validate_interface_name;

/// Team runner (load-balancing/failover strategy), stored in `team.runner`.
///
/// # Examples
///
/// ```rust
/// use nmrs::TeamRunner;
///
/// assert_eq!(TeamRunner::ActiveBackup.as_str(), "activebackup");
/// assert_eq!(TeamRunner::Lacp.to_string(), "lacp");
/// ```
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TeamRunner {
    /// Transmit in turn on each port (`roundrobin`).
    RoundRobin,
    /// Transmit on all ports (`broadcast`).
    Broadcast,
    /// One active port, others take over on failure (`activebackup`).
    ActiveBackup,
    /// Hash-based load balancing (`loadbalance`).
    LoadBalance,
    /// 802.3ad link aggregation (`lacp`).
    Lacp,
    /// Transmit on a randomly chosen port (`random`).
    Random,
}

impl TeamRunner {
    /// Returns the runner name used by NetworkManager.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::RoundRobin => "roundrobin",
            Self::Broadcast => "broadcast",
            Self::ActiveBackup => "activebackup",
            Self::LoadBalance => "loadbalance",
            Self::Lacp => "lacp",
            Self::Random => "random",
        }
    }
}

impl fmt::Display for TeamRunner {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Team controller configuration. Ports are separate profiles; see
/// [`builders::build_team_port_connection`](crate::builders::build_team_port_connection).
///
/// # Examples
///
/// ```rust
/// use nmrs::{TeamConfig, TeamRunner};
///
/// let config = TeamConfig::new("team0", TeamRunner::ActiveBackup);
///
/// let config = TeamConfig::new("team1", TeamRunner::Lacp)
///     .with_connection_name("Uplink")
///     .with_mtu(9000);
/// ```
#[non_exhaustive]
#[derive(Debug, Clone)]
pub struct TeamConfig {
    /// Team interface name (e.g., "team0").
    pub interface_name: String,
    /// Team runner.
    pub runner: TeamRunner,
    /// Connection name. Defaults to "Team {interface_name}".
    pub connection_name: Option<String>,
    /// MTU of the team interface.
    pub mtu: Option<u32>,
}

impl TeamConfig {
    /// Creates a team configuration.
    #[must_use]
    pub fn new(interface_name: impl Into<String>, runner: TeamRunner) -> Self {
        Self {
            interface_name: interface_name.into(),
            runner,
            connection_name: None,
            mtu: None,
        }
    }

    /// Sets a human-readable connection name.
    #[must_use]
    pub fn with_connection_name(mut self, name: impl Into<String>) -> Self {
        self.connection_name = Some(name.into());
        self
    }

    /// Sets the MTU.
    #[must_use]
    pub fn with_mtu(mut self, mtu: u32) -> Self {
        self.mtu = Some(mtu);
        self
    }

    /// Returns the connection name, defaulting to "Team {interface_name}".
    #[must_use]
    pub fn effective_connection_name(&self) -> String {
        self.connection_name
            .clone()
            .unwrap_or_else(|| format!("Team {}", self.interface_name))
    }

    /// Validates the configuration without any D-Bus call.
    ///
    /// Returns `ConnectionError::InvalidInput` if `interface_name` is not a
    /// valid kernel interface name.
    pub fn validate(&self) -> Result<(), ConnectionError> {
        validate_interface_name(&self.interface_name, "interface_name")
    }
}

/// A team device, returned by
/// [`NetworkManager::list_team_devices`](crate::NetworkManager::list_team_devices).
#[non_exhaustive]
#[derive(Debug, Clone)]
pub struct TeamDevice {
    /// D-Bus object path of the device.
    pub path: String,

    /// Interface name (e.g., "team0").
    pub interface: String,

    /// Hardware (MAC) address of the team.
    pub hw_address: String,

    /// Current device state.
    pub state: DeviceState,

    /// Interface names of the port devices attached to the team.
    pub ports: Vec<String>,

    /// JSON config applied on the device (`Device.Team.Config`); `None` if
    /// empty or unreadable.
    pub config: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    const ALL_RUNNERS: [(TeamRunner, &str); 6] = [
        (TeamRunner::RoundRobin, "roundrobin"),
        (TeamRunner::Broadcast, "broadcast"),
        (TeamRunner::ActiveBackup, "activebackup"),
        (TeamRunner::LoadBalance, "loadbalance"),
        (TeamRunner::Lacp, "lacp"),
        (TeamRunner::Random, "random"),
    ];

    #[test]
    fn runner_as_str_and_display_cover_every_variant() {
        for (runner, name) in ALL_RUNNERS {
            assert_eq!(runner.as_str(), name);
            assert_eq!(runner.to_string(), name);
        }
    }

    #[test]
    fn new_creates_defaults() {
        let config = TeamConfig::new("team0", TeamRunner::Lacp);
        assert_eq!(config.interface_name, "team0");
        assert_eq!(config.runner, TeamRunner::Lacp);
        assert!(config.connection_name.is_none());
        assert!(config.mtu.is_none());
    }

    #[test]
    fn effective_connection_name_default_and_custom() {
        let config = TeamConfig::new("team0", TeamRunner::Lacp);
        assert_eq!(config.effective_connection_name(), "Team team0");

        let config = config.with_connection_name("Uplink");
        assert_eq!(config.effective_connection_name(), "Uplink");
    }

    #[test]
    fn builder_methods_chain() {
        let config = TeamConfig::new("team0", TeamRunner::ActiveBackup)
            .with_connection_name("Uplink")
            .with_mtu(9000);
        assert_eq!(config.connection_name.as_deref(), Some("Uplink"));
        assert_eq!(config.mtu, Some(9000));
    }

    #[test]
    fn validate_accepts_valid_config() {
        TeamConfig::new("team0", TeamRunner::RoundRobin)
            .validate()
            .unwrap();
    }

    #[test]
    fn validate_rejects_invalid_interface_name() {
        for name in [
            "",
            "..",
            "a/b",
            "a:b",
            "has space",
            "waytoolonginterfacename",
        ] {
            let err = TeamConfig::new(name, TeamRunner::Lacp)
                .validate()
                .unwrap_err();
            assert!(
                matches!(err, ConnectionError::InvalidInput { ref field, .. } if field == "interface_name"),
                "{name:?}: {err:?}"
            );
        }
    }
}
