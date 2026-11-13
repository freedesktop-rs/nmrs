use nmrs::NetworkManager;
use nmrs::settings::wifi::WiFiSettings;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let nm = NetworkManager::new().await?;
    let mut found = 0;
    for saved in nm.list_saved_connections().await? {
        if saved.connection_type != "802-11-wireless" {
            continue;
        }
        let wifi: WiFiSettings = nm.wifi_settings(&saved.uuid).await?;

        let sec = wifi
            .wifi_security
            .as_ref()
            .map(|s| s.key_mgmt.to_string())
            .unwrap_or_else(|| "open".into());
        let has_psk = wifi
            .wifi_security
            .as_ref()
            .and_then(|s| s.psk.as_ref())
            .is_some();

        println!(
            "{}\n  uuid:                  {}\n  ssid:                  {:?}\n  mode:                  {}\n  band:                  {}\n  channel:               {}\n  hidden:                {}\n  bssid:                 {:?}\n  mac:                   {:?}\n  cloned-mac:            {:?}\n  mtu:                   {}\n  security:              {}\n  psk:                   {}\n  autoconnect:           {}\n  interface:             {}",
            saved.id,
            saved.uuid,
            String::from_utf8_lossy(wifi.wifi.ssid.as_bytes()),
            wifi.wifi.mode,
            wifi.wifi
                .band
                .as_ref()
                .map(ToString::to_string)
                .unwrap_or_else(|| "-".into()),
            wifi.wifi.channel,
            wifi.wifi.hidden,
            wifi.wifi.bssid,
            wifi.wifi.mac_address,
            wifi.wifi.cloned_mac_address,
            wifi.wifi.mtu,
            sec,
            has_psk,
            wifi.connection.autoconnect,
            wifi.connection.interface_name.as_deref().unwrap_or("*"),
        );

        if let Some(ieee) = &wifi.ieee_auth {
            let eaps: Vec<String> = ieee.eap.iter().map(|e| e.to_string()).collect();
            println!(
                "  eap:                   {}\n  identity:              {}\n  phase2-auth:           {}\n  phase2-autheap:        {}\n  subject-match:         {:?}\n  system-ca-certs:       {}",
                eaps.join(","),
                ieee.identity,
                ieee.phase2_auth
                    .as_ref()
                    .map(|p| p.to_string())
                    .unwrap_or_else(|| "-".into()),
                ieee.phase2_autheap
                    .as_ref()
                    .map(|p| p.to_string())
                    .unwrap_or_else(|| "-".into()),
                ieee.subject_match,
                ieee.system_ca_certs,
            );
        }
        println!(
            "  ipv4:                  {}\n  ipv4 gateway:          {:?}",
            wifi.ipv4.method, wifi.ipv4.gateway
        );
        for addr in &wifi.ipv4.address_data {
            println!(
                "  ipv4 addr:             {}/{} gw={:?}",
                addr.address, addr.prefix, addr.gateway
            );
        }
        println!("  ipv6:                  {}", wifi.ipv6.method);
        for addr in &wifi.ipv6.address_data {
            println!(
                "  ipv6 addr:             {}/{} gw={:?}",
                addr.address, addr.prefix, addr.gateway
            );
        }
        println!();
        found += 1;
    }
    if found == 0 {
        println!("no saved wifi connections");
    }
    Ok(())
}
