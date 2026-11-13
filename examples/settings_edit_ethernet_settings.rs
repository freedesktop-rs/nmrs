use nmrs::NetworkManager;
use nmrs::settings::common::BoolWithDefault;
use nmrs::settings::ethernet::EthernetSettings;
use std::io::{self, Write};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let nm = NetworkManager::new().await?;
    let name = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "Wired connection 1".into());

    let saved = nm
        .list_saved_connections()
        .await?
        .into_iter()
        .find(|s| s.id == name)
        .ok_or_else(|| format!("saved connection {name:?} not found"))?;

    let mut eth: EthernetSettings = nm.ethernet_settings(&saved.uuid).await?;
    println!(
        "loaded {} ({}) — accept_all={} wake_on_lan=0x{:x} auto_neg={} s390_options={:?}",
        saved.id,
        saved.uuid,
        eth.ethernet.accept_all_mac_addresses,
        eth.ethernet.wake_on_lan,
        eth.ethernet.auto_negotiate,
        eth.ethernet.s390_options,
    );

    eprintln!("WARNING: this will update the saved Ethernet profile; it will not activate it.");
    println!("Planned changes:");
    println!(
        "  ethernet.accept_all_mac_addresses: {} -> Yes",
        eth.ethernet.accept_all_mac_addresses
    );
    println!(
        "  ethernet.wake_on_lan: 0x{:x} -> 0x42",
        eth.ethernet.wake_on_lan
    );
    println!(
        "  ethernet.auto_negotiate: {} -> true",
        eth.ethernet.auto_negotiate
    );
    println!(
        "  ethernet.s390_options[\"portno\"]: {:?} -> \"1\"",
        eth.ethernet.s390_options.get("portno")
    );
    println!(
        "  ethernet.s390_options[\"layer2\"]: {:?} -> \"0\"",
        eth.ethernet.s390_options.get("layer2")
    );
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

    eth.ethernet.accept_all_mac_addresses = BoolWithDefault::Yes;
    eth.ethernet.wake_on_lan = 0x42;
    eth.ethernet.auto_negotiate = true;
    eth.ethernet
        .s390_options
        .insert("portno".into(), "1".into());
    eth.ethernet
        .s390_options
        .insert("layer2".into(), "0".into());

    nm.update_saved_connection(&saved.uuid, eth.to_patch()?)
        .await?;

    let eth = nm.ethernet_settings(&saved.uuid).await?;
    println!(
        "saved — accept_all={} wake_on_lan=0x{:x} auto_neg={} s390_options={:?}",
        eth.ethernet.accept_all_mac_addresses,
        eth.ethernet.wake_on_lan,
        eth.ethernet.auto_negotiate,
        eth.ethernet.s390_options,
    );
    Ok(())
}
