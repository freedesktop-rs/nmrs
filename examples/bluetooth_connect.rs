/// Connect to a Bluetooth device using NetworkManager.
use nmrs::models::BluetoothIdentity;
use nmrs::{NetworkManager, Result};
#[tokio::main]
async fn main() -> Result<()> {
    let nm = NetworkManager::new().await?;

    println!("Scanning for Bluetooth devices...");
    let devices = nm.list_bluetooth_devices().await?;

    if devices.is_empty() {
        println!("No Bluetooth devices found.");
        println!("\nMake sure:");
        println!("  1. Bluetooth is enabled");
        println!("  2. Device is paired (use 'bluetoothctl')");
        return Ok(());
    }

    // This will print all devices that have been explicitly paired using
    // `bluetoothctl pair <MAC_ADDRESS>`
    println!("\nAvailable Bluetooth devices:");
    for (i, device) in devices.iter().enumerate() {
        println!("  {}. {}", i + 1, device);
    }

    // Connect to the first device in the list
    if let Some(device) = devices.first() {
        println!("\nConnecting to: {}", device);

        let settings = BluetoothIdentity::new(device.bdaddr.clone(), device.bt_caps.into())?;

        let name = device
            .alias
            .as_ref()
            .or(device.name.as_ref())
            .map(|s| s.as_str())
            .unwrap_or("Bluetooth Device");

        match nm.connect_bluetooth(name, &settings).await {
            Ok(_) => println!("✓ Successfully connected to {name}"),
            Err(e) => {
                eprintln!("✗ Failed to connect: {}", e);
                return Ok(());
            }
        }

        /* match nm.forget_bluetooth(name).await {
            Ok(_) => println!("Disconnected {name}"),
            Err(e) => eprintln!("Failed to forget: {e}"),
        }*/
    }

    Ok(())
}
