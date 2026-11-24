# Bonding and Teaming

A bond or team combines interfaces (ports) into one logical interface (the
controller) for redundancy/throughput. The kernel `bonding` driver or
`teamd` does the aggregation.

```rust
use nmrs::{BondConfig, BondMode, ConnectByUuidConfig, ConnectionOptions, NetworkManager};

#[tokio::main]
async fn main() -> nmrs::Result<()> {
    let nm = NetworkManager::new().await?;
    let opts = ConnectionOptions::new(true);

    let config = BondConfig::new("bond0", BondMode::ActiveBackup)
        .with_miimon(100)
        .with_primary("eth0");
    let uuid = nm.create_bond(&config, &opts).await?;
    nm.add_bond_port("bond0", "eth0", &opts).await?;
    nm.add_bond_port("bond0", "eth1", &opts).await?;

    nm.connect_by_uuid(&uuid, ConnectByUuidConfig::default()).await?;
    Ok(())
}
```

Teams work the same way with `TeamConfig::new("team0", TeamRunner::ActiveBackup)`,
`create_team`, and `add_team_port`.

- `create_*` and `add_*_port` only save profiles and return their UUIDs.
  Activate the controller with `connect_by_uuid`; autoconnect ports come up
  with it.
- Ports are Ethernet profiles with `connection.controller` and
  `connection.port-type`.
  Ports have no IP settings; the controller gets `ipv4`/`ipv6` `auto`.
- Bonding needs the `bonding` kernel module. Teaming needs the NetworkManager
  team plugin and `teamd` (removed in RHEL 10), prefer bonds.

## Bonds

`BondConfig` methods; the profile name defaults to `Bond <interface_name>`.

| Method | Sets |
|---|---|
| `with_connection_name(name)` | `connection.id` |
| `with_mtu(mtu)` | MTU |
| `with_miimon(ms)` | `miimon` option |
| `with_primary(port)` | `primary` option; only `ActiveBackup`, `BalanceTlb`, `BalanceAlb` |
| `with_option(key, value)` | any other option, e.g. `lacp_rate`; not `mode` |
| `validate()` | checks without D-Bus; `create_bond` calls it |

Invalid interface names, empty option keys, `mode` in `with_option`, and a
misplaced `primary` return `InvalidInput`.

| `BondMode` | Name (`as_str()`) |
|---|---|
| `BalanceRr` | `balance-rr` |
| `ActiveBackup` | `active-backup` |
| `BalanceXor` | `balance-xor` |
| `Broadcast` | `broadcast` |
| `Ieee8023ad` | `802.3ad` (switch must run LACP) |
| `BalanceTlb` | `balance-tlb` |
| `BalanceAlb` | `balance-alb` |

`set_bond_mode(uuid, mode)` replaces only `mode`. NetworkManager rejects
options that conflict with the new mode (e.g. `primary` with `802.3ad`).
Reactivate to apply.

`list_bond_devices()` returns `BondDevice { path, interface, hw_address, state, carrier, ports }`,
where `ports` are interface names.

## Teams

`TeamConfig` has `with_connection_name` and `with_mtu`; the name defaults to
`Team <interface_name>`.

| `TeamRunner` | Name (`as_str()`) |
|---|---|
| `RoundRobin` | `roundrobin` |
| `Broadcast` | `broadcast` |
| `ActiveBackup` | `activebackup` |
| `LoadBalance` | `loadbalance` |
| `Lacp` | `lacp` |
| `Random` | `random` |

`set_team_runner(uuid, runner)` drops any custom `team.config` JSON, as `nmcli`
does. `list_team_devices()` returns `TeamDevice`: the `BondDevice` fields plus
`config`, the JSON `teamd` applied (`None` if unavailable).

## Builders

`build_bond_connection`, `build_bond_port_connection`, `build_team_connection`,
and `build_team_port_connection` return settings for `add_connection`, for
saving profiles without the helpers above.
