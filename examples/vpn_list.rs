use nmrs::NetworkManager;

#[tokio::main]
async fn main() -> nmrs::Result<()> {
    let nm = NetworkManager::new().await?;
    let vpns = nm.list_vpn_connections().await?;

    println!(
        "{:<20} {:<38} {:<16} {:<12} active",
        "id", "uuid", "type", "user"
    );
    println!("{}", "-".repeat(90));

    for vpn in &vpns {
        let type_label = match &vpn.vpn_type {
            nmrs::VpnType::WireGuard { .. } => "wireguard".to_string(),
            nmrs::VpnType::OpenVpn {
                connection_type, ..
            } => {
                format!(
                    "openvpn/{}",
                    connection_type
                        .map(|ct| format!("{ct:?}"))
                        .unwrap_or_default()
                )
            }
            nmrs::VpnType::OpenConnect { .. } => "openconnect".to_string(),
            nmrs::VpnType::StrongSwan { .. } => "strongswan".to_string(),
            nmrs::VpnType::Pptp { .. } => "pptp".to_string(),
            nmrs::VpnType::L2tp { .. } => "l2tp".to_string(),
            nmrs::VpnType::Generic { service_type, .. } => service_type.clone(),
            _ => "(unknown)".to_string(),
        };

        let user = vpn.user_name.as_deref().unwrap_or("(n/a)");
        let active_icon = if vpn.active { "●" } else { "○" };

        println!(
            "{:<20} {:<38} {:<16} {:<12} {}",
            vpn.id, vpn.uuid, type_label, user, active_icon
        );
    }

    Ok(())
}
