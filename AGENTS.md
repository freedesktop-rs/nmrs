# AGENTS.md

Context for AI agents working in this repository.

## Project overview

`nmrs` is a Rust library for managing network connections via NetworkManager over D-Bus.

## Architecture

```
nmrs/src/
  lib.rs                — public API surface, re-exports from api/
  api/
    network_manager.rs  — main entry point (NetworkManager struct)
    models/             — public types, enums, errors, traits
    builders/           — connection settings constructors (wifi, vpn, bluetooth)
  core/                 — internal business logic (connection, scanning, VPN, state waiting)
  dbus/                 — raw D-Bus proxy calls (NM, devices, access points)
  monitoring/           — real-time D-Bus signal subscriptions
  types/                — constants, device type registry
  util/                 — validation, cert handling, helpers
```

`lib.rs` re-exports commonly used types at the crate root for convenience.
Internal modules (`core`, `dbus`, `monitoring`, `types`, `util`) are not part of the public API.

## Build and test

Library and documentation tests do not require NetworkManager. Environmental
tests must use the isolated Docker harness or an explicit opt-in.

```bash
cargo check                                               # quick compile check
cargo fmt --all -- --check                                # formatting (default rustfmt, no config file)
cargo clippy --all-targets --all-features -- -D warnings  # lints (warnings are errors in CI)
cargo test -p nmrs --lib --all-features                   # unit tests only
cargo test --doc --all-features --workspace               # doc tests
cargo test --all-features --workspace                     # unit/docs; environmental tests stay ignored
docker compose run --build --rm test-integration          # isolated NM settings/agent/WireGuard/wired lifecycle
docker compose run --build --rm test-wifi-integration     # CI-equivalent virtual WiFi tests (Linux)
```

Integration tests are `#[ignore]` so normal test commands never touch the host
NetworkManager. The WiFi harness requires two `mac80211_hwsim` radios:
```bash
sudo modprobe mac80211_hwsim radios=2
docker compose run --build --rm test-wifi-integration
sudo modprobe -r mac80211_hwsim
```

For a deliberately selected local NetworkManager, the NM-only opt-in is:
```bash
NMRS_REQUIRE_NETWORKMANAGER=1 \
  cargo test --test integration_test --all-features \
  networkmanager_ -- --ignored --test-threads=1
```

Those tests create, update, and delete saved profiles; exercise a real
NetworkManager-to-agent secret exchange; and activate a native WireGuard
connection. The Docker harness additionally creates an isolated veth pair and
validates wired DHCP activation. Once a capability flag is set, missing
facilities, timeouts, and unexpected errors must fail rather than skip.

## Toolchain

- Edition 2024, resolver 3, stable Rust (MSRV: 1.90.0)
- Workspace lints in root `Cargo.toml`: `unused = "warn"`, clippy allows `too_many_arguments` and `type_complexity`
- CI runs: format, clippy, lib tests, doc tests, semver-checks (`cargo-semver-checks` for `nmrs`), cross-compile for aarch64

## Code conventions

- **Error handling**: all public fallible operations return `nmrs::Result<T>` (alias for `Result<T, ConnectionError>`). Use `ConnectionError` variants, not raw strings.
- **Builder pattern**: config structs use `with_*` builder methods returning `Self` with `#[must_use]`. See `WireGuardConfig`, `OpenVpnConfig`, `EapOptions`.
- **`#[non_exhaustive]`** on all public structs and enums.
- **Doc comments** on all public items with examples where practical. Doc examples use `nmrs::` paths (crate root re-exports).
- **No `unwrap()` in library code** — return errors via `?` or `ConnectionError`.
  - `unwrap_or_else()` is allowed if the error is expected and there is a fallback value. Document this with a comment.
- **Tests**: unit tests live in `#[cfg(test)] mod tests` within the module or in `api/models/tests.rs`. Integration tests in `nmrs/tests/`. Assert behavior, not implementation.

## Commit messages

Follow [Conventional Commits](https://www.conventionalcommits.org/en/v1.0.0/):

```
type(#issue): description
```

Examples: `fix(#24): handle missing DNS in VPN config`, `feat: add OpenVPN proxy support`.
Atomic commits — one logical change per commit.

## Changelog

[Keep a Changelog](https://keepachangelog.com/) format in `nmrs/CHANGELOG.md`.
Sections: `Added`, `Changed`, `Fixed`. Link PRs/issues in parentheses.

## Things to watch out for

- The `VpnCredentials` type is deprecated — prefer `WireGuardConfig` for new WireGuard code.
