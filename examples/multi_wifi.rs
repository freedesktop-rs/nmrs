//! Per-Wi-Fi-device enumeration and scoped operations.
//!
//! Lists every Wi-Fi interface NetworkManager manages, then triggers a
//! scan and prints the visible SSIDs on each radio independently.
//! Useful on laptops with USB Wi-Fi dongles or docks with a second adapter.
//!
//! Run with: `cargo run --example multi_wifi`

use nmrs::NetworkManager;

#[tokio::main]
async fn main() -> nmrs::Result<()> {
    let nm = NetworkManager::new().await?;

    let radios = nm.list_wifi_devices().await?;
    if radios.is_empty() {
        println!("No Wi-Fi devices found.");
        return Ok(());
    }

    println!("Found {} Wi-Fi radio(s):", radios.len());
    for r in &radios {
        println!(
            "  {:<10}  {}  state={:?}  active={}{}",
            r.interface,
            r.hw_address,
            r.state,
            r.is_active,
            r.active_ssid
                .as_ref()
                .map(|s| format!("  ssid={s}"))
                .unwrap_or_default(),
        );
    }

    for r in &radios {
        let scope = nm.wifi(&r.interface);
        println!("\n[{}] scanning...", r.interface);

        if let Err(e) = scope.scan().await {
            eprintln!("[{}] scan failed: {e}", r.interface);
            continue;
        }
        // Give NM a moment to populate scan results.
        tokio::time::sleep(std::time::Duration::from_secs(2)).await;

        let nets = scope.list_networks().await?;
        for n in nets {
            println!(
                "  {:>3}%  {:<32}  ({} BSSIDs)",
                n.strength.unwrap_or(0),
                n.ssid,
                n.bssids.len(),
            );
        }
    }

    Ok(())
}
