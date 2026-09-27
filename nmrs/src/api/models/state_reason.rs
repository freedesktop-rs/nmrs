use std::fmt::{Display, Formatter};

use super::error::ConnectionError;

/// NetworkManager device state reason codes.
///
/// These values come from the NM D-Bus API and indicate why a device
/// transitioned to its current state. Use `StateReason::from(code)` to
/// convert from the raw u32 values returned by NetworkManager.
///
/// The numeric codes follow `NMDeviceStateReason` in NetworkManager's
/// `nm-dbus-interface.h`. Codes NetworkManager adds in the future decode to
/// [`StateReason::Other`].
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StateReason {
    /// No reason given (`NONE`, 0).
    None,
    /// Unknown error (`UNKNOWN`, 1).
    Unknown,
    /// The device is now managed (`NOW_MANAGED`, 2).
    NowManaged,
    /// The device is now unmanaged (`NOW_UNMANAGED`, 3).
    NowUnmanaged,
    /// The device could not be readied for configuration (`CONFIG_FAILED`, 4).
    ConfigFailed,
    /// IP configuration could not be reserved: no available address, timeout,
    /// etc. (`IP_CONFIG_UNAVAILABLE`, 5).
    IpConfigUnavailable,
    /// The IP configuration is no longer valid (`IP_CONFIG_EXPIRED`, 6).
    IpConfigExpired,
    /// Secrets were required but not provided (`NO_SECRETS`, 7).
    ///
    /// This is what NetworkManager reports for a rejected Wi-Fi passphrase
    /// once it has given up asking secret agents for a replacement.
    NoSecrets,
    /// The 802.1x supplicant disconnected (`SUPPLICANT_DISCONNECT`, 8).
    SupplicantDisconnected,
    /// The 802.1x supplicant's configuration failed
    /// (`SUPPLICANT_CONFIG_FAILED`, 9).
    SupplicantConfigFailed,
    /// The 802.1x supplicant failed (`SUPPLICANT_FAILED`, 10).
    SupplicantFailed,
    /// The 802.1x supplicant took too long to authenticate
    /// (`SUPPLICANT_TIMEOUT`, 11).
    SupplicantTimeout,
    /// The PPP service failed to start (`PPP_START_FAILED`, 12).
    PppStartFailed,
    /// The PPP service disconnected (`PPP_DISCONNECT`, 13).
    PppDisconnected,
    /// PPP failed (`PPP_FAILED`, 14).
    PppFailed,
    /// The DHCP client failed to start (`DHCP_START_FAILED`, 15).
    DhcpStartFailed,
    /// The DHCP client reported an error (`DHCP_ERROR`, 16).
    DhcpError,
    /// The DHCP client failed (`DHCP_FAILED`, 17).
    DhcpFailed,
    /// The shared connection service failed to start
    /// (`SHARED_START_FAILED`, 18).
    SharedStartFailed,
    /// The shared connection service failed (`SHARED_FAILED`, 19).
    SharedFailed,
    /// The AutoIP service failed to start (`AUTOIP_START_FAILED`, 20).
    AutoIpStartFailed,
    /// The AutoIP service reported an error (`AUTOIP_ERROR`, 21).
    AutoIpError,
    /// The AutoIP service failed (`AUTOIP_FAILED`, 22).
    AutoIpFailed,
    /// The modem line is busy (`MODEM_BUSY`, 23).
    ModemBusy,
    /// No dial tone (`MODEM_NO_DIAL_TONE`, 24).
    ModemNoDialTone,
    /// No carrier could be established (`MODEM_NO_CARRIER`, 25).
    ModemNoCarrier,
    /// The dialing request timed out (`MODEM_DIAL_TIMEOUT`, 26).
    ModemDialTimeout,
    /// The dialing attempt failed (`MODEM_DIAL_FAILED`, 27).
    ModemDialFailed,
    /// Modem initialization failed (`MODEM_INIT_FAILED`, 28).
    ModemInitFailed,
    /// Failed to select the specified APN (`GSM_APN_FAILED`, 29).
    GsmApnSelectFailed,
    /// Not searching for networks (`GSM_REGISTRATION_NOT_SEARCHING`, 30).
    GsmNotSearching,
    /// Network registration was denied (`GSM_REGISTRATION_DENIED`, 31).
    GsmRegistrationDenied,
    /// Network registration timed out (`GSM_REGISTRATION_TIMEOUT`, 32).
    GsmRegistrationTimeout,
    /// Failed to register with the requested network
    /// (`GSM_REGISTRATION_FAILED`, 33).
    GsmRegistrationFailed,
    /// PIN check failed (`GSM_PIN_CHECK_FAILED`, 34).
    GsmPinCheckFailed,
    /// Necessary firmware for the device may be missing
    /// (`FIRMWARE_MISSING`, 35).
    FirmwareMissing,
    /// The device was removed (`REMOVED`, 36).
    DeviceRemoved,
    /// NetworkManager went to sleep (`SLEEPING`, 37).
    Sleeping,
    /// The device's active connection disappeared (`CONNECTION_REMOVED`, 38).
    ConnectionRemoved,
    /// The device was disconnected by the user or a client
    /// (`USER_REQUESTED`, 39).
    UserRequested,
    /// The carrier/link changed (`CARRIER`, 40).
    CarrierChanged,
    /// The device's existing connection was assumed (`CONNECTION_ASSUMED`, 41).
    ConnectionAssumed,
    /// The supplicant is now available (`SUPPLICANT_AVAILABLE`, 42).
    SupplicantAvailable,
    /// The modem could not be found (`MODEM_NOT_FOUND`, 43).
    ModemNotFound,
    /// The Bluetooth connection failed or timed out (`BT_FAILED`, 44).
    BluetoothFailed,
    /// The GSM modem's SIM card is not inserted (`GSM_SIM_NOT_INSERTED`, 45).
    GsmSimNotInserted,
    /// The GSM modem's SIM PIN is required (`GSM_SIM_PIN_REQUIRED`, 46).
    GsmSimPinRequired,
    /// The GSM modem's SIM PUK is required (`GSM_SIM_PUK_REQUIRED`, 47).
    GsmSimPukRequired,
    /// The GSM modem's SIM is wrong (`GSM_SIM_WRONG`, 48).
    GsmSimWrong,
    /// The InfiniBand device does not support connected mode
    /// (`INFINIBAND_MODE`, 49).
    InfinibandMode,
    /// A dependency of the connection failed (`DEPENDENCY_FAILED`, 50).
    DependencyFailed,
    /// Problem with the RFC 2684 Ethernet over ADSL bridge
    /// (`BR2684_FAILED`, 51).
    Br2684Failed,
    /// ModemManager is not running (`MODEM_MANAGER_UNAVAILABLE`, 52).
    ModemManagerUnavailable,
    /// The Wi-Fi network could not be found (`SSID_NOT_FOUND`, 53).
    SsidNotFound,
    /// A secondary connection of the base connection failed
    /// (`SECONDARY_CONNECTION_FAILED`, 54).
    SecondaryConnectionFailed,
    /// DCB or FCoE setup failed (`DCB_FCOE_FAILED`, 55).
    DcbFcoeFailed,
    /// teamd control failed (`TEAMD_CONTROL_FAILED`, 56).
    TeamdControlFailed,
    /// The modem failed or is no longer available (`MODEM_FAILED`, 57).
    ModemFailed,
    /// The modem is now ready and available (`MODEM_AVAILABLE`, 58).
    ModemAvailable,
    /// The SIM PIN was incorrect (`SIM_PIN_INCORRECT`, 59).
    SimPinIncorrect,
    /// A new connection activation was enqueued (`NEW_ACTIVATION`, 60).
    NewActivationEnqueued,
    /// The device's parent changed (`PARENT_CHANGED`, 61).
    ParentChanged,
    /// The device parent's management changed
    /// (`PARENT_MANAGED_CHANGED`, 62).
    ParentManagedChanged,
    /// Problem communicating with the Open vSwitch database
    /// (`OVSDB_FAILED`, 63).
    OvsdbFailed,
    /// A duplicate IP address was detected (`IP_ADDRESS_DUPLICATE`, 64).
    IpAddressDuplicate,
    /// The selected IP method is not supported
    /// (`IP_METHOD_UNSUPPORTED`, 65).
    IpMethodUnsupported,
    /// Configuration of SR-IOV parameters failed
    /// (`SRIOV_CONFIGURATION_FAILED`, 66).
    SriovConfigurationFailed,
    /// The Wi-Fi P2P peer could not be found (`PEER_NOT_FOUND`, 67).
    PeerNotFound,
    /// The device handler dispatcher returned an error
    /// (`DEVICE_HANDLER_FAILED`, 68).
    DeviceHandlerFailed,
    /// The device is unmanaged because its type is unmanaged by default
    /// (`UNMANAGED_BY_DEFAULT`, 69).
    UnmanagedByDefault,
    /// The device is unmanaged because it is an external device that is
    /// down or has no addresses (`UNMANAGED_EXTERNAL_DOWN`, 70).
    UnmanagedExternalDown,
    /// The device is unmanaged because the link is not initialized by udev
    /// (`UNMANAGED_LINK_NOT_INIT`, 71).
    UnmanagedLinkNotInit,
    /// The device is unmanaged because NetworkManager is quitting
    /// (`UNMANAGED_QUITTING`, 72).
    UnmanagedQuitting,
    /// The device is unmanaged because networking is disabled or the system
    /// is suspended (`UNMANAGED_MANAGER_DISABLED`, 73; formerly
    /// `UNMANAGED_SLEEPING`).
    UnmanagedManagerDisabled,
    /// The device is unmanaged by user decision in `NetworkManager.conf`
    /// (`UNMANAGED_USER_CONF`, 74).
    UnmanagedUserConf,
    /// The device is unmanaged by explicit user decision, e.g.
    /// `nmcli device set $DEV managed no` (`UNMANAGED_USER_EXPLICIT`, 75).
    UnmanagedUserExplicit,
    /// The device is unmanaged by user decision via a settings plugin
    /// (`UNMANAGED_USER_SETTINGS`, 76).
    UnmanagedUserSettings,
    /// The device is unmanaged via a udev rule (`UNMANAGED_USER_UDEV`, 77).
    UnmanagedUserUdev,
    /// NetworkManager was disabled, i.e. networking is off
    /// (`NETWORKING_OFF`, 78).
    NetworkingOff,
    /// The modem's operator code was not available and auto-configuration
    /// was requested (`MODEM_NO_OPERATOR_CODE`, 79).
    ModemNoOperatorCode,

    /// Never produced. Earlier releases decoded code 2 (`NOW_MANAGED`) to this
    /// variant by mistake; [`StateReason::UserRequested`] is the reason
    /// NetworkManager reports for a user-initiated disconnect.
    #[deprecated(since = "3.6.0", note = "never produced; see `UserRequested`")]
    UserDisconnected,
    /// Never produced. Earlier releases decoded code 3 (`NOW_UNMANAGED`) to
    /// this variant by mistake; NetworkManager has no such reason.
    #[deprecated(since = "3.6.0", note = "never produced")]
    DeviceDisconnected,
    /// Never produced. Earlier releases decoded code 45
    /// (`GSM_SIM_NOT_INSERTED`) to this variant by mistake; NetworkManager has
    /// no such reason.
    #[deprecated(since = "3.6.0", note = "never produced")]
    ModeSetFailed,
    /// Never produced. Earlier releases decoded code 24 (`MODEM_NO_DIAL_TONE`)
    /// to this variant by mistake; see the `Modem*` variants for the reasons
    /// NetworkManager actually reports.
    #[deprecated(since = "3.6.0", note = "never produced; see `ModemDialFailed`")]
    ModemConnectionFailed,
    /// Never produced. Earlier releases decoded code 57 (`MODEM_FAILED`) to
    /// this variant by mistake; [`StateReason::CarrierChanged`] is the
    /// carrier/link change reason.
    #[deprecated(since = "3.6.0", note = "never produced; see `CarrierChanged`")]
    Carrier,
    /// Never produced. Earlier releases decoded code 78 (`NETWORKING_OFF`) to
    /// this variant by mistake; NetworkManager has no such reason.
    #[deprecated(since = "3.6.0", note = "never produced")]
    ParentUnreachable,

    /// Reason code not mapped to a specific variant.
    Other(u32),
}

impl From<u32> for StateReason {
    fn from(code: u32) -> Self {
        match code {
            0 => Self::None,
            1 => Self::Unknown,
            2 => Self::NowManaged,
            3 => Self::NowUnmanaged,
            4 => Self::ConfigFailed,
            5 => Self::IpConfigUnavailable,
            6 => Self::IpConfigExpired,
            7 => Self::NoSecrets,
            8 => Self::SupplicantDisconnected,
            9 => Self::SupplicantConfigFailed,
            10 => Self::SupplicantFailed,
            11 => Self::SupplicantTimeout,
            12 => Self::PppStartFailed,
            13 => Self::PppDisconnected,
            14 => Self::PppFailed,
            15 => Self::DhcpStartFailed,
            16 => Self::DhcpError,
            17 => Self::DhcpFailed,
            18 => Self::SharedStartFailed,
            19 => Self::SharedFailed,
            20 => Self::AutoIpStartFailed,
            21 => Self::AutoIpError,
            22 => Self::AutoIpFailed,
            23 => Self::ModemBusy,
            24 => Self::ModemNoDialTone,
            25 => Self::ModemNoCarrier,
            26 => Self::ModemDialTimeout,
            27 => Self::ModemDialFailed,
            28 => Self::ModemInitFailed,
            29 => Self::GsmApnSelectFailed,
            30 => Self::GsmNotSearching,
            31 => Self::GsmRegistrationDenied,
            32 => Self::GsmRegistrationTimeout,
            33 => Self::GsmRegistrationFailed,
            34 => Self::GsmPinCheckFailed,
            35 => Self::FirmwareMissing,
            36 => Self::DeviceRemoved,
            37 => Self::Sleeping,
            38 => Self::ConnectionRemoved,
            39 => Self::UserRequested,
            40 => Self::CarrierChanged,
            41 => Self::ConnectionAssumed,
            42 => Self::SupplicantAvailable,
            43 => Self::ModemNotFound,
            44 => Self::BluetoothFailed,
            45 => Self::GsmSimNotInserted,
            46 => Self::GsmSimPinRequired,
            47 => Self::GsmSimPukRequired,
            48 => Self::GsmSimWrong,
            49 => Self::InfinibandMode,
            50 => Self::DependencyFailed,
            51 => Self::Br2684Failed,
            52 => Self::ModemManagerUnavailable,
            53 => Self::SsidNotFound,
            54 => Self::SecondaryConnectionFailed,
            55 => Self::DcbFcoeFailed,
            56 => Self::TeamdControlFailed,
            57 => Self::ModemFailed,
            58 => Self::ModemAvailable,
            59 => Self::SimPinIncorrect,
            60 => Self::NewActivationEnqueued,
            61 => Self::ParentChanged,
            62 => Self::ParentManagedChanged,
            63 => Self::OvsdbFailed,
            64 => Self::IpAddressDuplicate,
            65 => Self::IpMethodUnsupported,
            66 => Self::SriovConfigurationFailed,
            67 => Self::PeerNotFound,
            68 => Self::DeviceHandlerFailed,
            69 => Self::UnmanagedByDefault,
            70 => Self::UnmanagedExternalDown,
            71 => Self::UnmanagedLinkNotInit,
            72 => Self::UnmanagedQuitting,
            73 => Self::UnmanagedManagerDisabled,
            74 => Self::UnmanagedUserConf,
            75 => Self::UnmanagedUserExplicit,
            76 => Self::UnmanagedUserSettings,
            77 => Self::UnmanagedUserUdev,
            78 => Self::NetworkingOff,
            79 => Self::ModemNoOperatorCode,
            v => Self::Other(v),
        }
    }
}

impl Display for StateReason {
    #[allow(deprecated)]
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::None => write!(f, "none"),
            Self::Unknown => write!(f, "unknown"),
            Self::NowManaged => write!(f, "device is now managed"),
            Self::NowUnmanaged => write!(f, "device is now unmanaged"),
            Self::ConfigFailed => write!(f, "device configuration failed"),
            Self::IpConfigUnavailable => write!(f, "IP configuration unavailable"),
            Self::IpConfigExpired => write!(f, "IP configuration expired"),
            Self::NoSecrets => write!(f, "no secrets provided"),
            Self::SupplicantDisconnected => write!(f, "supplicant disconnected"),
            Self::SupplicantConfigFailed => write!(f, "supplicant config failed"),
            Self::SupplicantFailed => write!(f, "supplicant failed"),
            Self::SupplicantTimeout => write!(f, "supplicant timeout"),
            Self::PppStartFailed => write!(f, "PPP start failed"),
            Self::PppDisconnected => write!(f, "PPP disconnected"),
            Self::PppFailed => write!(f, "PPP failed"),
            Self::DhcpStartFailed => write!(f, "DHCP start failed"),
            Self::DhcpError => write!(f, "DHCP error"),
            Self::DhcpFailed => write!(f, "DHCP failed"),
            Self::SharedStartFailed => write!(f, "shared connection start failed"),
            Self::SharedFailed => write!(f, "shared connection failed"),
            Self::AutoIpStartFailed => write!(f, "AutoIP start failed"),
            Self::AutoIpError => write!(f, "AutoIP error"),
            Self::AutoIpFailed => write!(f, "AutoIP failed"),
            Self::ModemBusy => write!(f, "modem line busy"),
            Self::ModemNoDialTone => write!(f, "modem no dial tone"),
            Self::ModemNoCarrier => write!(f, "modem no carrier"),
            Self::ModemDialTimeout => write!(f, "modem dial timeout"),
            Self::ModemDialFailed => write!(f, "modem dial failed"),
            Self::ModemInitFailed => write!(f, "modem init failed"),
            Self::GsmApnSelectFailed => write!(f, "GSM APN select failed"),
            Self::GsmNotSearching => write!(f, "GSM not searching"),
            Self::GsmRegistrationDenied => write!(f, "GSM registration denied"),
            Self::GsmRegistrationTimeout => write!(f, "GSM registration timeout"),
            Self::GsmRegistrationFailed => write!(f, "GSM registration failed"),
            Self::GsmPinCheckFailed => write!(f, "GSM PIN check failed"),
            Self::FirmwareMissing => write!(f, "firmware missing"),
            Self::DeviceRemoved => write!(f, "device removed"),
            Self::Sleeping => write!(f, "sleeping"),
            Self::ConnectionRemoved => write!(f, "connection removed"),
            Self::UserRequested => write!(f, "user requested"),
            Self::CarrierChanged => write!(f, "carrier changed"),
            Self::ConnectionAssumed => write!(f, "connection assumed"),
            Self::SupplicantAvailable => write!(f, "supplicant available"),
            Self::ModemNotFound => write!(f, "modem not found"),
            Self::BluetoothFailed => write!(f, "bluetooth failed"),
            Self::GsmSimNotInserted => write!(f, "GSM SIM not inserted"),
            Self::GsmSimPinRequired => write!(f, "GSM SIM PIN required"),
            Self::GsmSimPukRequired => write!(f, "GSM SIM PUK required"),
            Self::GsmSimWrong => write!(f, "GSM SIM wrong"),
            Self::InfinibandMode => write!(f, "infiniband mode"),
            Self::DependencyFailed => write!(f, "dependency failed"),
            Self::Br2684Failed => write!(f, "BR2684 failed"),
            Self::ModemManagerUnavailable => write!(f, "ModemManager unavailable"),
            Self::SsidNotFound => write!(f, "SSID not found"),
            Self::SecondaryConnectionFailed => write!(f, "secondary connection failed"),
            Self::DcbFcoeFailed => write!(f, "DCB/FCoE setup failed"),
            Self::TeamdControlFailed => write!(f, "teamd control failed"),
            Self::ModemFailed => write!(f, "modem failed"),
            Self::ModemAvailable => write!(f, "modem available"),
            Self::SimPinIncorrect => write!(f, "SIM PIN incorrect"),
            Self::NewActivationEnqueued => write!(f, "new activation enqueued"),
            Self::ParentChanged => write!(f, "parent device changed"),
            Self::ParentManagedChanged => write!(f, "parent device management changed"),
            Self::OvsdbFailed => write!(f, "OVSDB failed"),
            Self::IpAddressDuplicate => write!(f, "duplicate IP address"),
            Self::IpMethodUnsupported => write!(f, "IP method unsupported"),
            Self::SriovConfigurationFailed => write!(f, "SR-IOV configuration failed"),
            Self::PeerNotFound => write!(f, "Wi-Fi P2P peer not found"),
            Self::DeviceHandlerFailed => write!(f, "device handler failed"),
            Self::UnmanagedByDefault => write!(f, "unmanaged by default"),
            Self::UnmanagedExternalDown => write!(f, "unmanaged external device down"),
            Self::UnmanagedLinkNotInit => write!(f, "unmanaged link not initialized"),
            Self::UnmanagedQuitting => write!(f, "unmanaged because NetworkManager is quitting"),
            Self::UnmanagedManagerDisabled => write!(f, "unmanaged because networking is disabled"),
            Self::UnmanagedUserConf => write!(f, "unmanaged by NetworkManager.conf"),
            Self::UnmanagedUserExplicit => write!(f, "unmanaged by explicit user request"),
            Self::UnmanagedUserSettings => write!(f, "unmanaged by settings plugin"),
            Self::UnmanagedUserUdev => write!(f, "unmanaged by udev rule"),
            Self::NetworkingOff => write!(f, "networking off"),
            Self::ModemNoOperatorCode => write!(f, "modem operator code unavailable"),
            Self::UserDisconnected => write!(f, "user disconnected"),
            Self::DeviceDisconnected => write!(f, "device disconnected"),
            Self::ModeSetFailed => write!(f, "mode set failed"),
            Self::ModemConnectionFailed => write!(f, "modem connection failed"),
            Self::Carrier => write!(f, "carrier"),
            Self::ParentUnreachable => write!(f, "parent device unreachable"),
            Self::Other(v) => write!(f, "unknown reason ({v})"),
        }
    }
}

/// Converts a NetworkManager device state reason code to a specific `ConnectionError`.
///
/// Maps authentication-related failures to `AuthFailed`, DHCP and other IP
/// configuration issues to `DhcpFailed`, a missing network to `NotFound`,
/// and other failures to `DeviceFailed` carrying the decoded [`StateReason`].
#[must_use]
pub fn reason_to_error(code: u32) -> ConnectionError {
    let reason = StateReason::from(code);
    match reason {
        StateReason::NoSecrets
        | StateReason::SupplicantDisconnected
        | StateReason::SupplicantFailed
        | StateReason::SimPinIncorrect
        | StateReason::GsmPinCheckFailed => ConnectionError::AuthFailed,

        StateReason::SupplicantConfigFailed => ConnectionError::SupplicantConfigFailed,

        StateReason::SupplicantTimeout => ConnectionError::SupplicantTimeout,

        StateReason::DhcpStartFailed
        | StateReason::DhcpError
        | StateReason::DhcpFailed
        | StateReason::IpConfigUnavailable
        | StateReason::IpConfigExpired => ConnectionError::DhcpFailed,

        StateReason::SsidNotFound => ConnectionError::NotFound,

        _ => ConnectionError::DeviceFailed(reason),
    }
}
