use std::fmt::{Display, Formatter};

use super::error::ConnectionError;

/// NetworkManager active connection state.
///
/// These values represent the lifecycle states of an active connection
/// as reported by the NM D-Bus API.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActiveConnectionState {
    /// Connection state is unknown.
    Unknown,
    /// Connection is activating (connecting).
    Activating,
    /// Connection is fully activated (connected).
    Activated,
    /// Connection is deactivating (disconnecting).
    Deactivating,
    /// Connection is fully deactivated (disconnected).
    Deactivated,
    /// Unknown state code not mapped to a specific variant.
    Other(u32),
}

impl From<u32> for ActiveConnectionState {
    fn from(code: u32) -> Self {
        match code {
            0 => Self::Unknown,
            1 => Self::Activating,
            2 => Self::Activated,
            3 => Self::Deactivating,
            4 => Self::Deactivated,
            v => Self::Other(v),
        }
    }
}

impl Display for ActiveConnectionState {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Unknown => write!(f, "unknown"),
            Self::Activating => write!(f, "activating"),
            Self::Activated => write!(f, "activated"),
            Self::Deactivating => write!(f, "deactivating"),
            Self::Deactivated => write!(f, "deactivated"),
            Self::Other(v) => write!(f, "unknown state ({v})"),
        }
    }
}

/// NetworkManager active connection state reason codes.
///
/// These values indicate why an active connection transitioned to its
/// current state. Use `ConnectionStateReason::from(code)` to convert
/// from the raw u32 values returned by NetworkManager signals.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConnectionStateReason {
    /// The reason is unknown.
    Unknown,
    /// No specific reason.
    None,
    /// User disconnected.
    UserDisconnected,
    /// Device disconnected.
    DeviceDisconnected,
    /// The NetworkManager service stopped.
    ServiceStopped,
    /// IP configuration was invalid.
    IpConfigInvalid,
    /// Connection timed out while activating.
    ConnectTimeout,
    /// Service start timed out.
    ServiceStartTimeout,
    /// Service failed to start.
    ServiceStartFailed,
    /// No secrets (password) were provided.
    NoSecrets,
    /// Login/authentication failed.
    LoginFailed,
    /// The connection was removed.
    ConnectionRemoved,
    /// A dependency failed.
    DependencyFailed,
    /// Device realization failed.
    DeviceRealizeFailed,
    /// Device was removed.
    DeviceRemoved,
    /// Unknown reason code not mapped to a specific variant.
    Other(u32),
}

impl From<u32> for ConnectionStateReason {
    fn from(code: u32) -> Self {
        match code {
            0 => Self::Unknown,
            1 => Self::None,
            2 => Self::UserDisconnected,
            3 => Self::DeviceDisconnected,
            4 => Self::ServiceStopped,
            5 => Self::IpConfigInvalid,
            6 => Self::ConnectTimeout,
            7 => Self::ServiceStartTimeout,
            8 => Self::ServiceStartFailed,
            9 => Self::NoSecrets,
            10 => Self::LoginFailed,
            11 => Self::ConnectionRemoved,
            12 => Self::DependencyFailed,
            13 => Self::DeviceRealizeFailed,
            14 => Self::DeviceRemoved,
            v => Self::Other(v),
        }
    }
}

impl Display for ConnectionStateReason {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Unknown => write!(f, "unknown"),
            Self::None => write!(f, "none"),
            Self::UserDisconnected => write!(f, "user disconnected"),
            Self::DeviceDisconnected => write!(f, "device disconnected"),
            Self::ServiceStopped => write!(f, "service stopped"),
            Self::IpConfigInvalid => write!(f, "IP configuration invalid"),
            Self::ConnectTimeout => write!(f, "connection timed out"),
            Self::ServiceStartTimeout => write!(f, "service start timed out"),
            Self::ServiceStartFailed => write!(f, "service start failed"),
            Self::NoSecrets => write!(f, "no secrets (password) provided"),
            Self::LoginFailed => write!(f, "login/authentication failed"),
            Self::ConnectionRemoved => write!(f, "connection was removed"),
            Self::DependencyFailed => write!(f, "dependency failed"),
            Self::DeviceRealizeFailed => write!(f, "device realization failed"),
            Self::DeviceRemoved => write!(f, "device was removed"),
            Self::Other(v) => write!(f, "unknown reason ({v})"),
        }
    }
}

/// Converts a connection state reason code to a specific `ConnectionError`.
///
/// Maps authentication-related failures to `AuthFailed`, timeout issues to `Timeout`,
/// and other failures to the appropriate variant.
#[must_use]
pub fn connection_state_reason_to_error(code: u32) -> ConnectionError {
    let reason = ConnectionStateReason::from(code);
    match reason {
        ConnectionStateReason::NoSecrets | ConnectionStateReason::LoginFailed => {
            ConnectionError::AuthFailed
        }
        ConnectionStateReason::ConnectTimeout | ConnectionStateReason::ServiceStartTimeout => {
            ConnectionError::Timeout
        }
        ConnectionStateReason::IpConfigInvalid => ConnectionError::DhcpFailed,
        _ => ConnectionError::ActivationFailed(reason),
    }
}
