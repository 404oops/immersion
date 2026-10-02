# Settings, storage, and background behavior

Open **Settings** from the main window. Changes to a folder layout trigger a rescan.

| Setting | What it does |
| --- | --- |
| **Projects folder** | Shows the active folder and lets you choose or add another. Multiple folders can be watched at once. |
| **Folder layout** | Choose **Bundles** or **Files** for the active folder. The choice is stored in that folder's `.immersion/settings.json`. |
| **Launch at login** | Starts Immersion when you sign in where the platform supports it. On macOS 13 or later, the packaged app uses Login Items and may need approval in System Settings. On Linux it writes an XDG autostart entry; it is not available in the Flatpak. |
| **Default sort mode** | Start the project list sorted by **Name** or **Last Opened**. |
| **Activity log level** | **Info** shows normal events; **Debug** adds scan, watcher, and storage details. |
| **Snapshot retention** | Number of recent versions with fast uncompressed copies, from 1 to 50; default 5. Older versions remain in compressed storage. |
| **Save notifications** | Show a notification after a new version is saved, where supported. The packaged macOS app is required. |
| **Appearance** | Follow the system, use Light, or use Dark. |
| **Theme hue / saturation** | Adjust the app's accent color. |
| **Export Activity Log** | Save the full log to a file for troubleshooting. |
| **Reset Configuration** | Clears saved settings, watched folder choices, and project notes; it does not remove `.musit` version history. |

## Where data lives

- `<project root>/.musit/` contains version metadata, staged recent copies, and compressed content objects. Keep this folder with its project if you move or back it up. Do not edit it by hand.
- `<watched folder>/.immersion/settings.json` stores that folder's layout and saved tab presentation.
- App settings and the project registry live in `~/Library/Application Support/musit/musit/` on macOS, `%APPDATA%\musit\musit\` on Windows, or `$XDG_DATA_HOME/musit/musit/` on Linux (default `~/.local/share/musit/musit/`). The files are `config.json` and `projects.json`. Project notes and primary-file choices are kept in this app registry.

The `.musit` format remains compatible with Immersion's earlier 0.1.x Qt builds. Keeping only the app registry is not enough to keep version history; keep the `.musit` folders as well.

## Closing the window

On macOS, closing the window hides Immersion in the menu bar while monitoring continues. Use the menu bar item to reopen it or quit. On Linux, closing the window hides it to the tray while monitoring continues when a StatusNotifierItem tray host is available; without one, closing quits. On Windows, closing the window quits. Once folders are configured, a launch at login starts hidden in the menu bar on macOS, in the tray on Linux (as soon as the tray appears), and minimized on Windows; opening Immersion yourself always shows the window. Use the app's quit action when you intend to stop monitoring on macOS or Linux.
