//! VLAN (802.1Q) connection configuration.
//!
//! This module provides types for configuring VLAN connections over a parent
//! Ethernet or other interface.

use super::error::ConnectionError;

/// VLAN connection configuration.
///
/// Configures a VLAN (802.1Q) virtual interface on top of a parent device.
/// VLANs allow you to segment network traffic on a single physical interface
/// into multiple logical networks.
///
/// # Examples
///
/// ```rust
/// use nmrs::VlanConfig;
///
/// // Basic VLAN on eth0 with ID 100
/// let config = VlanConfig::new("eth0", 100);
///
/// // VLAN with custom interface name and priority mapping
/// let config = VlanConfig::new("eth0", 200)
///     .with_interface_name("vlan200")
///     .with_mtu(1500)
///     .with_connection_name("Office VLAN");
/// ```
#[non_exhaustive]
#[derive(Debug, Clone)]
pub struct VlanConfig {
    /// Parent interface name (e.g., "eth0", "enp3s0").
    pub parent: String,

    /// VLAN ID (1-4094).
    pub id: u16,

    /// Optional name for the VLAN interface.
    /// Defaults to `{parent}.{id}` (e.g., "eth0.100").
    pub interface_name: Option<String>,

    /// Optional human-readable connection name.
    /// Defaults to "VLAN {id} on {parent}".
    pub connection_name: Option<String>,

    /// Optional MTU for the VLAN interface.
    pub mtu: Option<u32>,

    /// VLAN flags (bitmask).
    /// - 0x1: Reorder headers (default on)
    /// - 0x2: GVRP (GARP VLAN Registration Protocol)
    /// - 0x4: Loose binding (don't fail if parent is down)
    /// - 0x8: MVRP (Multiple VLAN Registration Protocol)
    pub flags: Option<u32>,

    /// Ingress priority mapping (802.1p to Linux priority).
    /// Format: "from:to" pairs, e.g., vec!["0:0", "1:1", "2:2"]
    pub ingress_priority_map: Option<Vec<String>>,

    /// Egress priority mapping (Linux priority to 802.1p).
    /// Format: "from:to" pairs, e.g., vec!["0:0", "1:1", "2:2"]
    pub egress_priority_map: Option<Vec<String>>,
}

impl VlanConfig {
    /// Creates a new VLAN configuration.
    ///
    /// # Arguments
    ///
    /// * `parent` - Parent interface name (e.g., "eth0")
    /// * `id` - VLAN ID (1-4094)
    ///
    /// # Examples
    ///
    /// ```rust
    /// use nmrs::VlanConfig;
    ///
    /// let config = VlanConfig::new("eth0", 100);
    /// assert_eq!(config.parent, "eth0");
    /// assert_eq!(config.id, 100);
    /// ```
    #[must_use]
    pub fn new(parent: impl Into<String>, id: u16) -> Self {
        Self {
            parent: parent.into(),
            id,
            interface_name: None,
            connection_name: None,
            mtu: None,
            flags: None,
            ingress_priority_map: None,
            egress_priority_map: None,
        }
    }

    /// Sets a custom interface name for the VLAN device.
    ///
    /// By default, the interface is named `{parent}.{id}` (e.g., "eth0.100").
    ///
    /// # Examples
    ///
    /// ```rust
    /// use nmrs::VlanConfig;
    ///
    /// let config = VlanConfig::new("eth0", 100)
    ///     .with_interface_name("office-vlan");
    /// ```
    #[must_use]
    pub fn with_interface_name(mut self, name: impl Into<String>) -> Self {
        self.interface_name = Some(name.into());
        self
    }

    /// Sets a human-readable connection name.
    ///
    /// This is the name shown in NetworkManager's connection list.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use nmrs::VlanConfig;
    ///
    /// let config = VlanConfig::new("eth0", 100)
    ///     .with_connection_name("Office Network");
    /// ```
    #[must_use]
    pub fn with_connection_name(mut self, name: impl Into<String>) -> Self {
        self.connection_name = Some(name.into());
        self
    }

    /// Sets the MTU (Maximum Transmission Unit) for the VLAN interface.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use nmrs::VlanConfig;
    ///
    /// let config = VlanConfig::new("eth0", 100)
    ///     .with_mtu(1496); // Account for VLAN header
    /// ```
    #[must_use]
    pub fn with_mtu(mut self, mtu: u32) -> Self {
        self.mtu = Some(mtu);
        self
    }

    /// Sets VLAN flags.
    ///
    /// Flags:
    /// - `0x1`: Reorder headers (default, recommended)
    /// - `0x2`: GVRP (GARP VLAN Registration Protocol)
    /// - `0x4`: Loose binding (don't fail if parent is down)
    /// - `0x8`: MVRP (Multiple VLAN Registration Protocol)
    ///
    /// # Examples
    ///
    /// ```rust
    /// use nmrs::VlanConfig;
    ///
    /// // Enable loose binding (allow VLAN even if parent is down)
    /// let config = VlanConfig::new("eth0", 100)
    ///     .with_flags(0x1 | 0x4);
    /// ```
    #[must_use]
    pub fn with_flags(mut self, flags: u32) -> Self {
        self.flags = Some(flags);
        self
    }

    /// Sets ingress priority mapping (802.1p priority to Linux skb priority).
    ///
    /// Each entry maps an incoming 802.1p priority (0-7) to a Linux priority.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use nmrs::VlanConfig;
    ///
    /// let config = VlanConfig::new("eth0", 100)
    ///     .with_ingress_priority_map(vec!["0:0", "1:1", "2:2"]);
    /// ```
    #[must_use]
    pub fn with_ingress_priority_map(mut self, map: Vec<impl Into<String>>) -> Self {
        self.ingress_priority_map = Some(map.into_iter().map(Into::into).collect());
        self
    }

    /// Sets egress priority mapping (Linux skb priority to 802.1p priority).
    ///
    /// Each entry maps an outgoing Linux priority to an 802.1p priority (0-7).
    ///
    /// # Examples
    ///
    /// ```rust
    /// use nmrs::VlanConfig;
    ///
    /// let config = VlanConfig::new("eth0", 100)
    ///     .with_egress_priority_map(vec!["0:0", "1:1", "2:2"]);
    /// ```
    #[must_use]
    pub fn with_egress_priority_map(mut self, map: Vec<impl Into<String>>) -> Self {
        self.egress_priority_map = Some(map.into_iter().map(Into::into).collect());
        self
    }

    /// Returns the effective interface name.
    ///
    /// Returns the custom interface name if set, otherwise `{parent}.{id}`.
    #[must_use]
    pub fn effective_interface_name(&self) -> String {
        self.interface_name
            .clone()
            .unwrap_or_else(|| format!("{}.{}", self.parent, self.id))
    }

    /// Returns the effective connection name.
    ///
    /// Returns the custom connection name if set, otherwise "VLAN {id} on {parent}".
    #[must_use]
    pub fn effective_connection_name(&self) -> String {
        self.connection_name
            .clone()
            .unwrap_or_else(|| format!("VLAN {} on {}", self.id, self.parent))
    }

    /// Validates the VLAN configuration.
    ///
    /// # Errors
    ///
    /// Returns `ConnectionError::InvalidVlanId` if the VLAN ID is out of range (1-4094).
    /// Returns `ConnectionError::InvalidInput` if the parent interface name is empty.
    pub fn validate(&self) -> Result<(), ConnectionError> {
        if self.id == 0 || self.id > 4094 {
            return Err(ConnectionError::InvalidVlanId { id: self.id });
        }
        if self.parent.is_empty() {
            return Err(ConnectionError::InvalidInput {
                field: "parent".to_string(),
                reason: "parent interface name cannot be empty".to_string(),
            });
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_creates_basic_config() {
        let config = VlanConfig::new("eth0", 100);
        assert_eq!(config.parent, "eth0");
        assert_eq!(config.id, 100);
        assert!(config.interface_name.is_none());
        assert!(config.connection_name.is_none());
    }

    #[test]
    fn effective_interface_name_default() {
        let config = VlanConfig::new("eth0", 100);
        assert_eq!(config.effective_interface_name(), "eth0.100");
    }

    #[test]
    fn effective_interface_name_custom() {
        let config = VlanConfig::new("eth0", 100).with_interface_name("office-vlan");
        assert_eq!(config.effective_interface_name(), "office-vlan");
    }

    #[test]
    fn effective_connection_name_default() {
        let config = VlanConfig::new("eth0", 100);
        assert_eq!(config.effective_connection_name(), "VLAN 100 on eth0");
    }

    #[test]
    fn effective_connection_name_custom() {
        let config = VlanConfig::new("eth0", 100).with_connection_name("Office Network");
        assert_eq!(config.effective_connection_name(), "Office Network");
    }

    #[test]
    fn builder_methods_chain() {
        let config = VlanConfig::new("enp3s0", 200)
            .with_interface_name("vlan200")
            .with_connection_name("Server VLAN")
            .with_mtu(1496)
            .with_flags(0x5)
            .with_ingress_priority_map(vec!["0:0", "1:1"])
            .with_egress_priority_map(vec!["0:0", "1:1"]);

        assert_eq!(config.parent, "enp3s0");
        assert_eq!(config.id, 200);
        assert_eq!(config.interface_name, Some("vlan200".to_string()));
        assert_eq!(config.connection_name, Some("Server VLAN".to_string()));
        assert_eq!(config.mtu, Some(1496));
        assert_eq!(config.flags, Some(0x5));
        assert_eq!(
            config.ingress_priority_map,
            Some(vec!["0:0".to_string(), "1:1".to_string()])
        );
    }

    #[test]
    fn validate_rejects_zero_id() {
        let config = VlanConfig::new("eth0", 0);
        assert!(matches!(
            config.validate().unwrap_err(),
            ConnectionError::InvalidVlanId { id: 0 }
        ));
    }

    #[test]
    fn validate_rejects_id_over_4094() {
        let config = VlanConfig::new("eth0", 4095);
        assert!(matches!(
            config.validate().unwrap_err(),
            ConnectionError::InvalidVlanId { id: 4095 }
        ));
    }

    #[test]
    fn validate_rejects_empty_parent() {
        let config = VlanConfig::new("", 100);
        assert!(matches!(
            config.validate().unwrap_err(),
            ConnectionError::InvalidInput { field, reason }
                if field == "parent" && reason == "parent interface name cannot be empty"
        ));
    }

    #[test]
    fn validate_accepts_valid_config() {
        let config = VlanConfig::new("eth0", 100);
        config.validate().unwrap();

        let config = VlanConfig::new("eth0", 1);
        config.validate().unwrap();

        let config = VlanConfig::new("eth0", 4094);
        config.validate().unwrap();
    }
}
