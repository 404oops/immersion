# Supported projects and layouts

Immersion discovers projects by supported file extensions, application bundles, and a few project root marker files. Matching an extension identifies the project type; it does not mean every asset the creative app uses is included in a snapshot.

## Choose the right layout

**Bundles** scans for a project in each subfolder or bundle. Use it when each song, session, scene, or document has its own directory, or for macOS package formats such as Logic `.logicx` and GarageBand `.band`.

**Files** discovers individual supported regular project files in the selected folder and its subfolders. Each file is a separate project. It skips bundle formats and their contents. If you choose Files for a folder that only contains `.logicx` bundles, the scan will find nothing.

You can watch multiple folders with different layouts. Changing a folder's layout in Settings rescans it. If a folder has several candidate project files, choose the primary file in the details panel. Immersion initially favors a filename with an explicit `v<number>` suffix, then the more recently modified candidate.

## Format families

The discovery templates cover these applications and file types:

| Family | Examples |
| --- | --- |
| Music production | Ableton Live (`.als`), Bitwig (`.bwproject`), FL Studio (`.flp`), REAPER (`.rpp`), Logic Pro (`.logicx` bundle), GarageBand (`.band` bundle), Cubase, Nuendo, Pro Tools, Studio One, Reason, Renoise, LMMS, SunVox, MuLab, Cakewalk, Ardour, Samplitude, Tracktion |
| Video and motion | DaVinci Resolve, Final Cut Pro, Premiere Pro, Media Composer, VEGAS Pro, HitFilm, After Effects, Nuke, Fusion |
| 3D and visual art | Blender, Cinema 4D, Houdini, Maya, LightWave 3D, Photoshop, GIMP, Krita, Affinity Photo, Clip Studio Paint, Capture One, Illustrator, Affinity Designer, Inkscape, CorelDRAW |
| Publishing and game projects | InDesign, Scrivener, Affinity Publisher, LaTeX, Unreal Engine, Godot, Unity |

For the exact extensions, marker files, and include/exclude patterns, see [`backup_template.rs`](https://github.com/404oops/immersion/blob/main/crates/musit-core/src/backup_template.rs). Some application project databases and cloud-managed libraries do not behave like normal local files; verify that your project appears and that a test restore works before relying on its history.

## Media and companion files

Templates include the main project file or bundle plus selected small companion files when appropriate. They generally exclude bulky media, renders, samples, and caches. For example, an Ableton template excludes its `Samples` and `Backup` directories, and a REAPER template excludes `Media` and peak-cache directories. Back up those resources separately.
