# Bridges

A bridge is two profiles, built like any other connection and saved with
`add_connection`: the bridge itself, and a separate port profile for each
interface you attach (what `nmcli connection add type bridge-slave` does).
Existing profiles for the port interface are left alone.

```rust,no_run
use nmrs::NetworkManager;
use nmrs::builders::{BridgeBuilder, BridgePortBuilder};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let nm = NetworkManager::new().await?;

    // autoconnect off: saving does not bring anything up.
    let bridge = BridgeBuilder::new("br0").stp(true).autoconnect(false).build()?;
    nm.add_connection(bridge).await?;

    let port = BridgePortBuilder::new("br0", "eth0").autoconnect(false).build()?;
    nm.add_connection(port).await?;
    Ok(())
}
```

- With `autoconnect` off, saving does not activate anything; bring the
  bridge and ports up with `connect_by_uuid`. With `autoconnect` on,
  NetworkManager activates a bridge profile as soon as it is saved (it is a
  software device), and an autoconnect port profile can claim its interface
  whenever that interface is idle, including at boot.
- **Activating a port takes the interface off whatever profile it was using.**
- The bridge gets `ipv4`/`ipv6` `auto`. Ports have no IP settings.
- `BridgePortBuilder`'s bridge may be the bridge interface name or its
  profile UUID.
- Ports are Ethernet interfaces, matching `nmcli`'s `bridge-slave` type.
  VLAN and bond ports are not supported yet.

## Options

`BridgeBuilder` methods. Unset = NM default. Out of range = `InvalidInput` from `build()`.

| Method | Range |
|---|---|
| `stp` | bool |
| `priority` | `u16` |
| `forward_delay` | 2-30 s (0 with STP off) |
| `hello_time` | 1-10 s (0 with STP off) |
| `max_age` | 6-40 s (0 with STP off) |
| `ageing_time` | 0-1000000 s |
