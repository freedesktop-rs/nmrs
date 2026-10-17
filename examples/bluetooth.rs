/// List Bluetooth devices using NetworkManager
use nmrs::{NetworkManager, Result};

#[tokio::main]
async fn main() -> Result<()> {
    let nm = NetworkManager::new().await?;

    println!("Scanning for Bluetooth devices...");
    let devices = nm.list_bluetooth_devices().await?;

    // List bluetooth devices
    for d in devices {
        println!("{d}");
    }

    Ok(())
}
