# Ethernet Management

nmrs supports wired (Ethernet) connections through NetworkManager. Ethernet connections are simpler than Wi-Fi since they don't require authentication in most cases. NetworkManager `veth` devices are included in these wired APIs and treated as Ethernet devices.

## Connecting

```rust
use nmrs::NetworkManager;

#[tokio::main]
async fn main() -> nmrs::Result<()> {
    let nm = NetworkManager::new().await?;

    nm.connect_wired().await?;
    println!("Ethernet connected!");

    Ok(())
}
```

`connect_wired()` finds the first available wired device and either activates an existing saved connection or creates a new one with DHCP. The connection will activate when a cable is plugged in.

## Listing Wired Devices

```rust
use nmrs::NetworkManager;

#[tokio::main]
async fn main() -> nmrs::Result<()> {
    let nm = NetworkManager::new().await?;

    let wired = nm.list_wired_devices().await?;
    for device in &wired {
        println!("{}: {} [{:?}]",
            device.interface,
            device.device_type,
            device.state,
        );
        println!("  MAC: {}", device.identity.current_mac);
        if let Some(ip) = &device.ip4_address {
            println!("  IPv4: {}", ip);
        }
        if let Some(driver) = &device.driver {
            println!("  Driver: {}", driver);
        }
        if let Some(speed) = device.speed_mbps {
            println!("  Speed: {} Mb/s", speed);
        }
    }

    Ok(())
}
```

For Ethernet-specific rows, `list_wired_device_details()` includes the raw
NetworkManager link speed, active connection id, MAC addresses, state, and IP
addresses:

```rust
use nmrs::NetworkManager;

#[tokio::main]
async fn main() -> nmrs::Result<()> {
    let nm = NetworkManager::new().await?;

    for device in nm.list_wired_device_details().await? {
        println!("{} [{:?}]", device.interface, device.state);
        println!("  MAC: {}", device.hw_address);
        println!("  permanent MAC: {:?}", device.permanent_hw_address);
        println!("  speed: {:?} Mb/s", device.speed_mbps);
        println!("  active profile: {:?}", device.active_connection_id);
        println!("  IPv4: {:?}", device.ip4_address);
        println!("  IPv6: {:?}", device.ip6_address);
    }

    Ok(())
}
```

`speed_mbps` preserves NetworkManager's raw value. Some drivers report `0`
when no carrier is present.

## Errors

| Error | Meaning |
|-------|---------|
| `ConnectionError::NoWiredDevice` | No Ethernet adapter found |
| `ConnectionError::Timeout` | DHCP or activation took too long |
| `ConnectionError::DhcpFailed` | Failed to obtain an IP address |

```rust
use nmrs::{NetworkManager, ConnectionError};

let nm = NetworkManager::new().await?;

match nm.connect_wired().await {
    Ok(_) => println!("Connected"),
    Err(ConnectionError::NoWiredDevice) => {
        eprintln!("No Ethernet adapter found");
    }
    Err(e) => eprintln!("Error: {}", e),
}
```

## How It Works

When you call `connect_wired()`:

1. nmrs finds the first managed wired device
2. Checks for an existing saved connection for that device
3. If a saved connection exists, activates it
4. If no saved connection exists, creates a new profile with DHCP and activates it
5. Waits for the connection to reach `Activated` state

The connection profile is saved for future use, so the device will auto-connect when a cable is plugged in.

## Next Steps

- [Device Management](./devices.md) – list all network devices
- [Connection Profiles](./profiles.md) – manage saved Ethernet profiles
- [Error Handling](./error-handling.md) – handle connection errors
