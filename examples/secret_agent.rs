/// Registers a long-lived NetworkManager secret agent, prints incoming
/// requests, and responds to Wi-Fi PSK prompts by reading a password from
/// stdin.
///
/// Run with:
///
/// ```sh
/// cargo run --example secret_agent
/// ```
///
/// Then trigger a password prompt (e.g. forget a saved Wi-Fi password and
/// reconnect). The agent will print each request and ask for input.
use std::io::{self, BufRead, Write};

use futures::StreamExt;
use nmrs::agent::{SecretAgent, SecretAgentFlags, SecretSetting};

#[tokio::main]
async fn main() -> nmrs::Result<()> {
    let (handle, mut requests) = SecretAgent::builder()
        .with_identifier("com.system76.nmrs.example.secret_agent")
        .register()
        .await?;

    println!("Secret agent registered. Waiting for requests…");
    println!("Keep this process running while NetworkManager may need secrets.");
    println!("After a NetworkManager restart, call SecretAgentHandle::reregister().\n");

    while let Some(req) = requests.next().await {
        println!("── Secret request ──");
        println!("  UUID:    {}", req.connection_uuid);
        println!("  Name:    {}", req.connection_id);
        println!("  Type:    {}", req.connection_type);
        println!("  Setting: {:?}", req.setting);
        println!("  Hints:   {:?}", req.hints);
        println!("  Flags:   {:?}", req.flags);

        if !req.flags.contains(SecretAgentFlags::ALLOW_INTERACTION) {
            println!("  → interaction not allowed, cancelling");
            req.responder.cancel().await?;
        } else {
            match req.setting {
                SecretSetting::WifiPsk { ref ssid } => {
                    print!("  Enter password for \"{ssid}\": ");
                    io::stdout().flush().expect("flush stdout");
                    let mut line = String::new();
                    io::stdin().lock().read_line(&mut line).expect("read stdin");
                    let psk = line.trim();
                    if psk.is_empty() {
                        println!("  → empty input, cancelling");
                        req.responder.cancel().await?;
                    } else {
                        req.responder.wifi_psk(psk).await?;
                        println!("  → sent PSK");
                    }
                }
                _ => {
                    println!("  → unsupported setting type, cancelling");
                    req.responder.cancel().await?;
                }
            }
        }
    }

    handle.unregister().await?;
    println!("Request stream closed; agent unregistered.");
    Ok(())
}
