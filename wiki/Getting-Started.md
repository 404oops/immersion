# Getting started

## Install

Download an available package from [GitHub Releases](https://github.com/404oops/immersion/releases/latest). On macOS, open the DMG and install the app. On Windows, run the per-user installer; administrator access is not required. The Windows installer can enable launch at login. On Linux, follow the [Linux build guide](https://github.com/404oops/immersion/blob/main/packaging/linux/README.md) for native packages, AppImage, tarball, and Flatpak options and desktop requirements.

## First launch

1. Select **Choose Projects Folder…** and choose the parent folder that contains your projects. Immersion needs read and write access to that folder to store versions.
2. Choose a layout:
   - **Bundles — One folder per project:** each project lives in a subfolder or an application bundle such as `.logicx`.
   - **Files — Loose project files:** supported files such as `.als`, `.flp`, or `.rpp` sit directly in the selected folder or its subfolders. Each file becomes its own project. Bundle formats are skipped in this mode.
3. Select **Continue**. Immersion scans the folder, lists the projects it finds, and creates an initial version for each project it can initialize.
4. If nothing is found, try **Try Other Layout**. If you have another collection, select **Add Another Folder…**. Select **Done** when finished.

Each watched folder has its own layout, stored in `<projects folder>/.immersion/settings.json`. A folder you add again normally remembers its layout and tab name.

## Everyday use

Leave Immersion running while you save in your creative app. File changes trigger new versions after they settle. Select a project in the list to view its history; double-click a project to open its primary file with the system's file association. When several files belong to a project, use the **File** picker in the details panel to choose the primary file you want to open and version.

See [the interface guide](https://github.com/404oops/immersion/wiki/Interface) for the controls and [versions and restoring](https://github.com/404oops/immersion/wiki/Versions-and-Restoring) before you replace a current file with an older one.
