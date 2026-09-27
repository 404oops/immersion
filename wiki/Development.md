# Building and contributing

The workspace has `crates/musit-core` for discovery, file watching, snapshot storage, and version metadata, and `crates/immersion` for the GPUI desktop interface and platform integration. The UI uses the [vampir](https://github.com/404oops/vampir) widget toolkit. The on-disk `.musit` format is compatible with earlier 0.1.x Qt builds.

## Build and test

Install a current Rust toolchain and platform build tools, then run:

```bash
cargo run -p immersion
cargo test --workspace
cargo fmt --all --check
```

A bare macOS `cargo run` build lacks packaged-app notifications and launch-at-login integration. To make a macOS app bundle or DMG:

```bash
scripts/bundle-macos.sh --production
scripts/package-macos.sh --production
```

On Windows, install NSIS and run:

```powershell
powershell -ExecutionPolicy Bypass -File scripts\package-windows.ps1 -Production
```

Packages are written to `dist/`. The release workflow tests the workspace, builds both packages, and publishes them for a `v<version>` tag matching `Cargo.toml`.

## Contributing

Open an [issue](https://github.com/404oops/immersion/issues) with a reproducible bug or a focused feature proposal. For code changes, keep platform differences explicit, run formatting and relevant tests, and explain any change to the `.musit` data format or restore behavior. Documentation for the GitHub Wiki is authored in this repository's `wiki/` directory. Code is licensed under [GPLv3](https://github.com/404oops/immersion/blob/main/LICENSE).
