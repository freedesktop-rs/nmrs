/// Scan for available WiFi networks and print their SSIDs and signal strengths.
use nmrs::NetworkManager;

#[tokio::main]
async fn main() -> nmrs::Result<()> {
    let nm = NetworkManager::new().await?;

    println!("Scanning for WiFi networks...");
    nm.scan_networks(None).await?;

    let networks = nm.list_networks(None).await?;
    for net in networks {
        println!("{:30} {}%", net.ssid, net.strength.unwrap_or(0));
    }

    Ok(())
}
