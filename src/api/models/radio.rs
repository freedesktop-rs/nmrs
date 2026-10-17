//! Radio and airplane-mode state types.
//!
//! NetworkManager tracks both a software-enabled flag (controlled via D-Bus)
//! and a hardware-enabled flag (reflecting the kernel rfkill state) for each
//! radio. [`RadioState`] captures both, and [`AirplaneModeState`] aggregates
//! Wi-Fi, WWAN, and Bluetooth into a single snapshot.
//!
//! On hosts without a given radio (e.g. a desktop with no Bluetooth adapter
//! or no WWAN modem), the corresponding [`RadioState`] is reported with
//! `present = false` so callers can ignore it when deciding whether the
//! system is in airplane mode.

/// Software and hardware enabled state for a single radio.
///
/// `enabled` reflects the user-facing toggle (can be written via D-Bus).
/// `hardware_enabled` reflects the kernel rfkill state and cannot be changed
/// from userspace — if `false`, setting `enabled = true` is accepted by NM
/// but the radio remains off until hardware is unkilled.
/// `present` reflects whether this kind of radio actually exists on the
/// host. If `false`, the `enabled` and `hardware_enabled` values are best-effort
/// defaults and should not factor into airplane-mode decisions.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub struct RadioState {
    /// Software-enabled: can the user turn this radio on via NM?
    pub enabled: bool,
    /// Hardware-enabled: is rfkill allowing this radio?
    /// If `false`, `enabled = true` is a no-op until hardware is unkilled.
    pub hardware_enabled: bool,
    /// Whether a controllable instance of this radio exists on the host.
    ///
    /// `false` means the system has no Wi-Fi card / no modem / no Bluetooth
    /// adapter (or BlueZ is not running). Consumers should treat such radios
    /// as not contributing to airplane-mode state.
    pub present: bool,
}

impl RadioState {
    /// Creates a new `RadioState` for a radio that is present on the host.
    ///
    /// Equivalent to `RadioState::with_presence(enabled, hardware_enabled, true)`.
    #[must_use]
    pub fn new(enabled: bool, hardware_enabled: bool) -> Self {
        Self::with_presence(enabled, hardware_enabled, true)
    }

    /// Creates a new `RadioState`, explicitly recording whether the radio
    /// exists on the host.
    #[must_use]
    pub fn with_presence(enabled: bool, hardware_enabled: bool, present: bool) -> Self {
        Self {
            enabled,
            hardware_enabled,
            present,
        }
    }
}

/// Aggregated radio state for all radios that `nmrs` can control.
///
/// Returned by [`NetworkManager::airplane_mode_state`](crate::NetworkManager::airplane_mode_state).
///
/// Radios with `present = false` (e.g. Bluetooth on a host without BlueZ,
/// WWAN on a host without a modem) are ignored by [`is_airplane_mode`] and
/// [`any_hardware_killed`]. This means a wifi-only laptop counts as being
/// in airplane mode the moment its Wi-Fi software switch is off, instead
/// of erroneously requiring an absent Bluetooth or WWAN radio to also be
/// off.
///
/// [`is_airplane_mode`]: AirplaneModeState::is_airplane_mode
/// [`any_hardware_killed`]: AirplaneModeState::any_hardware_killed
#[non_exhaustive]
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub struct AirplaneModeState {
    /// Wi-Fi radio state.
    pub wifi: RadioState,
    /// WWAN (mobile broadband) radio state.
    pub wwan: RadioState,
    /// Bluetooth radio state (sourced from BlueZ + rfkill).
    pub bluetooth: RadioState,
}

impl AirplaneModeState {
    /// Creates a new `AirplaneModeState`.
    #[must_use]
    pub fn new(wifi: RadioState, wwan: RadioState, bluetooth: RadioState) -> Self {
        Self {
            wifi,
            wwan,
            bluetooth,
        }
    }

    /// Returns `true` if every *present* radio is software-disabled.
    ///
    /// Radios with `present = false` are ignored. If no controllable radios
    /// exist on the host at all, this returns `false` — there is no
    /// meaningful "airplane mode" to be in.
    #[must_use]
    pub fn is_airplane_mode(&self) -> bool {
        let mut any_present = false;
        for radio in [&self.wifi, &self.wwan, &self.bluetooth] {
            if !radio.present {
                continue;
            }
            any_present = true;
            if radio.enabled {
                return false;
            }
        }
        any_present
    }

    /// Returns `true` if any *present* radio has its hardware kill switch active.
    #[must_use]
    pub fn any_hardware_killed(&self) -> bool {
        [&self.wifi, &self.wwan, &self.bluetooth]
            .into_iter()
            .any(|r| r.present && !r.hardware_enabled)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_off_is_airplane_mode() {
        let state = AirplaneModeState::new(
            RadioState::new(false, true),
            RadioState::new(false, true),
            RadioState::new(false, true),
        );
        assert!(state.is_airplane_mode());
        assert!(!state.any_hardware_killed());
    }

    #[test]
    fn any_on_is_not_airplane_mode() {
        let state = AirplaneModeState::new(
            RadioState::new(true, true),
            RadioState::new(false, true),
            RadioState::new(false, true),
        );
        assert!(!state.is_airplane_mode());
    }

    #[test]
    fn hardware_killed_detected() {
        let state = AirplaneModeState::new(
            RadioState::new(true, true),
            RadioState::new(true, false),
            RadioState::new(true, true),
        );
        assert!(state.any_hardware_killed());
    }

    #[test]
    fn no_hardware_kill() {
        let state = AirplaneModeState::new(
            RadioState::new(false, true),
            RadioState::new(false, true),
            RadioState::new(false, true),
        );
        assert!(!state.any_hardware_killed());
    }

    #[test]
    fn radio_state_new() {
        let rs = RadioState::new(true, false);
        assert!(rs.enabled);
        assert!(!rs.hardware_enabled);
        assert!(rs.present, "RadioState::new defaults to present = true");
    }

    #[test]
    fn radio_state_with_presence() {
        let rs = RadioState::with_presence(true, true, false);
        assert!(rs.enabled);
        assert!(rs.hardware_enabled);
        assert!(!rs.present);
    }

    #[test]
    fn absent_radios_do_not_block_airplane_mode() {
        // Wi-Fi-only host: Bluetooth and WWAN are absent. Disabling Wi-Fi
        // alone should put us in airplane mode.
        let state = AirplaneModeState::new(
            RadioState::new(false, true),
            RadioState::with_presence(true, true, false),
            RadioState::with_presence(true, false, false),
        );
        assert!(state.is_airplane_mode());
    }

    #[test]
    fn absent_radios_do_not_count_as_hardware_killed() {
        let state = AirplaneModeState::new(
            RadioState::new(true, true),
            RadioState::with_presence(true, false, false),
            RadioState::with_presence(true, false, false),
        );
        assert!(!state.any_hardware_killed());
    }

    #[test]
    fn no_radios_present_is_not_airplane_mode() {
        let state = AirplaneModeState::new(
            RadioState::with_presence(true, true, false),
            RadioState::with_presence(true, true, false),
            RadioState::with_presence(true, true, false),
        );
        assert!(
            !state.is_airplane_mode(),
            "with no controllable radios there is no airplane mode"
        );
    }

    #[test]
    fn one_radio_present_and_off_is_airplane_mode() {
        let state = AirplaneModeState::new(
            RadioState::new(false, true),
            RadioState::with_presence(true, true, false),
            RadioState::with_presence(true, true, false),
        );
        assert!(state.is_airplane_mode());
    }
}
