# Device Management

nmrs provides methods to list, inspect, and control network devices managed by NetworkManager.

## Listing All Devices

```rust
use nmrs::NetworkManager;

#[tokio::main]
async fn main() -> nmrs::Result<()> {
    let nm = NetworkManager::new().await?;

    let devices = nm.list_devices().await?;
    for device in &devices {
        println!("{}", device); // "wlan0 (Wi-Fi) [Activated]"
    }

    Ok(())
}
```

## The Device Struct

Each device provides the following information:

| Field | Type | Description |
|-------|------|-------------|
| `path` | `String` | D-Bus object path |
| `interface` | `String` | Interface name (e.g., `wlan0`, `eth0`) |
| `identity` | `DeviceIdentity` | MAC addresses (permanent and current) |
| `device_type` | `DeviceType` | Type of device |
| `state` | `DeviceState` | Current operational state |
| `managed` | `Option<bool>` | Whether NetworkManager manages this device |
| `driver` | `Option<String>` | Kernel driver name |
| `ip4_address` | `Option<String>` | IPv4 address with CIDR (when connected) |
| `ip6_address` | `Option<String>` | IPv6 address with CIDR (when connected) |
| `frequency` | `Option<u32>` | Active Wi-Fi frequency in MHz (Wi-Fi only) |
| `speed_mbps` | `Option<u32>` | Raw Ethernet link speed in Mb/s (Ethernet only) |

`speed_mbps` preserves NetworkManager's raw value. Some Ethernet drivers
report `0` when no carrier is present.

## Device Types

```rust
use nmrs::DeviceType;
```

| Variant | Description |
|---------|-------------|
| `DeviceType::Wifi` | Wi-Fi (802.11) wireless adapter |
| `DeviceType::Ethernet` | Wired Ethernet interface |
| `DeviceType::Bluetooth` | Bluetooth network device |
| `DeviceType::WifiP2P` | Wi-Fi Direct (peer-to-peer) |
| `DeviceType::Loopback` | Loopback interface (localhost) |
| `DeviceType::Vlan` | 802.1Q virtual VLAN |
| `DeviceType::Other(u32)` | Unknown type with raw code |

Devices that NetworkManager reports as `veth` are normalized to
`DeviceType::Ethernet`. They return `true` from `is_wired()` and appear in
`list_wired_devices()` and `list_wired_device_details()`.

### Type Helper Methods

```rust
let device = &devices[0];

if device.is_wireless() {
    println!("{} is a Wi-Fi adapter", device.interface);
}

if device.is_wired() {
    println!("{} is an Ethernet interface", device.interface);
}

if device.is_bluetooth() {
    println!("{} is a Bluetooth device", device.interface);
}

if device.is_loopback() {
    println!("{} is the loopback interface", device.interface);
}

if device.is_vlan() {
    println!("{} is a VLAN", device.interface);
}
```

`DeviceType` also provides capability queries:

```rust
let dt = &device.device_type;

dt.supports_scanning();         // true for Wifi, WifiP2P
dt.requires_specific_object();  // true for Wifi, WifiP2P
dt.has_global_enabled_state();  // true for Wifi
dt.connection_type_str();       // "802-11-wireless", "802-3-ethernet", etc.
dt.to_code();                   // raw NM type code (2 for Wifi, 1 for Ethernet)
```

## Wired Details

For Ethernet-specific UI rows, use `list_wired_device_details()` instead of
falling back to raw D-Bus:

```rust
use nmrs::NetworkManager;

let nm = NetworkManager::new().await?;

for device in nm.list_wired_device_details().await? {
    println!("{} {:?}", device.interface, device.state);
    println!("  MAC: {}", device.hw_address);
    println!("  speed: {:?}", device.speed_mbps);
    println!("  connection: {:?}", device.active_connection_id);
    println!("  IPv4: {:?}", device.ip4_address);
}
```

## Device States

```rust
use nmrs::DeviceState;
```

| State | Description |
|-------|-------------|
| `Unmanaged` | Not managed by NetworkManager |
| `Unavailable` | Managed but not ready (e.g., Wi-Fi disabled) |
| `Disconnected` | Available but not connected |
| `Prepare` | Preparing to connect |
| `Config` | Being configured |
| `NeedAuth` | Waiting for credentials |
| `IpConfig` | Requesting IP configuration |
| `IpCheck` | Verifying IP connectivity |
| `Secondaries` | Waiting for secondary connections |
| `Activated` | Fully connected and operational |
| `Deactivating` | Disconnecting |
| `Failed` | Connection failed |
| `Other(u32)` | Unknown state with raw code |

### Transitional States

Use `is_transitional()` to check if a device is in a connecting or disconnecting state:

```rust
if device.state.is_transitional() {
    println!("{} is in a transitional state: {}", device.interface, device.state);
}
```

Transitional states include: `Prepare`, `Config`, `NeedAuth`, `IpConfig`, `IpCheck`, `Secondaries`, and `Deactivating`.

## Filtered Device Lists

```rust
let nm = NetworkManager::new().await?;

// Only wireless devices
let wireless = nm.list_wireless_devices().await?;

// Only wired devices
let wired = nm.list_wired_devices().await?;

// Only Bluetooth devices (returns BluetoothDevice, not Device)
let bluetooth = nm.list_bluetooth_devices().await?;
```

## Wi-Fi Radio Control

Check and control the Wi-Fi radio globally:

```rust
let nm = NetworkManager::new().await?;

// Check current state (software + hardware)
let state = nm.wifi_state().await?;
println!("Wi-Fi enabled: {}", state.enabled);
println!("Wi-Fi hardware enabled: {}", state.hardware_enabled);

// Global toggle
nm.set_wireless_enabled(false).await?;  // Disable
nm.set_wireless_enabled(true).await?;   // Enable
```

> **Note:** `wifi_state().hardware_enabled` reflects the rfkill state. If the hardware switch is off, enabling Wi-Fi via software will have no effect.

For per-device Wi-Fi enable/disable, see [Per-Device Scoping](./wifi-per-device.md).

## Waiting for Wi-Fi Ready

After enabling Wi-Fi, the device may take a moment to become ready:

```rust
let nm = NetworkManager::new().await?;

nm.set_wireless_enabled(true).await?;
nm.wait_for_wifi_ready().await?;

// Now safe to scan and connect
nm.scan_networks(None).await?;
```

## Finding a Device by Interface Name

```rust
let nm = NetworkManager::new().await?;

let device_path = nm.get_device_by_interface("wlan0").await?;
println!("D-Bus path: {}", device_path.as_str());
```

## Device Identity

Each device has both a permanent (factory) and current MAC address:

```rust
for device in nm.list_devices().await? {
    println!("{}: permanent={}, current={}",
        device.interface,
        device.identity.permanent_mac,
        device.identity.current_mac,
    );
}
```

If MAC randomization is enabled, the current MAC will differ from the permanent one.

## Checking Connection Progress

Before starting a new connection, check if any device is currently connecting:

```rust
if nm.is_connecting().await? {
    println!("A connection operation is in progress");
}
```

## Next Steps

- [WiFi Management](./wifi.md) – Wi-Fi-specific operations
- [Bluetooth](./bluetooth.md) – Bluetooth device management
- [Ethernet Management](./ethernet.md) – wired connections
- [Real-Time Monitoring](./monitoring.md) – subscribe to device state changes
