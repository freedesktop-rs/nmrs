# WireGuard Setup

[WireGuard](https://www.wireguard.com/) is a modern, high-performance VPN protocol. nmrs provides full WireGuard support through NetworkManager's native WireGuard integration — no additional plugins required.

## Prerequisites

- NetworkManager 1.16+ (WireGuard support was added in 1.16)
- The `wireguard` kernel module must be loaded (built into Linux 5.6+, available as a module on older kernels)
- A WireGuard configuration from your VPN provider or server administrator

## Quick Start

```rust
use nmrs::{NetworkManager, WireGuardConfig, WireGuardPeer};

#[tokio::main]
async fn main() -> nmrs::Result<()> {
    let nm = NetworkManager::new().await?;

    let peer = WireGuardPeer::new(
        "SERVER_PUBLIC_KEY_BASE64",
        "vpn.example.com:51820",
        vec!["0.0.0.0/0".into()],
    ).with_persistent_keepalive(25);

    let config = WireGuardConfig::new(
        "MyVPN",
        "vpn.example.com:51820",
        "CLIENT_PRIVATE_KEY_BASE64",
        "10.0.0.2/24",
        vec![peer],
    ).with_dns(vec!["1.1.1.1".into()]);

    nm.connect_vpn(config).await?;

    println!("VPN connected!");
    Ok(())
}
```

## Understanding WireGuard Concepts

| Concept | Description |
|---------|-------------|
| **Private Key** | Your client's secret key (base64, 44 chars). Never share this. |
| **Public Key** | The server's public key (base64, 44 chars). Provided by server admin. |
| **Endpoint** | Server address in `host:port` format (e.g., `vpn.example.com:51820`) |
| **Address** | Your client's IP within the VPN tunnel (e.g., `10.0.0.2/24`) |
| **Allowed IPs** | IP ranges to route through the tunnel. `0.0.0.0/0` routes everything. |
| **DNS** | DNS servers to use while the VPN is active |
| **Persistent Keepalive** | Seconds between keepalive packets (helps with NAT traversal) |

## WireGuardConfig Fields

| Field | Required | Description |
|-------|----------|-------------|
| `name` | Yes | Connection profile name |
| `gateway` | Yes | Server endpoint (`host:port`) |
| `private_key` | Yes | Client private key (base64) |
| `address` | Yes | Client IP with CIDR (`10.0.0.2/24`) |
| `peers` | Yes | At least one `WireGuardPeer` |
| `dns` | No | DNS servers for the VPN |
| `mtu` | No | MTU size (typical: 1420) |
| `uuid` | No | Custom UUID (auto-generated if omitted) |

## Building Configuration

```rust
use nmrs::{WireGuardConfig, WireGuardPeer};

let peer = WireGuardPeer::new(
    "HIgo9xNzJMWLKAShlKl6/bUT1VI9Q0SDBXGtLXkPFXc=",
    "vpn.example.com:51820",
    vec!["0.0.0.0/0".into(), "::/0".into()],
).with_persistent_keepalive(25)
 .with_preshared_key("OPTIONAL_PSK_BASE64");

let config = WireGuardConfig::new(
    "HomeVPN",
    "vpn.example.com:51820",
    "YBk6X3pP8KjKz7+HFWzVHNqL3qTZq8hX9VxFQJ4zVmM=",
    "10.0.0.2/24",
    vec![peer],
).with_dns(vec!["1.1.1.1".into(), "8.8.8.8".into()])
 .with_mtu(1420);
```

## WireGuardPeer Configuration

| Field | Required | Description |
|-------|----------|-------------|
| `public_key` | Yes | Peer's WireGuard public key (base64) |
| `gateway` | Yes | Peer endpoint (`host:port`) |
| `allowed_ips` | Yes | IP ranges to route through this peer |
| `preshared_key` | No | Additional shared secret for post-quantum security |
| `persistent_keepalive` | No | Keepalive interval in seconds |

Peers with a preshared key also get `preshared-key-flags = 0`
(`NM_SETTING_SECRET_FLAG_NONE`). NetworkManager defaults peers to
`NOT_REQUIRED` and discards not-required secrets on save.

### Multiple Peers

WireGuard supports multiple peers with different routing rules:

```rust
use nmrs::WireGuardPeer;

let full_tunnel = WireGuardPeer::new(
    "peer1_public_key",
    "vpn.example.com:51820",
    vec!["0.0.0.0/0".into()],
).with_persistent_keepalive(25);

let split_tunnel = WireGuardPeer::new(
    "peer2_public_key",
    "office.example.com:51820",
    vec!["10.0.0.0/8".into(), "192.168.0.0/16".into()],
);
```

### Routing with Allowed IPs

| Configuration | Effect |
|--------------|--------|
| `["0.0.0.0/0"]` | Full tunnel — all traffic goes through VPN |
| `["0.0.0.0/0", "::/0"]` | Full tunnel with IPv6 |
| `["10.0.0.0/8"]` | Split tunnel — only 10.x.x.x traffic |
| `["192.168.1.0/24"]` | Split tunnel — only one subnet |

## Validation

nmrs validates all WireGuard parameters before sending them to NetworkManager:

- **Private/public keys:** Must be valid base64, approximately 44 characters
- **Address:** Must include CIDR notation (e.g., `10.0.0.2/24`)
- **Gateway:** Must be in `host:port` format with a valid port
- **Peers:** At least one peer is required, each with a valid public key and non-empty allowed IPs

Invalid parameters produce specific error variants:

| Error | Cause |
|-------|-------|
| `InvalidPrivateKey` | Key missing, wrong length, or invalid base64 |
| `InvalidPublicKey` | Peer key invalid |
| `InvalidAddress` | Missing CIDR prefix or invalid IP |
| `InvalidGateway` | Missing port or invalid format |
| `InvalidPeers` | No peers, or peer has no allowed IPs |

## Security Best Practices

- **Never hardcode private keys** — use environment variables or a secrets manager
- **Use preshared keys** when available for additional post-quantum security
- **Set persistent keepalive** to 25 seconds if behind NAT
- **Use split tunneling** when you only need to reach specific networks

## Next Steps

- [VPN Management](./vpn-management.md) – list, disconnect, and remove VPN connections
- [OpenVPN Setup](./vpn-openvpn.md) – set up OpenVPN connections
- [Custom Timeouts](../advanced/timeouts.md) – adjust VPN connection timeouts
- [Error Handling](./error-handling.md) – handle VPN-specific errors
