# Changelog

All notable changes to the `nmrs` crate will be documented in this file.

## [Unreleased]

### Added

- Isolated NetworkManager integration contracts now cover saved-profile events,
  secret-agent registration, wired DHCP activation, and virtual WPA Wi-Fi
  discovery/authentication/reconnection without touching developer profiles. ([#505](https://github.com/freedesktop-rs/nmrs/pull/505))
- Expanded unit coverage for exact D-Bus settings payloads, activation races,
  monitor lifecycle behavior, secret-agent concurrency, saved-profile decoding,
  validation boundaries, OpenVPN parsing, and certificate storage cleanup. ([#505](https://github.com/freedesktop-rs/nmrs/pull/505))

### Changed

- Network, device, and settings monitors now return only after their initial
  D-Bus subscriptions are installed, so a mutation immediately after startup
  cannot race the subscription task. ([#505](https://github.com/freedesktop-rs/nmrs/pull/505))
- Supplying a non-empty PSK or EAP configuration for an existing Wi-Fi profile
  now applies the fresh credentials; an empty PSK continues to request the
  stored secret. ([#505](https://github.com/freedesktop-rs/nmrs/pull/505))

### Fixed

- Preserve complete OpenVPN, VLAN, WireGuard, Bluetooth, Wi-Fi, and access-point
  settings when constructing or decoding NetworkManager payloads. ([#505](https://github.com/freedesktop-rs/nmrs/pull/505))
- Preserve saved Wi-Fi profiles when stored-secret activation fails, while
  removing newly created profiles whose fresh authentication fails. ([#505](https://github.com/freedesktop-rs/nmrs/pull/505))
- Recheck active-connection state at timeout boundaries and retain typed
  NetworkManager failure reasons instead of reporting false timeouts. ([#505](https://github.com/freedesktop-rs/nmrs/pull/505))
- Keep active-connection snapshots usable when NetworkManager removes an
  enumerated connection object while its properties are being read. ([#505](https://github.com/freedesktop-rs/nmrs/pull/505))
- Retain the discovering interface on inactive Wi-Fi scan results, matching
  the documented `Network::device` contract. ([#505](https://github.com/freedesktop-rs/nmrs/pull/505))
- Wait through NetworkManager's transient `Unavailable` state while a managed
  Wi-Fi radio recovers from a rfkill transition. ([#505](https://github.com/freedesktop-rs/nmrs/pull/505))
- Map secret-agent registration conflicts to the documented typed errors and
  handle concurrent same-key requests, cancellation, closed responders, and
  bounded-queue backpressure without false cancellation or hangs. ([#505](https://github.com/freedesktop-rs/nmrs/pull/505))
- Ignore unmanaged interfaces during automatic device selection and recognize
  NetworkManager veth devices as wired Ethernet for selection and reporting. ([#505](https://github.com/freedesktop-rs/nmrs/pull/505))
- Harden IPv6, WireGuard, OpenVPN, rfkill, and certificate-storage validation,
  decoding, error reporting, and temporary-file cleanup. ([#505](https://github.com/freedesktop-rs/nmrs/pull/505))

## [3.4.0] - 2026-07-08
### Added 
- Expose existing secrets on SecretRequest for re-auth prefill ([#460](https://github.com/freedesktop-rs/nmrs/pull/460))
- `MonitorHandle` returned by `monitor_network_changes` and `monitor_device_changes` for graceful shutdown ([#461](https://github.com/freedesktop-rs/nmrs/pull/461))
- `NetworkManager::dbus_connection()` and `nmrs::raw` (`zbus` / `zvariant` re-exports) for advanced builder workflows ([#462](https://github.com/freedesktop-rs/nmrs/pull/464))
- `NetworkManager::add_connection()` and `NetworkManager::add_and_activate_connection()` for submitting builder output without custom zbus proxies ([#260](https://github.com/freedesktop-rs/nmrs/issues/260), [#465](https://github.com/freedesktop-rs/nmrs/pull/465))
- mdbook docs for builder submission workflow, `add_connection()`, and `add_and_activate_connection()` ([#462](https://github.com/freedesktop-rs/nmrs/pull/464))

### Fixed
- `monitor_network_changes` now detects hotplugged Wi-Fi devices instead of only monitoring devices present at startup ([#461](https://github.com/freedesktop-rs/nmrs/pull/461))
- Monitors return `Ok(())` on clean shutdown instead of always returning `Err(Stuck(...))` ([#461](https://github.com/freedesktop-rs/nmrs/pull/461))

### Changed
- **Breaking:** `monitor_network_changes` and `monitor_device_changes` now return `Result<MonitorHandle>` instead of `Result<()>` ([#461](https://github.com/freedesktop-rs/nmrs/pull/461))

## [3.3.0] - 2026-06-30
### Added

- `Device::speed_mbps` and `NetworkManager::list_wired_device_details()` expose
  Ethernet link speed, active connection id, MAC addresses, state, and IPs for
  wired-device UI rows. ([#454](https://github.com/freedesktop-rs/nmrs/pull/454))
- `NetworkEvent`, `SettingsChange`, `NetworkManager::network_events()`, and
  `NetworkManager::settings_events()` provide stream-based refresh triggers for
  GUI network applets. ([#455](https://github.com/freedesktop-rs/nmrs/pull/455))
- `NetworkSnapshot`, typed `ActiveConnection` models,
  `NetworkManager::snapshot()`, and `NetworkManager::list_active_connections()`
  provide point-in-time state reads for applet refreshes after `NetworkEvent`. ([#456](https://github.com/freedesktop-rs/nmrs/pull/456))
- `NetworkSnapshot::wifi_groups()`, `known_wifi_by_ssid()`, `saved_vpn_map()`,
  and `applet_summary()` derive applet-ready Wi-Fi and VPN rows from snapshot
  data without additional D-Bus calls. ([#458](https://github.com/freedesktop-rs/nmrs/pull/458))
- Secret-agent lifecycle docs now recommend one long-lived applet registration,
  keeping `SecretAgentHandle` alive, and calling
  `SecretAgentHandle::reregister()` after NetworkManager restarts. ([#454](https://github.com/freedesktop-rs/nmrs/pull/454))

## [3.2.2] - 2026-06-30

### Fixed

- `SecretAgent` now registers without owning a policy-controlled system bus
  name and serves the standard NetworkManager secret-agent object path, so
  credential prompts can reach the registered agent ([#451](https://github.com/freedesktop-rs/nmrs/issues/451))

### Fixed

- `SecretAgent` now registers without owning a policy-controlled system bus name and serves the standard NetworkManager secret-agent object path, so credential prompts can reach the registered agent. ([#452](https://github.com/freedesktop-rs/nmrs/pull/452))

## [3.2.1] - 2026-06-16

### Added

- `NetworkManager::get_saved_connection_uuid()` — resolve a profile UUID from `connection.id` (usually the Wi-Fi SSID) for use with `update_saved_connection` ([#442](https://github.com/freedesktop-rs/nmrs/issues/442))
- `DeviceState::is_enabled()`, `Device.frequency`, and `WifiDevice.active_frequency_mhz` expose device usability and active Wi-Fi AP frequency without requiring separate AP lookups ([#445](https://github.com/freedesktop-rs/nmrs/pull/445))

### Fixed

- `NetworkManager::update_saved_connection()` now merges `SettingsPatch` into the full existing settings map before calling NetworkManager `Update`, avoiding missing required fields such as `connection.type` ([#446](https://github.com/freedesktop-rs/nmrs/pull/446))

## [3.2.0] - 2026-05-31

### Added

- Add EAP-TLS support for WPA-Enterprise Wi-Fi, including TLS certificate/key path or blob configuration on `EapOptions` and `EapMethod::Tls` ([#434](https://github.com/freedesktop-rs/nmrs/pull/434))
- Add WPA3-Enterprise 192-bit Wi-Fi support via `WifiSecurity::Wpa3Eap192bit` and `WifiConnectionBuilder::wpa3_eap_192_bit()` ([#434](https://github.com/freedesktop-rs/nmrs/pull/434))
- Add race-free `try_connect` and `try_connect_to_bssid` APIs for `NetworkManager` and `WifiScope`, plus `ConnectionError::ConnectionInProgress` ([#427](https://github.com/freedesktop-rs/nmrs/pull/427))

### Changed

- Send EAP certificate and private-key file paths to NetworkManager as NUL-terminated byte arrays, matching NetworkManager's D-Bus settings format ([#434](https://github.com/freedesktop-rs/nmrs/pull/434))

### Fixed

- Guard connect operations with async mutex to prevent TOCTOU race between `is_connecting()` and `connect()` ([#427](https://github.com/freedesktop-rs/nmrs/pull/427))
- Make `EapOptionsBuilder::build()` reject EAP-TLS configs missing a private key or client certificate ([#436](https://github.com/freedesktop-rs/nmrs/pull/436))
- Switch saved connection lookup to use `validate_connection_name()` instead of `validate_ssid()`, allowing VPN profile names longer than Wi-Fi SSIDs ([#426](https://github.com/freedesktop-rs/nmrs/pull/426))

## [3.1.5] - 2026-05-20

### Fixed

- Replace futures::executor::block_on with .await in VPN active connection map to prevent panic
  when called from an existing async runtime [#423](https://github.com/freedesktop-rs/nmrs/pull/423))

## [3.1.4] - 2026-05-17

### Fixed

- WireGuard builder now sets `service-type` property correctly ([#421](https://github.com/freedesktop-rs/nmrs/pull/421))

## [3.1.3] - 2026-05-14

### Fixed

- Add `process` feature to tokio to fix build error on some systems

(No changes documented)

## [3.1.2] - 2026-05-14

- `set_bluetooth_radio_enabled` now toggles kernel rfkill before BlueZ adapter `Powered`, fixing airplane-mode state desync with rfkill-based consumers ([#417](https://github.com/freedesktop-rs/nmrs/issues/418))

## [3.1.1] - 2026-05-13

### Fixed

- `set_airplane_mode` now treats `BluetoothToggleFailed` as a non-fatal
  warning (like missing BlueZ) when the aggregate airplane toggle has already
  toggled Wi-Fi/WWAN successfully. This avoids reporting total failure after
  those radios were disabled, preventing UI/state divergence where airplane
  mode appears to fail while radios remain disabled. On hosts with no
  Wi-Fi/WWAN device detected, `set_airplane_mode` can still return
  `BluetoothToggleFailed`. ([#417](https://github.com/freedesktop-rs/nmrs/issues/417))

## [3.1.0] - 2026-05-08

- Implement loopback support ([#391](https://github.com/freedesktop-rs/nmrs/issues/391))
- Implement add VLAN (802.1Q) device support with VlanConfig model and connection builder([#392](https://github.com/freedesktop-rs/nmrs/issues/392))

### Added

- `RadioState::present` indicates whether a controllable instance of the radio
  exists on the host. `RadioState::with_presence(enabled, hardware_enabled,
present)` constructor; `RadioState::new` keeps existing behavior and defaults
  `present = true`. ([#396](https://github.com/freedesktop-rs/nmrs/issues/396))

### Fixed

- `NetworkManager::wifi_state` and `wwan_state` now set `RadioState::present`
  from NetworkManager device enumeration (with `present = true` when
  enumeration is incomplete), instead of always reporting present. ([#396](https://github.com/freedesktop-rs/nmrs/pull/396))
- `AirplaneModeState::is_airplane_mode` no longer returns `false` on hosts
  without Bluetooth/WWAN. Radios reported with `present = false` are now
  ignored when computing both `is_airplane_mode` and `any_hardware_killed`. ([#396](https://github.com/freedesktop-rs/nmrs/pull/396))
- `set_airplane_mode` no longer returns `BluezUnavailable` (and therefore no
  longer leaves Wi-Fi soft-killed while reporting failure to the caller) when
  the host has no Bluetooth stack. A missing BlueZ is treated as a successful
  no-op for the Bluetooth leg of the toggle. ([#396](https://github.com/freedesktop-rs/nmrs/pull/396))
- `set_bluetooth_radio_enabled` waits up to 2s overall for adapters' `Powered`
  property to actually flip before returning, so a read-after-write of
  `airplane_mode_state()` no longer observes the pre-toggle Bluetooth state
  and concludes that airplane mode failed to engage. ([#396](https://github.com/freedesktop-rs/nmrs/pull/396))

## [3.0.1] - 2026-04-25

### Changed

- Lower MSRV from 1.94.0 to 1.90.0

## [3.0.0] - 2026-04-24

### Added

- `ConnectionError::IncompleteBuilder` for builders missing required fields ([#350](https://github.com/freedesktop-rs/nmrs/issues/350))
- `nmrs::agent` module: NetworkManager secret agent for credential prompting over D-Bus (`SecretAgent`, `SecretAgentBuilder`, `SecretAgentHandle`, `SecretRequest`, `SecretResponder`, `SecretSetting`, `SecretAgentFlags`, `SecretAgentCapabilities`, `CancelReason`, `SecretStoreEvent`) ([#370](https://github.com/freedesktop-rs/nmrs/pull/370))
- `AccessPoint` model preserving per-AP BSSID, frequency, security flags, and device state; `list_access_points(interface)` for full AP enumeration ([#373](https://github.com/freedesktop-rs/nmrs/pull/373))
- Airplane-mode surface: `RadioState`, `AirplaneModeState`, `wifi_state()`, `wwan_state()`, `bluetooth_radio_state()`, `airplane_mode_state()`, `set_wireless_enabled()`, `set_wwan_enabled()`, `set_bluetooth_radio_enabled()`, `set_airplane_mode()` ([#372](https://github.com/freedesktop-rs/nmrs/pull/372))
- Kernel rfkill awareness: hardware kill switch state via `/sys/class/rfkill` ([#372](https://github.com/freedesktop-rs/nmrs/pull/372))
- `HardwareRadioKilled` and `BluezUnavailable` error variants ([#372](https://github.com/freedesktop-rs/nmrs/pull/372))
- Per-Wi-Fi-device scoping: `WifiDevice` model, `list_wifi_devices()`, `wifi_device_by_interface()`, `WifiScope` builder via `nm.wifi("wlan1")`, `set_wifi_enabled(interface, bool)` for per-radio enable/disable ([#375](https://github.com/freedesktop-rs/nmrs/pull/375))
- `WifiInterfaceNotFound` and `NotAWifiDevice` error variants ([#375](https://github.com/freedesktop-rs/nmrs/pull/375))
- Saved profile enumeration: `SavedConnection`, `SavedConnectionBrief`, `SettingsSummary`, `SettingsPatch`, `WifiSecuritySummary`, `WifiKeyMgmt`, `VpnSecretFlags`; `list_saved_connections()`, `list_saved_connections_brief()`, `list_saved_connection_ids()`, `get_saved_connection()`, `get_saved_connection_raw()`, `delete_saved_connection()`, `update_saved_connection()`, `reload_saved_connections()`; D-Bus proxies `NMSettingsProxy` / `NMSettingsConnectionProxy`; example `saved_list` ([#376](https://github.com/freedesktop-rs/nmrs/pull/376))
- Connectivity state surface: `ConnectivityState`, `ConnectivityReport`, `connectivity()`, `check_connectivity()`, `connectivity_report()`, `captive_portal_url()`; `ConnectivityCheckDisabled` error variant ([#377](https://github.com/freedesktop-rs/nmrs/pull/377))
- Generic VPN support: `VpnType` now carries protocol-specific metadata for OpenVPN, OpenConnect, strongSwan, PPTP, L2TP, and a `Generic` catch-all; `VpnKind` (Plugin vs WireGuard); `VpnConnection` enriched with `uuid`, `active`, `user_name`, `password_flags`, `service_type`; `connect_vpn_by_uuid()`, `connect_vpn_by_id()`, `disconnect_vpn_by_uuid()`, `active_vpn_connections()` ([#378](https://github.com/freedesktop-rs/nmrs/pull/378))

### Changed

- `VpnCredentialsBuilder::build()` and `EapOptionsBuilder::build()` return `Result` (no panics on missing fields); `VpnCredentialsBuilder` may return `ConnectionError::InvalidPeers` when no peers are set ([#350](https://github.com/freedesktop-rs/nmrs/issues/350))
- `VpnType` is now a data-carrying enum; the old tag enum is renamed to `VpnKind`. `VpnConfig::vpn_type()` renamed to `vpn_kind()`. `VpnConnectionInfo.vpn_type` renamed to `vpn_kind`. ([#378](https://github.com/freedesktop-rs/nmrs/pull/378))
- `list_saved_connections()` now returns `Vec<SavedConnection>` (full decode + summaries). Use `list_saved_connection_ids()` for the previous `Vec<String>` behavior (connection `id` names only). ([#376](https://github.com/freedesktop-rs/nmrs/pull/376))
- `connect`, `connect_to_bssid`, `disconnect`, `scan_networks`, and `list_networks` now take an `interface: Option<&str>` parameter. Pass `None` to preserve previous behavior, or `Some("wlan1")` to scope to a specific Wi-Fi interface. For an ergonomic per-interface API, use `nm.wifi("wlan1")` to obtain a `WifiScope`. ([#375](https://github.com/freedesktop-rs/nmrs/pull/375))
- `set_wifi_enabled` now requires an `interface: &str` argument and toggles only that radio (via `Device.Autoconnect` + `Device.Disconnect()`). For the global wireless killswitch use `set_wireless_enabled(bool)`. ([#375](https://github.com/freedesktop-rs/nmrs/pull/375))
- `VpnConfig` trait and `WireGuardConfig`; `NetworkManager::connect_vpn` accepts `VpnConfig` implementors; `VpnCredentials` deprecated with compatibility bridges ([#303](https://github.com/freedesktop-rs/nmrs/pull/303))
- Introduce `VpnConfig` trait and refactor `connect_vpn` signature ([#303](https://github.com/freedesktop-rs/nmrs/pull/303))
- OpenVPN connection settings model expansion ([#309](https://github.com/freedesktop-rs/nmrs/pull/309))
- Multi-VPN plumbing: `detect_vpn_type()`, `VpnType::OpenVpn`, and shared detection across connect, disconnect, and list VPN flows ([#311](https://github.com/freedesktop-rs/nmrs/pull/311))
- `.ovpn` profile lexer/parser and auth-user-pass inference for translating OpenVPN configs toward NetworkManager ([#314](https://github.com/freedesktop-rs/nmrs/pull/314), [#340](https://github.com/freedesktop-rs/nmrs/pull/340))
- Unit tests and parser refactors for `.ovpn` parsing ([#316](https://github.com/freedesktop-rs/nmrs/pull/316))
- OpenVPN builder, validation, compression, proxy, routing, resilience, TLS hardening, import, cert-store, and `VpnDetails` support ([#315](https://github.com/freedesktop-rs/nmrs/pull/315), [#323](https://github.com/freedesktop-rs/nmrs/pull/323), [#326](https://github.com/freedesktop-rs/nmrs/pull/326), [#345](https://github.com/freedesktop-rs/nmrs/pull/345), [#346](https://github.com/freedesktop-rs/nmrs/pull/346), [#347](https://github.com/freedesktop-rs/nmrs/pull/347), [#348](https://github.com/freedesktop-rs/nmrs/pull/348), [#349](https://github.com/freedesktop-rs/nmrs/pull/349))
- `VpnConfiguration` to dispatch WireGuard vs OpenVPN; `connect_vpn` wired to the OpenVPN builder ([#322](https://github.com/freedesktop-rs/nmrs/pull/322))
- Support for specifying Bluetooth adapter in `BluetoothIdentity` ([#267](https://github.com/freedesktop-rs/nmrs/pull/267))

### Fixed

- Wi-Fi `ensure_disconnected` no longer deactivates every active connection (VPN, wired, other radios); only the target Wi-Fi device is torn down. VPN disconnect, Wi-Fi/Bluetooth `Device::Disconnect` D-Bus failures propagate instead of being swallowed ([#351](https://github.com/freedesktop-rs/nmrs/issues/351))
- OpenVPN settings decoding now uses D-Bus `Dict` values for `vpn.data` / `vpn.secrets` and extracts gateways from `vpn.data` correctly ([#337](https://github.com/freedesktop-rs/nmrs/pull/337), [#344](https://github.com/freedesktop-rs/nmrs/pull/344))
- Line-accurate source locations for `.ovpn` directives and blocks ([#318](https://github.com/freedesktop-rs/nmrs/pull/318))
- `key_direction` when nested under `tls_auth` and as a standalone directive ([#320](https://github.com/freedesktop-rs/nmrs/pull/320))

## [2.4.0] - 2026-04-24

### Fixed

- `list_networks` fills `device`, `ip4_address`, and `ip6_address` for the access point currently in use on each Wi-Fi interface ([#368](https://github.com/freedesktop-rs/nmrs/pull/368))
- `monitor_network_changes` now fires for Wi-Fi access point signal strength changes, not only access point additions and removals ([#367](https://github.com/freedesktop-rs/nmrs/pull/367))
- Add `Send` bound to monitoring stream trait objects so `monitor_network_changes` and `monitor_device_changes` work with `tokio::spawn` ([#359](https://github.com/freedesktop-rs/nmrs/pull/359))

## [2.3.0] - 2026-04-10

### Added

- `is_hotspot` method for networks in AP mode 3 ([#324](https://github.com/freedesktop-rs/nmrs/pull/324))

### Fixed

- Add `Send` bound to `for_each_access_point` callback future ([#330](https://github.com/freedesktop-rs/nmrs/pull/330))
- Removed stale `nmrs-aur` submodule gitlink ([#331](https://github.com/freedesktop-rs/nmrs/pull/331))

## [2.2.0] - 2026-03-17

### Added

- Concurrency protection ([#268](https://github.com/freedesktop-rs/nmrs/pull/268))
- Expose `WirelessHardwareEnabled` in API to reflect rkfill state ([#284](https://github.com/freedesktop-rs/nmrs/pull/284))

### Changed

- Convert BDADDR to BlueZ device path via `bluez_device_path` helper ([#266](https://github.com/freedesktop-rs/nmrs/pull/266))

### Fixed

- Let NetworkManager negotiate mixed-mode (WPA1+WPA2) security ([#271](https://github.com/freedesktop-rs/nmrs/pull/271))

## [2.1.0] - 2026-02-28

### Added

- `#[must_use]` attributes across public API: constructors, builder methods, and pure functions ([#220](https://github.com/freedesktop-rs/nmrs/issues/220))

## [2.0.1] - 2026-02-25

### Changed

- Completed IPv6 support ([#208](https://github.com/freedesktop-rs/nmrs/pull/208))
- Replace magic number with named constant for device states ([#230](https://github.com/freedesktop-rs/nmrs/pull/230))
- Replaced hardcoded root paths with `Default` impl ([#224](https://github.com/freedesktop-rs/nmrs/pull/224))
- Add context to D-Bus operation errors ([#240](https://github.com/freedesktop-rs/nmrs/pull/240))
- Replace `println!` with `debug!` ([#234](https://github.com/freedesktop-rs/nmrs/pull/234))
- Idempotence enforcement for `forget_vpn()` ([#232](https://github.com/freedesktop-rs/nmrs/pull/232))

### Added

- validate bluetooth address in `populate_bluez_info` & `BluetoothIdentity::new` ([#215](https://github.com/freedesktop-rs/nmrs/issues/215))
- `WifiMode` enum for WiFi connection mode ([#263](https://github.com/freedesktop-rs/nmrs/issues/263))

## [2.0.0] - 2026-01-19

### Added

- Configurable timeout values for connection and disconnection operations ([#185](https://github.com/freedesktop-rs/nmrs/issues/185))
- Builder pattern for `VpnCredentials` and `EapOptions` ([#188](https://github.com/freedesktop-rs/nmrs/issues/188))
- Bluetooth device support ([#198](https://github.com/freedesktop-rs/nmrs/pull/198))
- Input validation before any D-Bus operations ([#173](https://github.com/freedesktop-rs/nmrs/pull/173))
  ~~- CI: adjust workflow to auto-update nix hashes on PRs ([#182](https://github.com/freedesktop-rs/nmrs/pull/182))~~
- More helpful methods to `network_manager` facade ([#190](https://github.com/freedesktop-rs/nmrs/pull/190))
- Explicitly clean up signal streams to ensure unsubscription ([#197](https://github.com/freedesktop-rs/nmrs/pull/197))

### Fixed

- Better error message for empty passkeys ([#198](https://github.com/freedesktop-rs/nmrs/pull/198))
- Race condition in signal subscription ([#191](https://github.com/freedesktop-rs/nmrs/pull/191))

### Changed

- Various enums and structs marked non-exhaustive ([#198](https://github.com/freedesktop-rs/nmrs/pull/198))
- Expose `NMWiredProxy` and propogate speed through + write in field and display for BT device type ([#198](https://github.com/freedesktop-rs/nmrs/pull/198))

## [1.3.5] - 2026-01-13

### Changed

- Add `Debug` derive to `NetworkManager` ([#171](https://github.com/freedesktop-rs/nmrs/pull/171))

## [1.3.0] - 2026-01-12

### Changed

- Dedupe DBus proxy construction across connection logic ([#165](https://github.com/freedesktop-rs/nmrs/pull/165))
- Added contextual logging throughout VPN, connection, and device operations to preserve error context and improve debugging capabilities ([#168](https://github.com/freedesktop-rs/nmrs/pull/168))

### Fixed

- VPN operations no longer silently swallow D-Bus errors - now log warnings when proxy creation or method calls fail ([#168](https://github.com/freedesktop-rs/nmrs/pull/168))
- Connection cleanup operations (disconnect, deactivate, delete) now log failures instead of ignoring them ([#168](https://github.com/freedesktop-rs/nmrs/pull/168))
- VPN error mapping now attempts to extract actual connection state reasons instead of defaulting to `Unknown` ([#168](http://github.com/freedesktop-rs/nmrs/pull/168))
- MAC address retrieval errors are now logged with appropriate context ([#168](https://github.com/freedesktop-rs/nmrs/pull/168))
- Access point property retrieval failures are now logged for better diagnostics ([#168](https://github.com/freedesktop-rs/nmrs/pull/168))

## [1.2.0] - 2026-01-05

### Added

- Docker image for reproducing testing/dev environment ([#159](https://github.com/freedesktop-rs/nmrs/pull/159))

### Fixed

- Change `decode_ssid_or_empty` to return a borrowed slice instead of `String` ([#154](https://github.com/freedesktop-rs/nmrs/pull/154))

### Changed

- Condense device finding logic under one helper: `find_device_by_type` ([#158](https://github.com/freedesktop-rs/nmrs/pull/158))

## [1.1.0] - 2025-12-19

### Fixed

- Native WireGuard profile structure ([#135](https://github.com/freedesktop-rs/nmrs/issues/135))

### Added

- Added WireGuard connection example to docs ([#137](https://github.com/freedesktop-rs/nmrs/pull/137))

## [1.0.1] - 2025-12-15

### Changed

- Update docs for various structs, enums and functions ([#132](https://github.com/freedesktop-rs/nmrs/pull/132))

## [1.0.0] - 2025-12-15

### Added

- Full WireGuard VPN support ([#92](https://github.com/freedesktop-rs/nmrs/issues/92))

## [0.5.0-beta] - 2025-12-15

### Changed

- Refactored connection monitoring from polling to event-driven D-Bus signals for faster response times and lower CPU usage ([#46](https://github.com/freedesktop-rs/nmrs/issues/46))
- Replaced `tokio` with `futures-timer` for runtime-agnostic async support (fixes GTK/glib compatibility)

### Added

- `ActiveConnectionState` and `ConnectionStateReason` enums for detailed connection status tracking ([#46](https://github.com/freedesktop-rs/nmrs/issues/46))
- `monitor_network_changes()` API for real-time network list updates via D-Bus signals
- `NetworkManager` is now `Clone`
- Full support for Ethernet devices ([#88](https://github.com/freedesktop-rs/nmrs/issues/88))

### Fixed

- `forget()` now verifies device is disconnected before deleting saved connections ([#124](https://github.com/freedesktop-rs/nmrs/issues/124))
- `list_networks()` preserves security flags when deduplicating APs ([#123](https://github.com/freedesktop-rs/nmrs/issues/123))
- Fixed race condition in signal subscription where rapid state changes could be missed

## [0.4.0-beta] - 2025-12-11

### Breaking Changes

- Expanded `ConnectionError` enum with new variants (`AuthFailed`, `SupplicantConfigFailed`, `SupplicantTimeout`, `DhcpFailed`, `Timeout`, `Stuck`, `NoWifiDevice`, `WifiNotReady`, `NoSavedConnection`, `Failed(StateReason)`) - exhaustive matches will need a wildcard ([#82](https://github.com/freedesktop-rs/nmrs/issues/82))
- Return types changed from `zbus::Result<T>` to `Result<T, ConnectionError>` for structured error handling
- Renamed crate from `nmrs-core` to `nmrs`

### Added

- `StateReason` enum and `reason_to_error()` for mapping NetworkManager failure codes to typed errors ([#82](https://github.com/freedesktop-rs/nmrs/issues/82), [#85](https://github.com/freedesktop-rs/nmrs/issues/85))
- Comprehensive documentation across all modules ([#82](https://github.com/freedesktop-rs/nmrs/issues/82))
- Logging support via `log` crate facade ([#87](https://github.com/freedesktop-rs/nmrs/issues/87))

### Changed

- Decomposed `connect()` into smaller helper functions ([#81](https://github.com/freedesktop-rs/nmrs/issues/81))
- Extracted disconnect + wait logic to unified helper ([#79](https://github.com/freedesktop-rs/nmrs/issues/79))
- Unified state polling logic ([#80](https://github.com/freedesktop-rs/nmrs/issues/80))
- Eliminated network lookup duplication via shared helper function ([#83](https://github.com/freedesktop-rs/nmrs/issues/83))
- Replaced `eprintln!` with structured logging (`debug!`, `info!`, `warn!`, `error!`) ([#87](https://github.com/freedesktop-rs/nmrs/issues/87))

### Fixed

- Auth error mapping now properly distinguishes supplicant failures, DHCP errors, and timeouts ([#82](https://github.com/freedesktop-rs/nmrs/issues/82), [#85](https://github.com/freedesktop-rs/nmrs/issues/85), [#116](https://github.com/freedesktop-rs/nmrs/issues/116))
- `bitrate` property now fetches real connection speeds ([#110](https://github.com/freedesktop-rs/nmrs/issues/110))

## [0.3.0-beta] - 2025-12-08

### Added

- CI: Automated release workflow ([#105](https://github.com/freedesktop-rs/nmrs/pull/105))

## [0.2.0-beta] - 2025-12-03

### Added

- CI: Nix derivation test ([#57](https://github.com/freedesktop-rs/nmrs/pull/57))
- Prevent multiple instances from running by introducing a file lock ([#65](https://github.com/freedesktop-rs/nmrs/pull/65))
- CI+tests: Cross platform builds, API testing, unit testing and integration testing ([#95](https://github.com/freedesktop-rs/nmrs/pull/96))
- Minor refactors (see issue #77) - ([#91](https://github.com/freedesktop-rs/nmrs/pull/91))

## [0.1.1-beta] - 2025-11-21

### Added

- Added GNOME/GTK dependencies to `flake.nix` for NixOS development ([#53](https://github.com/freedesktop-rs/nmrs/pull/53))

## [0.1.0-beta] - 2025-11-20

### Added

- Initial BETA release of nmrs core library
- WPA/WPA2 network connection support
- EAP connections (initial support)
- Ability to forget previously saved networks
- Authentication-failure handling
- DBus proxy that subscribes directly to NetworkManager signals
- Nix flake for reproducible development environment ([#47](https://github.com/freedesktop-rs/nmrs/pull/47))
- Initial model/builder tests ([#48](https://github.com/freedesktop-rs/nmrs/pull/48))

### Fixed

- Network connection failure states with better error handling ([#52](https://github.com/freedesktop-rs/nmrs/pull/52))
- Network deduplication
- DBus API mismatches ([#49](https://github.com/freedesktop-rs/nmrs/pull/49))
- Saved connections handling

### Known Issues

- EAP connections default to no certificates (advanced certificate management coming in future releases)

[1.2.0]: https://github.com/freedesktop-rs/nmrs/compare/nmrs-v1.1.0...nmrs-v1.2.0
[1.3.0]: https://github.com/freedesktop-rs/nmrs/compare/nmrs-v1.2.0...nmrs-v1.3.0
[1.3.5]: https://github.com/freedesktop-rs/nmrs/compare/nmrs-v1.3.0...nmrs-v1.3.5
[2.0.0]: https://github.com/freedesktop-rs/nmrs/compare/nmrs-v1.3.5...nmrs-v2.0.0
[2.0.1]: https://github.com/freedesktop-rs/nmrs/compare/nmrs-v2.0.0...nmrs-v2.0.1
[2.2.0]: https://github.com/freedesktop-rs/nmrs/compare/nmrs-v2.0.1...nmrs-v2.2.0
[2.3.0]: https://github.com/freedesktop-rs/nmrs/compare/nmrs-v2.2.0...nmrs-v2.3.0
[2.4.0]: https://github.com/freedesktop-rs/nmrs/compare/nmrs-v2.3.0...nmrs-v2.4.0
[3.0.0]: https://github.com/freedesktop-rs/nmrs/compare/nmrs-v2.4.0...nmrs-v3.0.0
[3.0.1]: https://github.com/freedesktop-rs/nmrs/compare/nmrs-v3.0.0...nmrs-v3.0.1
[3.1.0]: https://github.com/freedesktop-rs/nmrs/compare/nmrs-v3.0.1...nmrs-v3.1.0
[3.1.1]: https://github.com/freedesktop-rs/nmrs/compare/nmrs-v3.1.0...nmrs-v3.1.1
[3.1.2]: https://github.com/freedesktop-rs/nmrs/compare/nmrs-v1.2.0...nmrs-v3.1.2
[3.1.3]: https://github.com/freedesktop-rs/nmrs/compare/nmrs-v1.2.0...nmrs-v3.1.3
[3.1.4]: https://github.com/freedesktop-rs/nmrs/compare/nmrs-v1.2.0...nmrs-v3.1.4
[3.1.5]: https://github.com/freedesktop-rs/nmrs/compare/nmrs-v1.2.0...nmrs-v3.1.5
[3.2.0]: https://github.com/freedesktop-rs/nmrs/compare/nmrs-v1.2.0...nmrs-v3.2.0
[3.2.1]: https://github.com/freedesktop-rs/nmrs/compare/nmrs-v1.2.0...nmrs-v3.2.1
[3.2.2]: https://github.com/freedesktop-rs/nmrs/compare/nmrs-v1.2.0...nmrs-v3.2.2
[3.3.0]: https://github.com/freedesktop-rs/nmrs/compare/nmrs-v1.2.0...nmrs-v3.3.0
[3.4.0]: https://github.com/freedesktop-rs/nmrs/compare/nmrs-v1.2.0...nmrs-v3.4.0
[Unreleased]: https://github.com/freedesktop-rs/nmrs/compare/nmrs-v3.4.0...HEAD
[1.1.0]: https://github.com/freedesktop-rs/nmrs/compare/nmrs-v1.0.1...nmrs-v1.1.0
[1.0.1]: https://github.com/freedesktop-rs/nmrs/compare/nmrs-v1.0.0...nmrs-v1.0.1
[1.0.0]: https://github.com/freedesktop-rs/nmrs/compare/v0.5.0-beta...nmrs-v1.0.0
[0.5.0-beta]: https://github.com/freedesktop-rs/nmrs/compare/v0.4.0-beta...v0.5.0-beta
[0.4.0-beta]: https://github.com/freedesktop-rs/nmrs/compare/v0.3.0-beta...v0.4.0-beta
[0.3.0-beta]: https://github.com/freedesktop-rs/nmrs/compare/v0.2.0-beta...v0.3.0-beta
[0.2.0-beta]: https://github.com/freedesktop-rs/nmrs/compare/v0.1.1-beta...v0.2.0-beta
[0.1.1-beta]: https://github.com/freedesktop-rs/nmrs/compare/v0.1.0-beta...v0.1.1-beta
[0.1.0-beta]: https://github.com/freedesktop-rs/nmrs/releases/tag/v0.1.0-beta
