# Changelog

See the full changelog on GitHub: [**nmrs** CHANGELOG](https://github.com/freedesktop-rs/nmrs/blob/master/CHANGELOG.md)

## nmrs (Library) Highlights

### 3.1.0

- Loopback device support and a new `DeviceType::Vlan` variant
- VLAN (802.1Q) support: `VlanConfig` model and `build_vlan_connection`
- `RadioState::present` reports whether a radio actually exists on the host
- `airplane_mode_state()` and `set_airplane_mode()` are now correct on
  Wi-Fi-only / Bluetooth-less hosts (BlueZ-missing is treated as a no-op)
- `set_bluetooth_radio_enabled` now waits for the adapter `Powered`
  property to flip before returning

### 3.0.x

- `nmrs::agent` module — register a NetworkManager **secret agent**
  (`SecretAgent`, `SecretAgentBuilder`, `SecretRequest`, `SecretResponder`,
  …) for Wi-Fi/VPN/802.1X credential prompts over D-Bus
- Per-Wi-Fi-device scoping: `WifiDevice`, `list_wifi_devices()`,
  `wifi_device_by_interface()`, and `nm.wifi("wlan1")` → `WifiScope`
- New `interface: Option<&str>` parameter on `connect`, `connect_to_bssid`,
  `disconnect`, `scan_networks`, and `list_networks` (**3.0 break**)
- `set_wifi_enabled(interface, bool)` now toggles a single radio;
  `set_wireless_enabled(bool)` is the global software killswitch
- Airplane-mode surface: `RadioState`, `AirplaneModeState`, `wifi_state`,
  `wwan_state`, `bluetooth_radio_state`, plus rfkill awareness
- Connectivity surface: `connectivity()`, `check_connectivity()`,
  `connectivity_report()`, `captive_portal_url()`, `ConnectivityCheckDisabled`
- Generic VPN model: `VpnType` is now data-carrying (WireGuard, OpenVPN,
  OpenConnect, strongSwan, PPTP, L2TP, Generic). `VpnKind` distinguishes
  plugin VPNs from kernel WireGuard. `VpnConnection` gained `uuid`,
  `active`, `user_name`, `password_flags`, `service_type`. New
  `connect_vpn_by_uuid`, `connect_vpn_by_id`, `disconnect_vpn_by_uuid`,
  `active_vpn_connections`.
- OpenVPN end-to-end: full `OpenVpnConfig` with TLS hardening, ciphers,
  proxy, routes, `redirect_gateway`; `OpenVpnBuilder`; `.ovpn` import
  via `nm.import_ovpn(path, user, pass)`; cert-store handling for inline
  certs.
- Saved profile management: `list_saved_connections{,_brief,_ids}`,
  `get_saved_connection{,_raw}`, `delete_saved_connection`,
  `update_saved_connection`, `reload_saved_connections`,
  `SavedConnection`, `SavedConnectionBrief`, `SettingsSummary`,
  `SettingsPatch`.
- `AccessPoint` model + `list_access_points(interface)` for per-BSSID
  enumeration.
- `ConnectionError::IncompleteBuilder` for builders missing required
  fields; `Builder::build()` returns `Result` instead of panicking.
- Edition 2024, MSRV bumped to 1.90.0 (3.0.1).

### 2.x

- Concurrency protection (`is_connecting()`), `WirelessHardwareEnabled`,
  BDADDR → BlueZ path resolution, mixed-mode WPA1+WPA2 (2.2.0)
- `#[must_use]` annotations on public builder APIs (2.1.0)
- IPv6 address support, `WifiMode` builder, input validation for SSIDs/
  credentials/addresses, idempotent `forget_vpn()` (2.0.1)
- Bluetooth support (PAN and DUN), configurable `TimeoutConfig`,
  `VpnCredentials` / `EapOptions` builder patterns, `ConnectionOptions`,
  `ConnectionBuilder`, `WireGuardBuilder` with validation (2.0.0)

### 1.x

- WireGuard VPN support
- VPN error handling improvements
- Docker image for testing
- Initial release with Wi-Fi and Ethernet support
