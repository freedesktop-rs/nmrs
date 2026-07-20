# WiFi Auto-Connect

This example demonstrates a program that automatically connects to a preferred network from a priority-ordered list.

## Features

- Scans for available networks
- Matches against a list of preferred networks (in priority order)
- Connects to the highest-priority available network
- Falls back to lower-priority networks if the preferred one isn't found

## Code

```rust
use nmrs::{NetworkManager, WifiSecurity, ConnectionError};
use std::collections::HashMap;

struct PreferredNetwork {
    ssid: String,
    security: WifiSecurity,
}

#[tokio::main]
async fn main() -> nmrs::Result<()> {
    let nm = NetworkManager::new().await?;

    // Define preferred networks in priority order (highest first)
    let preferred = vec![
        PreferredNetwork {
            ssid: "HomeWiFi".into(),
            security: WifiSecurity::WpaPsk {
                psk: std::env::var("HOME_WIFI_PSK").unwrap_or_default(),
            },
        },
        PreferredNetwork {
            ssid: "OfficeWiFi".into(),
            security: WifiSecurity::WpaPsk {
                psk: std::env::var("OFFICE_WIFI_PSK").unwrap_or_default(),
            },
        },
        PreferredNetwork {
            ssid: "CafeOpen".into(),
            security: WifiSecurity::Open,
        },
    ];

    // Check if already connected
    if let Some(ssid) = nm.current_ssid().await {
        if preferred.iter().any(|p| p.ssid == ssid) {
            println!("Already connected to preferred network: {}", ssid);
            return Ok(());
        }
    }

    // Scan and list visible networks
    println!("Scanning for networks...");
    nm.scan_networks(None).await?;
    let visible = nm.list_networks(None).await?;

    let visible_ssids: HashMap<&str, &nmrs::Network> = visible
        .iter()
        .map(|n| (n.ssid.as_str(), n))
        .collect();

    // Try each preferred network in order
    for pref in &preferred {
        if let Some(net) = visible_ssids.get(pref.ssid.as_str()) {
            println!(
                "Found '{}' ({}%) — connecting...",
                pref.ssid,
                net.strength.unwrap_or(0),
            );

            match nm.connect(&pref.ssid, None, pref.security.clone()).await {
                Ok(_) => {
                    println!("Connected to '{}'!", pref.ssid);
                    return Ok(());
                }
                Err(ConnectionError::AuthFailed) => {
                    eprintln!("Auth failed for '{}', trying next...", pref.ssid);
                    continue;
                }
                Err(e) => {
                    eprintln!("Failed to connect to '{}': {}", pref.ssid, e);
                    continue;
                }
            }
        }
    }

    eprintln!("No preferred networks found");
    Ok(())
}
```

## Running

```bash
HOME_WIFI_PSK="my_home_password" OFFICE_WIFI_PSK="office_pass" cargo run --example wifi_auto_connect
```

## How It Works

1. Checks if already connected to a preferred network
2. Scans for visible networks
3. Iterates through the preferred list in order
4. Attempts to connect to the first match
5. On auth failure, tries the next preferred network
6. Reports if no preferred network was found

## Enhancements

- **Persistent loop:** Wrap in a loop with a timer to continuously monitor and reconnect
- **Signal threshold:** Skip networks below a minimum signal strength
- **Saved profiles:** After `has_saved_connection()` succeeds, pass
  `WifiSecurity::Open` or an empty PSK to request the profile's stored settings
- **Monitoring:** Use `monitor_network_changes()` to react to new networks appearing
