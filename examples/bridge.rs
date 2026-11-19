//! Saves a bridge profile and, optionally, a port profile for an interface.
//! Autoconnect is off, so nothing is activated; use `connect_by_uuid` to bring them up.
//! Usage: cargo run --example bridge -- br0 [eth0]

use nmrs::NetworkManager;
use nmrs::builders::{BridgeBuilder, BridgePortBuilder};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args().skip(1);
    let bridge = args.next().unwrap_or_else(|| "br0".into());
    let port = args.next();
    let nm = NetworkManager::new().await?;

    let settings = BridgeBuilder::new(&bridge)
        .stp(true)
        .autoconnect(false)
        .build()?;
    println!(
        "saved bridge {bridge}: {}",
        nm.add_connection(settings).await?
    );

    if let Some(iface) = port {
        let settings = BridgePortBuilder::new(&bridge, &iface)
            .autoconnect(false)
            .build()?;
        println!("saved port {iface}: {}", nm.add_connection(settings).await?);
    }
    Ok(())
}
