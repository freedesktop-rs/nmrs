# Contributing

Thank you for wanting to contribute to `nmrs`!

I'm fairly accepting to all PR's, only with a couple caveats:

- Do not submit low-effort or purely LLM generated code. If you absolutely must, please disclose _how_ you used AI otherwise I will close the PR.
- Please try to (when possible) contribute to an [issue](https://github.com/freedesktop-rs/nmrs/issues). For smaller changes with no existing issue, a PR is fine but for larger changes, please open an issue first.

## Requirements

**To run or develop nmrs you need:**

- Rust (stable) via `rustup`
- Linux and NetworkManager only for environmental integration tests

I also provide a `Dockerfile` you can build if you don't use Linux and use macOS instead.

**To build the image:**

```bash
docker build -t nmrs-lib .
```

**To run tests:**

```bash
docker compose run --rm test
```

This starts an isolated system D-Bus and NetworkManager instance, waits for it
to be ready, runs the workspace tests, and executes the NetworkManager profile
CRUD integration contract. It does not use the host system bus.

**To run an interactive shell:**

```bash
docker compose run shell
```

If you just want quick builds/tests without the full NetworkManager environment:

```bash
docker run --rm nmrs-lib cargo test -p nmrs --lib
docker run --rm -it -v $(pwd):/app nmrs-lib   # mounts local changes
```

If you decide to run the shell, ensure you run all commands from within the nmrs directory, not root.

```bash
cargo test -p nmrs --lib     # run library unit tests
cargo build -p nmrs          # build the library
cargo check                  # you get the point...
```

## When your branch falls behind `master`

If the respective branch for a PR goes out of sync, I prefer you _rebase_.
I've exposed this setting for you to to automatically do so as a contributor on any PR you open.

## Issues and Commit Message Hygiene

Make your commit messages at least slightly meaningful. I don't really care much about the format, but please try to be descriptive to the reader.

A good example:

```log
fix(#24): fixed bug where something was happening
```

## Tests

All unit, documentation, and applicable environmental tests must pass before a
merge takes place.

### Unit and documentation tests

```bash
cargo test --locked --lib --all-features --workspace
cargo test --locked --doc --all-features --workspace
```

The integration tests are marked `#[ignore]`. A normal `cargo test` compiles
them but does not contact or mutate any NetworkManager instance. Do not remove
that boundary or add tests which silently return success when a required daemon,
device, or access point is missing.

### Isolated NetworkManager integration

```bash
docker compose run --build --rm test-integration
```

This starts a private system D-Bus and NetworkManager, plus a veth-backed DHCP
network which cannot select Docker's own `eth0`. It validates real saved-profile
creation, decoding, update, deletion, exact direct and unified settings events,
a NetworkManager-routed secret request and reply, native WireGuard activation,
wired discovery, DHCP activation, typed active-connection data, and disconnect
cleanup. The harness sets
`NMRS_REQUIRE_NETWORKMANAGER=1` and `NMRS_REQUIRE_WIRED=1`; once a capability is
declared, missing services and unexpected D-Bus errors fail the test.

### Deterministic WiFi integration

The WiFi contract requires two `mac80211_hwsim` radios. The container configures
one as a WPA2 access point, supplies DHCP with dnsmasq, and gives only the other
radio to its private NetworkManager. It asserts discovery, WPA authentication,
network and device callback delivery, DHCP, disconnect, saved-credential
reconnect, forget, and the missing-password error after cleanup.

```bash
sudo modprobe mac80211_hwsim radios=2
docker compose run --build --rm test-wifi-integration
sudo modprobe -r mac80211_hwsim
```

The WiFi runner sets `NMRS_REQUIRE_WIFI=1`, `NMRS_WIFI_INTERFACE`,
`NMRS_EXPECT_WIFI_SSID`, and `NMRS_WIFI_PASSWORD`. If a declared facility is
missing, the test fails rather than being reported as a pass.

To run the NM-only contracts against a deliberately selected local daemon, opt
in explicitly:

```bash
NMRS_REQUIRE_NETWORKMANAGER=1 \
  cargo test --test integration_test --all-features \
  networkmanager_ -- --ignored --test-threads=1
```

These tests create and delete a NetworkManager profile and register a temporary
secret agent. The wired contract is intentionally available only when its
separate capability and private interface are supplied. Prefer the Docker
harness unless modifying the selected daemon is intentional.

> [!NOTE]
>
> This method only works on linux

## License

All contributions fall under the [MIT License](https://github.com/freedesktop-rs/nmrs?tab=MIT-1-ov-file).
