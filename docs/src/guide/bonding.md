# Bonds

A bond is two or more profiles, built like any other connection and saved
with `add_connection`: the bond itself, and a separate port profile for each
interface you attach (what `nmcli connection add type bond-slave` does).
Existing profiles for the port interface are left alone.

```rust,no_run
use nmrs::NetworkManager;
use nmrs::builders::{BondBuilder, BondMode, BondPortBuilder};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let nm = NetworkManager::new().await?;

    // autoconnect off: saving does not bring anything up.
    let bond = BondBuilder::new("bond0")
        .mode(BondMode::ActiveBackup)
        .miimon(100)
        .autoconnect(false)
        .build()?;
    nm.add_connection(bond).await?;

    for port in ["eth0", "eth1"] {
        let settings = BondPortBuilder::new("bond0", port).autoconnect(false).build()?;
        nm.add_connection(settings).await?;
    }
    Ok(())
}
```

- With `autoconnect` off, saving does not activate anything; bring the
  bond and ports up with `connect_by_uuid`. With `autoconnect` on,
  NetworkManager activates a bond profile as soon as it is saved (it is a
  software device), and an autoconnect port profile can claim its interface
  whenever that interface is idle, including at boot.
- **Activating a port takes the interface off whatever profile it was using.**
- The bond gets `ipv4`/`ipv6` `auto`. Ports have no IP settings.
- `BondPortBuilder`'s bond may be the bond interface name or its profile
  UUID.
- Ports are Ethernet interfaces, matching `nmcli`'s `bond-slave` type.

## Options

`BondBuilder` methods write `bond.options`. Unset = NM default (mode
`balance-rr`). NetworkManager validates option values and combinations when
the profile is saved, so an invalid one is an error from `add_connection`,
not from `build()`.

| Method | Option |
|---|---|
| `mode` | `mode` (`BondMode`) |
| `miimon` | `miimon` (ms) |
| `primary` | `primary` (interface name) |
| `option(key, value)` | any other bonding option, e.g. `lacp_rate` |
