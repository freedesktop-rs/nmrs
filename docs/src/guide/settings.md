# Typed, Editable Settings

The [`nmrs::settings`](../../api/settings.md) module mirrors NetworkManager's
`a{sa{sv}}` settings dictionary with strongly typed Rust structs. Unlike the
read-only decode in [`SavedConnection`](../../api/models.md), these structs are
*editable*: you can load a profile, mutate individual fields, and write a merge
patch back.

Every connection type has a corresponding `*Settings` struct:

| Type | Accessor | Section |
|------|----------|---------|
| Wi-Fi | `nm.wifi_settings(uuid)` | `WiFiSettings` |
| Ethernet | `nm.ethernet_settings(uuid)` | `EthernetSettings` |
| WireGuard | `nm.wireguard_settings(uuid)` | `WireGuardSettings` |
| VPN / OpenVPN | `nm.vpn_settings(uuid)` / `nm.openvpn_settings(uuid)` | `VpnSettings` |
| Bluetooth | `nm.bluetooth_settings(uuid)` | `BluetoothSettings` |
| Bond | `nm.bond_settings(uuid)` | `BondSettings` |
| Bridge | `nm.bridge_settings(uuid)` | `BridgeSettings` |
| VLAN | `nm.vlan_settings(uuid)` | `VlanSettings` |
| GSM / CDMA | `nm.gsm_settings(uuid)` / `nm.cdma_settings(uuid)` | `GsmSettings` / `CdmaSettings` |

## Loading and editing

```rust
use nmrs::NetworkManager;
use nmrs::settings::common::IpMethod;
use nmrs::settings::WiFiSettings;

#[tokio::main]
async fn main() -> nmrs::Result<()> {
    let nm = NetworkManager::new().await?;

    let saved = nm
        .list_saved_connections()
        .await?
        .into_iter()
        .find(|s| s.id == "HomeWiFi")
        .expect("profile exists");

    // Fetch and parse in one call:
    let mut wifi: WiFiSettings = nm.settings(&saved.uuid).await?;

    wifi.connection.autoconnect = false;
    wifi.ipv4.method = IpMethod::Manual;
    wifi.ipv4.address_data.push(nmrs::settings::common::Ipv4AddressData {
        address: "192.168.1.42".parse().unwrap(),
        prefix: 24,
        gateway: None,
    });

    // Build a merge patch and apply it.
    nm.update_saved_connection(&saved.uuid, wifi.to_patch()?).await?;
    Ok(())
}
```

`to_patch()` returns a [`SettingsPatch`](../../api/models.md) that only contains
the keys you touched; NetworkManager preserves everything else in the saved
profile.

## Parsing from an already-loaded profile

If you have already fetched a [`SavedConnection`], reuse its cached raw settings
to avoid an extra D-Bus round trip:

```rust
use nmrs::settings::{TypedSettings, WiFiSettings};

let wifi: WiFiSettings = saved.parse_settings()?;
```

## Error handling

All settings methods return [`nmrs::Result`]. Parse failures surface as
[`ConnectionError::SettingsParse`](../../api/errors.md); the underlying
[`nmrs::settings::SettingsError`] converts into it automatically through `?`, so
you rarely need to name it.

## IP configuration

Both the editable `IpV4Section` / `IpV6Section` and the decoded
[`IpSettings`](../../api/models.md) share the canonical [`nmrs::IpMethod`]
enum. You can move between the two representations:

```rust
use nmrs::IpSettings;
use nmrs::settings::IpV4Section;
use std::net::Ipv4Addr;

let decoded: IpSettings<Ipv4Addr> = some_section.clone().into();
let editable: IpV4Section = decoded.into();
```

The `IpV4Section` / `IpV6Section` types carry every key NetworkManager
documents (DHCP hostnames, routing rules, address generation mode, …), so a
profile can be loaded, changed, and written back without dropping fields.

## Examples

The repository ships four runnable examples: Wi-Fi and Ethernet profile
inspection, plus Wi-Fi and Ethernet edits. The edit examples print the exact
changes and require typing `yes` before they update the saved profile.

## Next Steps

- [Connection Profiles](./profiles.md) – the rest of the saved-connection API
- [Models Module](../../api/models.md) – the read-only `SavedConnection` decode
- [Error Handling](./error-handling.md) – handling connection errors
