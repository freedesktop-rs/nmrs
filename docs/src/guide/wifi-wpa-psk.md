# WPA-PSK Networks

WPA-PSK (Wi-Fi Protected Access with Pre-Shared Key) is the most common security type for home and small-office Wi-Fi networks. You provide a password, and nmrs handles the WPA handshake.

## Connecting with a Password

```rust
use nmrs::{NetworkManager, WifiSecurity};

#[tokio::main]
async fn main() -> nmrs::Result<()> {
    let nm = NetworkManager::new().await?;

    nm.connect("HomeWiFi", None, WifiSecurity::WpaPsk {
        psk: "my_secure_password".into(),
    }).await?;

    println!("Connected!");
    Ok(())
}
```

The `WifiSecurity::WpaPsk` variant works with WPA, WPA2, and WPA3 Personal networks. NetworkManager negotiates the strongest supported protocol automatically.

## Password Requirements

- A new profile requires a non-empty password. An empty PSK requests the stored
  password only when a saved profile already exists; otherwise nmrs returns
  `ConnectionError::MissingPassword`.
- WPA-PSK passwords are typically 8–63 characters (ASCII passphrase) or exactly 64 hex characters (raw PSK)
- nmrs passes the password directly to NetworkManager, which handles validation

## Reading the Password at Runtime

Avoid hardcoding passwords. Read them from environment variables, user input, or a secrets manager:

```rust
use nmrs::{NetworkManager, WifiSecurity};

#[tokio::main]
async fn main() -> nmrs::Result<()> {
    let nm = NetworkManager::new().await?;

    let password = std::env::var("WIFI_PASSWORD")
        .expect("Set WIFI_PASSWORD environment variable");

    nm.connect("HomeWiFi", None, WifiSecurity::WpaPsk {
        psk: password,
    }).await?;

    Ok(())
}
```

## Reconnecting to Saved Networks

After the first successful connection, NetworkManager saves the credentials in
a connection profile. Request its stored password with an empty PSK:

```rust
let nm = NetworkManager::new().await?;

if nm.has_saved_connection("HomeWiFi").await? {
    nm.connect("HomeWiFi", None, WifiSecurity::WpaPsk {
        psk: String::new(),
    }).await?;
}
```

`WifiSecurity::Open` also reuses an existing saved profile. A non-empty PSK is
not ignored: it asks nmrs to build a fresh profile with that password. If
activation with the empty-PSK stored-secret request fails, the original saved
profile is preserved.

## Error Handling

The most common errors for WPA-PSK connections:

| Error | Meaning |
|-------|---------|
| `ConnectionError::AuthFailed` | Wrong password |
| `ConnectionError::MissingPassword` | Empty password string and no saved profile to reuse |
| `ConnectionError::NotFound` | Network not in range |
| `ConnectionError::Timeout` | Connection took too long |
| `ConnectionError::DhcpFailed` | Connected to AP but DHCP failed |

```rust
use nmrs::{NetworkManager, WifiSecurity, ConnectionError};

let nm = NetworkManager::new().await?;

match nm.connect("HomeWiFi", None, WifiSecurity::WpaPsk {
    psk: "password".into(),
}).await {
    Ok(_) => println!("Connected!"),
    Err(ConnectionError::AuthFailed) => {
        eprintln!("Wrong password — check and try again");
    }
    Err(ConnectionError::MissingPassword) => {
        eprintln!("No saved password is available; provide a non-empty PSK");
    }
    Err(e) => eprintln!("Error: {}", e),
}
```

## Next Steps

- [WPA-EAP (Enterprise)](./wifi-enterprise.md) – for corporate/university networks
- [Hidden Networks](./wifi-hidden.md) – connecting to non-broadcast SSIDs
- [Connection Profiles](./profiles.md) – managing saved connections
