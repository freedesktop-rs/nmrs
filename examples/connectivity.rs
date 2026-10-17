use nmrs::NetworkManager;

#[tokio::main]
async fn main() -> nmrs::Result<()> {
    let nm = NetworkManager::new().await?;
    let report = nm.connectivity_report().await?;

    println!("state:          {:?}", report.state);
    println!("check enabled:  {}", report.check_enabled);
    println!("check uri:      {:?}", report.check_uri);
    println!("captive portal: {:?}", report.captive_portal_url);

    if report.state == nmrs::ConnectivityState::Portal
        && let Some(url) = report.captive_portal_url
    {
        println!("-> open {url} in your browser to authenticate");
    }

    Ok(())
}
