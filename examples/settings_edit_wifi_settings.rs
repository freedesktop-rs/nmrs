use nmrs::NetworkManager;
use nmrs::settings::common::{IpMethod, Ipv4AddressData};
use nmrs::settings::wifi::WiFiSettings;
use std::{
    io::{self, Write},
    net::Ipv4Addr,
};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let nm = NetworkManager::new().await?;
    let name = std::env::args().nth(1).unwrap_or_else(|| "MyWiFi".into());

    let saved = nm
        .list_saved_connections()
        .await?
        .into_iter()
        .find(|s| s.id == name)
        .ok_or_else(|| format!("saved connection {name:?} not found"))?;

    let mut wifi: WiFiSettings = nm.wifi_settings(&saved.uuid).await?;
    println!(
        "loaded {} ({}) — ssid={:?} autoconnect={}",
        saved.id,
        saved.uuid,
        String::from_utf8_lossy(wifi.wifi.ssid.as_bytes()),
        wifi.connection.autoconnect,
    );

    eprintln!("WARNING: this will update the saved Wi-Fi profile; it will not activate it.");
    println!("Planned changes:");
    println!(
        "  connection.autoconnect: {} -> false",
        wifi.connection.autoconnect
    );
    println!("  ipv4.method: {:?} -> Manual", wifi.ipv4.method);
    println!("  append IPv4 address: 192.168.1.42/24 (no gateway)");
    print!(
        "Apply these changes to {:?} ({})? Type 'yes' to continue: ",
        saved.id, saved.uuid
    );
    io::stdout().flush()?;
    let mut confirmation = String::new();
    io::stdin().read_line(&mut confirmation)?;
    if confirmation.trim() != "yes" {
        println!("Cancelled; no profile changes made.");
        return Ok(());
    }

    wifi.connection.autoconnect = false;
    wifi.ipv4.method = IpMethod::Manual;
    wifi.ipv4.address_data.push(Ipv4AddressData {
        address: Ipv4Addr::new(192, 168, 1, 42),
        prefix: 24,
        gateway: None,
    });

    nm.update_saved_connection(&saved.uuid, wifi.to_patch()?)
        .await?;

    let wifi = nm.wifi_settings(&saved.uuid).await?;
    println!(
        "saved — autoconnect={} ipv4={:?}",
        wifi.connection.autoconnect, wifi.ipv4.method
    );
    Ok(())
}
