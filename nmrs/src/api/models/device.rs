use std::fmt::{Display, Formatter};

use zvariant::OwnedObjectPath;

/// Represents a network device managed by NetworkManager.
///
/// A device can be a WiFi adapter, Ethernet interface, or other network hardware.
///
/// # Examples
///
/// ```no_run
/// use nmrs::NetworkManager;
///
/// # async fn example() -> nmrs::Result<()> {
/// let nm = NetworkManager::new().await?;
/// let devices = nm.list_devices().await?;
///
/// for device in devices {
///     println!("Interface: {}", device.interface);
///     println!("  Type: {}", device.device_type);
///     println!("  State: {}", device.state);
///     println!("  MAC: {}", device.identity.current_mac);
///
///     if device.is_wireless() {
///         println!("  This is a WiFi device");
///     } else if device.is_wired() {
///         println!("  This is an Ethernet device");
///     } else if device.is_bluetooth() {
///         println!("  This is a Bluetooth device");
///     } else if device.is_loopback() {
///         println!("  This is a loopback device");
///     }
///
///     if let Some(driver) = &device.driver {
///         println!("  Driver: {}", driver);
///     }
/// }
/// # Ok(())
/// # }
/// ```
#[non_exhaustive]
#[derive(Debug, Clone)]
pub struct Device {
    /// D-Bus object path
    pub path: String,
    /// Interface name (e.g., "wlan0", "eth0")
    pub interface: String,
    /// Device hardware identity (MAC addresses)
    pub identity: DeviceIdentity,
    /// Type of device (WiFi, Ethernet, etc.)
    pub device_type: DeviceType,
    /// Current device state
    pub state: DeviceState,
    /// Whether NetworkManager manages this device
    pub managed: Option<bool>,
    /// Kernel driver name
    pub driver: Option<String>,
    /// Assigned IPv4 address with CIDR notation (only present when connected)
    pub ip4_address: Option<String>,
    /// Assigned IPv6 address with CIDR notation (only present when connected)
    pub ip6_address: Option<String>,
    /// Operating frequency in MHz for the active Wi-Fi connection, if known.
    pub frequency: Option<u32>,
    /// Link speed in megabits per second for Ethernet devices, if known.
    ///
    /// This is the raw value reported by NetworkManager. Some drivers report
    /// `0` when no carrier is present.
    pub speed_mbps: Option<u32>,
}

/// A Wi-Fi device summary returned by
/// [`list_wifi_devices`](crate::NetworkManager::list_wifi_devices).
///
/// Use this on multi-radio machines (laptops with USB dongles, docks with a
/// second wireless adapter, etc.) to discover the available interfaces and
/// pick one to scope subsequent operations to. Pair with
/// [`NetworkManager::wifi`](crate::NetworkManager::wifi) for ergonomic
/// per-interface calls.
#[non_exhaustive]
#[derive(Debug, Clone)]
pub struct WifiDevice {
    /// D-Bus object path of the device.
    pub path: OwnedObjectPath,
    /// Interface name (e.g. `"wlan0"`).
    pub interface: String,
    /// Current MAC address (may be randomized).
    pub hw_address: String,
    /// Permanent (factory-burned) MAC, if NM exposes it.
    pub permanent_hw_address: Option<String>,
    /// Kernel driver name, if available.
    pub driver: Option<String>,
    /// Current device state.
    pub state: DeviceState,
    /// Whether NetworkManager manages this device.
    pub managed: bool,
    /// Whether NM will autoconnect known networks on this device.
    pub autoconnect: bool,
    /// `true` if the device currently has an active access point.
    pub is_active: bool,
    /// SSID of the currently active AP, if any.
    pub active_ssid: Option<String>,
    /// Operating frequency in MHz of the currently active AP, if any.
    pub active_frequency_mhz: Option<u32>,
}

/// A wired Ethernet device summary returned by
/// [`list_wired_device_details`](crate::NetworkManager::list_wired_device_details).
///
/// Use this when Ethernet-specific details such as link speed, hardware
/// address, or active connection id are needed without falling back to raw
/// D-Bus calls.
#[non_exhaustive]
#[derive(Debug, Clone)]
pub struct WiredDevice {
    /// D-Bus object path of the device.
    pub path: String,
    /// Interface name (e.g. `"eth0"`).
    pub interface: String,
    /// Current MAC address.
    pub hw_address: String,
    /// Permanent (factory-burned) MAC, if NM exposes it.
    pub permanent_hw_address: Option<String>,
    /// Link speed in megabits per second, if NM exposes it.
    ///
    /// This is the raw NetworkManager value. Some drivers report `0` when no
    /// carrier is present.
    pub speed_mbps: Option<u32>,
    /// Active connection profile id, if this device is connected.
    pub active_connection_id: Option<String>,
    /// Current device state.
    pub state: DeviceState,
    /// Assigned IPv4 address with CIDR notation, if connected.
    pub ip4_address: Option<String>,
    /// Assigned IPv6 address with CIDR notation, if connected.
    pub ip6_address: Option<String>,
}

/// Represents the hardware identity of a network device.
///
/// Contains MAC addresses that uniquely identify the device. The permanent
/// MAC is burned into the hardware, while the current MAC may be different
/// if MAC address randomization or spoofing is enabled.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct DeviceIdentity {
    /// The permanent (factory-assigned) MAC address.
    pub permanent_mac: String,
    /// The current MAC address in use (may differ if randomized/spoofed).
    pub current_mac: String,
}

impl DeviceIdentity {
    /// Creates a new `DeviceIdentity`.
    ///
    /// # Arguments
    ///
    /// * `permanent_mac` - The permanent (factory-assigned) MAC address
    /// * `current_mac` - The current MAC address in use
    #[must_use]
    pub fn new(permanent_mac: String, current_mac: String) -> Self {
        Self {
            permanent_mac,
            current_mac,
        }
    }
}

/// NetworkManager device types.
///
/// Represents the type of network hardware managed by NetworkManager.
/// This enum uses a registry-based system to support adding new device
/// types without breaking the API.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq)]
pub enum DeviceType {
    /// Wired Ethernet device.
    Ethernet,
    /// Wi-Fi (802.11) wireless device.
    Wifi,
    /// Wi-Fi P2P (peer-to-peer) device.
    WifiP2P,
    /// Loopback device (localhost).
    Loopback,
    /// Bluetooth
    Bluetooth,
    /// VLAN (802.1Q) virtual device.
    Vlan,
    /// Unknown or unsupported device type with raw code.
    ///
    /// Use the methods on `DeviceType` to query capabilities of unknown device types,
    /// which will consult the internal device type registry.
    Other(u32),
}

impl DeviceType {
    /// Returns whether this device type supports network scanning.
    ///
    /// Currently only WiFi and WiFi P2P devices support scanning.
    /// For unknown device types, consults the internal device type registry.
    #[must_use]
    pub fn supports_scanning(&self) -> bool {
        match self {
            Self::Wifi | Self::WifiP2P => true,
            Self::Other(code) => crate::types::device_type_registry::supports_scanning(*code),
            _ => false,
        }
    }

    /// Returns whether this device type requires a specific object (like an access point).
    ///
    /// WiFi devices require an access point to connect to, while Ethernet can connect
    /// without a specific target.
    /// For unknown device types, consults the internal device type registry.
    #[must_use]
    pub fn requires_specific_object(&self) -> bool {
        match self {
            Self::Wifi | Self::WifiP2P => true,
            Self::Other(code) => {
                crate::types::device_type_registry::requires_specific_object(*code)
            }
            _ => false,
        }
    }

    /// Returns whether this device type has a global enabled/disabled state.
    ///
    /// WiFi has a global radio killswitch that can enable/disable all WiFi devices.
    /// For unknown device types, consults the internal device type registry.
    #[must_use]
    pub fn has_global_enabled_state(&self) -> bool {
        match self {
            Self::Wifi => true,
            Self::Other(code) => {
                crate::types::device_type_registry::has_global_enabled_state(*code)
            }
            _ => false,
        }
    }

    /// Returns the NetworkManager connection type string for this device.
    ///
    /// This is used when creating connection profiles for this device type.
    /// For unknown device types, consults the internal device type registry.
    #[must_use]
    pub fn connection_type_str(&self) -> &'static str {
        match self {
            Self::Ethernet => "802-3-ethernet",
            Self::Wifi => "802-11-wireless",
            Self::WifiP2P => "wifi-p2p",
            Self::Loopback => "loopback",
            Self::Bluetooth => "bluetooth",
            Self::Vlan => "vlan",
            Self::Other(code) => {
                crate::types::device_type_registry::connection_type_for_code(*code)
                    .unwrap_or("generic")
            }
        }
    }

    /// Returns the raw NetworkManager type code for this device.
    #[must_use]
    pub fn to_code(&self) -> u32 {
        match self {
            Self::Ethernet => 1,
            Self::Wifi => 2,
            Self::WifiP2P => 30,
            Self::Loopback => 32,
            Self::Bluetooth => 5,
            Self::Vlan => 11,
            Self::Other(code) => *code,
        }
    }
}

/// NetworkManager device states.
///
/// Represents the current operational state of a network device.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq)]
pub enum DeviceState {
    /// Device is not managed by NetworkManager.
    Unmanaged,
    /// Device is managed but not yet available (e.g., Wi-Fi disabled).
    Unavailable,
    /// Device is available but not connected.
    Disconnected,
    /// Device is preparing to connect.
    Prepare,
    /// Device is being configured.
    Config,
    /// Device requires authentication credentials.
    NeedAuth,
    /// Device is requesting IP configuration.
    IpConfig,
    /// Device is verifying IP connectivity.
    IpCheck,
    /// Device is waiting for secondary connections.
    Secondaries,
    /// Device is fully connected and operational.
    Activated,
    /// Device is disconnecting.
    Deactivating,
    /// Device connection failed.
    Failed,
    /// Unknown or unsupported state with raw code.
    Other(u32),
}

impl DeviceState {
    /// Returns `true` if the device is in a transitional (in-progress) state.
    ///
    /// Transitional states indicate an active connection or disconnection
    /// operation: Prepare, Config, NeedAuth, IpConfig, IpCheck, Secondaries,
    /// or Deactivating.
    #[must_use]
    pub fn is_transitional(&self) -> bool {
        matches!(
            self,
            Self::Prepare
                | Self::Config
                | Self::NeedAuth
                | Self::IpConfig
                | Self::IpCheck
                | Self::Secondaries
                | Self::Deactivating
        )
    }

    /// Returns `true` if the device state indicates the device is usable.
    ///
    /// This is derived only from the NetworkManager device state. For actual
    /// Wi-Fi radio power and rfkill state, use
    /// [`NetworkManager::wifi_state`](crate::NetworkManager::wifi_state).
    #[must_use]
    pub fn is_enabled(&self) -> bool {
        matches!(
            self,
            Self::Disconnected
                | Self::Prepare
                | Self::Config
                | Self::NeedAuth
                | Self::IpConfig
                | Self::IpCheck
                | Self::Secondaries
                | Self::Activated
                | Self::Deactivating
        )
    }
}

impl Device {
    /// Returns `true` if this is a wired (Ethernet) device.
    #[must_use]
    pub fn is_wired(&self) -> bool {
        crate::types::device_type_registry::is_wired(self.device_type.to_code())
    }

    /// Returns `true` if this is a wireless (Wi-Fi) device.
    #[must_use]
    pub fn is_wireless(&self) -> bool {
        matches!(self.device_type, DeviceType::Wifi)
    }

    /// Returns 'true' if this is a Bluetooth (DUN or PANU) device.
    #[must_use]
    pub fn is_bluetooth(&self) -> bool {
        matches!(self.device_type, DeviceType::Bluetooth)
    }

    /// Returns `true` if this is a loopback device (e.g., `lo`).
    #[must_use]
    pub fn is_loopback(&self) -> bool {
        matches!(self.device_type, DeviceType::Loopback)
    }

    /// Returns `true` if this is a VLAN (802.1Q) device.
    #[must_use]
    pub fn is_vlan(&self) -> bool {
        matches!(self.device_type, DeviceType::Vlan)
    }
}

impl Display for Device {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{} ({}) [{}]",
            self.interface, self.device_type, self.state
        )
    }
}

impl From<u32> for DeviceType {
    fn from(value: u32) -> Self {
        match value {
            1 => DeviceType::Ethernet,
            2 => DeviceType::Wifi,
            5 => DeviceType::Bluetooth,
            11 => DeviceType::Vlan,
            30 => DeviceType::WifiP2P,
            32 => DeviceType::Loopback,
            v => DeviceType::Other(v),
        }
    }
}

impl From<u32> for DeviceState {
    fn from(value: u32) -> Self {
        match value {
            10 => Self::Unmanaged,
            20 => Self::Unavailable,
            30 => Self::Disconnected,
            40 => Self::Prepare,
            50 => Self::Config,
            60 => Self::NeedAuth,
            70 => Self::IpConfig,
            80 => Self::IpCheck,
            90 => Self::Secondaries,
            100 => Self::Activated,
            110 => Self::Deactivating,
            120 => Self::Failed,
            v => Self::Other(v),
        }
    }
}

impl Display for DeviceType {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            DeviceType::Ethernet => write!(f, "Ethernet"),
            DeviceType::Wifi => write!(f, "Wi-Fi"),
            DeviceType::WifiP2P => write!(f, "Wi-Fi P2P"),
            DeviceType::Loopback => write!(f, "Loopback"),
            DeviceType::Bluetooth => write!(f, "Bluetooth"),
            DeviceType::Vlan => write!(f, "VLAN"),
            DeviceType::Other(v) => write!(
                f,
                "{}",
                crate::types::device_type_registry::display_name_for_code(*v)
            ),
        }
    }
}

impl Display for DeviceState {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Unmanaged => write!(f, "Unmanaged"),
            Self::Unavailable => write!(f, "Unavailable"),
            Self::Disconnected => write!(f, "Disconnected"),
            Self::Prepare => write!(f, "Preparing"),
            Self::Config => write!(f, "Configuring"),
            Self::NeedAuth => write!(f, "NeedAuth"),
            Self::IpConfig => write!(f, "IpConfig"),
            Self::IpCheck => write!(f, "IpCheck"),
            Self::Secondaries => write!(f, "Secondaries"),
            Self::Activated => write!(f, "Activated"),
            Self::Deactivating => write!(f, "Deactivating"),
            Self::Failed => write!(f, "Failed"),
            Self::Other(v) => write!(f, "Other({v})"),
        }
    }
}
