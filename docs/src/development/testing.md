# Testing

nmrs includes unit tests, integration tests, and model tests. Since many operations require a running NetworkManager daemon, tests are divided into offline and online categories.

## Running Tests

### Unit Tests

Unit tests cover validation, model construction, and builder logic. They run without NetworkManager:

```bash
cd nmrs
cargo test --lib --all-features
```

### Specific Test Modules

```bash
# Model tests
cargo test --lib api::models::tests

# Builder tests
cargo test --lib api::builders

# Validation tests
cargo test --lib util::validation
```

### Integration Tests

Environmental integration tests are `#[ignore]` so a normal `cargo test` never
contacts or mutates the host NetworkManager. Run the NM-only contract through
the isolated Docker harness:

```bash
docker compose run --build --rm test-integration
```

The harness sets `NMRS_REQUIRE_NETWORKMANAGER=1` and provisions a private veth
pair with DHCP before setting `NMRS_REQUIRE_WIRED=1`. It covers saved settings,
exact direct and unified settings events, a NetworkManager-routed secret request
and reply, native WireGuard activation, wired discovery, typed active-connection
data, DHCP activation, disconnect, and cleanup. Once a capability is declared,
an unavailable daemon, a D-Bus error, a missing event, or a timeout is a test
failure. There are no skip-as-pass branches.

To target a deliberately selected local daemon instead, opt in explicitly. The
NM-only contracts create, update, and delete a saved profile and register a
temporary secret agent, so prefer Docker unless those operations are
intentional:

```bash
NMRS_REQUIRE_NETWORKMANAGER=1 \
  cargo test --test integration_test --all-features \
  networkmanager_ -- --ignored --test-threads=1
```

## Test Categories

### Model Tests (`api/models/tests.rs`)

Comprehensive tests for all data types:
- Device type conversions and display formatting
- Device state conversions and transitional state detection
- Wi-Fi security type construction and methods
- EAP options construction (direct and builder)
- VPN credentials construction (direct and builder)
- WireGuard peer configuration
- Bluetooth identity validation
- Timeout config and connection options
- Error type formatting

### Builder Tests

Each builder module includes its own tests:
- `connection_builder.rs` — base settings, IPv4/IPv6 configuration, custom sections
- `wireguard_builder.rs` — WireGuard settings, validation, multiple peers
- `wifi_builder.rs` — Wi-Fi settings, bands, modes

### Validation Tests

`util/validation.rs` tests input validation:
- SSID validation
- Connection name validation
- Wi-Fi security validation (empty passwords, etc.)
- VPN credential validation
- Bluetooth address validation

## Writing Tests

### Offline Tests

For logic that doesn't require D-Bus:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_device_type_from_code() {
        assert_eq!(DeviceType::from(1), DeviceType::Ethernet);
        assert_eq!(DeviceType::from(2), DeviceType::Wifi);
    }
}
```

### Async Tests

For code that uses async:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_something_async() {
        // Test async logic
    }
}
```

## Docker Testing

For reproducible testing with a real NetworkManager instance:

```bash
docker compose run --build --rm test-integration
```

This starts a private system D-Bus and NetworkManager instance, provisions a
veth-backed DHCP network, waits for both to be ready, and runs the settings,
secret-agent, native WireGuard, and wired lifecycle contracts. It fails if any
declared facility is unavailable.

### Virtual Wi-Fi Integration

On a Linux host, the CI-equivalent test target uses two virtual radios created
with `mac80211_hwsim`. One radio advertises a WPA2-PSK test network using
`hostapd` and serves DHCP using dnsmasq. The isolated NetworkManager manages the
other radio.

```bash
sudo modprobe mac80211_hwsim radios=2
docker compose run --build --rm test-wifi-integration
sudo modprobe -r mac80211_hwsim
```

This service uses host networking and is therefore intended for Linux hosts and
the GitHub Actions runner, not Docker Desktop.

The harness provides `NMRS_REQUIRE_WIFI=1`, the exact interface, SSID, and
password, then asserts AP discovery, WPA authentication, DHCP activation,
network and device callback delivery, disconnect, saved-credential reconnect,
forget, and the exact missing-password error after cleanup. Missing declared
capabilities and unexpected errors fail.

It also mounts the host's `/run/udev` read-only so NetworkManager can manage
the newly created hwsim links.

The self-hosted runner service account needs passwordless `sudo` permission for
`modprobe mac80211_hwsim radios=2` and `modprobe -r mac80211_hwsim`; CI invokes
both commands with `sudo -n`.

### Approving Wi-Fi CI Runs

Pull requests send the virtual Wi-Fi job to the `self-hosted-pr-integration`
GitHub Actions environment before it is assigned to the self-hosted runner. To
require manual approval, create that environment in the repository's **Settings
> Environments**, add yourself as a required reviewer, and leave **Prevent
self-review** disabled. The job will show **Waiting for review** in the Actions
run; select **Review deployments** and approve that environment to start only
the Wi-Fi integration job.

Pushes to `master` use the unprotected `self-hosted-integration` environment
and start automatically.

## CI/CD

Tests run automatically via GitHub Actions on every push and pull request. The CI workflow:

1. Checks formatting (`cargo fmt --check`)
2. Runs clippy (`cargo clippy`)
3. Runs unit tests (`cargo test --lib`)
4. Runs the ignored integration contracts against isolated NetworkManager and virtual Wi-Fi harnesses
5. Builds documentation (`mdbook build`)

## Next Steps

- [Contributing](./contributing.md) – contribution guidelines
- [Architecture](./architecture.md) – understand the codebase
