# Contributing

Thank you for wanting to contribute to `nmrs`!

I'm fairly accepting to all PR's, only with a couple caveats:

- Do not submit low-effort or purely LLM generated code. If you absolutely must, please disclose _how_ you used AI otherwise I will close the PR.
- Please try to (when possible) contribute to an [issue](https://github.com/freedesktop-rs/nmrs/issues). For smaller changes with no existing issue, a PR is fine but for larger changes, please open an issue first.

## Requirements

**To run or develop nmrs you need:**

- Rust (stable) via `rustup`
- A running `NetworkManager` instance

I also provide a `Dockerfile` you can build if you don't use Linux and use macOS instead.

**To build the image:**

```bash
docker build -t nmrs-lib .
```

**To run tests:**

```bash
docker compose run --rm test
```

This starts an isolated system D-Bus and NetworkManager instance, waits for it
to be ready, and requires integration tests to connect to it.

**To run an interactive shell:**

```bash
docker compose run shell
```

If you just want quick builds/tests without the full NetworkManager environment:

```bash
docker run --rm nmrs-lib cargo test -p nmrs --lib
docker run --rm -it -v $(pwd):/app nmrs-lib   # mounts local changes
```

If you decide to run the shell, ensure you run all commands from within the nmrs directory, not root.

```bash
cargo test -p nmrs           # run library tests
cargo build -p nmrs          # build the library
cargo check                  # you get the point...
```

## When your branch falls behind `master`

If the respective branch for a PR goes out of sync, I prefer you _rebase_.
I've exposed this setting for you to to automatically do so as a contributor on any PR you open.

## Issues and Commit Message Hygiene

Make your commit messages at least slightly meaningful. I don't really care much about the format, but please try to be descriptive to the reader.

A good example:

```log
fix(#24): fixed bug where something was happening
```

## Tests

All tests must pass before a merge takes place.

### Ensure NetworkManager is running

```bash
sudo systemctl start NetworkManager
```

### Test everything (unit + integration)

```bash
cargo test --all-features
```

### Integration tests

These require WiFi hardware. Please make sure you
run this locally before your PR to ensure everything works.

```bash
cargo test --test integration_test --all-features
```

If you do not have access to WiFi hardware (for whatever odd reason that is), you can do something like this:

```bash
sudo modprobe mac80211_hwsim radios=2
cargo test --test integration_test --all-features
sudo modprobe -r mac80211_hwsim
```

For the same virtual-radio setup used in CI, on a Linux host with Docker:

```bash
sudo modprobe mac80211_hwsim radios=2
docker compose run --build --rm test-wifi-integration
sudo modprobe -r mac80211_hwsim
```

> [!NOTE]
>
> This method only works on linux

## License

All contributions fall under the [MIT License](https://github.com/freedesktop-rs/nmrs?tab=MIT-1-ov-file).
