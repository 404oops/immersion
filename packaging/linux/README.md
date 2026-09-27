# Linux builds

Immersion uses X11. On Wayland desktops it needs XWayland; the native Wayland
backend is disabled. With a StatusNotifierItem tray host, closing the window
minimizes it and project watching continues. Use the tray icon to open or quit
Immersion. If the desktop has no tray host, closing the window quits so the
process cannot become inaccessible. Launching Immersion again also reopens a
running instance. Launch at login is not implemented on Linux yet.

## Native install

Install Rust and the development packages for xkbcommon, X11, fontconfig,
FreeType, Vulkan, and Wayland (a transitive GPUI build requirement despite
X11-only runtime), plus pkg-config. On Debian/Ubuntu:

```sh
sudo apt install libxkbcommon-dev libxkbcommon-x11-dev libwayland-dev libx11-dev libxcb1-dev libfontconfig1-dev libfreetype-dev libvulkan-dev pkg-config
scripts/install-linux.sh
```

The script builds the release binary and installs it, the desktop entry, and
256/512-pixel icons under `~/.local`. Pass a prefix to install elsewhere.
X11 receives the app icon directly from Immersion.

The host desktop needs an `xdg-desktop-portal` file chooser backend. Immersion
uses GPUI's portal picker to choose project folders. `xdg-open` opens project
files in their default DAW.

## Release packages

Tagged releases produce x86_64 `.deb` (Debian/Ubuntu), `.rpm` (Fedora),
`.pkg.tar.zst` (Arch), `.AppImage`, and `.tar.gz` files. The native packages
install the desktop entry and icons. The tarball contains `bin/immersion` and
matching `share/` files; run it from the extracted folder or copy them under
`~/.local` with the same relative paths. The AppImage is executable directly.

On x86_64 Linux, build all five formats with:

```sh
cargo build --release --locked -p immersion
scripts/package-linux.sh
```

The packaging script needs [nFPM](https://nfpm.goreleaser.com/) and
[linuxdeploy](https://github.com/linuxdeploy/linuxdeploy). It uses the same
binary for each native format. The AppImage bundles host shared libraries via
linuxdeploy; the `.tar.gz` binary uses host libraries and needs compatible
X11, fontconfig, FreeType, Vulkan, and desktop portal packages. The release
workflow builds on Ubuntu 22.04 for an older glibc baseline. Check the
release artifacts on each distribution before treating them as certified.

## Flatpak development build

Install Flatpak and flatpak-builder on Linux, add Flathub, then run:

```sh
flatpak remote-add --if-not-exists flathub https://flathub.org/repo/flathub.flatpakrepo
flatpak install --user flathub org.freedesktop.Platform//26.08 org.freedesktop.Sdk//26.08 org.freedesktop.Sdk.Extension.rust-stable//26.08
flatpak-builder --user --install --install-deps-from=flathub --force-clean packaging/flatpak/build packaging/flatpak/io.github._404oops.immersion.yml
flatpak run io.github._404oops.immersion
```

The manifest builds with network access disabled. Its checked-in
`cargo-sources.json` lists every crate in `Cargo.lock`; regenerate it whenever
the lockfile changes using the
[Flatpak Cargo generator](https://github.com/flatpak/flatpak-builder-tools/tree/master/cargo):

```sh
python3 flatpak-cargo-generator.py Cargo.lock -o packaging/flatpak/cargo-sources.json
```

The Flatpak can read and write projects in the home directory and common
removable-drive mounts (`/run/media`, `/media`, `/mnt`). Immersion must watch
whole folders and write `.musit` history alongside project files. For a
project mounted elsewhere, grant that path explicitly:

```sh
flatpak override --user --filesystem=/path/to/projects io.github._404oops.immersion
```

Flatpak keeps Immersion's app settings under its own `~/.var/app/` directory;
it does not automatically import settings from a native install. Project
history in each `.musit` directory remains compatible.

Flatpak is lower priority than the native packages. Tray support depends on
the desktop's StatusNotifierItem host and sandbox D-Bus behavior. When no tray
is available, closing the window quits. Save notifications use GPUI's Linux
notification backend and the manifest's notification D-Bus permission.
