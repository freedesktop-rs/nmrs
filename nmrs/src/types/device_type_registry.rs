//! Device type registry for extensible device type support.
//!
//! This module provides a trait-based system for registering and working with
//! different network device types. It enables adding new device types without
//! breaking the public API.

use std::collections::HashMap;
use std::sync::OnceLock;

use super::constants::device_type;

/// Trait for device type-specific behavior.
///
/// Implement this trait to add support for a new device type.
/// The trait provides metadata about the device type and type-specific
/// operations that may be needed.
pub trait DeviceTypeInfo: Send + Sync {
    /// Returns the NetworkManager D-Bus constant for this device type.
    fn nm_type_code(&self) -> u32;

    /// Returns the human-readable name of this device type.
    fn display_name(&self) -> &'static str;

    /// Returns the NetworkManager connection type string.
    ///
    /// This is used when creating connections for this device type.
    /// Examples: "802-11-wireless", "802-3-ethernet", "wireguard", "bluetooth"
    fn connection_type(&self) -> &'static str;

    /// Returns whether this device type supports scanning for networks.
    fn supports_scanning(&self) -> bool {
        false
    }

    /// Returns whether this device type requires an access point or similar target.
    fn requires_specific_object(&self) -> bool {
        false
    }

    /// Returns whether this device type can be globally enabled/disabled.
    fn has_global_enabled_state(&self) -> bool {
        false
    }
}

/// WiFi device type implementation.
struct WifiDeviceType;

impl DeviceTypeInfo for WifiDeviceType {
    fn nm_type_code(&self) -> u32 {
        2
    }

    fn display_name(&self) -> &'static str {
        "Wi-Fi"
    }

    fn connection_type(&self) -> &'static str {
        "802-11-wireless"
    }

    fn supports_scanning(&self) -> bool {
        true
    }

    fn requires_specific_object(&self) -> bool {
        true
    }

    fn has_global_enabled_state(&self) -> bool {
        true
    }
}

/// Ethernet device type implementation.
struct EthernetDeviceType;

impl DeviceTypeInfo for EthernetDeviceType {
    fn nm_type_code(&self) -> u32 {
        1
    }

    fn display_name(&self) -> &'static str {
        "Ethernet"
    }

    fn connection_type(&self) -> &'static str {
        "802-3-ethernet"
    }
}

/// Linux virtual Ethernet pair device type implementation.
struct VethDeviceType;

impl DeviceTypeInfo for VethDeviceType {
    fn nm_type_code(&self) -> u32 {
        device_type::VETH
    }

    fn display_name(&self) -> &'static str {
        "Veth"
    }

    fn connection_type(&self) -> &'static str {
        "802-3-ethernet"
    }
}

/// WiFi P2P device type implementation.
struct WifiP2PDeviceType;

impl DeviceTypeInfo for WifiP2PDeviceType {
    fn nm_type_code(&self) -> u32 {
        30
    }

    fn display_name(&self) -> &'static str {
        "Wi-Fi P2P"
    }

    fn connection_type(&self) -> &'static str {
        "wifi-p2p"
    }

    fn supports_scanning(&self) -> bool {
        true
    }
}

/// Loopback device type implementation.
struct LoopbackDeviceType;

impl DeviceTypeInfo for LoopbackDeviceType {
    fn nm_type_code(&self) -> u32 {
        32
    }

    fn display_name(&self) -> &'static str {
        "Loopback"
    }

    fn connection_type(&self) -> &'static str {
        "loopback"
    }
}

/// Bridge device type implementation.
struct BridgeDeviceType;

impl DeviceTypeInfo for BridgeDeviceType {
    fn nm_type_code(&self) -> u32 {
        13
    }

    fn display_name(&self) -> &'static str {
        "Bridge"
    }

    fn connection_type(&self) -> &'static str {
        "bridge"
    }
}

/// Bond device type implementation.
struct BondDeviceType;

impl DeviceTypeInfo for BondDeviceType {
    fn nm_type_code(&self) -> u32 {
        12
    }

    fn display_name(&self) -> &'static str {
        "Bond"
    }

    fn connection_type(&self) -> &'static str {
        "bond"
    }
}

/// VLAN device type implementation.
struct VlanDeviceType;

impl DeviceTypeInfo for VlanDeviceType {
    fn nm_type_code(&self) -> u32 {
        11
    }

    fn display_name(&self) -> &'static str {
        "VLAN"
    }

    fn connection_type(&self) -> &'static str {
        "vlan"
    }
}

/// TUN/TAP device type implementation.
struct TunDeviceType;

impl DeviceTypeInfo for TunDeviceType {
    fn nm_type_code(&self) -> u32 {
        16
    }

    fn display_name(&self) -> &'static str {
        "TUN"
    }

    fn connection_type(&self) -> &'static str {
        "tun"
    }
}

/// WireGuard device type implementation.
struct WireGuardDeviceType;

impl DeviceTypeInfo for WireGuardDeviceType {
    fn nm_type_code(&self) -> u32 {
        29
    }

    fn display_name(&self) -> &'static str {
        "WireGuard"
    }

    fn connection_type(&self) -> &'static str {
        "wireguard"
    }
}

/// Global registry of device types.
///
/// This registry maps NetworkManager type codes to device type information.
/// It's populated once at first access and remains immutable thereafter.
static DEVICE_TYPE_REGISTRY: OnceLock<HashMap<u32, Box<dyn DeviceTypeInfo>>> = OnceLock::new();

/// Initializes and returns the device type registry.
fn registry() -> &'static HashMap<u32, Box<dyn DeviceTypeInfo>> {
    DEVICE_TYPE_REGISTRY.get_or_init(|| {
        let mut map: HashMap<u32, Box<dyn DeviceTypeInfo>> = HashMap::new();

        let types: Vec<Box<dyn DeviceTypeInfo>> = vec![
            Box::new(EthernetDeviceType),
            Box::new(VethDeviceType),
            Box::new(WifiDeviceType),
            Box::new(WifiP2PDeviceType),
            Box::new(LoopbackDeviceType),
            Box::new(BridgeDeviceType),
            Box::new(BondDeviceType),
            Box::new(VlanDeviceType),
            Box::new(TunDeviceType),
            Box::new(WireGuardDeviceType),
        ];

        for type_info in types {
            map.insert(type_info.nm_type_code(), type_info);
        }

        map
    })
}

/// Looks up device type information by NetworkManager type code.
///
/// Returns `None` if the device type is not recognized.
pub fn get_device_type_info(code: u32) -> Option<&'static dyn DeviceTypeInfo> {
    registry().get(&code).map(|b| &**b)
}

/// Returns the display name for a device type code.
///
/// If the code is not recognized, returns a generic "Other(N)" string.
pub fn display_name_for_code(code: u32) -> String {
    get_device_type_info(code)
        .map(|info| info.display_name().to_string())
        .unwrap_or_else(|| format!("Other({})", code))
}

/// Returns the connection type string for a device type code.
///
/// Returns `None` if the device type is not recognized or doesn't
/// have an associated connection type.
pub fn connection_type_for_code(code: u32) -> Option<&'static str> {
    get_device_type_info(code).map(|info| info.connection_type())
}

/// Returns whether a device type supports scanning.
pub fn supports_scanning(code: u32) -> bool {
    get_device_type_info(code)
        .map(|info| info.supports_scanning())
        .unwrap_or(false)
}

/// Returns whether a device type requires a specific object (like an AP).
pub fn requires_specific_object(code: u32) -> bool {
    get_device_type_info(code)
        .map(|info| info.requires_specific_object())
        .unwrap_or(false)
}

/// Returns whether a device type has a global enabled state.
pub fn has_global_enabled_state(code: u32) -> bool {
    get_device_type_info(code)
        .map(|info| info.has_global_enabled_state())
        .unwrap_or(false)
}

/// Returns whether a raw NetworkManager device type uses wired Ethernet settings.
pub fn is_wired(code: u32) -> bool {
    connection_type_for_code(code) == Some("802-3-ethernet")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn registry_matches_networkmanager_metadata() {
        let expected = [
            (1, "Ethernet", "802-3-ethernet", false, false, false, true),
            (2, "Wi-Fi", "802-11-wireless", true, true, true, false),
            (11, "VLAN", "vlan", false, false, false, false),
            (12, "Bond", "bond", false, false, false, false),
            (13, "Bridge", "bridge", false, false, false, false),
            (16, "TUN", "tun", false, false, false, false),
            (20, "Veth", "802-3-ethernet", false, false, false, true),
            (29, "WireGuard", "wireguard", false, false, false, false),
            (30, "Wi-Fi P2P", "wifi-p2p", true, false, false, false),
            (32, "Loopback", "loopback", false, false, false, false),
        ];

        assert_eq!(registry().len(), expected.len());
        for (code, name, connection_type, scanning, specific_object, global_state, wired) in
            expected
        {
            let info = get_device_type_info(code)
                .unwrap_or_else(|| panic!("device type {code} should be registered"));
            assert_eq!(info.nm_type_code(), code);
            assert_eq!(info.display_name(), name);
            assert_eq!(display_name_for_code(code), name);
            assert_eq!(info.connection_type(), connection_type);
            assert_eq!(connection_type_for_code(code), Some(connection_type));
            assert_eq!(supports_scanning(code), scanning);
            assert_eq!(requires_specific_object(code), specific_object);
            assert_eq!(has_global_enabled_state(code), global_state);
            assert_eq!(is_wired(code), wired);
        }
    }

    #[test]
    fn unknown_code_has_safe_fallbacks() {
        assert!(get_device_type_info(999).is_none());
        assert_eq!(display_name_for_code(999), "Other(999)");
        assert_eq!(connection_type_for_code(999), None);
        assert!(!supports_scanning(999));
        assert!(!requires_specific_object(999));
        assert!(!has_global_enabled_state(999));
        assert!(!is_wired(999));
    }
}
