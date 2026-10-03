# Linux builds

Linux builds are provided so Immersion can run on Linux. If you use one, you
are responsible for diagnosing and fixing any issues yourself; the project
does not provide Linux troubleshooting or fixes. These builds are for people
comfortable troubleshooting their own setup.

## Get started

1. Download the package for your computer and Linux distribution from [GitHub Releases](https://github.com/404oops/immersion/releases/latest). Packages are available for x86_64 and aarch64. Choose your distribution's `.deb`, `.rpm`, or `.pkg.tar.zst` package if available; AppImage and `.tar.gz` are also offered.
2. Install the package with your package manager. For AppImage, make the file executable and run it. For `.tar.gz`, follow the [manual install instructions](https://github.com/404oops/immersion/blob/main/packaging/linux/README.md) so the app and desktop files stay together.
3. Open Immersion and choose a folder containing your creative projects. Select **Files** for loose documents such as `.blend`, `.afphoto`, or `.sketch`; select **Bundles** when each project has its own folder or is an application bundle.
4. Change and save a supported project file, then check that a new version appears. Immersion saves history in a `.musit` folder beside that file or bundle. Files in the same folder share that `.musit` folder, so Immersion needs permission to read and write there.

Immersion needs X11, or XWayland if you use a Wayland desktop. It also needs a
working `xdg-desktop-portal` file chooser so you can select folders. It uses
`xdg-open` to open projects in their usual apps.

## Window and tray behavior

If your desktop supports the system tray icon Immersion uses, closing the window
hides the app in the tray and it keeps watching projects. Use the icon to reopen
or quit Immersion. If there is no compatible tray, closing the window quits.
Starting Immersion again reopens it if it is already running.

**Launch at login** is available for native installs. Once you have configured
folders, a login launch starts in the tray when a tray host appears. If no
tray appears within 30 seconds, the window stays open. Launch at login is not
available in the Flatpak development build.

## Build or install manually

The [Linux install and build instructions](https://github.com/404oops/immersion/blob/main/packaging/linux/README.md)
cover native dependencies, package building, the Flatpak development build,
filesystem permissions, and desktop integration. Flatpak is a development
option; native release packages are the simpler starting point.
