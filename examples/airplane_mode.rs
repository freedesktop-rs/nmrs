use nmrs::NetworkManager;

#[tokio::main]
async fn main() -> nmrs::Result<()> {
    let nm = NetworkManager::new().await?;

    println!("before: {:#?}", nm.airplane_mode_state().await?);

    nm.set_airplane_mode(true).await?;
    tokio::time::sleep(std::time::Duration::from_secs(2)).await;

    println!("during: {:#?}", nm.airplane_mode_state().await?);

    nm.set_airplane_mode(false).await?;
    println!("after:  {:#?}", nm.airplane_mode_state().await?);

    Ok(())
}
