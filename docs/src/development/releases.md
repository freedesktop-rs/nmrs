# Release Process

This page documents the release process for nmrs.

## Versioning

nmrs follows [Semantic Versioning](https://semver.org/):

- **Major** (X.0.0) — breaking API changes
- **Minor** (0.X.0) — new features, backward-compatible
- **Patch** (0.0.X) — bug fixes, backward-compatible

## Changelog

See [`CHANGELOG.md`](https://github.com/freedesktop-rs/nmrs/blob/master/CHANGELOG.md) for the full changelog.

## Release Checklist

1. Update version in `Cargo.toml`
2. Update the changelog
3. Run `cargo test`
4. Run `cargo clippy`
5. Run `cargo fmt --check`
6. Build documentation (`mdbook build` in `docs/`)
7. Create a git tag: `git tag v2.2.0`
8. Push the tag: `git push origin v2.2.0`
9. Publish to crates.io: `cargo publish -p nmrs`

## Distribution Channels

| Channel | Package |
|---------|---------|
| [crates.io](https://crates.io/crates/nmrs) | `nmrs` library |

## API Stability

- All public types are `#[non_exhaustive]` — new fields/variants can be added in minor releases
- Existing API signatures are preserved across minor releases
- Deprecated items are documented and kept for at least one minor release

## Next Steps

- [Contributing](./contributing.md) – how to contribute
- [Changelog](../appendix/changelog.md) – full version history
