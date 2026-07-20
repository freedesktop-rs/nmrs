//! Point-in-time NetworkManager snapshot model.

use std::collections::HashMap;

use super::{
    AccessPoint, ActiveConnection, AirplaneModeState, ConnectivityReport, Device, RadioState,
    SavedConnection, SavedConnectionBrief, SavedVpnSummary, SettingsSummary, VpnKind, WifiDevice,
    WifiNetworkGroup,
};

/// Applet-oriented view derived from a [`NetworkSnapshot`].
///
/// This summary groups visible Wi-Fi APs, indexes saved Wi-Fi profiles by SSID,
/// and indexes saved VPN profiles by UUID. It is a pure transformation of the
/// snapshot and performs no D-Bus reads.
#[non_exhaustive]
#[derive(Debug, Clone)]
pub struct AppletNetworkSummary {
    /// Active connections classified for applet rendering.
    pub active_connections: Vec<ActiveConnection>,
    /// Visible Wi-Fi groups keyed by interface and SSID.
    pub wifi_groups: Vec<WifiNetworkGroup>,
    /// Saved Wi-Fi profiles keyed by SSID.
    pub known_wifi: HashMap<String, Vec<SavedConnectionBrief>>,
    /// Saved VPN profiles keyed by UUID.
    pub saved_vpns: HashMap<String, SavedVpnSummary>,
    /// Connectivity and captive-portal state.
    pub connectivity: ConnectivityReport,
    /// Aggregated airplane-mode state.
    pub airplane_mode: AirplaneModeState,
}

/// Point-in-time state needed by GUI network applets.
///
/// Build this with [`NetworkManager::snapshot`](crate::NetworkManager::snapshot)
/// after receiving a [`NetworkEvent`](super::NetworkEvent).
#[non_exhaustive]
#[derive(Debug, Clone)]
pub struct NetworkSnapshot {
    /// Wi-Fi radio state.
    pub wifi: RadioState,
    /// WWAN/mobile broadband radio state.
    pub wwan: RadioState,
    /// Bluetooth radio state.
    pub bluetooth: RadioState,
    /// Aggregated airplane-mode state.
    pub airplane_mode: AirplaneModeState,
    /// Connectivity and captive-portal state.
    pub connectivity: ConnectivityReport,
    /// Active connections classified for applet rendering.
    pub active_connections: Vec<ActiveConnection>,
    /// Visible access points, one entry per BSSID.
    pub access_points: Vec<AccessPoint>,
    /// All saved connection profiles.
    pub saved_connections: Vec<SavedConnection>,
    /// Saved Wi-Fi profiles.
    pub saved_wifi_profiles: Vec<SavedConnection>,
    /// Saved VPN profiles, including kernel WireGuard profiles.
    pub saved_vpn_profiles: Vec<SavedConnection>,
    /// Wi-Fi device summaries.
    pub wifi_devices: Vec<WifiDevice>,
    /// Wired Ethernet devices from the broad device model.
    pub wired_devices: Vec<Device>,
}

impl NetworkSnapshot {
    /// Groups visible access points by `(interface, ssid)` for applet Wi-Fi rows.
    ///
    /// Each group keeps every BSSID for advanced UI behavior and exposes the
    /// strongest AP as the representative row. Matching saved profiles are
    /// attached when their SSID, optional interface binding, and optional BSSID
    /// binding match the visible group.
    #[must_use]
    pub fn wifi_groups(&self) -> Vec<WifiNetworkGroup> {
        let mut grouped: HashMap<(String, String), Vec<AccessPoint>> = HashMap::new();
        for ap in &self.access_points {
            if ap.is_hidden() {
                continue;
            }
            grouped
                .entry((ap.interface.clone(), ap.ssid.clone()))
                .or_default()
                .push(ap.clone());
        }

        let mut groups = grouped
            .into_iter()
            .filter_map(|((interface, ssid), mut access_points)| {
                sort_access_points(&mut access_points, &self.active_connections, &interface);
                let strongest = access_points.first()?.clone();
                let saved_profiles = self
                    .saved_wifi_profiles
                    .iter()
                    .filter(|profile| {
                        saved_wifi_matches_group(profile, &interface, &ssid, &access_points)
                    })
                    .map(saved_brief)
                    .collect::<Vec<_>>();
                let active = active_wifi_matches_group(
                    &self.active_connections,
                    &interface,
                    &ssid,
                    &access_points,
                );
                let known = !saved_profiles.is_empty();

                Some(WifiNetworkGroup {
                    ssid,
                    interface,
                    strongest,
                    access_points,
                    saved_profiles,
                    active,
                    known,
                })
            })
            .collect::<Vec<_>>();

        groups.sort_by(|a, b| {
            a.interface
                .cmp(&b.interface)
                .then_with(|| a.ssid.cmp(&b.ssid))
        });
        groups
    }

    /// Returns saved Wi-Fi profiles keyed by SSID.
    ///
    /// Duplicate profiles for the same SSID are preserved. Hidden profiles are
    /// included when their saved settings contain an SSID, even if no visible AP
    /// appears in this snapshot.
    #[must_use]
    pub fn known_wifi_by_ssid(&self) -> HashMap<String, Vec<SavedConnectionBrief>> {
        let mut known: HashMap<String, Vec<SavedConnectionBrief>> = HashMap::new();
        for profile in &self.saved_wifi_profiles {
            if let Some(ssid) = saved_wifi_ssid(profile) {
                known
                    .entry(ssid.to_string())
                    .or_default()
                    .push(saved_brief(profile));
            }
        }
        known
    }

    /// Returns saved VPN profiles keyed by UUID.
    ///
    /// Both NetworkManager plugin VPNs and native WireGuard profiles are
    /// included. A summary is marked active when a typed active VPN connection
    /// has the same UUID.
    #[must_use]
    pub fn saved_vpn_map(&self) -> HashMap<String, SavedVpnSummary> {
        let mut vpns = HashMap::new();
        for profile in &self.saved_vpn_profiles {
            if !is_saved_vpn_profile(profile) {
                continue;
            }

            let active = self.active_connections.iter().any(|connection| {
                matches!(connection, ActiveConnection::Vpn(vpn) if vpn.uuid == profile.uuid)
            });
            let summary = SavedVpnSummary {
                uuid: profile.uuid.clone(),
                id: profile.id.clone(),
                kind: saved_vpn_kind(profile),
                active,
            };
            vpns.insert(summary.uuid.clone(), summary);
        }
        vpns
    }

    /// Builds an applet-oriented summary from this snapshot.
    ///
    /// This is equivalent to calling [`Self::wifi_groups`],
    /// [`Self::known_wifi_by_ssid`], and [`Self::saved_vpn_map`] on the same
    /// snapshot and bundling the results with active connection, connectivity,
    /// and airplane-mode state.
    #[must_use]
    pub fn applet_summary(&self) -> AppletNetworkSummary {
        AppletNetworkSummary {
            active_connections: self.active_connections.clone(),
            wifi_groups: self.wifi_groups(),
            known_wifi: self.known_wifi_by_ssid(),
            saved_vpns: self.saved_vpn_map(),
            connectivity: self.connectivity.clone(),
            airplane_mode: self.airplane_mode,
        }
    }

    /// Returns individually reported APs whose raw SSID is empty.
    #[must_use]
    pub fn hidden_access_points(&self) -> Vec<AccessPoint> {
        self.access_points
            .iter()
            .filter(|access_point| access_point.is_hidden())
            .cloned()
            .collect()
    }
}

pub(crate) fn saved_wifi_profiles(saved: &[SavedConnection]) -> Vec<SavedConnection> {
    saved
        .iter()
        .filter(|profile| saved_wifi_ssid(profile).is_some())
        .cloned()
        .collect()
}

fn sort_access_points(
    access_points: &mut [AccessPoint],
    active_connections: &[ActiveConnection],
    interface: &str,
) {
    access_points.sort_by(|a, b| {
        b.strength
            .cmp(&a.strength)
            .then_with(|| {
                let a_active = ap_matches_active_bssid(a, active_connections, interface);
                let b_active = ap_matches_active_bssid(b, active_connections, interface);
                b_active.cmp(&a_active)
            })
            .then_with(|| a.bssid.cmp(&b.bssid))
    });
}

fn ap_matches_active_bssid(
    ap: &AccessPoint,
    active_connections: &[ActiveConnection],
    interface: &str,
) -> bool {
    ap.is_active
        || active_connections.iter().any(|connection| {
            let ActiveConnection::Wifi(active) = connection else {
                return false;
            };
            active.interface.as_deref() == Some(interface)
                && active
                    .bssid
                    .as_deref()
                    .is_some_and(|bssid| bssid_eq(&ap.bssid, bssid))
        })
}

fn active_wifi_matches_group(
    active_connections: &[ActiveConnection],
    interface: &str,
    ssid: &str,
    access_points: &[AccessPoint],
) -> bool {
    active_connections.iter().any(|connection| {
        let ActiveConnection::Wifi(active) = connection else {
            return false;
        };
        if active.interface.as_deref() != Some(interface) {
            return false;
        }
        active.ssid == ssid
            || active
                .bssid
                .as_deref()
                .is_some_and(|bssid| access_points.iter().any(|ap| bssid_eq(&ap.bssid, bssid)))
    })
}

fn saved_wifi_matches_group(
    profile: &SavedConnection,
    interface: &str,
    ssid: &str,
    access_points: &[AccessPoint],
) -> bool {
    if saved_wifi_ssid(profile) != Some(ssid) {
        return false;
    }
    if let Some(bound_interface) = profile.interface_name.as_deref()
        && !bound_interface.is_empty()
        && bound_interface != interface
    {
        return false;
    }
    if let Some(bssid) = saved_wifi_bssid(profile) {
        return access_points.iter().any(|ap| bssid_eq(&ap.bssid, bssid));
    }
    true
}

fn saved_wifi_ssid(profile: &SavedConnection) -> Option<&str> {
    if profile.connection_type != "802-11-wireless" {
        return None;
    }
    match &profile.summary {
        SettingsSummary::Wifi { ssid, .. } => Some(ssid.as_str()),
        _ => None,
    }
}

fn saved_wifi_bssid(profile: &SavedConnection) -> Option<&str> {
    match &profile.summary {
        SettingsSummary::Wifi { bssid, .. } => bssid
            .as_deref()
            .filter(|saved_bssid| !saved_bssid.is_empty()),
        _ => None,
    }
}

fn saved_brief(profile: &SavedConnection) -> SavedConnectionBrief {
    SavedConnectionBrief {
        path: profile.path.clone(),
        uuid: profile.uuid.clone(),
        id: profile.id.clone(),
        connection_type: profile.connection_type.clone(),
    }
}

fn is_saved_vpn_profile(profile: &SavedConnection) -> bool {
    matches!(profile.connection_type.as_str(), "vpn" | "wireguard")
        || matches!(&profile.summary, SettingsSummary::WireGuard { .. })
}

fn saved_vpn_kind(profile: &SavedConnection) -> Option<VpnKind> {
    if profile.connection_type == "wireguard"
        || matches!(&profile.summary, SettingsSummary::WireGuard { .. })
    {
        Some(VpnKind::WireGuard)
    } else if profile.connection_type == "vpn"
        || matches!(&profile.summary, SettingsSummary::Vpn { .. })
    {
        Some(VpnKind::Plugin)
    } else {
        None
    }
}

fn bssid_eq(left: &str, right: &str) -> bool {
    left.eq_ignore_ascii_case(right)
}

pub(crate) fn saved_vpn_profiles(saved: &[SavedConnection]) -> Vec<SavedConnection> {
    saved
        .iter()
        .filter(|profile| is_saved_vpn_profile(profile))
        .cloned()
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::api::models::{
        ActiveConnectionState, ActiveVpnConnection, ActiveWifiConnection, ApMode,
        ConnectivityState, DeviceState, SecurityFeatures, VpnSecretFlags,
    };
    use zvariant::OwnedObjectPath;

    fn saved(connection_type: &str) -> SavedConnection {
        SavedConnection {
            path: OwnedObjectPath::try_from("/org/freedesktop/NetworkManager/Settings/1")
                .expect("valid object path"),
            uuid: format!("{connection_type}-uuid"),
            id: format!("{connection_type}-id"),
            connection_type: connection_type.to_string(),
            interface_name: None,
            autoconnect: true,
            autoconnect_priority: 0,
            timestamp_unix: 0,
            permissions: Vec::new(),
            unsaved: false,
            filename: None,
            summary: SettingsSummary::Other {
                sections: vec!["connection".into()],
            },
        }
    }

    fn object_path(path: &str) -> OwnedObjectPath {
        OwnedObjectPath::try_from(path).expect("valid object path")
    }

    fn snapshot(
        access_points: Vec<AccessPoint>,
        saved_wifi_profiles: Vec<SavedConnection>,
        saved_vpn_profiles: Vec<SavedConnection>,
        active_connections: Vec<ActiveConnection>,
    ) -> NetworkSnapshot {
        let radio = RadioState::with_presence(false, true, false);
        NetworkSnapshot {
            wifi: radio,
            wwan: radio,
            bluetooth: radio,
            airplane_mode: AirplaneModeState::new(radio, radio, radio),
            connectivity: ConnectivityReport {
                state: ConnectivityState::Unknown,
                check_enabled: false,
                check_uri: None,
                captive_portal_url: None,
            },
            active_connections,
            access_points,
            saved_connections: Vec::new(),
            saved_wifi_profiles,
            saved_vpn_profiles,
            wifi_devices: Vec::new(),
            wired_devices: Vec::new(),
        }
    }

    fn ap(interface: &str, ssid: &str, bssid: &str, strength: u8) -> AccessPoint {
        AccessPoint {
            path: object_path(&format!(
                "/org/freedesktop/NetworkManager/AccessPoint/{}",
                bssid.replace(':', "")
            )),
            device_path: object_path(&format!(
                "/org/freedesktop/NetworkManager/Devices/{interface}"
            )),
            interface: interface.to_string(),
            ssid: ssid.to_string(),
            ssid_bytes: ssid.as_bytes().to_vec(),
            bssid: bssid.to_string(),
            frequency_mhz: 2412,
            max_bitrate_kbps: 54000,
            strength,
            mode: ApMode::Infrastructure,
            security: SecurityFeatures::default(),
            last_seen_secs: None,
            is_active: false,
            device_state: DeviceState::Disconnected,
        }
    }

    fn hidden_ap(interface: &str, bssid: &str, strength: u8) -> AccessPoint {
        let mut access_point = ap(interface, "<Hidden Network>", bssid, strength);
        access_point.ssid_bytes.clear();
        access_point
    }

    fn saved_wifi(
        index: usize,
        uuid: &str,
        ssid: &str,
        interface_name: Option<&str>,
        bssid: Option<&str>,
    ) -> SavedConnection {
        SavedConnection {
            path: object_path(&format!("/org/freedesktop/NetworkManager/Settings/{index}")),
            uuid: uuid.to_string(),
            id: ssid.to_string(),
            connection_type: "802-11-wireless".to_string(),
            interface_name: interface_name.map(ToOwned::to_owned),
            autoconnect: true,
            autoconnect_priority: 0,
            timestamp_unix: 0,
            permissions: Vec::new(),
            unsaved: false,
            filename: None,
            summary: SettingsSummary::Wifi {
                ssid: ssid.to_string(),
                mode: Some("infrastructure".into()),
                security: None,
                band: None,
                channel: None,
                bssid: bssid.map(ToOwned::to_owned),
                hidden: false,
                mac_randomization: None,
            },
        }
    }

    fn saved_vpn(index: usize, uuid: &str) -> SavedConnection {
        SavedConnection {
            path: object_path(&format!("/org/freedesktop/NetworkManager/Settings/{index}")),
            uuid: uuid.to_string(),
            id: format!("VPN {index}"),
            connection_type: "vpn".to_string(),
            interface_name: None,
            autoconnect: false,
            autoconnect_priority: 0,
            timestamp_unix: 0,
            permissions: Vec::new(),
            unsaved: false,
            filename: None,
            summary: SettingsSummary::Vpn {
                service_type: "org.freedesktop.NetworkManager.openvpn".into(),
                user_name: None,
                password_flags: VpnSecretFlags(0),
                data_keys: Vec::new(),
                persistent: false,
            },
        }
    }

    fn saved_wireguard(index: usize, uuid: &str) -> SavedConnection {
        SavedConnection {
            path: object_path(&format!("/org/freedesktop/NetworkManager/Settings/{index}")),
            uuid: uuid.to_string(),
            id: format!("WireGuard {index}"),
            connection_type: "wireguard".to_string(),
            interface_name: None,
            autoconnect: false,
            autoconnect_priority: 0,
            timestamp_unix: 0,
            permissions: Vec::new(),
            unsaved: false,
            filename: None,
            summary: SettingsSummary::WireGuard {
                listen_port: None,
                mtu: None,
                fwmark: None,
                peer_count: 1,
                first_peer_endpoint: None,
            },
        }
    }

    fn active_wifi(interface: &str, ssid: &str, bssid: Option<&str>) -> ActiveConnection {
        ActiveConnection::Wifi(ActiveWifiConnection {
            id: ssid.to_string(),
            uuid: format!("{ssid}-active"),
            ssid: ssid.to_string(),
            interface: Some(interface.to_string()),
            bssid: bssid.map(ToOwned::to_owned),
            strength: None,
            ip4_address: None,
            ip6_address: None,
            state: ActiveConnectionState::Activated,
        })
    }

    fn active_vpn(uuid: &str) -> ActiveConnection {
        ActiveConnection::Vpn(ActiveVpnConnection {
            id: "VPN".into(),
            uuid: uuid.to_string(),
            interface: None,
            ip4_address: None,
            ip6_address: None,
            state: ActiveConnectionState::Activated,
        })
    }

    #[test]
    fn filters_saved_wifi_profiles() {
        let profiles = vec![
            saved_wifi(1, "wifi-uuid", "Cafe", None, None),
            saved("vpn"),
            saved("802-3-ethernet"),
        ];

        let wifi = saved_wifi_profiles(&profiles);

        assert_eq!(wifi.len(), 1);
        assert_eq!(wifi[0].connection_type, "802-11-wireless");
    }

    #[test]
    fn filters_saved_vpn_profiles() {
        let profiles = vec![saved("802-11-wireless"), saved("vpn"), saved("wireguard")];

        let vpn = saved_vpn_profiles(&profiles);

        assert_eq!(vpn.len(), 2);
        assert!(vpn.iter().any(|profile| profile.connection_type == "vpn"));
        assert!(
            vpn.iter()
                .any(|profile| profile.connection_type == "wireguard")
        );
    }

    #[test]
    fn groups_duplicate_ssids_on_one_interface() {
        let snapshot = snapshot(
            vec![
                ap("wlan0", "Cafe", "AA:AA:AA:AA:AA:01", 30),
                ap("wlan0", "Cafe", "AA:AA:AA:AA:AA:02", 80),
            ],
            Vec::new(),
            Vec::new(),
            Vec::new(),
        );

        let groups = snapshot.wifi_groups();

        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0].ssid, "Cafe");
        assert_eq!(groups[0].interface, "wlan0");
        assert_eq!(groups[0].access_points.len(), 2);
        assert_eq!(groups[0].strongest.bssid, "AA:AA:AA:AA:AA:02");
    }

    #[test]
    fn keeps_same_ssid_on_two_interfaces_separate() {
        let snapshot = snapshot(
            vec![
                ap("wlan0", "Cafe", "AA:AA:AA:AA:AA:01", 70),
                ap("wlan1", "Cafe", "BB:BB:BB:BB:BB:01", 90),
            ],
            Vec::new(),
            Vec::new(),
            Vec::new(),
        );

        let groups = snapshot.wifi_groups();

        assert_eq!(groups.len(), 2);
        assert!(groups.iter().any(|group| group.interface == "wlan0"));
        assert!(groups.iter().any(|group| group.interface == "wlan1"));
    }

    #[test]
    fn attaches_interface_bound_profile_only_to_matching_group() {
        let snapshot = snapshot(
            vec![
                ap("wlan0", "Cafe", "AA:AA:AA:AA:AA:01", 70),
                ap("wlan1", "Cafe", "BB:BB:BB:BB:BB:01", 90),
            ],
            vec![
                saved_wifi(10, "wlan1-profile", "Cafe", Some("wlan1"), None),
                saved_wifi(11, "missing-interface", "Cafe", Some("wlan9"), None),
            ],
            Vec::new(),
            Vec::new(),
        );

        let groups = snapshot.wifi_groups();
        let wlan0 = groups
            .iter()
            .find(|group| group.interface == "wlan0")
            .unwrap();
        let wlan1 = groups
            .iter()
            .find(|group| group.interface == "wlan1")
            .unwrap();

        assert!(!wlan0.known);
        assert!(wlan0.saved_profiles.is_empty());
        assert!(wlan1.known);
        assert_eq!(wlan1.saved_profiles.len(), 1);
        assert_eq!(wlan1.saved_profiles[0].uuid, "wlan1-profile");
    }

    #[test]
    fn matches_bssid_pinned_saved_profiles() {
        let snapshot = snapshot(
            vec![
                ap("wlan0", "Cafe", "AA:AA:AA:AA:AA:01", 70),
                ap("wlan0", "Cafe", "AA:AA:AA:AA:AA:02", 90),
            ],
            vec![
                saved_wifi(10, "matches", "Cafe", None, Some("AA:AA:AA:AA:AA:02")),
                saved_wifi(11, "misses", "Cafe", None, Some("CC:CC:CC:CC:CC:01")),
            ],
            Vec::new(),
            Vec::new(),
        );

        let groups = snapshot.wifi_groups();

        assert!(groups[0].known);
        assert_eq!(groups[0].saved_profiles.len(), 1);
        assert_eq!(groups[0].saved_profiles[0].uuid, "matches");
    }

    #[test]
    fn keeps_hidden_saved_profiles_without_visible_aps() {
        let snapshot = snapshot(
            Vec::new(),
            vec![saved_wifi(10, "hidden", "Hidden", None, None)],
            Vec::new(),
            Vec::new(),
        );

        let groups = snapshot.wifi_groups();
        let known = snapshot.known_wifi_by_ssid();

        assert!(groups.is_empty());
        assert_eq!(known["Hidden"][0].uuid, "hidden");
    }

    #[test]
    fn detects_active_wifi_group_and_prefers_active_bssid_on_tie() {
        let snapshot = snapshot(
            vec![
                ap("wlan0", "Cafe", "AA:AA:AA:AA:AA:01", 70),
                ap("wlan0", "Cafe", "AA:AA:AA:AA:AA:02", 70),
                ap("wlan1", "Cafe", "BB:BB:BB:BB:BB:01", 90),
            ],
            Vec::new(),
            Vec::new(),
            vec![active_wifi("wlan0", "Cafe", Some("AA:AA:AA:AA:AA:02"))],
        );

        let groups = snapshot.wifi_groups();
        let wlan0 = groups
            .iter()
            .find(|group| group.interface == "wlan0")
            .expect("wlan0 group");
        let wlan1 = groups
            .iter()
            .find(|group| group.interface == "wlan1")
            .expect("wlan1 group");

        assert!(wlan0.active);
        assert!(!wlan1.active);
        assert_eq!(wlan0.strongest.bssid, "AA:AA:AA:AA:AA:02");
    }

    #[test]
    fn marks_saved_vpn_active_by_uuid() {
        let snapshot = snapshot(
            Vec::new(),
            Vec::new(),
            vec![saved_vpn(20, "active-vpn"), saved_wireguard(21, "wg-vpn")],
            vec![active_vpn("active-vpn")],
        );

        let vpns = snapshot.saved_vpn_map();

        assert!(vpns["active-vpn"].active);
        assert!(!vpns["wg-vpn"].active);
        assert_eq!(vpns["active-vpn"].kind, Some(VpnKind::Plugin));
        assert_eq!(vpns["wg-vpn"].kind, Some(VpnKind::WireGuard));
    }

    #[test]
    fn hidden_networks_not_shown() {
        let snapshot = snapshot(
            vec![
                hidden_ap("wlan0", "AA:AA:AA:AA:AA:01", 70),
                ap("wlan1", "Cafe", "BB:BB:BB:BB:BB:01", 90),
            ],
            Vec::new(),
            Vec::new(),
            Vec::new(),
        );
        let groups = snapshot.wifi_groups();
        assert_eq!(groups.len(), 1);
    }

    #[test]
    fn hidden_networks_available() {
        let snapshot = snapshot(
            vec![
                hidden_ap("wlan0", "AA:AA:AA:AA:AA:01", 70),
                ap("wlan1", "Cafe", "BB:BB:BB:BB:BB:01", 90),
            ],
            Vec::new(),
            Vec::new(),
            Vec::new(),
        );
        let hidden_access_points = snapshot.hidden_access_points();
        let groups = snapshot.wifi_groups();
        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0].ssid, "Cafe");
        assert_eq!(hidden_access_points.len(), 1);
        assert!(hidden_access_points[0].is_hidden());
        assert_eq!(hidden_access_points[0].ssid, "<Hidden Network>");
    }
}
