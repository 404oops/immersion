# Settings, storage, and background behavior

Open **Settings** from the main window. If you change a folder's Bundles or Files setting, Immersion scans that folder again.

| Setting | What it does |
| --- | --- |
| **Projects folder** | Shows the folder selected now. You can add more folders and Immersion will watch them all. |
| **Folder layout** | Choose **Bundles** or **Files** for this folder. Immersion saves the choice in `.immersion/settings.json` inside it. |
| **Launch at login** | Starts Immersion when you sign in, where available. On macOS 13 or later, you may need to allow the packaged app in System Settings. On Linux, this adds a startup entry. This setting is unavailable in the Flatpak build. |
| **Default sort mode** | Show projects by **Name** or **Last Opened**. Last Opened uses the time you opened a project through Immersion since starting the app. Otherwise, it uses when the main file last changed. |
| **Activity log level** | **Info** shows normal activity. **Debug** adds details about scanning, file changes, and saved versions. |
| **Snapshot retention** | Keep fast copies of the 1–50 most recent versions per file; default 5. Older versions still have compressed copies. |
| **Save notifications** | Show a notification after a new version is saved on macOS or Linux, if the desktop permits it. On macOS, use the packaged app. Windows does not currently show these notifications. |
| **Appearance** | Follow the system, use Light, or use Dark. |
| **Theme hue / saturation** | Adjust the app's accent color. |
| **Export Activity Log** | Save the full log to a file for troubleshooting. |
| **Reset Configuration** | Clears app settings, the list of watched folders, and project notes. It does not delete `.musit` version history. |

## Where data lives

- `<project folder>/.musit/` holds saved versions. Separate project files in the same folder share one `.musit` folder. For a bundle such as `Song.logicx`, the `.musit` folder is beside the bundle, not inside it. Keep `.musit` with your project files when moving or backing them up. Do not edit it by hand.
- `<watched folder>/.immersion/settings.json` remembers that folder's Bundles or Files choice and tab appearance.
- App settings and the project list are stored in `~/Library/Application Support/musit/musit/` on macOS, `%APPDATA%\musit\musit\` on Windows, or `$XDG_DATA_HOME/musit/musit/` on Linux (usually `~/.local/share/musit/musit/`). The files are `config.json` and `projects.json`. Project notes and main-file choices are stored there too.

The `.musit` folders also work with Immersion's earlier 0.1.x builds. Backing up only the app settings will not save your version history; back up the `.musit` folders too. **Reset Configuration** leaves both `.musit` and `.immersion/settings.json` in your project folders. If you add a folder again, Immersion can reuse its saved layout and tab name.

## Closing the window

On **macOS**, closing the window hides Immersion in the menu bar; it keeps watching your projects. Use the menu bar icon to reopen or quit it. On **Windows**, closing the window quits Immersion. On **Linux**, closing the window hides Immersion in the system tray if your desktop has a compatible tray; otherwise it quits.

If you turn on **Launch at login**, Immersion starts in the background after you have set up folders: in the menu bar on macOS, in the tray on Linux when available, and minimized on Windows. Opening Immersion yourself shows its window. To stop watching projects on macOS or Linux, quit the app rather than just closing its window.
