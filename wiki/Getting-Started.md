# Getting started

## Install

Download Immersion from [GitHub Releases](https://github.com/404oops/immersion/releases/latest).

- **Mac:** The DMG is for Apple silicon and needs macOS 12 or later. Open the DMG and install the app.
- **Windows:** Choose the x64 or ARM64 installer that matches your computer. It installs for your account without administrator access and can start Immersion when you sign in.
- **Linux:** See the [Linux build guide](Linux.md) for packages, other install options, and desktop requirements.

If Apple or Microsoft has ended support for your version of macOS or Windows, Immersion does not support it either. macOS 12 is only the oldest version the app can install on.

## First launch

1. Select **Choose Projects Folder…** and pick the folder that holds your projects. Immersion needs permission to read and write there so it can save versions.
2. Choose a layout:
   - **Bundles — One folder per project:** Choose this when each project has its own folder or is a folder-like file such as `.logicx`. Files in the same folder appear as one project.
   - **Files — Loose project files:** Choose this when each `.als`, `.blend`, `.afphoto`, or `.sketch` file should appear as its own project. Immersion also finds supported files in subfolders, but skips folder-like projects such as `.logicx`.
3. Select **Continue**. Immersion looks for projects and tries to save a first version of each project's main file or folder-like file.
4. If nothing is found, try **Try Other Layout**. If you have another collection, select **Add Another Folder…**. Select **Done** when finished.

Immersion saves your Bundles or Files choice and tab name in `<projects folder>/.immersion/settings.json`. If you add the folder again later and that file is still there, Immersion remembers those choices.

## Everyday use

Leave Immersion running while you work. After you save a supported file and the changes settle, Immersion saves a new version. Select a project to see its history, or double-click it to open its main file in the usual app. If a project in Bundles mode contains several project files, use **File** in the details panel to choose which one opens and whose history the graph shows. Immersion may save separate histories for the other files in that folder.

See [the interface guide](Interface.md) for the controls and [versions and restoring](Versions-and-Restoring.md) before you replace a current file with an older one.
