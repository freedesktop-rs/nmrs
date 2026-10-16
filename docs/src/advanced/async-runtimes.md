# Async Runtime Support

nmrs is built on async Rust and uses [`zbus`](https://docs.rs/zbus) for D-Bus communication. While the examples in this book use Tokio, nmrs works with any async runtime.

## Tokio (Recommended)

Tokio is the most commonly used runtime and the one used in all examples:

```rust
use nmrs::NetworkManager;

#[tokio::main]
async fn main() -> nmrs::Result<()> {
    let nm = NetworkManager::new().await?;
    let networks = nm.list_networks(None).await?;
    println!("{} networks found", networks.len());
    Ok(())
}
```

Add to your `Cargo.toml`:

```toml
[dependencies]
nmrs = "3.1"
tokio = { version = "1", features = ["macros", "rt-multi-thread"] }
```

### Single-Threaded Tokio

For lightweight applications, you can use the current-thread runtime:

```rust
#[tokio::main(flavor = "current_thread")]
async fn main() -> nmrs::Result<()> {
    let nm = nmrs::NetworkManager::new().await?;
    // ...
    Ok(())
}
```

## async-std

```rust
use nmrs::NetworkManager;

#[async_std::main]
async fn main() -> nmrs::Result<()> {
    let nm = NetworkManager::new().await?;
    let networks = nm.list_networks(None).await?;
    println!("{} networks found", networks.len());
    Ok(())
}
```

```toml
[dependencies]
nmrs = "3.1"
async-std = { version = "1", features = ["attributes"] }
```

## smol

```rust
use nmrs::NetworkManager;

fn main() -> nmrs::Result<()> {
    smol::block_on(async {
        let nm = NetworkManager::new().await?;
        let networks = nm.list_networks(None).await?;
        println!("{} networks found", networks.len());
        Ok(())
    })
}
```

```toml
[dependencies]
nmrs = "3.1"
smol = "2"
```

## GLib/GTK (for GUI applications)

When building GTK4 applications, use the GLib main context:

```rust
use nmrs::NetworkManager;

// Inside a GTK application's async context
glib::MainContext::default().spawn_local(async {
    let nm = NetworkManager::new().await.unwrap();

    let networks = nm.list_networks(None).await.unwrap();
    for net in &networks {
        println!("{}: {}%", net.ssid, net.strength.unwrap_or(0));
    }
});
```

## How It Works

nmrs uses `zbus` for D-Bus communication. When you call `NetworkManager::new().await`, zbus establishes a connection to the system D-Bus and picks the runtime that drives it:

- Inside a Tokio runtime, the connection's I/O and internal tasks run on that runtime (nmrs enables zbus's `tokio` feature).
- Anywhere else, zbus's own runtime ([`zruntime`](https://docs.rs/zruntime)) drives the connection from a helper thread, so async-std, smol, and GLib keep working without configuration.

Either way, nothing in nmrs's API depends on which runtime is in use.

The `NetworkManager` struct is `Clone` and `Send`, so you can share it across tasks regardless of which runtime you use.

## Thread Safety

`NetworkManager` is:

- **`Clone`** — clones share the same D-Bus connection (cheap)
- **`Send`** — can be moved across threads
- **`Sync`** — can be shared via `Arc` (though `Clone` is usually simpler)

However, **concurrent connection operations are not supported**. Don't call `connect()` from multiple tasks simultaneously. Use `is_connecting()` to check if a connection is in progress.

## Next Steps

- [Custom Timeouts](./timeouts.md) – configure operation timeouts
- [D-Bus Architecture](./dbus.md) – understand the D-Bus layer
