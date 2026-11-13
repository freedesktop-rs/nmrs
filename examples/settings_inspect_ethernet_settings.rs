use nmrs::NetworkManager;
use nmrs::settings::ethernet::EthernetSettings;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let nm = NetworkManager::new().await?;
    let mut found = 0;
    for saved in nm.list_saved_connections().await? {
        if saved.connection_type != "802-3-ethernet" {
            continue;
        }
        let eth: EthernetSettings = nm.ethernet_settings(&saved.uuid).await?;
        println!(
            "{}\n  uuid:                       {}\n  mtu:                        {}\n  auto_negotiate:             {}\n  speed:                      {}\n  mac:                        {:?}\n  duplex:                     {:?}\n  accept_all_mac_addresses:   {}\n  wake_on_lan:                0x{:x}\n  mac_address_denylist:       {:?}\n  s390_options:               {:?}\n  autoconnect:                {}\n  ipv4:                       {}",
            saved.id,
            saved.uuid,
            eth.ethernet.mtu,
            eth.ethernet.auto_negotiate,
            eth.ethernet.speed,
            eth.ethernet.mac_address,
            eth.ethernet.duplex,
            eth.ethernet.accept_all_mac_addresses,
            eth.ethernet.wake_on_lan,
            eth.ethernet.mac_address_denylist,
            eth.ethernet.s390_options,
            eth.connection.autoconnect,
            eth.ipv4.method,
        );
        for addr in &eth.ipv4.address_data {
            println!(
                "  ipv4 addr:   {}/{} gw={:?}",
                addr.address, addr.prefix, addr.gateway
            );
        }
        println!();
        found += 1;
    }
    if found == 0 {
        println!("no saved ethernet connections");
    }
    Ok(())
}
