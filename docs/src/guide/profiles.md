# Connection Profiles

NetworkManager stores connection profiles for every network you've connected to. These profiles contain the configuration needed to reconnect — SSID, credentials, IP settings, and more. nmrs provides methods to list, query, and remove these profiles.

## Listing Saved Connections

`nmrs` exposes three flavors of "list the saved profiles", trading off
detail for cost:

| Method | Cost | Returns |
|--------|------|---------|
| [`list_saved_connections`](../api/network-manager.md#connection-profile-methods) | One `GetSettings` call per profile, full decode | `Vec<SavedConnection>` |
| [`list_saved_connections_brief`](../api/network-manager.md#connection-profile-methods) | One `GetSettings` per profile, minimal decode | `Vec<SavedConnectionBrief>` |
| [`list_saved_connection_ids`](../api/network-manager.md#connection-profile-methods) | Same calls as `_brief`, only the names | `Vec<String>` |

```rust
use nmrs::NetworkManager;

#[tokio::main]
async fn main() -> nmrs::Result<()> {
    let nm = NetworkManager::new().await?;

    // Full decode — useful for showing IP / autoconnect / security in a UI.
    for conn in nm.list_saved_connections().await? {
        println!("  {:<32} {:<12} {}", conn.id, conn.connection_type, conn.uuid);
    }

    // Lightweight — just names and types, useful for menus.
    for brief in nm.list_saved_connections_brief().await? {
        println!("  {:<32} ({})", brief.id, brief.connection_type);
    }

    Ok(())
}
```

Each `SavedConnection` includes the profile `id` (display name), `uuid`,
`connection_type` (`"802-11-wireless"`, `"vpn"`, `"wireguard"`, `"bluetooth"`,
…), and a decoded [`SettingsSummary`](../api/models.md#settingspatch).

## Checking for a Saved Connection

```rust
let nm = NetworkManager::new().await?;

if nm.has_saved_connection("HomeWiFi").await? {
    println!("Profile exists for HomeWiFi");
} else {
    println!("No saved profile — credentials will be needed");
}
```

## How Saved Profiles Affect Connection

When you call `connect()` with an SSID that has a saved profile, the supplied
`WifiSecurity` selects reuse or a fresh credential path:

- `WifiSecurity::Open` activates the saved profile.
- `WifiSecurity::WpaPsk` with an empty PSK activates the saved profile using its
  stored secret.
- `WifiSecurity::WpaPsk` with a non-empty PSK builds a fresh profile using that
  password.
- `WifiSecurity::WpaEap` and `WifiSecurity::Wpa3Eap192bit` build a fresh profile
  using the supplied EAP configuration.

Reusing a profile preserves its autoconnect, priority, IP configuration, and
other saved settings. Explicit fresh credentials are never silently ignored.

```rust
let nm = NetworkManager::new().await?;

// First connection — credentials are required and saved
nm.connect("HomeWiFi", None, WifiSecurity::WpaPsk {
    psk: "password".into(),
}).await?;

// Later reconnection — request the PSK stored in the saved profile
nm.connect("HomeWiFi", None, WifiSecurity::WpaPsk {
    psk: String::new(),
}).await?;
```

If activation with an empty-PSK stored-secret request fails, nmrs returns the
activation error without deleting the saved profile. The caller can retry,
inspect the profile, supply a non-empty replacement PSK, or remove it explicitly
with `forget()`.

If a saved profile is missing or has stale secrets, NetworkManager may ask a
registered secret agent for credentials during activation. GUI apps should
register one long-lived agent at startup, keep the returned handle alive for
the process lifetime, and re-register after NetworkManager restarts:

```rust
use futures::StreamExt;
use nmrs::agent::{SecretAgent, SecretAgentFlags, SecretSetting};

let (handle, mut requests) = SecretAgent::builder()
    .with_identifier("com.system76.CosmicAppletNetwork")
    .register()
    .await?;

tokio::spawn(async move {
    while let Some(request) = requests.next().await {
        if !request.flags.contains(SecretAgentFlags::ALLOW_INTERACTION) {
            let _ = request.responder.no_secrets().await;
            continue;
        }

        match request.setting {
            SecretSetting::WifiPsk { ref ssid } => {
                println!("prompt for password: {ssid}");
                let _ = request.responder.cancel().await;
            }
            _ => {
                let _ = request.responder.cancel().await;
            }
        }
    }
});

// Keep `handle` alive while the applet is running.
// Call `handle.reregister().await?` after NetworkManager restarts.
```

## Forgetting (Deleting) Connections

### Wi-Fi Connections

```rust
let nm = NetworkManager::new().await?;

nm.forget("HomeWiFi").await?;
println!("Wi-Fi profile deleted");
```

If currently connected to that network, `forget()` disconnects first, then deletes all saved profiles matching the SSID.

### VPN Connections

```rust
nm.forget_vpn("MyVPN").await?;
```

### Bluetooth Connections

```rust
nm.forget_bluetooth("My Phone").await?;
```

## Loading a Single Profile by UUID

```rust
let nm = NetworkManager::new().await?;

let profile = nm.get_saved_connection("a1b2c3d4-...").await?;
println!("{} ({}) autoconnect={}", profile.id, profile.connection_type, profile.autoconnect);
```

For the raw `GetSettings` map (advanced consumers building their own
decoder), use `nm.get_saved_connection_raw(uuid)`.

## Updating a Profile

`update_saved_connection` merges a [`SettingsPatch`](../api/models.md#settingspatch)
into an existing profile via NM's `Update` / `UpdateUnsaved` methods.
This is the right call to flip `autoconnect`, change a priority, or
update DNS without rebuilding the entire profile.

**Important:** the first argument is the profile **UUID** (`connection.uuid`),
not the Wi-Fi SSID. [`Network`](../api/types.md#network) values from a scan
do not include the profile UUID — resolve it first.

### Look up UUID by profile name (SSID)

For Wi-Fi profiles, `connection.id` is usually the SSID. The same string
works with `has_saved_connection`, `forget`, and
[`get_saved_connection_uuid`](../api/network-manager.md#connection-profile-methods):

```rust
use nmrs::{NetworkManager, SettingsPatch};

let nm = NetworkManager::new().await?;

if let Some(uuid) = nm.get_saved_connection_uuid("HomeWiFi").await? {
    let mut patch = SettingsPatch::default();
    patch.autoconnect = Some(false);
    nm.update_saved_connection(&uuid, patch).await?;
}
```

### Update while listing saved profiles

When iterating [`list_saved_connections`](../api/network-manager.md#connection-profile-methods),
each [`SavedConnection`](../api/models.md#savedconnection) already carries `uuid`
and `id`. Match on `id` (or compare against your target SSID) and pass
**`saved.uuid`** to `update_saved_connection`:

```rust
use nmrs::{NetworkManager, SettingsPatch};

let nm = NetworkManager::new().await?;
let target = "HomeWiFi";

for saved in nm.list_saved_connections().await? {
    if saved.id == target {
        let mut patch = SettingsPatch::default();
        patch.autoconnect = Some(!saved.autoconnect);
        nm.update_saved_connection(&saved.uuid, patch).await?;
    }
}
```

Common mistake: using a scanned [`Network`](../api/types.md#network)'s `ssid`
where a UUID is required, or calling `update_saved_connection(&network.ssid, …)`.
There is no `uuid` field on `Network` — use `get_saved_connection_uuid` or
`SavedConnection::uuid` instead.

## Deleting by UUID

When the profile UUID is known, you can delete it directly:

```rust
nm.delete_saved_connection("a1b2c3d4-...").await?;
```

For SSID-based deletion (which also disconnects first if active), use
`forget`, `forget_vpn`, or `forget_bluetooth` as shown above.

## Reloading from Disk

If you've edited keyfiles in `/etc/NetworkManager/system-connections/`
out-of-band, ask NetworkManager to re-read them:

```rust
nm.reload_saved_connections().await?;
```

## Getting the D-Bus Path

For advanced use cases, you can retrieve the D-Bus object path of a saved
connection by SSID:

```rust
let nm = NetworkManager::new().await?;

if let Some(path) = nm.get_saved_connection_path("HomeWiFi").await? {
    println!("D-Bus path: {}", path.as_str());
}
```

## Profile Lifecycle

1. **Created** — when you first connect to a network, NetworkManager creates a profile
2. **Persisted** — profiles are saved to `/etc/NetworkManager/system-connections/`
3. **Reused** — `WifiSecurity::Open` or an empty PSK activates the saved profile
4. **Rebuilt** — a non-empty PSK or EAP configuration creates a fresh profile
5. **Preserved on stored-secret failure** — a failed empty-PSK activation does not delete the saved profile
6. **Deleted** — calling `forget()`, `forget_vpn()`, or `forget_bluetooth()` removes it

## Next Steps

- [WiFi Management](./wifi.md) – scan and connect to Wi-Fi networks
- [VPN Management](./vpn-management.md) – manage VPN profiles
- [Bluetooth](./bluetooth.md) – Bluetooth connection profiles
