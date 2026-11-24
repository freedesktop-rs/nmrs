//! Bond controller configuration and device info.

use std::collections::BTreeMap;
use std::fmt;

use super::device::DeviceState;
use super::error::ConnectionError;
use crate::util::validation::validate_interface_name;

/// Bonding mode, stored as `mode` in `bond.options`.
///
/// # Examples
///
/// ```rust
/// use nmrs::BondMode;
///
/// assert_eq!(BondMode::ActiveBackup.as_str(), "active-backup");
/// assert_eq!(BondMode::Ieee8023ad.to_string(), "802.3ad");
/// ```
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BondMode {
    /// Round-robin transmission across ports (`balance-rr`).
    BalanceRr,
    /// One active port, others take over on failure (`active-backup`).
    ActiveBackup,
    /// Transmit policy based on a hash of packet headers (`balance-xor`).
    BalanceXor,
    /// Transmit on all ports (`broadcast`).
    Broadcast,
    /// IEEE 802.3ad dynamic link aggregation, LACP (`802.3ad`).
    Ieee8023ad,
    /// Adaptive transmit load balancing (`balance-tlb`).
    BalanceTlb,
    /// Adaptive load balancing, transmit and receive (`balance-alb`).
    BalanceAlb,
}

impl BondMode {
    /// Returns the mode name used by NetworkManager and the kernel.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::BalanceRr => "balance-rr",
            Self::ActiveBackup => "active-backup",
            Self::BalanceXor => "balance-xor",
            Self::Broadcast => "broadcast",
            Self::Ieee8023ad => "802.3ad",
            Self::BalanceTlb => "balance-tlb",
            Self::BalanceAlb => "balance-alb",
        }
    }

    /// Whether the `primary` bonding option is allowed in this mode.
    fn supports_primary(self) -> bool {
        matches!(
            self,
            Self::ActiveBackup | Self::BalanceTlb | Self::BalanceAlb
        )
    }
}

impl fmt::Display for BondMode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Bond controller configuration. Ports are separate profiles; see
/// [`builders::build_bond_port_connection`](crate::builders::build_bond_port_connection).
///
/// # Examples
///
/// ```rust
/// use nmrs::{BondConfig, BondMode};
///
/// let config = BondConfig::new("bond0", BondMode::ActiveBackup);
///
/// let config = BondConfig::new("bond1", BondMode::Ieee8023ad)
///     .with_connection_name("Uplink")
///     .with_miimon(100)
///     .with_mtu(9000)
///     .with_option("lacp_rate", "fast");
/// ```
#[non_exhaustive]
#[derive(Debug, Clone)]
pub struct BondConfig {
    /// Bond interface name (e.g., "bond0").
    pub interface_name: String,

    /// Bonding mode.
    pub mode: BondMode,

    /// Connection name. Defaults to "Bond {interface_name}".
    pub connection_name: Option<String>,

    /// MTU of the bond interface.
    pub mtu: Option<u32>,

    /// Bonding options other than `mode` (e.g. `miimon`), written to `bond.options`.
    pub options: BTreeMap<String, String>,
}

impl BondConfig {
    /// Creates a bond configuration.
    #[must_use]
    pub fn new(interface_name: impl Into<String>, mode: BondMode) -> Self {
        Self {
            interface_name: interface_name.into(),
            mode,
            connection_name: None,
            mtu: None,
            options: BTreeMap::new(),
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

    /// Sets the MII link monitoring interval in milliseconds (`miimon`).
    #[must_use]
    pub fn with_miimon(mut self, interval_ms: u32) -> Self {
        self.options
            .insert("miimon".to_string(), interval_ms.to_string());
        self
    }

    /// Sets the preferred port (`primary`). Only valid in active-backup,
    /// balance-tlb, and balance-alb; [`validate`](Self::validate) rejects it otherwise.
    #[must_use]
    pub fn with_primary(mut self, port: impl Into<String>) -> Self {
        self.options.insert("primary".to_string(), port.into());
        self
    }

    /// Sets a bonding option. `mode` belongs in [`BondConfig::mode`];
    /// [`validate`](Self::validate) rejects it here.
    #[must_use]
    pub fn with_option(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.options.insert(key.into(), value.into());
        self
    }

    /// Returns the connection name, defaulting to "Bond {interface_name}".
    #[must_use]
    pub fn effective_connection_name(&self) -> String {
        self.connection_name
            .clone()
            .unwrap_or_else(|| format!("Bond {}", self.interface_name))
    }

    /// Validates the configuration without any D-Bus call.
    ///
    /// # Errors
    ///
    /// Returns `ConnectionError::InvalidInput` if:
    /// - `interface_name` is not a valid kernel interface name
    /// - an option key is empty
    /// - the `mode` option is set in `options` (use [`BondConfig::mode`])
    /// - `primary` is set while the mode is not `active-backup`,
    ///   `balance-tlb`, or `balance-alb`
    /// - the `primary` value is not a valid kernel interface name
    pub fn validate(&self) -> Result<(), ConnectionError> {
        validate_interface_name(&self.interface_name, "interface_name")?;

        for key in self.options.keys() {
            if key.is_empty() {
                return Err(ConnectionError::InvalidInput {
                    field: "options".to_string(),
                    reason: "option key cannot be empty".to_string(),
                });
            }
            if key == "mode" {
                return Err(ConnectionError::InvalidInput {
                    field: "options".to_string(),
                    reason: "the bonding mode must be set with BondConfig::mode, not options"
                        .to_string(),
                });
            }
        }

        if let Some(primary) = self.options.get("primary") {
            if !self.mode.supports_primary() {
                return Err(ConnectionError::InvalidInput {
                    field: "options.primary".to_string(),
                    reason: format!(
                        "primary is not supported in {} mode (only active-backup, balance-tlb, balance-alb)",
                        self.mode
                    ),
                });
            }
            validate_interface_name(primary, "options.primary")?;
        }

        Ok(())
    }
}

/// A bond device, returned by
/// [`NetworkManager::list_bond_devices`](crate::NetworkManager::list_bond_devices).
#[non_exhaustive]
#[derive(Debug, Clone)]
pub struct BondDevice {
    /// D-Bus object path of the device.
    pub path: String,

    /// Interface name (e.g., "bond0").
    pub interface: String,

    /// Hardware (MAC) address of the bond.
    pub hw_address: String,

    /// Current device state.
    pub state: DeviceState,

    /// Interface names of the port devices attached to the bond.
    pub ports: Vec<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    const ALL_MODES: [(BondMode, &str); 7] = [
        (BondMode::BalanceRr, "balance-rr"),
        (BondMode::ActiveBackup, "active-backup"),
        (BondMode::BalanceXor, "balance-xor"),
        (BondMode::Broadcast, "broadcast"),
        (BondMode::Ieee8023ad, "802.3ad"),
        (BondMode::BalanceTlb, "balance-tlb"),
        (BondMode::BalanceAlb, "balance-alb"),
    ];

    #[test]
    fn mode_as_str_and_display_cover_every_variant() {
        for (mode, name) in ALL_MODES {
            assert_eq!(mode.as_str(), name);
            assert_eq!(mode.to_string(), name);
        }
    }

    #[test]
    fn new_creates_defaults() {
        let config = BondConfig::new("bond0", BondMode::ActiveBackup);
        assert_eq!(config.interface_name, "bond0");
        assert_eq!(config.mode, BondMode::ActiveBackup);
        assert!(config.connection_name.is_none());
        assert!(config.mtu.is_none());
        assert!(config.options.is_empty());
    }

    #[test]
    fn effective_connection_name_default_and_custom() {
        let config = BondConfig::new("bond0", BondMode::BalanceRr);
        assert_eq!(config.effective_connection_name(), "Bond bond0");

        let config = config.with_connection_name("Uplink");
        assert_eq!(config.effective_connection_name(), "Uplink");
    }

    #[test]
    fn builder_methods_chain() {
        let config = BondConfig::new("bond0", BondMode::ActiveBackup)
            .with_connection_name("Uplink")
            .with_mtu(9000)
            .with_miimon(100)
            .with_primary("eth0")
            .with_option("updelay", "200");

        assert_eq!(config.connection_name.as_deref(), Some("Uplink"));
        assert_eq!(config.mtu, Some(9000));
        assert_eq!(config.options["miimon"], "100");
        assert_eq!(config.options["primary"], "eth0");
        assert_eq!(config.options["updelay"], "200");
        assert_eq!(config.options.len(), 3);
    }

    #[test]
    fn with_option_overwrites_existing_key() {
        let config = BondConfig::new("bond0", BondMode::BalanceRr)
            .with_miimon(100)
            .with_option("miimon", "50");
        assert_eq!(config.options["miimon"], "50");
    }

    #[test]
    fn validate_accepts_valid_configs() {
        BondConfig::new("bond0", BondMode::BalanceRr)
            .validate()
            .unwrap();
        BondConfig::new("bond0", BondMode::ActiveBackup)
            .with_miimon(100)
            .with_primary("eth0")
            .validate()
            .unwrap();
        BondConfig::new("bond0", BondMode::Ieee8023ad)
            .with_option("lacp_rate", "fast")
            .validate()
            .unwrap();
    }

    #[test]
    fn validate_rejects_invalid_interface_name() {
        for name in ["", ".", "a/b", "has space", "waytoolonginterfacename"] {
            let err = BondConfig::new(name, BondMode::BalanceRr)
                .validate()
                .unwrap_err();
            assert!(
                matches!(err, ConnectionError::InvalidInput { ref field, .. } if field == "interface_name"),
                "{name:?}: {err:?}"
            );
        }
    }

    #[test]
    fn validate_rejects_empty_option_key() {
        let err = BondConfig::new("bond0", BondMode::BalanceRr)
            .with_option("", "1")
            .validate()
            .unwrap_err();
        assert!(matches!(
            err,
            ConnectionError::InvalidInput { ref field, .. } if field == "options"
        ));
    }

    #[test]
    fn validate_rejects_mode_in_options() {
        let err = BondConfig::new("bond0", BondMode::BalanceRr)
            .with_option("mode", "active-backup")
            .validate()
            .unwrap_err();
        assert!(matches!(
            err,
            ConnectionError::InvalidInput { ref field, ref reason }
                if field == "options" && reason.contains("BondConfig::mode")
        ));
    }

    #[test]
    fn validate_rejects_primary_in_unsupported_modes() {
        for mode in [
            BondMode::BalanceRr,
            BondMode::BalanceXor,
            BondMode::Broadcast,
            BondMode::Ieee8023ad,
        ] {
            let err = BondConfig::new("bond0", mode)
                .with_primary("eth0")
                .validate()
                .unwrap_err();
            assert!(
                matches!(err, ConnectionError::InvalidInput { ref field, .. } if field == "options.primary"),
                "{mode}: {err:?}"
            );
        }
    }

    #[test]
    fn validate_accepts_primary_in_supported_modes() {
        for mode in [
            BondMode::ActiveBackup,
            BondMode::BalanceTlb,
            BondMode::BalanceAlb,
        ] {
            BondConfig::new("bond0", mode)
                .with_primary("eth0")
                .validate()
                .unwrap();
        }
    }

    #[test]
    fn validate_rejects_invalid_primary_name() {
        for name in ["", "a/b", "has space", "waytoolonginterfacename"] {
            let err = BondConfig::new("bond0", BondMode::ActiveBackup)
                .with_primary(name)
                .validate()
                .unwrap_err();
            assert!(
                matches!(err, ConnectionError::InvalidInput { ref field, .. } if field == "options.primary"),
                "{name:?}: {err:?}"
            );
        }
    }
}
