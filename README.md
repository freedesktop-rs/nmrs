# <p align="center"> nmrs 🦀

[![Crates.io](https://img.shields.io/crates/v/nmrs)](https://crates.io/crates/nmrs)
[![Crates.io Downloads](https://img.shields.io/crates/d/nmrs)](https://crates.io/crates/nmrs)
[![Discord](https://img.shields.io/badge/chat-on%20discord-7289da?logo=discord&logoColor=white)](https://discord.gg/Sk3VfrHrN4)
[![Documentation](https://docs.rs/nmrs/badge.svg)](https://docs.rs/nmrs)
[![User Guide](https://img.shields.io/badge/docs-mdBook-blue)](https://freedesktop-rs.github.io/nmrs/)
[![CI](https://github.com/freedesktop-rs/nmrs/actions/workflows/ci.yml/badge.svg)](https://github.com/freedesktop-rs/nmrs/actions/workflows/ci.yml)
[![License](https://img.shields.io/crates/l/nmrs)](LICENSE)

An async-first Rust API for [NetworkManager](https://networkmanager.dev/) over [D-Bus](https://dbus.freedesktop.org/doc/dbus-specification.html). The goal is to provide a safe and simple high-level API for managing Wi-Fi connections on Linux systems, built on [`zbus`](https://docs.rs/zbus) for reliable D-Bus communication.

## Documentation

- **[User Guide](https://freedesktop-rs.github.io/nmrs/)** - Comprehensive guide with tutorials and examples
- **[API Documentation](https://docs.rs/nmrs)** - Complete API reference on docs.rs
- **[Discord](https://discord.gg/Sk3VfrHrN4)** - Join our community for help and discussion

## Getting Started

_Please consider joining the [**Discord**](https://discord.gg/Sk3VfrHrN4). It's a welcoming community to both developers who want to contribute and/or learn about and discuss nmrs as well as users that would like to be engaged with the development process._

The best way to get started with `nmrs` is the [User Guide](https://freedesktop-rs.github.io/nmrs/), which includes comprehensive tutorials and examples. For detailed API information, see the [API documentation](https://docs.rs/nmrs).

## Sample usage

We'll create a simple example that scans for available networks and connects to one. Note that these examples require NetworkManager to be running on your Linux system with D-Bus access, obviously.

### Listing Networks

Scan for and display available Wi-Fi networks:

```rust,no_run
use nmrs::NetworkManager;

#[tokio::main]
async fn main() -> nmrs::Result<()> {
    let nm = NetworkManager::new().await?;

    // Scan for networks
    let networks = nm.list_networks(None).await?;

    for net in networks {
        println!(
            "{} - Signal: {}%, Security: {:?}",
            net.ssid,
            net.strength.unwrap_or(0),
            net.security
        );
    }

    Ok(())
}
```

### Now let's connect to a network...

Connect to a WPA-PSK protected network:

```rust,no_run
use nmrs::{NetworkManager, WifiSecurity};

#[tokio::main]
async fn main() -> nmrs::Result<()> {
    let nm = NetworkManager::new().await?;

    // Connect to a network
    nm.connect("MyNetwork", None, WifiSecurity::WpaPsk {
        psk: "password123".into()
    }).await?;

    // Check current connection
    if let Some(ssid) = nm.current_ssid().await {
        println!("Connected to: {}", ssid);
    }

    Ok(())
}
```

### Error Handling

All operations return `Result<T, ConnectionError>` with specific error variants:

```rust,no_run
use nmrs::{NetworkManager, WifiSecurity, ConnectionError};

#[tokio::main]
async fn main() -> nmrs::Result<()> {
    let nm = NetworkManager::new().await?;

    match nm.connect("MyNetwork", None, WifiSecurity::WpaPsk {
        psk: "wrong_password".into()
    }).await {
        Ok(_) => println!("Connected successfully"),
        Err(ConnectionError::AuthFailed) => eprintln!("Authentication failed - wrong password"),
        Err(ConnectionError::NotFound) => eprintln!("Network not found or out of range"),
        Err(ConnectionError::Timeout) => eprintln!("Connection timed out"),
        Err(e) => eprintln!("Error: {}", e),
    }

    Ok(())
}
```

To follow and/or discuss the development of nmrs, you can join the [public Discord channel](https://discord.gg/Sk3VfrHrN4).

# Some cool projects using nmrs

- [cosmic-settings](https://github.com/pop-os/cosmic-settings) by [@pop-os](https://github.com/pop-os)
- [cosmic-applets](https://github.com/pop-os/cosmic-applets) by [@pop-os](https://github.com/pop-os)
- [nmrs-tui](https://github.com/y2w8/nmrs-tui) by [@y2w8](https://github.com/y2w8)
- [gaypanel](https://codeberg.org/pastthepixels/gaypanel) by [@pastthepixels](https://codeberg.org/pastthepixels)
- [nmrs-gui](https://github.com/freedesktop-rs/nmrs-gui) by [@freedesktop-rs](https://github.com/freedesktop-rs)
- [android-auto](https://github.com/uglyoldbob/android-auto) by [uglyoldbob](https://github.com/uglyoldbob)

# Roadmap / Implementation Status

If something is missing that you'd like to see, please file a PR or issue, adding it to this roadmap.

### Wi-Fi

- [x] Scan and list Wi-Fi networks
- [x] List individual access points with BSSID, frequency, signal, and security flags
- [x] Per-interface Wi-Fi scoping with `nm.wifi("wlan0")`
- [x] Open networks
- [x] WPA-PSK personal networks
- [x] WPA-EAP PEAP/MSCHAPv2
- [x] WPA-EAP TTLS/PAP
- [x] EAP-TLS with certificate/key paths or blobs
- [x] WPA3-Enterprise 192-bit mode
- [x] Hidden networks
- [x] BSSID-specific connection
- [x] Race-free `try_connect` / `try_connect_to_bssid`
- [ ] Wi-Fi P2P connection management

### Wired, Bluetooth, And VLAN

- [x] Ethernet DHCP connections
- [x] Bluetooth PAN/DUN device discovery and connection
- [x] VLAN profile builder and validation
- [x] Loopback device detection
- [ ] Bond profile builder
- [ ] Bridge profile builder
- [ ] TUN/TAP profile builder
- [ ] MACVLAN / MACsec / VRF / VXLAN profile builders

### VPN

- [x] WireGuard profile builder and connection support
- [x] OpenVPN profile builder
- [x] `.ovpn` import support
- [x] Saved VPN discovery for WireGuard and plugin VPNs
- [x] Connect/disconnect saved VPNs by UUID or name
- [x] Active VPN listing and connection details
- [x] Generic plugin VPN detection (OpenConnect, strongSwan, PPTP, L2TP, etc.)
- [ ] Builders/importers for non-OpenVPN plugin VPN profiles

### Profiles, Radio, And Connectivity

- [x] Saved connection listing, raw access, decoded summaries, update, delete, and reload
- [x] Profile reuse for saved Wi-Fi and Ethernet connections
- [x] Secret agent for NetworkManager credential prompts
- [x] Real-time network and device monitoring
- [x] Wi-Fi, WWAN, Bluetooth, and aggregate airplane-mode radio state
- [x] Connectivity state, forced connectivity checks, and captive-portal URL detection
- [x] IPv4, IPv6, DHCPv4, and DHCPv6 settings

### Device And D-Bus Surface

- [x] NetworkManager facade
- [x] Device enumeration and typed device models for Ethernet, Wi-Fi, Bluetooth, VLAN, Loopback, and Wi-Fi P2P
- [x] Device metadata registry for Bond, Bridge, TUN, WireGuard, and other known NetworkManager type codes
- [x] Access Point
- [x] Active Connection
- [x] Settings
- [x] Settings Connection
- [x] Agent Manager
- [x] VPN Connection
- [ ] Checkpoint
- [ ] DNS Manager
- [ ] PPP
- [ ] Modem / WWAN connection management
- [ ] WiMAX NSP

</details>

## Contributing

Contributions are welcome. Please read [CONTRIBUTING.md](./CONTRIBUTING.md) for guidelines.

Environmental tests are opt-in and ignored by a normal `cargo test`, so local
test runs never probe the host NetworkManager implicitly. Use
`docker compose run --build --rm test-integration` for isolated settings,
NetworkManager-routed secrets, native WireGuard activation, and veth-backed
wired DHCP lifecycles, or
`test-wifi-integration` with two `mac80211_hwsim` radios for the deterministic
WPA/DHCP and callback-monitor lifecycle. See the contributing guide for the
exact commands.

## Requirements

- **Rust**: 1.90.0+
- **NetworkManager**: Running and accessible via D-Bus
- **Linux**: This library is Linux-specific

## License

This project is dual-licensed under either of the following licenses, at your option:

- MIT License
- Apache License, Version 2.0

You may use, copy, modify, and distribute this software under the terms of either license.

See the following files for full license texts:

- [MIT License](./LICENSE-MIT)
- [Apache License 2.0](./LICENSE-APACHE)

## Contributors

Thank you to everyone who has helped build, test, document, and review `nmrs`.

<!-- readme: contributors -start -->
<table>
  <tr>
    <td align="center"><a href="https://github.com/cachebag"><img src="https://avatars.githubusercontent.com/u/111914307?v=4" width="100px;" alt="cachebag"/><br /><sub><b>cachebag</b></sub></a></td>
    <td align="center"><a href="https://github.com/stoutes"><img src="https://avatars.githubusercontent.com/u/31317041?v=4" width="100px;" alt="stoutes"/><br /><sub><b>stoutes</b></sub></a></td>
    <td align="center"><a href="https://github.com/justinjest"><img src="https://avatars.githubusercontent.com/u/97318401?v=4" width="100px;" alt="justinjest"/><br /><sub><b>justinjest</b></sub></a></td>
    <td align="center"><a href="https://github.com/Dandiggas"><img src="https://avatars.githubusercontent.com/u/78769670?v=4" width="100px;" alt="Dandiggas"/><br /><sub><b>Dandiggas</b></sub></a></td>
    <td align="center"><a href="https://github.com/Biqydu"><img src="https://avatars.githubusercontent.com/u/165430853?v=4" width="100px;" alt="Biqydu"/><br /><sub><b>Biqydu</b></sub></a></td>
    <td align="center"><a href="https://github.com/pluiee"><img src="https://avatars.githubusercontent.com/u/93393389?v=4" width="100px;" alt="pluiee"/><br /><sub><b>pluiee</b></sub></a></td>
  </tr>
  <tr>
    <td align="center"><a href="https://github.com/JonnieCache"><img src="https://avatars.githubusercontent.com/u/211093?v=4" width="100px;" alt="JonnieCache"/><br /><sub><b>JonnieCache</b></sub></a></td>
    <td align="center"><a href="https://github.com/morehwachege"><img src="https://avatars.githubusercontent.com/u/76877744?v=4" width="100px;" alt="morehwachege"/><br /><sub><b>morehwachege</b></sub></a></td>
    <td align="center"><a href="https://github.com/tristanmsct"><img src="https://avatars.githubusercontent.com/u/69300092?v=4" width="100px;" alt="tristanmsct"/><br /><sub><b>tristanmsct</b></sub></a></td>
    <td align="center"><a href="https://github.com/Rifat-R"><img src="https://avatars.githubusercontent.com/u/81259132?v=4" width="100px;" alt="Rifat-R"/><br /><sub><b>Rifat-R</b></sub></a></td>
    <td align="center"><a href="https://github.com/of-the-stars"><img src="https://avatars.githubusercontent.com/u/47869156?v=4" width="100px;" alt="of-the-stars"/><br /><sub><b>of-the-stars</b></sub></a></td>
    <td align="center"><a href="https://github.com/lkramer"><img src="https://avatars.githubusercontent.com/u/58181?v=4" width="100px;" alt="lkramer"/><br /><sub><b>lkramer</b></sub></a></td>
  </tr>
  <tr>
    <td align="center"><a href="https://github.com/okhsunrog"><img src="https://avatars.githubusercontent.com/u/42293787?v=4" width="100px;" alt="okhsunrog"/><br /><sub><b>okhsunrog</b></sub></a></td>
    <td align="center"><a href="https://github.com/neon-commits01"><img src="https://avatars.githubusercontent.com/u/235033610?v=4" width="100px;" alt="neon-commits01"/><br /><sub><b>neon-commits01</b></sub></a></td>
    <td align="center"><a href="https://github.com/jrb0001"><img src="https://avatars.githubusercontent.com/u/2380263?v=4" width="100px;" alt="jrb0001"/><br /><sub><b>jrb0001</b></sub></a></td>
    <td align="center"><a href="https://github.com/joncorv"><img src="https://avatars.githubusercontent.com/u/151096562?v=4" width="100px;" alt="joncorv"/><br /><sub><b>joncorv</b></sub></a></td>
    <td align="center"><a href="https://github.com/PineappleJammingg"><img src="https://avatars.githubusercontent.com/u/199402636?v=4" width="100px;" alt="PineappleJammingg"/><br /><sub><b>PineappleJammingg</b></sub></a></td>
    <td align="center"><a href="https://github.com/AK78gz"><img src="https://avatars.githubusercontent.com/u/89071188?v=4" width="100px;" alt="AK78gz"/><br /><sub><b>AK78gz</b></sub></a></td>
  </tr>
  <tr>
    <td align="center"><a href="https://github.com/pwsandoval"><img src="https://avatars.githubusercontent.com/u/15174704?v=4" width="100px;" alt="pwsandoval"/><br /><sub><b>pwsandoval</b></sub></a></td>
    <td align="center"><a href="https://github.com/ritiek"><img src="https://avatars.githubusercontent.com/u/20314742?v=4" width="100px;" alt="ritiek"/><br /><sub><b>ritiek</b></sub></a></td>
    <td align="center"><a href="https://github.com/shubhsingh5901"><img src="https://avatars.githubusercontent.com/u/110416544?v=4" width="100px;" alt="shubhsingh5901"/><br /><sub><b>shubhsingh5901</b></sub></a></td>
    <td align="center"><a href="https://github.com/cinnamonstic"><img src="https://avatars.githubusercontent.com/u/182801542?v=4" width="100px;" alt="cinnamonstic"/><br /><sub><b>cinnamonstic</b></sub></a></td>
    <td align="center"><a href="https://github.com/theroguevigilante"><img src="https://avatars.githubusercontent.com/u/206333897?v=4" width="100px;" alt="theroguevigilante"/><br /><sub><b>theroguevigilante</b></sub></a></td>
    <td align="center"><a href="https://github.com/tuned-willow"><img src="https://avatars.githubusercontent.com/u/250158319?v=4" width="100px;" alt="tuned-willow"/><br /><sub><b>tuned-willow</b></sub></a></td>
  </tr>
</table>
<!-- readme: contributors -end -->
