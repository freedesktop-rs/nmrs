# Error Handling

nmrs uses a single error type, `ConnectionError`, for all operations. Each variant describes a specific failure mode, making it straightforward to handle errors precisely.

## The Result Type

nmrs re-exports a `Result` type alias:

```rust
pub type Result<T> = std::result::Result<T, ConnectionError>;
```

All public API methods return `nmrs::Result<T>`.

## ConnectionError Variants

For a complete listing of every variant and its payload, see the
[Error Types reference](../api/errors.md). The tables below group the
most commonly handled variants by category.

### Network & Wi-Fi Errors

| Variant | Description |
|---------|-------------|
| `NotFound` | Network not visible during scan |
| `ApBssidNotFound { ssid, bssid }` | No AP matching both the SSID and BSSID |
| `InvalidBssid(String)` | Invalid BSSID format |
| `AuthFailed` | Wrong password or rejected credentials |
| `MissingPassword` | Empty PSK provided without a saved profile to reuse |
| `NoWifiDevice` | No Wi-Fi adapter found |
| `WifiNotReady` | Wi-Fi device not ready in time |
| `WifiInterfaceNotFound { interface }` | Specified Wi-Fi interface doesn't exist |
| `NotAWifiDevice { interface }` | Interface exists but isn't Wi-Fi |
| `HardwareRadioKilled` | Hardware kill switch is on |
| `NoWiredDevice` | No Ethernet adapter found |
| `DhcpFailed` | Failed to obtain an IP address via DHCP |
| `Timeout` | Operation timed out waiting for activation |
| `Stuck(String)` | Connection stuck in an unexpected state |

### Authentication Errors

| Variant | Description |
|---------|-------------|
| `SupplicantConfigFailed` | wpa_supplicant configuration error |
| `SupplicantTimeout` | wpa_supplicant timed out during auth |

### VPN Errors

| Variant | Description |
|---------|-------------|
| `NoVpnConnection` | No VPN connection (or not active) |
| `VpnNotFound(String)` | VPN connection not found by UUID/name |
| `VpnFailed(String)` | VPN connection failed with details |
| `VpnIdAmbiguous(String)` | Multiple VPNs share the same name; use UUID |
| `IncompleteBuilder(String)` | VPN/Wi-Fi builder missing required fields |
| `InvalidPrivateKey(String)` | Bad WireGuard private key |
| `InvalidPublicKey(String)` | Bad WireGuard public key |
| `InvalidAddress(String)` | Bad IP address or CIDR notation |
| `InvalidGateway(String)` | Bad gateway format (host:port) |
| `InvalidPeers(String)` | Invalid peer configuration |
| `ParseError(OvpnParseError)` | Failed to parse a `.ovpn` file |

### Bluetooth Errors

| Variant | Description |
|---------|-------------|
| `NoBluetoothDevice` | No Bluetooth adapter found |
| `BluezUnavailable(String)` | BlueZ not running or no adapters |
| `BluetoothToggleFailed(String)` | Adapter exists but failed to power on/off |

### Profile & Settings Errors

| Variant | Description |
|---------|-------------|
| `NoSavedConnection` | No saved profile for the requested network |
| `SavedConnectionNotFound(String)` | No saved profile with that UUID |
| `MalformedSavedConnection(String)` | Saved settings missing/invalid keys |
| `InvalidVlanId { id }` | VLAN ID outside `1..=4094` |
| `InvalidInput { field, reason }` | Generic config-field validation failure |

### Connectivity Errors

| Variant | Description |
|---------|-------------|
| `ConnectivityCheckDisabled` | NM connectivity checks are disabled in config |

### Secret Agent Errors

| Variant | Description |
|---------|-------------|
| `AgentRegistration { context }` | Secret agent failed to register with NM |
| `AgentNotRegistered` | Used a handle whose registration was already torn down |
| `AgentAlreadyRegistered` | Secret agent registration conflict |

For applet-style GUI code, register one secret agent during startup and keep
the returned `SecretAgentHandle` alive for the application lifetime. If
NetworkManager restarts, call `SecretAgentHandle::reregister()` after it is
available again. Dropping or unregistering the handle stops credential prompts
from reaching the app.

### Low-Level Errors

| Variant | Description |
|---------|-------------|
| `Dbus(zbus::Error)` | D-Bus communication error |
| `DbusOperation { context, source }` | D-Bus error with context |
| `DeviceFailed(StateReason)` | Device failure with NM reason code |
| `ActivationFailed(ConnectionStateReason)` | Activation failure with reason |
| `InvalidUtf8(Utf8Error)` | Invalid UTF-8 in SSID |

## Basic Error Handling

Use the `?` operator for simple propagation:

```rust
use nmrs::{NetworkManager, WifiSecurity};

#[tokio::main]
async fn main() -> nmrs::Result<()> {
    let nm = NetworkManager::new().await?;
    nm.connect("MyWiFi", None, WifiSecurity::Open).await?;
    Ok(())
}
```

## Pattern Matching

Handle specific errors differently:

```rust
use nmrs::{NetworkManager, WifiSecurity, ConnectionError};

let nm = NetworkManager::new().await?;

match nm.connect("MyWiFi", None, WifiSecurity::WpaPsk {
    psk: "password".into(),
}).await {
    Ok(_) => println!("Connected!"),
    Err(ConnectionError::NotFound) => {
        eprintln!("Network not in range");
    }
    Err(ConnectionError::AuthFailed) => {
        eprintln!("Wrong password");
    }
    Err(ConnectionError::Timeout) => {
        eprintln!("Connection timed out");
    }
    Err(ConnectionError::DhcpFailed) => {
        eprintln!("Connected to AP but DHCP failed");
    }
    Err(ConnectionError::NoWifiDevice) => {
        eprintln!("No Wi-Fi adapter found");
    }
    Err(e) => eprintln!("Unexpected error: {}", e),
}
```

## Retry Logic

Implement retries for transient failures:

```rust
use nmrs::{NetworkManager, WifiSecurity, ConnectionError};

let nm = NetworkManager::new().await?;

for attempt in 1..=3 {
    match nm.connect("MyWiFi", None, WifiSecurity::WpaPsk {
        psk: "password".into(),
    }).await {
        Ok(_) => {
            println!("Connected on attempt {}", attempt);
            break;
        }
        Err(ConnectionError::Timeout) if attempt < 3 => {
            eprintln!("Attempt {} timed out, retrying...", attempt);
            continue;
        }
        Err(e) => return Err(e),
    }
}
```

## VPN Error Handling

```rust
use nmrs::{NetworkManager, ConnectionError};

let nm = NetworkManager::new().await?;

match nm.get_vpn_info("MyVPN").await {
    Ok(info) => println!("VPN IP: {:?}", info.ip4_address),
    Err(ConnectionError::NoVpnConnection) => {
        eprintln!("VPN is not active");
    }
    Err(e) => eprintln!("Error: {}", e),
}
```

## Converting to Other Error Types

`ConnectionError` implements `std::error::Error` and `Display`, so it works with error handling crates like `anyhow`:

```rust
use anyhow::Result;
use nmrs::NetworkManager;

async fn connect() -> Result<()> {
    let nm = NetworkManager::new().await?;
    nm.connect("MyWiFi", None, nmrs::WifiSecurity::Open).await?;
    Ok(())
}
```

### Radio / Airplane-mode Errors

| Variant | Description |
|---------|-------------|
| `HardwareRadioKilled` | Hardware kill switch is on; Wi-Fi cannot be enabled until the switch is toggled |
| `BluezUnavailable(String)` | Bluetooth stack (BlueZ) is not running or no adapters are present |
| `BluetoothToggleFailed(String)` | A BlueZ adapter exists but failed to power on/off when toggling airplane mode |

## Non-Exhaustive

`ConnectionError` is marked `#[non_exhaustive]`, which means new variants may be added in future versions without a breaking change. Always include a wildcard arm in match expressions:

```rust
match result {
    Err(ConnectionError::AuthFailed) => { /* ... */ }
    Err(ConnectionError::NotFound) => { /* ... */ }
    Err(e) => { /* catch-all for current and future variants */ }
    Ok(_) => {}
}
```

## Next Steps

- [WiFi Management](./wifi.md) – Wi-Fi-specific operations
- [VPN Management](./vpn-management.md) – VPN-specific errors
- [Custom Timeouts](../advanced/timeouts.md) – prevent timeout errors
