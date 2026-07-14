# Quick Start

This guide will get you up and running with nmrs in minutes.

## Prerequisites

Make sure you have:
- Rust 1.90.0 or later installed
- NetworkManager running on your Linux system
- Basic familiarity with async Rust

## Create a New Project

```bash
cargo new nmrs-demo
cd nmrs-demo
cargo add nmrs
cargo add tokio --features macros,rt-multi-thread
```

## Your First nmrs Program

Let's create a simple program that lists available Wi-Fi networks:

```rust
use nmrs::NetworkManager;

#[tokio::main]
async fn main() -> nmrs::Result<()> {
    let nm = NetworkManager::new().await?;

    // `None` enumerates every Wi-Fi device. Pass `Some("wlan0")` to scope
    // the listing to a single interface (see "Per-Device Wi-Fi Scoping").
    let networks = nm.list_networks(None).await?;

    for net in networks {
        let kind = if net.is_eap {
            "WPA-EAP"
        } else if net.is_psk {
            "WPA-PSK"
        } else if net.secured {
            "Other"
        } else {
            "Open"
        };

        println!(
            "SSID: {:<20} Signal: {:>3}% Security: {}",
            net.ssid,
            net.strength.unwrap_or(0),
            kind,
        );
    }

    Ok(())
}
```

Run it:

```bash
cargo run
```

You should see output like:

```
SSID: MyHomeNetwork       Signal:  85% Security: WpaPsk
SSID: CoffeeShopWiFi      Signal:  62% Security: Open
SSID: Neighbor5G          Signal:  45% Security: WpaEap
```

## Connecting to a Network

Now let's connect to a WiFi network:

```rust
use nmrs::{NetworkManager, WifiSecurity};

#[tokio::main]
async fn main() -> nmrs::Result<()> {
    let nm = NetworkManager::new().await?;
    
    // Connect to a WPA-PSK protected network
    nm.connect("MyHomeNetwork", None, WifiSecurity::WpaPsk {
        psk: "your_password_here".into()
    }).await?;
    
    println!("Connected successfully!");
    
    // Verify the connection
    if let Some(ssid) = nm.current_ssid().await {
        println!("Current network: {}", ssid);
    }
    
    Ok(())
}
```

## Error Handling

nmrs provides detailed error types for better error handling:

```rust
use nmrs::{NetworkManager, WifiSecurity, ConnectionError};

#[tokio::main]
async fn main() -> nmrs::Result<()> {
    let nm = NetworkManager::new().await?;
    
    match nm.connect("MyNetwork", None, WifiSecurity::WpaPsk {
        psk: "password123".into()
    }).await {
        Ok(_) => println!("✓ Connected successfully"),
        Err(ConnectionError::AuthFailed) => {
            eprintln!("✗ Authentication failed - wrong password?");
        }
        Err(ConnectionError::NotFound) => {
            eprintln!("✗ Network not found or out of range");
        }
        Err(ConnectionError::Timeout) => {
            eprintln!("✗ Connection timed out");
        }
        Err(ConnectionError::DhcpFailed) => {
            eprintln!("✗ Failed to obtain IP address");
        }
        Err(e) => eprintln!("✗ Error: {}", e),
    }
    
    Ok(())
}
```

## Device Management

List all network devices:

```rust
use nmrs::NetworkManager;

#[tokio::main]
async fn main() -> nmrs::Result<()> {
    let nm = NetworkManager::new().await?;
    
    let devices = nm.list_devices().await?;
    
    for device in devices {
        println!(
            "Interface: {:<10} Type: {:<10} State: {:?}",
            device.interface,
            device.device_type,
            device.state
        );
    }
    
    Ok(())
}
```

## Working with Connection Profiles

List saved connection profiles:

```rust
use nmrs::NetworkManager;

#[tokio::main]
async fn main() -> nmrs::Result<()> {
    let nm = NetworkManager::new().await?;
    
    let profiles = nm.list_saved_connection_ids().await?;
    
    println!("Saved connections:");
    for profile in profiles {
        println!("  - {}", profile);
    }
    
    Ok(())
}
```

## Real-Time Monitoring

Monitor network changes:

```rust
use nmrs::NetworkManager;
use std::sync::Arc;

#[tokio::main]
async fn main() -> nmrs::Result<()> {
    let nm = Arc::new(NetworkManager::new().await?);
    let nm_clone = nm.clone();
    
    // Monitor network changes
    let monitor_handle = nm.monitor_network_changes(move || {
        println!("Networks changed! Scanning...");
        // In a real app, you'd update your UI here
    }).await?;

    // Wait a moment for any network changes
    tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;

    // Stop monitoring with a clean shutdown
    monitor_handle.stop().await?;
    println!("Monitoring stopped...");
    Ok(())
}
```

## Complete Example: Network Scanner

Here's a complete example that puts it all together:

```rust
use nmrs::{NetworkManager, WifiSecurity};
use std::io::{self, Write};

#[tokio::main]
async fn main() -> nmrs::Result<()> {
    let nm = NetworkManager::new().await?;
    
    println!("Scanning for networks...\n");
    let networks = nm.list_networks(None).await?;

    for (i, net) in networks.iter().enumerate() {
        println!(
            "{:2}. {:<25} Signal: {:>3}% secured={}",
            i + 1,
            net.ssid,
            net.strength.unwrap_or(0),
            net.secured,
        );
    }

    print!("\nEnter network number to connect (or 0 to exit): ");
    io::stdout().flush().unwrap();

    let mut input = String::new();
    io::stdin().read_line(&mut input).unwrap();
    let choice: usize = input.trim().parse().unwrap_or(0);

    if choice == 0 || choice > networks.len() {
        println!("Exiting...");
        return Ok(());
    }

    let selected = &networks[choice - 1];

    // `Network` exposes `secured` / `is_psk` / `is_eap` rather than a single
    // `WifiSecurity` value. Use them to pick the right authentication path.
    let security = if !selected.secured {
        WifiSecurity::Open
    } else {
        print!("Enter password: ");
        io::stdout().flush().unwrap();
        let mut password = String::new();
        io::stdin().read_line(&mut password).unwrap();
        WifiSecurity::WpaPsk {
            psk: password.trim().to_string(),
        }
    };

    println!("Connecting to {}...", selected.ssid);
    nm.connect(&selected.ssid, None, security).await?;
    println!("Connected successfully");

    Ok(())
}
```

## Next Steps

Now that you've got the basics, explore more features:

- [WiFi Management](../guide/wifi.md) - Advanced WiFi features
- [VPN Connections](../guide/vpn.md) - Set up WireGuard VPNs
- [Device Management](../guide/devices.md) - Control network devices
- [Error Handling](../guide/error-handling.md) - Comprehensive error handling
- [Examples](../examples/wifi-scanner.md) - More complete examples

## Using Different Async Runtimes

`nmrs` is built on top of [`zbus`], which supports several async executors.
The choice of runtime is made by `zbus` at compile time; `nmrs` itself does
not bring its own runtime. The examples in this book use Tokio, but
async-std or smol works just as well.

[`zbus`]: https://docs.rs/zbus

### async-std

```toml
[dependencies]
nmrs = "3.1"
async-std = { version = "1.12", features = ["attributes"] }
```

```rust
#[async_std::main]
async fn main() -> nmrs::Result<()> {
    let nm = nmrs::NetworkManager::new().await?;
    // ... your code
    Ok(())
}
```

### smol

```toml
[dependencies]
nmrs = "3.1"
smol = "2.0"
```

```rust
fn main() -> nmrs::Result<()> {
    smol::block_on(async {
        let nm = nmrs::NetworkManager::new().await?;
        // ... your code
        Ok(())
    })
}
```

See [Async Runtime Support](../advanced/async-runtimes.md) for a deeper look.
