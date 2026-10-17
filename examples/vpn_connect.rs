/// Connect to a WireGuard VPN using NetworkManager and print the assigned IP address.
///
/// This example demonstrates creating a `WireGuardConfig`,
/// the preferred API for configuring VPN connections.
use nmrs::{NetworkManager, WireGuardConfig, WireGuardPeer};

#[tokio::main]
async fn main() -> nmrs::Result<()> {
    let nm = NetworkManager::new().await?;

    // Create a WireGuard peer with keepalive
    let peer = WireGuardPeer::new(
        std::env::var("WG_PUBLIC_KEY").expect("Set WG_PUBLIC_KEY env var"),
        "vpn.example.com:51820",
        vec!["0.0.0.0/0".into()],
    )
    .with_persistent_keepalive(25);

    let creds = WireGuardConfig::new(
        "ExampleVPN",
        "vpn.example.com:51820",
        std::env::var("WG_PRIVATE_KEY").expect("Set WG_PRIVATE_KEY env var"),
        "10.0.0.2/24",
        vec![peer],
    )
    .with_dns(vec!["1.1.1.1".into()]);

    println!("Connecting to VPN...");
    nm.connect_vpn(creds).await?;

    let info = nm.get_vpn_info("ExampleVPN").await?;
    println!("Connected! IP: {:?}", info.ip4_address);

    Ok(())
}
