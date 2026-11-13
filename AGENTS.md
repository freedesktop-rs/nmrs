# AGENTS.md

Context for AI agents working in this repository.

## Project overview

`nmrs` is a Rust library for managing network connections through
NetworkManager over D-Bus. It is a library crate only; nothing here is a
binary. Consumers include `nmrs-gui` and `nmrs-cli`.

## Layout

```
src/
  lib.rs                — public API surface; re-exports from api/
  api/
    network_manager.rs  — NetworkManager struct, the single entry point
    wifi_scope.rs       — per-interface Wi-Fi handle (NetworkManager::wifi_device_by_interface)
    models/             — public types, enums, ConnectionError, traits
    builders/           — connection settings constructors (wifi, vpn, wireguard, openvpn, bluetooth, vlan)
  agent/                — secret agent: D-Bus object server, request routing, timeouts
  core/                 — orchestration of D-Bus calls (connect, scan, state waiting, settings patching, .ovpn parser)
  dbus/                 — zbus proxy definitions; one file per NM interface
  monitoring/           — D-Bus signal subscriptions turned into event streams
  types/                — constants, timeouts, device type registry
  util/                 — validation, cert store, settings helpers
tests/integration_test.rs — environmental tests, all #[ignore]
scripts/ci/             — isolated NetworkManager harness used by docker compose and CI
docs/                   — mdBook user guide, deployed by docs.yml
```

`core`, `dbus`, `monitoring`, `types`, `util`, and `agent` internals are
not public API. Only what `lib.rs` re-exports is.

## Commands

These match CI exactly. Everything runs with `--locked`; a dependency
change must include `Cargo.lock`.

```bash
cargo fmt --all -- --check
cargo clippy --locked --all-targets --all-features -- -D warnings
cargo test --locked --lib --all-features --workspace
cargo test --locked --doc --all-features --workspace
cargo semver-checks check-release --release-type minor --baseline-rev "$(git describe --tags --abbrev=0 --match 'nmrs-v*')"
RUSTDOCFLAGS="-D warnings" cargo doc --locked --no-deps --all-features
cargo +<rust-version> check --locked
```

Unit and doc tests never touch NetworkManager. Integration tests are
`#[ignore]` and opt in through capability variables
(`NMRS_REQUIRE_NETWORKMANAGER`, `NMRS_REQUIRE_WIRED`, `NMRS_REQUIRE_WIFI`,
`NMRS_REQUIRE_WIFI_SAE`). Once a capability is set, a missing daemon,
device, or access point fails the test; it never skips. Do not add tests
that return success when the thing they need is absent.

```bash
docker compose run --build --rm test-integration        # private D-Bus + NM, veth DHCP, WireGuard, secret agent
sudo modprobe mac80211_hwsim radios=3
docker compose run --build --rm test-wifi-integration   # WPA2 AP, SAE-only AP, one client radio
sudo modprobe -r mac80211_hwsim
```

Against a deliberately chosen local NetworkManager (mutates its profiles):

```bash
NMRS_REQUIRE_NETWORKMANAGER=1 cargo test --test integration_test --all-features networkmanager_ -- --ignored --test-threads=1
```

## Toolchain

- Edition 2024, stable Rust. `rust-version` in `Cargo.toml` is the MSRV;
  the Dockerfile pin and CI `stable` are not.
- `[lints]` in `Cargo.toml`: `unused = "warn"`; clippy allows
  `too_many_arguments` and `type_complexity`. Do not add `#[allow]` in code
  for lints already handled there.

## Code conventions

- Every public fallible operation returns `nmrs::Result<T>`
  (`Result<T, ConnectionError>`). Use existing variants before adding one;
  wrap raw zbus failures in `ConnectionError::DbusOperation { context, source }`
  with a context string that names the D-Bus call and object.
- No `unwrap()`, `expect()`, or `panic!` in library code. `unwrap_or_else`
  is fine when the fallback is a real value.
- Public structs and enums are `#[non_exhaustive]`. Config structs use
  `with_*` builders returning `Self` with `#[must_use]`.
- Every public item has a doc comment. Examples use `nmrs::` paths and
  must compile under `cargo test --doc`.
- Timeouts are named constants: `types::constants::timeouts` for shared
  ones, a module-level `const` for ones used in a single file
  (`CONNECTION_TIMEOUT`, `BLUEZ_POWER_SETTLE_TIMEOUT`). No `Duration`
  literals at call sites.
- Unit tests live in `#[cfg(test)] mod tests` next to the code or in
  `api/models/tests.rs`. Assert observable behavior through the public
  API, not which internal function was called.
- A bug fix includes a test that fails without the fix.

## NetworkManager semantics

These are the facts that most past bugs have come from. Do not simplify
code that encodes them.

- **Activation failure reasons live on the device, not the connection.**
  `ActiveConnection.StateChanged` only reports `Deactivated` with reason
  `DeviceDisconnected`. The real cause (wrong PSK, SSID not found, DHCP
  timeout) is the reason on `Device.StateChanged` as it enters `Failed`.
  `core/state_wait.rs` watches both; `api/models/state_reason.rs` maps
  reasons to `ConnectionError`.
- **`GetSettings` never returns secrets.** Secrets come from `GetSecrets`
  and are gated by `*-flags` (`psk-flags`, `password-flags`): `0` NM
  stores it, `1` an agent owns it and NM asks every activation, `2` not
  saved, `4` not required. Builders write `psk-flags = 0`.
- **`Settings.Connection.Update` replaces the whole `a{sa{sv}}`.**
  `update_saved_connection` does `GetSettings`, merges the
  `SettingsPatch` delta, then `Update` (or `UpdateUnsaved` when the
  profile's `Unsaved` property is true). A merge can only add or replace
  keys; removing one needs a dedicated code path (#593).
- **SSIDs are `ay`, not strings.** `AccessPoint::ssid_bytes` is
  authoritative; the `String` form is lossy display only.
- **`RequestScan` is fire-and-forget.** Results arrive through
  `LastScan` and `AccessPointAdded/Removed`. `scan_networks` skips
  `Unmanaged` and `Unavailable` devices unless an interface is named.
- **Two radio bits.** Software-enabled (`WirelessEnabled`, writable) and
  hardware-enabled (rfkill, read-only). A radio is usable only when both
  are set; `HardwareRadioKilled` is the error for the second.
- **Aggregate radio operations tolerate Bluetooth, conditionally.**
  `set_airplane_mode` toggles Wi-Fi, WWAN, and BlueZ concurrently.
  `BluezUnavailable` is always swallowed. `BluetoothToggleFailed` is
  swallowed only when a Wi-Fi or WWAN device exists, because those radios
  have already been switched and failing would misreport their state; on
  a Bluetooth-only host it is returned. See
  `finalize_airplane_toggle_results` in `core/airplane.rs`.
- **BlueZ `Powered` settles asynchronously.** Wait for the property to
  reach the target with the settle timeout before reading it back.
- **Secret agent.** NM's own `GetSecrets` timeout is 120 s; the agent
  auto-replies `NoSecrets` at that bound. `unregister()` must remove the
  object from the object server even when NM's `Unregister` fails, or the
  path can never be reused in the process.
- **Proxies borrow the `Connection`.** Build them where needed from
  `&Connection`; do not store them in long-lived structs. Signal streams
  are dropped when the wait ends.
- **`.ovpn` import** follows OpenVPN's parser: backslash escapes in
  double-quoted and unquoted arguments, single quotes literal, inline
  `<cert>` blocks stored through `util/cert_store`. Keys written to
  `vpn.data` must be ones NetworkManager-openvpn accepts; it rejects
  unknown keys at activation, not at save.

## CI

`ci.yml`: format, clippy, lib + doc tests, a library check on the MSRV,
rustdoc with warnings denied, and `cargo-semver-checks` on every push and
PR. No job installs system packages: the dependency tree is pure Rust. The
`integration` job runs the two Docker harnesses on a self-hosted runner; for
pull requests it waits on the `self-hosted-pr-integration` environment
approval, so a pending integration job on a fork PR is not a failure.

`review.yml` is the review bot. It runs when the `mel` label is added or
when the maintainer mentions `@mel-the-caddie` on a PR. Do not try to
trigger it another way.

## Changelog and commits

Commits: `type(#issue): description`, one logical change each.

`CHANGELOG.md` is Keep a Changelog. Entries go under `[Unreleased]` in
`Added`, `Changed`, or `Fixed`, and cite the pull request in parentheses,
never the issue. A breaking change is marked `**Breaking:**` in its
entry; `scripts/bump_version.py` refuses a minor or patch bump while one
is present.

## Releasing

On a branch:

```bash
python3 scripts/bump_version.py X.Y.Z stable
git commit -am "chore(nmrs): prepare X.Y.Z release"
```

Open a PR to master. After it merges:

```bash
git tag nmrs-vX.Y.Z
git push origin nmrs-vX.Y.Z
```

`publish-lib.yml` verifies the tag matches `Cargo.toml`, publishes to
crates.io, and creates the GitHub release.

## Reviewing

Blocking: data loss, wrong behavior on the error path, a swallowed D-Bus
error, a race in a state wait, a behavior change in `core/` with no test.
Not blocking: style, naming, formatting. For the full checklist see
`.github/instructions/`.

## Deprecated

`VpnCredentials` is kept for compatibility. New WireGuard code uses
`WireGuardConfig`.
