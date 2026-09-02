# Scripts

Utility scripts for managing nmrs releases and running the isolated
NetworkManager test harness.

## `bump_version.py`

Prepares a release by updating version numbers and changelog.

### Usage

```bash
python3 scripts/bump_version.py <version> <release_type> [--allow-breaking]
```

### Arguments

- `version`: Version number in semver format (e.g., `3.1.0`)
- `release_type`: Either `beta` or `stable`
- `--allow-breaking`: Permit a non-major bump despite a breaking change in
  `[Unreleased]`. Use deliberately.

### Examples

```bash
# Prepare nmrs 3.1.0 stable release
python3 scripts/bump_version.py 3.1.0 stable

# Prepare nmrs 3.2.0 beta release
python3 scripts/bump_version.py 3.2.0 beta
```

### What it does

1. Refuses the bump if `[Unreleased]` documents a breaking change and the new
   version is not a major bump (see below)
2. Updates `version` in `nmrs/Cargo.toml`
3. Updates `nmrs/CHANGELOG.md` (moves Unreleased section to new version, and
   adds the compare link for the new tag)
4. Refreshes `Cargo.lock` via `cargo update --workspace` so `--locked` builds
   keep working

### Breaking-change gate

Cargo auto-upgrades minor and patch releases, so a breaking change shipped in
one reaches existing users without opt-in. Before touching any file, the script
scans the `[Unreleased]` section for a `**Breaking:**` marker and refuses a
minor or patch bump if it finds one:

```
✗ [Unreleased] documents a breaking change, but 3.4.2 -> 3.5.0 is a minor bump.
```

`cargo-semver-checks` runs in CI but does not cover every break, and the version
is bumped after CI has already run, so it never evaluates the version actually
being published. The changelog is the reliable signal because breaking changes
are labelled there by hand.

## Releasing

```bash
# 1. Bump version, update changelog, refresh lockfile
python3 scripts/bump_version.py 3.1.0 stable

# 2. Review and commit
git diff
git commit -am "chore(nmrs): prepare 3.1.0 release"

# 3. Push to master and tag
git push origin master
git tag nmrs-v3.1.0
git push origin nmrs-v3.1.0

# CI automatically publishes to crates.io
```

## `ci/`

Supporting files for the isolated NetworkManager integration harness. These are
normally invoked through `docker compose` rather than directly — see
`docker-compose.yml` and the testing section of the root `README.md`.

### `ci/run-networkmanager-tests.sh`

Brings up a self-contained NetworkManager stack inside a privileged container,
runs the requested test suite against it, and tears everything down. Nothing
touches the host's NetworkManager profiles.

It starts a private D-Bus system bus, `udevd`, and NetworkManager, then
provisions whichever fixtures the selected mode needs:

- two `mac80211_hwsim` virtual Wi-Fi networks (`hostapd` + `wpa_supplicant` +
  `dnsmasq`): WPA2-PSK on SSID `nmrs-hwsim` and SAE-only on `nmrs-hwsim-sae`
- a wired `veth` pair (`nmrs-client` / `nmrs-server`) with its own `dnsmasq`
  for DHCP
- a WireGuard interface for native-tunnel activation

All logs and runtime state go to a temporary directory, and every process is
stopped on exit.

```bash
bash scripts/ci/run-networkmanager-tests.sh <mode>
```

| mode | what it runs | compose service |
| --- | --- | --- |
| `all` (default) | full workspace suite, then wired + NetworkManager integration tests | `test`, `test-all` |
| `integration` | NetworkManager and wired integration tests only | `test-integration` |
| `wifi-integration` | virtual Wi-Fi integration tests only | `test-wifi-integration` |
| `shell` | drops into a shell with the stack running, for debugging | `shell` |

Integration tests are `#[ignore]` by default, so the script sets the opt-in
capability variables (`NMRS_REQUIRE_NETWORKMANAGER`, `NMRS_REQUIRE_WIRED`, and
friends) before running them with `--ignored`. Once a capability flag is set,
missing facilities and timeouts fail rather than skip.

The `wifi-integration` mode needs two `mac80211_hwsim` radios on the host and
runs with `network_mode: host`:

```bash
sudo modprobe mac80211_hwsim radios=3
docker compose run --build --rm test-wifi-integration
sudo modprobe -r mac80211_hwsim
```

### `ci/dbus-system.conf`

D-Bus system bus configuration for the harness. It listens on a container-local
socket and allows any client to own and call NetworkManager bus names, which
the real system policy would refuse. The container is the security boundary —
this config is not suitable outside it.
