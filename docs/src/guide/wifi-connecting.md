# Connecting to Networks

This page covers the general flow for connecting to Wi-Fi networks with nmrs. For security-specific details, see the dedicated pages on [WPA-PSK](./wifi-wpa-psk.md), [WPA-EAP](./wifi-enterprise.md), and [Hidden Networks](./wifi-hidden.md).

## Basic Connection Flow

Connecting to a Wi-Fi network requires two things: the SSID and the security credentials.

```rust
use nmrs::{NetworkManager, WifiSecurity};

#[tokio::main]
async fn main() -> nmrs::Result<()> {
    let nm = NetworkManager::new().await?;

    // Open network (no password)
    nm.connect("CafeWiFi", None, WifiSecurity::Open).await?;

    // WPA-PSK network (password)
    nm.connect("HomeWiFi", None, WifiSecurity::WpaPsk {
        psk: "my_password".into(),
    }).await?;

    Ok(())
}
```

## What Happens During Connect

When you call `connect()`, nmrs performs the following steps:

1. **Validates** the SSID and credentials
2. **Searches** for the network among visible access points
3. **Checks** for a saved connection profile matching the SSID
4. **Reuses** the saved profile for `Open` or empty-PSK requests, or **builds** a
   fresh profile when explicit PSK or EAP credentials are supplied
5. **Activates** the connection via NetworkManager
6. **Waits** for the device to reach the `Activated` state
7. **Returns** `Ok(())` on success, or a specific error on failure

The entire process respects the configured [timeout](../advanced/timeouts.md). The default connection timeout is 30 seconds.

## Checking Connection State

### Current Network

```rust
let nm = NetworkManager::new().await?;

// Get the full Network object
if let Some(network) = nm.current_network().await? {
    println!("Connected to: {} ({}%)",
        network.ssid,
        network.strength.unwrap_or(0),
    );
}

// Or just the SSID
if let Some(ssid) = nm.current_ssid().await {
    println!("SSID: {}", ssid);
}

// SSID + frequency
if let Some((ssid, freq)) = nm.current_connection_info().await {
    println!("Connected to {} at {:?} MHz", ssid, freq);
}
```

### Check If Connected to a Specific Network

```rust
if nm.is_connected("HomeWiFi").await? {
    println!("Already connected to HomeWiFi");
}
```

### Check If a Connection Is In Progress

Before starting a new connection, check if one is already underway. Concurrent connection attempts are not supported and may cause undefined behavior.

```rust
if nm.is_connecting().await? {
    eprintln!("A connection is already in progress");
    return Ok(());
}

nm.connect("HomeWiFi", None, WifiSecurity::WpaPsk {
    psk: "password".into(),
}).await?;
```

## Disconnecting

```rust
let nm = NetworkManager::new().await?;

// Disconnect from the current Wi-Fi network
nm.disconnect(None).await?;
```

`disconnect()` deactivates the current wireless connection and waits for the device to reach the `Disconnected` state. If no connection is active, it returns `Ok(())`.

## Saved Connections

When nmrs connects to a network, NetworkManager saves a connection profile. A
later call reuses that profile when `security` is `WifiSecurity::Open` or an
empty `WifiSecurity::WpaPsk`. Passing a non-empty PSK or an EAP configuration
instead tells nmrs to build a fresh profile with those credentials.

```rust
let nm = NetworkManager::new().await?;

// Check if a profile exists
if nm.has_saved_connection("HomeWiFi").await? {
    println!("Profile exists — stored settings can be reused");
}

// WifiSecurity::Open requests reuse when a saved profile exists.
nm.connect("HomeWiFi", None, WifiSecurity::Open).await?;
```

For a saved WPA-PSK network, an empty PSK also explicitly requests the stored
secret. If that activation fails, nmrs preserves the saved profile so it can be
retried, inspected, or removed with `forget()`.

See [Connection Profiles](./profiles.md) for more on managing saved connections.

## Error Handling

`connect()` returns specific error variants for different failure modes:

```rust
use nmrs::{NetworkManager, WifiSecurity, ConnectionError};

let nm = NetworkManager::new().await?;

match nm.connect("MyNetwork", None, WifiSecurity::WpaPsk {
    psk: "password".into(),
}).await {
    Ok(_) => println!("Connected!"),
    Err(ConnectionError::NotFound) => {
        eprintln!("Network not visible — is it in range?");
    }
    Err(ConnectionError::AuthFailed) => {
        eprintln!("Wrong password");
    }
    Err(ConnectionError::Timeout) => {
        eprintln!("Connection timed out — try increasing the timeout");
    }
    Err(ConnectionError::DhcpFailed) => {
        eprintln!("Failed to get an IP address");
    }
    Err(e) => eprintln!("Connection failed: {}", e),
}
```

See [Error Handling](./error-handling.md) for a full reference of error types.

## Next Steps

- [WPA-PSK Networks](./wifi-wpa-psk.md) – password-protected home/office networks
- [WPA-EAP (Enterprise)](./wifi-enterprise.md) – corporate/university 802.1X networks
- [Hidden Networks](./wifi-hidden.md) – connecting to non-broadcast SSIDs
- [Error Handling](./error-handling.md) – comprehensive error handling guide
