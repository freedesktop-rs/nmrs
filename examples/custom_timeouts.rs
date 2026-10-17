/// Example demonstrating custom timeout configuration for NetworkManager operations.
///
/// This shows how to configure longer timeouts for slow networks or enterprise
/// authentication that may take more time to complete.
use nmrs::{NetworkManager, TimeoutConfig, WifiSecurity};
use std::time::Duration;

#[tokio::main]
async fn main() -> nmrs::Result<()> {
    // Configure custom timeouts for slow networks
    let config = TimeoutConfig::new()
        .with_connection_timeout(Duration::from_secs(60)) // Wait up to 60s for connection
        .with_disconnect_timeout(Duration::from_secs(20)); // Wait up to 20s for disconnection

    // Create NetworkManager with custom timeout configuration
    let nm = NetworkManager::with_config(config).await?;

    println!("NetworkManager configured with custom timeouts:");
    println!(
        "  Connection timeout: {:?}",
        nm.timeout_config().connection_timeout
    );
    println!(
        "  Disconnect timeout: {:?}",
        nm.timeout_config().disconnect_timeout
    );

    // Connect to a network (will use the custom 60s timeout)
    println!("\nConnecting to network...");
    nm.connect(
        "MyNetwork",
        None,
        WifiSecurity::WpaPsk {
            psk: std::env::var("WIFI_PASSWORD").unwrap_or_else(|_| "password".to_string()),
        },
    )
    .await?;

    println!("Connected successfully!");

    // You can also use default timeouts
    let nm_default = NetworkManager::new().await?;
    println!("\nDefault NetworkManager timeouts:");
    println!(
        "  Connection timeout: {:?}",
        nm_default.timeout_config().connection_timeout
    );
    println!(
        "  Disconnect timeout: {:?}",
        nm_default.timeout_config().disconnect_timeout
    );

    Ok(())
}
