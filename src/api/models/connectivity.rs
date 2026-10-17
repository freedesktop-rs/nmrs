//! Connectivity state and captive-portal awareness.
//!
//! NetworkManager periodically (or on demand via
//! [`crate::NetworkManager::check_connectivity`]) probes a well-known URL to
//! determine whether the host has actual internet access or is behind a captive
//! portal. The result is exposed as a [`ConnectivityState`].
//!
//! UIs should watch for [`ConnectivityState::Portal`] and prompt the user to
//! open their browser at the captive portal URL (see
//! [`crate::NetworkManager::captive_portal_url`]).

use std::fmt;

/// NM's `NMConnectivityState` enum.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash)]
pub enum ConnectivityState {
    /// NM has not checked yet.
    Unknown,
    /// No network connection at all.
    None,
    /// Connected behind a captive portal.
    Portal,
    /// Connected but no internet (upstream unreachable).
    Limited,
    /// Connected and internet-reachable.
    Full,
}

impl ConnectivityState {
    /// `true` only when the host has verified internet connectivity.
    #[must_use]
    pub fn is_usable_for_internet(self) -> bool {
        matches!(self, Self::Full)
    }

    /// `true` when NM detected a captive portal.
    #[must_use]
    pub fn is_captive(self) -> bool {
        matches!(self, Self::Portal)
    }
}

impl From<u32> for ConnectivityState {
    fn from(v: u32) -> Self {
        match v {
            0 => Self::Unknown,
            1 => Self::None,
            2 => Self::Portal,
            3 => Self::Limited,
            4 => Self::Full,
            _ => Self::Unknown,
        }
    }
}

impl From<ConnectivityState> for u32 {
    fn from(s: ConnectivityState) -> u32 {
        match s {
            ConnectivityState::Unknown => 0,
            ConnectivityState::None => 1,
            ConnectivityState::Portal => 2,
            ConnectivityState::Limited => 3,
            ConnectivityState::Full => 4,
        }
    }
}

impl fmt::Display for ConnectivityState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unknown => write!(f, "unknown"),
            Self::None => write!(f, "none"),
            Self::Portal => write!(f, "portal"),
            Self::Limited => write!(f, "limited"),
            Self::Full => write!(f, "full"),
        }
    }
}

/// Snapshot of NM's connectivity subsystem.
///
/// Returned by [`crate::NetworkManager::connectivity_report`].
#[non_exhaustive]
#[derive(Debug, Clone)]
pub struct ConnectivityReport {
    /// Current connectivity state.
    pub state: ConnectivityState,
    /// Whether NM is allowed to probe.
    pub check_enabled: bool,
    /// URL NM probes when checking (may be empty if disabled).
    pub check_uri: Option<String>,
    /// Captive-portal URL detected by NM, if state is [`ConnectivityState::Portal`].
    /// `None` when NM has not filled in the URL or the NM version doesn't expose it.
    pub captive_portal_url: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip_all() {
        for code in 0..=4 {
            let s = ConnectivityState::from(code);
            assert_eq!(u32::from(s), code);
        }
    }

    #[test]
    fn out_of_range_maps_to_unknown() {
        assert_eq!(ConnectivityState::from(99), ConnectivityState::Unknown);
    }

    #[test]
    fn is_captive() {
        assert!(ConnectivityState::Portal.is_captive());
        assert!(!ConnectivityState::Full.is_captive());
        assert!(!ConnectivityState::None.is_captive());
    }

    #[test]
    fn is_usable() {
        assert!(ConnectivityState::Full.is_usable_for_internet());
        assert!(!ConnectivityState::Portal.is_usable_for_internet());
        assert!(!ConnectivityState::Limited.is_usable_for_internet());
        assert!(!ConnectivityState::Unknown.is_usable_for_internet());
    }

    #[test]
    fn display_test() {
        assert_eq!(format!("{}", ConnectivityState::Unknown), "unknown");
        assert_eq!(format!("{}", ConnectivityState::None), "none");
        assert_eq!(format!("{}", ConnectivityState::Portal), "portal");
        assert_eq!(format!("{}", ConnectivityState::Limited), "limited");
        assert_eq!(format!("{}", ConnectivityState::Full), "full");
    }
}
