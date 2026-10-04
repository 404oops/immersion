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

Packages are written to `dist/`. For a `v<version>` tag matching `Cargo.toml`, the release workflow tests the workspace, builds the macOS DMG and Windows x64 and ARM64 installers, builds the Linux packages described in the [Linux build instructions](https://github.com/404oops/immersion/blob/main/packaging/linux/README.md), and publishes the artifacts.

## Automatic updates

Windows x64/ARM64 per-user installations and writable macOS ARM64 app bundles
check for signed stable updates automatically. Existing releases without the
updater need one manual upgrade to an updater-enabled release. Linux packages
continue to use their existing package-management workflows.

On Windows, a background thread checks on launch and every six hours. It selects
assets for the **running binary's architecture**, verifies an Ed25519-signed
manifest and the installer's size/SHA-256, and stages complete downloads only.
On the next launch, before project watchers start, a copied helper waits for the
old process to exit, holds the single-instance guard, runs NSIS silently, and
relaunches the app. The install path, launch-at-login setting, and `--autostart`
launch mode are preserved. NSIS stages the executable and uses `ReplaceFileW`
with `Immersion.previous.exe` as a recovery backup; a locked binary fails the
upgrade without truncating the original. Failed staging is discarded and normal
launch continues. A failed installer is not immediately retried in a loop.

On macOS, the bundle carries Sparkle 2.10.0, pinned and checksum-verified by
`scripts/fetch-sparkle.sh`. Sparkle validates both the signed appcast and update
archive before extraction and replaces the complete `.app` on normal quit. The
app never forces a restart while monitoring projects. Its delegate suppresses
later restart reminders after an update has been staged. Developer binaries,
apps on mounted DMGs, and bundles whose parent directory is not writable skip
automatic updates; there is no attempt to acquire administrator privileges.
The existing ad-hoc signing default remains. Developer ID signing and
notarization are separate distribution requirements; Ed25519 update signing
provides update authenticity and does not replace Apple's signing program.
The bundle includes Sparkle's license and its third-party notices at
`Contents/Resources/Sparkle-LICENSE.txt` (MIT/BSD-style licenses). Windows uses
`ed25519-dalek` (BSD-3-Clause), `reqwest` (MIT or Apache-2.0), and `winreg` (MIT);
their sources and transitive dependencies are recorded in `Cargo.lock`.

To disable updates before launch, set `IMMERSION_DISABLE_UPDATES` or create an
empty `Immersion/disable-updates` file in the platform configuration directory:
`~/Library/Application Support/Immersion/disable-updates` on macOS, or
`%APPDATA%\Immersion\disable-updates` on Windows. Remove the file and relaunch to
resume updates. Installation and download failures are logged to stderr. No
project files or `.musit` formats are changed by the updater.

### Release signing

Release CI requires the repository variable `IMMERSION_UPDATE_PUBLIC_KEY` and
secret `IMMERSION_UPDATE_PRIVATE_KEY`. The public key is base64-encoded raw
Ed25519 public bytes. The private key is a base64-encoded 32-byte seed exported by
Sparkle's `generate_keys` tool (legacy 64-byte seed+public exports are accepted).
Keep this key across releases: deployed installations trust their embedded
public key. Do not generate a different key for each release.

The key configured for this implementation is stored in the owner's macOS
Keychain under the Sparkle account `immersion`. To export a protected backup,
use `generate_keys --account immersion -x <private-key-file>` from the pinned
Sparkle tools and restrict that file's permissions. For another repository,
configure its signing secret/variable before enabling the release workflow.
No private key belongs in the repository or a release artifact.

The release workflow embeds the public key in Windows builds and the macOS
plist, publishes a signed manifest for each Windows architecture, and publishes
a signed `appcast-macos-arm64.xml` plus app-bundle ZIP. Signing checks that the
private seed matches the embedded public key; mismatches stop publication.

Additional updater checks:

```bash
python3 -m venv --clear target/update-signing
target/update-signing/bin/python -m pip install 'cryptography>=46,<47'
target/update-signing/bin/python -m unittest discover -s scripts/tests -v
```

On Windows, `scripts/test-windows-updater.ps1` exercises the actual NSIS
installer with disposable files/registry keys, including a path with spaces,
a locked executable, successful replacement, retained backup, uninstall
metadata, and startup preference preservation. It runs in both Windows CI jobs.
Real upgrade/relaunch and macOS Sparkle install-on-quit behavior still require
native end-to-end testing before release.

## Contributing

Open an [issue](https://github.com/404oops/immersion/issues) with a reproducible bug or a focused feature proposal. For code changes, keep platform differences explicit, run formatting and relevant tests, and explain any change to the `.musit` data format or restore behavior. Documentation for the GitHub Wiki is authored in this repository's `wiki/` directory. Code is licensed under [GPLv3](https://github.com/404oops/immersion/blob/main/LICENSE).
