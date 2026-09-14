# Immersion — Music Project Versioning

Automatic version snapshots for DAW project files (Bitwig, Ableton, FL
Studio, Reaper, Logic, ...). Rust + [GPUI](https://www.gpui.rs/) rewrite of
the original Qt/QML app; on-disk data (`.musit` folders, config, registry)
is fully compatible with the 0.1.x Qt builds.

The main window shows your projects folders as tabs (all of them watched and
versioned simultaneously; discovery scans run on a worker thread, one folder
at a time). Each tab splits into the project list (left, click to select,
double-click to open) and the version graph (right), with a draggable
details panel below holding project info, notes, and version actions. The
activity log lives in Settings. First run walks through a wizard: pick a
folder, choose its layout, then optionally add more folders.

## Workspace

- `crates/musit-core` — engine: project discovery, file watching, snapshot
  service, content-addressed object store (SHA-256 + Qt-compatible zlib
  framing), version metadata log.
- `crates/immersion` — GPUI desktop app and platform glue (menu bar status
  item, notifications, launch-at-login, single instance).

The widget toolkit the app is built from is [vampir], pulled from crates.io.
It depends on nothing but GPUI, and is MIT licensed.

[vampir]: https://github.com/404oops/vampir

## Build & run

```bash
cargo run -p immersion            # debug build, runs unbundled
cargo test -p musit-core          # engine test suite (incl. e2e parity test)
```

## Packaging

Distribution builds use the `production` cargo profile (fat LTO, stripped);
without the flag the scripts package a plain `release` build. Output lands
in `dist/`.

```bash
scripts/bundle-macos.sh [--production]    # target/<profile>/Immersion.app only
scripts/package-macos.sh [--production]   # dist/Immersion-<version>.dmg
```

On macOS, notifications and launch-at-login require the real `.app` bundle —
the bare cargo binary skips both. Set `IMMERSION_SIGN_IDENTITY` to sign with
a Developer ID certificate instead of the ad-hoc signature.

```powershell
# Requires NSIS (winget install NSIS.NSIS)
powershell -ExecutionPolicy Bypass -File scripts\package-windows.ps1 [-Production]
# -> dist\ImmersionSetup-<version>.exe
```

The Windows installer is per-user (no admin prompt), offers a
launch-at-login checkbox, and registers an Add/Remove Programs entry. The
exe icon/version block is embedded at build time via `build.rs`.

Releases are built by CI: bump `version` in `Cargo.toml`, commit, then push
a matching `v<version>` tag. `.github/workflows/release.yml` runs the tests,
packages the DMG and the Windows installer with the `production` profile,
and publishes both on the GitHub release for that tag.

## Platform support

macOS is fully supported. The crates compile on Windows/Linux and the
single-instance guard works there, but the status item, notifications, and
hide-to-tray behavior are currently macOS-only (closing the window quits on
other platforms).
