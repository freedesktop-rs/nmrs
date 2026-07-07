# D-Bus Architecture

nmrs communicates with NetworkManager over the [D-Bus](https://dbus.freedesktop.org/doc/dbus-specification.html) system bus. Understanding this architecture helps with debugging and explains why certain operations work the way they do.

## Overview

```
┌─────────────┐     D-Bus (system bus)     ┌──────────────────┐
│   Your App  │ ◄────────────────────────► │  NetworkManager  │
│   (nmrs)    │                            │    Daemon        │
└─────────────┘                            └──────────────────┘
       │                                          │
       │  zbus (Rust D-Bus library)               │
       │                                          │
       ▼                                          ▼
  nmrs::dbus module                    D-Bus interfaces:
  (proxy types)                        - org.freedesktop.NetworkManager
                                       - org.freedesktop.NetworkManager.Device
                                       - org.freedesktop.NetworkManager.Device.Wireless
                                       - org.freedesktop.NetworkManager.AccessPoint
                                       - org.freedesktop.NetworkManager.Connection.Active
                                       - org.freedesktop.NetworkManager.Settings
                                       - ...
```

## How nmrs Uses D-Bus

### Connection Establishment

When you call `NetworkManager::new()`, nmrs connects to the system D-Bus using `zbus`:

```rust
let nm = NetworkManager::new().await?;
// Internally: zbus::Connection::system().await
```

This creates a persistent D-Bus connection that's shared across all operations.

### Advanced access

For builder workflows and other low-level D-Bus calls, nmrs exposes the same
connection through [`NetworkManager::dbus_connection()`](../api/network-manager.md#advanced-d-bus-access).
Pair it with [`nmrs::raw`](../api/raw.md) (`zbus` / `zvariant` re-exports) when
you need D-Bus methods that nmrs does not wrap yet.

For builder output, prefer the high-level
[`add_connection`](../api/network-manager.md#saving-profiles-without-activating)
and
[`add_and_activate_connection`](../api/network-manager.md#activating-builder-output)
methods instead of calling D-Bus directly.

### Method Calls

API methods like `list_devices()` translate to D-Bus method calls:

```
nmrs: nm.list_devices()
  → D-Bus: GetDevices() on org.freedesktop.NetworkManager
  ← D-Bus: Array of device object paths
  → D-Bus: Get properties for each device path
  ← D-Bus: Device properties (interface, type, state, etc.)
  → nmrs: Vec<Device>
```

### Signal Monitoring

`monitor_network_changes()` subscribes to D-Bus signals:

```
nmrs: nm.monitor_network_changes(callback)
  → D-Bus: Subscribe to AccessPointAdded/Removed and AP Strength changes
  ← D-Bus: Signal whenever an AP appears, disappears, or changes strength
  → nmrs: Invoke callback
```

### Connection Settings

When connecting to a network, nmrs builds a settings dictionary and sends it via D-Bus:

```
nmrs: nm.connect("MyWiFi", None, WifiSecurity::WpaPsk { psk: "..." })
  → Build settings HashMap
  → D-Bus: AddAndActivateConnection(settings, device_path, specific_object)
  ← D-Bus: Active connection path
  → D-Bus: Monitor StateChanged signal
  ← D-Bus: State transitions until Activated or Failed
  → nmrs: Ok(()) or Err(ConnectionError)
```

## D-Bus Proxy Types

nmrs wraps D-Bus interfaces in typed proxy structs (defined in the internal `dbus` module):

| Proxy | D-Bus Interface | Purpose |
|-------|----------------|---------|
| `NMProxy` | `org.freedesktop.NetworkManager` | Main NM interface |
| `NMDeviceProxy` | `org.freedesktop.NetworkManager.Device` | Device properties and control |
| `NMWirelessProxy` | `org.freedesktop.NetworkManager.Device.Wireless` | Wi-Fi scanning, AP list |
| `NMAccessPointProxy` | `org.freedesktop.NetworkManager.AccessPoint` | AP signal, SSID, security |
| `NMActiveConnectionProxy` | `org.freedesktop.NetworkManager.Connection.Active` | Active connection state |
| `NMWiredProxy` | `org.freedesktop.NetworkManager.Device.Wired` | Wired device properties |
| `NMBluetoothProxy` | `org.freedesktop.NetworkManager.Device.Bluetooth` | Bluetooth properties |

These proxy types are **not** part of the public API. Normal callers interact
with NetworkManager through the high-level [`NetworkManager`](../api/network-manager.md)
methods.

Advanced callers can define their own minimal `#[zbus::proxy]` traits on top of
[`dbus_connection()`](../api/network-manager.md#advanced-d-bus-access) and
[`nmrs::raw`](../api/raw.md). For builder output, use
[`add_connection`](../api/network-manager.md#saving-profiles-without-activating)
or
[`add_and_activate_connection`](../api/network-manager.md#activating-builder-output)
first.

## D-Bus Errors

D-Bus communication errors surface as `ConnectionError::Dbus` or `ConnectionError::DbusOperation`:

```rust
use nmrs::ConnectionError;

match result {
    Err(ConnectionError::Dbus(e)) => {
        eprintln!("D-Bus error: {}", e);
    }
    Err(ConnectionError::DbusOperation { context, source }) => {
        eprintln!("{}: {}", context, source);
    }
    _ => {}
}
```

Common causes:
- NetworkManager is not running
- D-Bus system bus is not available
- Insufficient permissions (PolicyKit)

## Permissions

NetworkManager uses PolicyKit for authorization. Most operations require the user to be in the `network` group or to have appropriate PolicyKit rules. See [Requirements](../getting-started/requirements.md) for setup details.

## Debugging D-Bus

### Monitor D-Bus Traffic

Use `dbus-monitor` to see raw D-Bus messages:

```bash
sudo dbus-monitor --system "interface='org.freedesktop.NetworkManager'"
```

### Check NetworkManager State

```bash
nmcli general status
nmcli device status
nmcli connection show
```

### Verify D-Bus Service

```bash
busctl list | grep NetworkManager
```

## Next Steps

- [Raw Module](../api/raw.md) – `zbus` / `zvariant` re-exports for advanced callers
- [Builders Module](../api/builders.md) – construct and submit custom settings dictionaries
- [Logging and Debugging](./logging.md) – enable nmrs debug logging
- [Architecture](../development/architecture.md) – internal code structure
