# Troubleshooting and FAQ

## No projects were found

Check that the selected folder contains supported local project files and that the chosen **Bundles** or **Files** layout matches how they are stored. A bundle such as `.logicx` will not be discovered in Files mode. Use **Try Other Layout** during setup or **Settings → Change Folder Layout** later. Verify file permissions and inspect the activity log after the scan.

## A project appears but new versions do not

Keep Immersion running while you save. On macOS it can continue in the menu bar after the window closes; on Windows closing the window quits. Confirm the folder is still added and its tab says it is monitoring. Switch the activity log to **Debug** for watcher details. Check that the project folder and its `.musit` location are writable. If the log reports a failed watcher or versioning initialization, restart Immersion after fixing access to the folder.

## The wrong file opens or is versioned

Select the project and use the **File** picker in the details panel to choose its primary file. Double-clicking the project opens that file through the operating system's file association. If no association exists, Immersion may open the containing folder instead.

## A restored project looks incomplete

Immersion saves the files in its format template; external media and many cache folders are excluded. Restore missing audio, video, samples, or linked assets from your separate backup. If your creative app was open during the restore, close it and reopen the project from disk.

## The graph says no saved version matches the file

The on-disk file has changed since the selected snapshot, or the latest write has not been recorded yet. Save and wait for the watcher to settle. Before a restore, Immersion attempts a safety snapshot if the current on-disk file is not already represented in history.

## What does a dashed node mean?

Its quick uncompressed staged copy was compacted. The version remains available through compressed object storage when that object is present. Adjust **Snapshot retention** to control how many recent copies stay uncompressed.

## Can I remove or reset without losing history?

Removing a watched folder stops monitoring and removes its tab, but leaves `.musit` history. **Reset Configuration** clears app settings, folder choices, and project notes, but also leaves `.musit` history. **Delete** on a selected version permanently removes that specific snapshot.

## Where can I report a problem?

In **Settings**, choose **Export Activity Log** and review the file before sharing it, since Debug mode can include local paths. Open a [GitHub issue](https://github.com/404oops/immersion/issues) with your operating system, Immersion version, project format, steps to reproduce, and relevant log lines. Avoid attaching private project files unless you intend to share them.
