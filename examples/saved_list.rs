//! List all saved NetworkManager connection profiles with decoded summaries.
//!
//! Run: `cargo run --example saved_list`
//!
//! Secrets (Wi-Fi PSK, VPN passwords, etc.) are not shown — only non-secret
//! settings returned by `GetSettings`.

use nmrs::{NetworkManager, SettingsSummary};

#[tokio::main]
async fn main() -> nmrs::Result<()> {
    let nm = NetworkManager::new().await?;

    let mut profiles = nm.list_saved_connections().await?;
    profiles.sort_by(|a, b| a.id.cmp(&b.id));

    for c in profiles {
        print!("{:<32} {:<22} {}", c.id, c.connection_type, c.uuid);
        if !c.autoconnect {
            print!("  [manual]");
        }
        if c.unsaved {
            print!("  [unsaved]");
        }
        println!();

        match &c.summary {
            SettingsSummary::Wifi {
                ssid,
                security,
                hidden,
                ..
            } => {
                println!("    ssid={ssid:?} hidden={hidden} security={security:?}");
            }
            SettingsSummary::Vpn {
                service_type,
                user_name,
                data_keys,
                ..
            } => {
                println!("    vpn={service_type} user={user_name:?} data_keys={data_keys:?}");
            }
            SettingsSummary::WireGuard {
                peer_count,
                first_peer_endpoint,
                listen_port,
                ..
            } => {
                println!(
                    "    wireguard listen={listen_port:?} peers={peer_count} endpoint={first_peer_endpoint:?}"
                );
            }
            SettingsSummary::Ethernet { mac_address, .. } => {
                println!("    ethernet mac={mac_address:?}");
            }
            SettingsSummary::Gsm { apn, .. } => {
                println!("    gsm apn={apn:?}");
            }
            SettingsSummary::Bluetooth { bdaddr, bt_type } => {
                println!("    bluetooth {bdaddr} type={bt_type}");
            }
            SettingsSummary::Cdma { number, .. } => {
                println!("    cdma number={number:?}");
            }
            SettingsSummary::Other { sections } => {
                println!("    other sections={sections:?}");
            }
            _ => println!("    (additional summary variant)"),
        }
    }

    Ok(())
}
