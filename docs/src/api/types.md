# Core Types

This page lists the primary types exported by nmrs. For complete API documentation, see [docs.rs/nmrs](https://docs.rs/nmrs).

## NetworkManager

The main entry point for all operations.

```rust
use nmrs::NetworkManager;

let nm = NetworkManager::new().await?;
let nm = NetworkManager::with_config(config).await?;
```

- `Clone` — clones share the same D-Bus connection
- `Send + Sync` — safe to share across tasks
- See [NetworkManager API](./network-manager.md) for all methods
- See [Raw Module](./raw.md) for the `zbus` re-export used with `dbus_connection()`

## Result Type

```rust
pub type Result<T> = std::result::Result<T, ConnectionError>;
```

All public methods return `nmrs::Result<T>`.

## Wi-Fi Types

| Type | Description |
|------|-------------|
| `Network` | A discovered Wi-Fi network (SSID, signal, security flags) |
| `NetworkInfo` | Detailed network information (channel, speed, bars) |
| `AccessPoint` | A single AP with BSSID, frequency, and security flags |
| `WifiDevice` | A Wi-Fi device with interface, MAC, state, and active SSID |
| `WifiScope` | Per-interface operations scope (from `nm.wifi("wlan1")`) |
| `WifiSecurity` | Authentication type: `Open`, `WpaPsk`, `WpaEap` |
| `EapOptions` | Enterprise Wi-Fi (802.1X) configuration |
| `EapOptionsBuilder` | Builder for `EapOptions` |
| `EapMethod` | Outer EAP method: `Peap`, `Ttls` |
| `Phase2` | Inner auth method: `Mschapv2`, `Pap` |

## Device Types

| Type | Description |
|------|-------------|
| `Device` | A network device (interface, type, state, MAC) |
| `DeviceIdentity` | Device MAC addresses (permanent and current) |
| `DeviceType` | Device kind: `Wifi`, `Ethernet`, `Bluetooth`, `WifiP2P`, `Loopback`, `Other(u32)` |
| `DeviceState` | Operational state: `Disconnected`, `Activated`, `Failed`, etc. |

## Radio / Airplane-Mode Types

| Type | Description |
|------|-------------|
| `RadioState` | Combined software (`enabled`) and hardware (`hardware_enabled`) radio state |
| `AirplaneModeState` | Aggregated state across Wi-Fi, WWAN, and Bluetooth |

## VPN Types

| Type | Description |
|------|-------------|
| `VpnConfig` | Sealed trait for VPN configurations |
| `VpnConfiguration` | Dispatch enum: `WireGuard(WireGuardConfig)` or `OpenVpn(OpenVpnConfig)` |
| `WireGuardConfig` | WireGuard VPN configuration |
| `WireGuardPeer` | WireGuard peer configuration |
| `OpenVpnConfig` | OpenVPN configuration |
| `OpenVpnAuthType` | OpenVPN auth: `Password`, `Tls`, `PasswordTls`, `StaticKey` |
| `OpenVpnCompression` | Compression mode: `No`, `Lzo` (deprecated), `Lz4`, `Lz4V2`, `Yes` |
| `OpenVpnProxy` | Proxy: `Http { ... }`, `Socks { ... }` |
| `VpnRoute` | Static IPv4 route for split tunneling |
| `VpnType` | Protocol-specific metadata (data-carrying enum) |
| `VpnKind` | `Plugin` (OpenVPN, etc.) vs `WireGuard` |
| `VpnConnection` | A saved/active VPN connection with rich metadata |
| `VpnConnectionInfo` | Detailed active VPN info (IP, DNS, gateway, protocol details) |
| `VpnDetails` | Protocol-specific active connection details |
| `VpnCredentials` | **Deprecated** — use `WireGuardConfig` instead |

## Connectivity Types

| Type | Description |
|------|-------------|
| `ConnectivityState` | NM connectivity: `Full`, `Portal`, `Limited`, `None`, `Unknown` |
| `ConnectivityReport` | Full report with state, check URI, and captive portal URL |
| `NetworkSnapshot` | Point-in-time applet state |
| `AppletNetworkSummary` | Applet-ready summary derived from a snapshot |
| `WifiNetworkGroup` | Visible APs grouped by interface and SSID |

## Saved Connection Types

| Type | Description |
|------|-------------|
| `SavedConnection` | Full decoded saved profile |
| `SavedConnectionBrief` | Lightweight profile (`uuid`, `id`, `type`) |
| `SavedVpnSummary` | Lightweight saved VPN status keyed by UUID |
| `SettingsSummary` | Decoded settings within a profile |
| `SettingsPatch` | Partial update for `update_saved_connection` |

## Bluetooth Types

| Type | Description |
|------|-------------|
| `BluetoothDevice` | A Bluetooth device with BlueZ info |
| `BluetoothIdentity` | Bluetooth MAC + network role for connecting |
| `BluetoothNetworkRole` | Role: `PanU`, `Dun` |

## Configuration Types

| Type | Description |
|------|-------------|
| `TimeoutConfig` | Connection/disconnection timeouts |
| `ConnectionOptions` | Autoconnect, priority, retry settings |
| `MonitorHandle` | Handle returned by monitor APIs; call `stop().await?` to shut down |

## Raw Module

| Module | Description |
|--------|-------------|
| `nmrs::raw::zbus` | Re-exported zbus dependency for advanced D-Bus work (`Variant`, `OwnedObjectPath`, builder output types) |
| `nmrs::raw::zvariant` | zbus's deprecated `zvariant` compatibility module; use `raw::zbus` for new code |

See [Raw Module](./raw.md) and [`dbus_connection()`](./network-manager.md#advanced-d-bus-access).

## Error Types

| Type | Description |
|------|-------------|
| `ConnectionError` | All possible error variants |
| `StateReason` | Device state reason codes |
| `ConnectionStateReason` | Activation/deactivation reason codes |
| `ActiveConnectionState` | Connection lifecycle states |

## Builder Types

| Type | Description |
|------|-------------|
| `ConnectionBuilder` | Base connection settings builder |
| `WifiConnectionBuilder` | Wi-Fi connection builder |
| `WireGuardBuilder` | WireGuard VPN builder |
| `OpenVpnBuilder` | OpenVPN builder (also imports `.ovpn` files) |
| `IpConfig` | IP address with CIDR prefix |
| `Route` | Static route configuration |
| `WifiBand` | Wi-Fi band: `Bg` (2.4 GHz), `A` (5 GHz) |
| `WifiMode` | Wi-Fi mode: `Infrastructure`, `Adhoc`, `Ap` |

## Re-exports

nmrs re-exports commonly used types at the crate root for convenience:

```rust
use nmrs::{
    NetworkManager, WifiScope,
    WifiSecurity, EapOptions, EapMethod, Phase2,
    WireGuardConfig, WireGuardPeer,
    OpenVpnConfig, OpenVpnAuthType,
    VpnConfig, VpnConfiguration, VpnType, VpnKind,
    TimeoutConfig, ConnectionOptions,
    ConnectionError, DeviceType, DeviceState,
    RadioState, AirplaneModeState,
    ConnectivityState, ConnectivityReport,
};
```

Less commonly used types are available through the `models` and `builders` modules:

```rust
use nmrs::models::{BluetoothIdentity, BluetoothNetworkRole, BluetoothDevice};
use nmrs::builders::{ConnectionBuilder, WireGuardBuilder, OpenVpnBuilder, IpConfig, Route};
use nmrs::raw::zbus;
```
