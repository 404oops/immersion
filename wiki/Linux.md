# Linux builds

Immersion can run on Linux, but Linux builds are experimental and unsupported.
Distributions, desktop environments, graphics drivers, portals, and package
setups vary enough that Linux-specific problems are often hard to reproduce or
fix reliably across systems. Use these builds if you are comfortable checking
your own desktop setup and troubleshooting installation issues.

## Get started

1. Download the package for your CPU and distribution from [GitHub Releases](https://github.com/404oops/immersion/releases/latest). Releases provide x86_64 and aarch64 `.deb`, `.rpm`, `.pkg.tar.zst`, `.AppImage`, and `.tar.gz` packages. Use the package manager format for your distribution when possible.
2. Install the package with your package manager, or make the AppImage executable and run it. The tarball contains a binary and matching desktop files for a manual install. See the [Linux install and build instructions](https://github.com/404oops/immersion/blob/main/packaging/linux/README.md) for details.
3. Open Immersion and choose a folder containing your creative projects. Select **Files** for loose documents such as `.blend`, `.afphoto`, or `.sketch`; select **Bundles** when each project has its own folder or is an application bundle.
4. Save a project and check that a new version appears. Immersion writes its history to a `.musit` folder beside your projects, so it needs read and write access to the selected folder.

Immersion needs an X11 session or XWayland on a Wayland desktop. Your desktop
also needs an `xdg-desktop-portal` file chooser backend to select folders.
Opening a project uses the system's default file association through
`xdg-open`.

## Window and tray behavior

If your desktop has a StatusNotifierItem tray host, closing the window hides
Immersion in the tray while it continues watching projects. Use the tray icon
to reopen or quit it. Without a tray host, closing the window quits. Starting
Immersion again reopens a running instance.

**Launch at login** is available for native installs. Once you have configured
folders, a login launch starts in the tray when a tray host appears. If no
tray appears within 30 seconds, the window stays open. Launch at login is not
available in the Flatpak development build.

## Build or install manually

The [Linux install and build instructions](https://github.com/404oops/immersion/blob/main/packaging/linux/README.md)
cover native dependencies, package building, the Flatpak development build,
filesystem permissions, and desktop integration. Flatpak is a development
option; native release packages are the simpler starting point.
