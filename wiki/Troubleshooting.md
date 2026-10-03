# Troubleshooting and FAQ

## No projects were found

Check that the folder contains a [supported project file](Supported-Projects.md) and that you chose the right layout. **Bundles** groups files in one folder as a project. **Files** shows each supported file separately, but skips folder-like projects such as `.logicx`. Select **Try Other Layout** during setup, or **Settings → Change Folder Layout** later. If projects still do not appear, check that Immersion can read the folder and look at the activity log in Settings.

## A project appears but new versions do not

Keep Immersion running while you work. On macOS, it can keep running in the menu bar after you close the window. On Linux, it can keep running in the tray if your desktop has a compatible tray. On Windows, closing the window quits the app. When you start Immersion again, it tries to save the latest state of a file changed while it was closed. It cannot recover every save you made during that time.

Make sure the folder is still listed and its tab says Immersion is watching it. In Settings, switch **Activity log level** to **Debug** and check the log. Immersion needs permission to write to the project folder and its `.musit` folder. If the log says watching or version setup failed, fix the folder access and restart Immersion.

## The wrong file opens or is versioned

Select the project and choose the right file under **File** in the details panel. Double-clicking the project opens that file in its usual app. If your computer has no app set to open it, Immersion may open the containing folder instead.

## A restored project looks incomplete

Immersion does not save every file a project uses. Audio, video, samples, linked assets, and temporary files may be missing from the saved version. Recover them from your separate backup. If the creative app was open during the restore, close it and reopen the project from disk.

## The graph says no saved version matches the file

Immersion cannot tell whether the file or bundle on disk matches a saved version. It may still be waiting to save a recent change, or it may be unable to read a file. Save your work, wait a moment, and check the activity log for errors. If no version says **Current** before you restore, copy your current project somewhere safe. Immersion tries to save it first, but that extra save can fail without stopping the restore.

## What does a dashed node mean?

Immersion removed that version's quick copy to save space. You can still restore the version if its compressed copy is available. Change **Snapshot retention** in Settings to choose how many recent versions keep quick copies.

## Can I remove or reset without losing history?

Removing a folder tab stops Immersion from watching that folder, but does not delete its saved versions or folder settings. **Reset Configuration** clears app settings, the watched-folder list, and project notes. It also leaves saved versions and folder settings on disk, so adding the folder again can bring back its layout and tab name. **Delete** on a selected version permanently removes that version.

## Where can I report a problem?

On macOS or Windows, you can report a problem in [GitHub Issues](https://github.com/404oops/immersion/issues). Include your operating system and version, Immersion version, project format, steps to repeat the problem, and relevant lines from the activity log. Use **Settings → Export Activity Log** if you need the full log. Read it before sharing: Debug logs may contain file paths. Share private project files only if you mean to make them public.

If Apple or Microsoft has ended support for your version of macOS or Windows, Immersion does not support it either. [Linux builds are experimental and unsupported](Linux.md).
