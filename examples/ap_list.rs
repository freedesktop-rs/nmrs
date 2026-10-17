use nmrs::NetworkManager;

#[tokio::main]
async fn main() -> nmrs::Result<()> {
    let nm = NetworkManager::new().await?;
    let mut aps = nm.list_access_points(None).await?;
    aps.sort_by_key(|ap| std::cmp::Reverse(ap.strength));

    for ap in &aps {
        let active = if ap.is_active { "*" } else { " " };
        println!(
            "{active} {:>3}%  {:<24} {}  {} MHz  {:?}",
            ap.strength,
            ap.ssid,
            ap.bssid,
            ap.frequency_mhz,
            ap.security.preferred_connect_type()
        );
    }

    Ok(())
}
