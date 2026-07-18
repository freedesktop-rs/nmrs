# NetworkManager API

The `NetworkManager` struct is the primary entry point for all nmrs operations. It manages a D-Bus connection to the NetworkManager daemon.

## Construction

```rust
use nmrs::{NetworkManager, TimeoutConfig};
use std::time::Duration;

// Default timeouts (30s connect, 10s disconnect)
let nm = NetworkManager::new().await?;

// Custom timeouts
let config = TimeoutConfig::new()
    .with_connection_timeout(Duration::from_secs(60))
    .with_disconnect_timeout(Duration::from_secs(20));
let nm = NetworkManager::with_config(config).await?;

// Read current config
let config = nm.timeout_config();
```

## Saving Profiles Without Activating

```rust
use nmrs::builders::build_wifi_connection;
use nmrs::{ConnectionOptions, NetworkManager, WifiSecurity};

let nm = NetworkManager::new().await?;
let settings = build_wifi_connection(
    "GuestWiFi",
    &WifiSecurity::WpaPsk { psk: "password".into() },
    &ConnectionOptions::new(true),
);
let profile = nm.add_connection(settings).await?;
```

| Method | Returns | Description |
|--------|---------|-------------|
| `add_connection(settings)` | `Result<OwnedObjectPath>` | Save a profile via `Settings.AddConnection` without activating it ([#463](https://github.com/freedesktop-rs/nmrs/issues/463)) |

## Activating Builder Output

```rust
use nmrs::builders::{WifiConnectionBuilder, WifiMode};
use nmrs::NetworkManager;

let nm = NetworkManager::new().await?;
let settings = WifiConnectionBuilder::new("Hotspot")
    .wpa_psk("password")
    .mode(WifiMode::Ap)
    .ipv4_shared()
    .build();

nm.add_and_activate_connection(settings, Some("wlan0"), None).await?;
```

| Method | Returns | Description |
|--------|---------|-------------|
| `add_and_activate_connection(settings, interface, specific_object)` | `Result<(OwnedObjectPath, OwnedObjectPath)>` | Create and activate a profile in one step ([#260](https://github.com/freedesktop-rs/nmrs/issues/260)) |

- `interface`: device name such as `"wlan0"`, or `None` to auto-pick the first device matching `connection.type`
- `specific_object`: access-point path for client Wi-Fi, or `None` for AP mode / Ethernet / VPN (`"/"`)

## Advanced D-Bus Access

```rust
use nmrs::raw::{zbus, zvariant};

let conn = nm.dbus_connection(); // &zbus::Connection
```

| Method | Returns | Description |
|--------|---------|-------------|
| `dbus_connection()` | `&zbus::Connection` | Shared system bus connection for advanced D-Bus calls |

Use this with [`nmrs::raw`](./raw.md) and the [builders](./builders.md) module
only when you need NetworkManager methods that nmrs does not wrap yet. For
builder output, prefer
[`add_connection`](./network-manager.md#saving-profiles-without-activating) and
[`add_and_activate_connection`](./network-manager.md#activating-builder-output).

## Wi-Fi Methods

| Method | Returns | Description |
|--------|---------|-------------|
| `scan_networks(interface)` | `Result<()>` | Trigger active Wi-Fi scan (`None` = all devices) |
| `list_networks(interface)` | `Result<Vec<Network>>` | List visible networks (`None` = all devices) |
| `list_access_points(interface)` | `Result<Vec<AccessPoint>>` | List individual APs by BSSID |
| `connect(ssid, interface, security)` | `Result<()>` | Connect to a Wi-Fi network |
| `connect_to_bssid(ssid, bssid, interface, security)` | `Result<()>` | Connect to a specific AP |
| `disconnect(interface)` | `Result<()>` | Disconnect from current network (`None` = first Wi-Fi device) |
| `current_network()` | `Result<Option<Network>>` | Get current Wi-Fi network |
| `current_ssid()` | `Option<String>` | Get current SSID |
| `current_connection_info()` | `Option<(String, Option<u32>)>` | Get SSID + frequency |
| `is_connected(ssid)` | `Result<bool>` | Check if connected to a specific network |
| `show_details(network)` | `Result<NetworkInfo>` | Get detailed network info |

## Per-Device Wi-Fi Methods

| Method | Returns | Description |
|--------|---------|-------------|
| `list_wifi_devices()` | `Result<Vec<WifiDevice>>` | List all Wi-Fi devices |
| `wifi_device_by_interface(name)` | `Result<WifiDevice>` | Look up a Wi-Fi device by name |
| `wifi(interface)` | `WifiScope` | Build a scope pinned to one interface |
| `set_wifi_enabled(interface, bool)` | `Result<()>` | Enable/disable one Wi-Fi radio |

## Radio / Airplane-Mode Methods

| Method | Returns | Description |
|--------|---------|-------------|
| `wifi_state()` | `Result<RadioState>` | Software + hardware Wi-Fi state |
| `wwan_state()` | `Result<RadioState>` | Software + hardware WWAN state |
| `bluetooth_radio_state()` | `Result<RadioState>` | Software + hardware Bluetooth state |
| `airplane_mode_state()` | `Result<AirplaneModeState>` | Aggregated across all radios |
| `set_wireless_enabled(bool)` | `Result<()>` | Global Wi-Fi software toggle |
| `set_wwan_enabled(bool)` | `Result<()>` | Global WWAN toggle |
| `set_bluetooth_radio_enabled(bool)` | `Result<()>` | Toggle all BlueZ adapters |
| `set_airplane_mode(bool)` | `Result<()>` | Toggle all three radios |
| `wait_for_wifi_ready()` | `Result<()>` | Wait for Wi-Fi device to become ready |

## Ethernet Methods

| Method | Returns | Description |
|--------|---------|-------------|
| `connect_wired()` | `Result<()>` | Connect first available Ethernet device |

For this method and the wired device-listing methods below, Ethernet includes
devices that NetworkManager reports as `veth`.

## VPN Methods

| Method | Returns | Description |
|--------|---------|-------------|
| `connect_vpn(config)` | `Result<()>` | Connect with a `WireGuardConfig` or `OpenVpnConfig` |
| `import_ovpn(path, user, pass)` | `Result<()>` | Import `.ovpn` file and connect |
| `connect_vpn_by_uuid(uuid)` | `Result<()>` | Activate a saved VPN by UUID |
| `connect_vpn_by_id(id)` | `Result<()>` | Activate a saved VPN by display name |
| `disconnect_vpn(name)` | `Result<()>` | Disconnect a VPN by name |
| `disconnect_vpn_by_uuid(uuid)` | `Result<()>` | Disconnect a VPN by UUID |
| `list_vpn_connections()` | `Result<Vec<VpnConnection>>` | List all saved VPNs |
| `active_vpn_connections()` | `Result<Vec<VpnConnection>>` | List only active VPNs |
| `forget_vpn(name)` | `Result<()>` | Delete a saved VPN profile |
| `get_vpn_info(name)` | `Result<VpnConnectionInfo>` | Get active VPN details |

## Connectivity Methods

| Method | Returns | Description |
|--------|---------|-------------|
| `connectivity()` | `Result<ConnectivityState>` | Current NM connectivity state |
| `check_connectivity()` | `Result<ConnectivityState>` | Force a connectivity re-check |
| `connectivity_report()` | `Result<ConnectivityReport>` | Full report with captive portal URL |
| `captive_portal_url()` | `Result<Option<String>>` | Captive portal URL if in Portal state |

## Bluetooth Methods

| Method | Returns | Description |
|--------|---------|-------------|
| `list_bluetooth_devices()` | `Result<Vec<BluetoothDevice>>` | List Bluetooth devices |
| `connect_bluetooth(name, identity)` | `Result<()>` | Connect to a Bluetooth device |
| `forget_bluetooth(name)` | `Result<()>` | Delete a Bluetooth profile |

## Device Methods

| Method | Returns | Description |
|--------|---------|-------------|
| `list_devices()` | `Result<Vec<Device>>` | List all network devices |
| `list_wireless_devices()` | `Result<Vec<Device>>` | List Wi-Fi devices |
| `list_wired_devices()` | `Result<Vec<Device>>` | List Ethernet devices |
| `list_wired_device_details()` | `Result<Vec<WiredDevice>>` | List Ethernet devices with link speed, active connection id, and IPs |
| `get_device_by_interface(name)` | `Result<OwnedObjectPath>` | Find device by interface name |
| `is_connecting()` | `Result<bool>` | Check if any device is connecting |
| `list_active_connections()` | `Result<Vec<ActiveConnection>>` | List typed active wired, Wi-Fi, VPN, and other connections |
| `snapshot()` | `Result<NetworkSnapshot>` | Read point-in-time applet state after a `NetworkEvent` |
| `network_events()` | `Result<NetworkEventStream>` | Unified refresh-trigger stream for GUI state updates |
| `settings_events()` | `Result<SettingsEventStream>` | Saved connection add/remove/update stream |

## Connection Profile Methods

| Method | Returns | Description |
|--------|---------|-------------|
| `list_saved_connections()` | `Result<Vec<SavedConnection>>` | Full decode of all saved profiles |
| `list_saved_connections_brief()` | `Result<Vec<SavedConnectionBrief>>` | Lightweight profile listing |
| `list_saved_connection_ids()` | `Result<Vec<String>>` | Just the profile names |
| `get_saved_connection(uuid)` | `Result<SavedConnection>` | Load one profile by UUID |
| `get_saved_connection_raw(uuid)` | `Result<HashMap<...>>` | Raw `GetSettings` map |
| `delete_saved_connection(uuid)` | `Result<()>` | Delete a profile by UUID |
| `update_saved_connection(uuid, patch)` | `Result<()>` | Merge a `SettingsPatch` into a profile |
| `reload_saved_connections()` | `Result<()>` | Re-read profiles from disk |
| `has_saved_connection(ssid)` | `Result<bool>` | Check if a Wi-Fi profile exists |
| `get_saved_connection_path(ssid)` | `Result<Option<OwnedObjectPath>>` | Get profile D-Bus path |
| `get_saved_connection_uuid(name)` | `Result<Option<String>>` | Get profile UUID by `connection.id` (usually SSID) |
| `forget(ssid)` | `Result<()>` | Delete a Wi-Fi profile |

## Monitoring Methods

| Method | Returns | Description |
|--------|---------|-------------|
| `monitor_network_changes(callback)` | `Result<MonitorHandle>` | Watch for AP and signal strength changes |
| `monitor_device_changes(callback)` | `Result<MonitorHandle>` | Watch for device state changes |

Call `MonitorHandle::stop().await?` to shut down a monitor cleanly.

## Thread Safety

`NetworkManager` is `Clone`, `Send`, and `Sync`. Clones share the same D-Bus connection.

**Important:** Concurrent connection operations (calling `connect()` from multiple tasks) are not supported. Use `is_connecting()` to guard against this.

## Full API Reference

For complete documentation with all method signatures, see [docs.rs/nmrs](https://docs.rs/nmrs).
