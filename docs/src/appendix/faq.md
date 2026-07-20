# FAQ

## General

### What is nmrs?

nmrs is a Rust library for managing network connections on Linux via NetworkManager's D-Bus interface. It provides a safe, async API for Wi-Fi, Ethernet, Bluetooth, and VPN management.

### What does nmrs stand for?

**N**etwork**M**anager **R**u**s**t — nmrs.

### Is nmrs production-ready?

Yes. nmrs is at version 3.1.x with a stable API. All public types are
marked `#[non_exhaustive]` to allow backward-compatible additions, and
the public surface is enforced in CI with
[`cargo-semver-checks`](https://crates.io/crates/cargo-semver-checks).

### What Linux distributions are supported?

Any distribution that runs NetworkManager. This includes Ubuntu, Fedora, Arch Linux, Debian, openSUSE, NixOS, and many others.

### Does nmrs work on macOS or Windows?

No. nmrs is Linux-specific since it communicates with NetworkManager over D-Bus, which is a Linux service.

## Library

### Which async runtime should I use?

nmrs works with any async runtime (Tokio, async-std, smol, GLib). Tokio is recommended and used in all examples. See [Async Runtime Support](../advanced/async-runtimes.md).

### Can I use nmrs without an async runtime?

No. D-Bus communication is inherently async. You can use `block_on()` from smol or `tokio::runtime::Runtime::block_on()` if you need a synchronous wrapper.

### Is NetworkManager the only way to manage Wi-Fi on Linux?

No, but it's the most widely used network management daemon. Other options include `iwd`, `connman`, and `wpa_supplicant` (direct). nmrs specifically targets NetworkManager.

### Do I need root permissions?

Usually no. NetworkManager uses PolicyKit for authorization, and most desktop Linux setups grant network management permissions to the logged-in user. If you're running in a headless environment, you may need to configure PolicyKit rules. See [Requirements](../getting-started/requirements.md).

### Can I connect to multiple networks simultaneously?

A device can only have one active connection at a time. However, you can have different connections on different devices (e.g., Wi-Fi on `wlan0` and Ethernet on `eth0` simultaneously).

### Can I make concurrent connection calls?

No. Concurrent connection operations (calling `connect()` from multiple tasks) are not supported. Use `is_connecting()` to check before starting a new connection.

### How do I handle saved connections?

When nmrs connects to a network, NetworkManager saves the profile. To reconnect
with its stored settings, pass `WifiSecurity::Open` or an empty
`WifiSecurity::WpaPsk` password. A non-empty PSK or an EAP configuration is an
explicit fresh-credential request, so nmrs builds a fresh profile instead of
ignoring it. If activation with a stored PSK fails, nmrs returns the error but
keeps the saved profile. Use `forget()` to delete a saved profile intentionally.

## VPN

### Which VPN protocols are supported?

WireGuard and OpenVPN. WireGuard uses NetworkManager's native kernel integration (no plugin needed). OpenVPN requires the `networkmanager-openvpn` plugin.

### Do I need the WireGuard kernel module?

Yes. WireGuard is built into the Linux kernel since version 5.6. On older kernels, install the `wireguard` module. NetworkManager's WireGuard support requires NM 1.16+.

### Can I import a `.ovpn` file?

Yes. Use `nm.import_ovpn("client.ovpn", Some("user"), Some("pass")).await?` to parse and activate an OpenVPN profile in one call. Inline certificates are extracted and persisted automatically.

### Can I import a `.conf` WireGuard file?

Not directly. Extract the values from the config file and pass them to `WireGuardConfig::new()`.

## Troubleshooting

### Where can I get help?

- **Discord:** [discord.gg/Sk3VfrHrN4](https://discord.gg/Sk3VfrHrN4)
- **GitHub Issues:** [github.com/freedesktop-rs/nmrs/issues](https://github.com/freedesktop-rs/nmrs/issues)
- **Troubleshooting Guide:** [Troubleshooting](./troubleshooting.md)
