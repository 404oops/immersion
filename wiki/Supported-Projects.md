# Supported projects and layouts

Immersion finds projects by their file names, file types, and, for some apps, files that identify a project folder. Finding a project does not mean Immersion saves every file that project uses.

## Choose the right layout

Choose **Bundles** when each project has its own folder, or when you use a folder-like project such as Logic `.logicx` or GarageBand `.band`. Immersion shows supported files in the same folder as one project. This also applies to files directly in the folder you chose to watch. If that folder holds unrelated files, choose **Files** to show them separately.

Choose **Files** when each supported project file should appear separately. Immersion looks in the folder you chose and its subfolders. It skips folder-like projects and the files inside them. For example, a folder containing only `.logicx` projects will appear empty in Files mode.

For example, if `Projects/Song A/` contains `Draft.als` and `Final.als`, Bundles shows one project named **Song A** with a **File** choice between the two. Files shows **Draft** and **Final** as separate projects. In either layout, their version data is stored in `Projects/Song A/.musit/`.

You can watch several folders and choose a different layout for each. Changing a layout in Settings makes Immersion scan that folder again. If a project in Bundles mode contains several project files, choose its main file in the details panel. That choice controls which file opens and whose history appears in the graph. Immersion first prefers a name with a version number such as `v2`, then the file changed most recently. In Files mode, each file is already a separate project.

## Format families

Immersion recognizes these apps and file types:

| Family | Examples |
| --- | --- |
| Music production | Ableton Live (`.als`), Bitwig (`.bwproject`), FL Studio (`.flp`), REAPER (`.rpp`), Logic Pro (`.logicx` bundle), GarageBand (`.band` bundle), Cubase, Nuendo, Pro Tools, Studio One, Reason, Renoise, LMMS, SunVox, MuLab, Cakewalk, Ardour, Samplitude, Tracktion |
| Video and motion | DaVinci Resolve, Final Cut Pro, Premiere Pro, Media Composer, VEGAS Pro, HitFilm, After Effects, Nuke, Fusion |
| 3D and visual art | Blender, Cinema 4D, Houdini, Maya, LightWave 3D, Photoshop, GIMP, Krita, Affinity Photo (`.afphoto`), Pixelmator Pro (`.pxd`), Procreate (`.procreate`), Aseprite (`.ase`/`.aseprite`), Clip Studio Paint, Capture One, Illustrator, Affinity Designer (`.afdesign`), Sketch (`.sketch`), Inkscape, CorelDRAW |
| Publishing and game projects | InDesign, Scrivener, Affinity Publisher (`.afpub`), LaTeX, Unreal Engine, Godot, Unity |

Affinity `.afphoto`, `.afdesign`, and `.afpub` documents are regular files, so use the **Files** layout when they are stored as loose documents. Sketch, Pixelmator Pro, Procreate, and Aseprite documents also use **Files**. The Affinity `~` backup variants are recognized as separate files.

For the exact list of files Immersion recognizes and saves, see [`backup_template.rs`](https://github.com/404oops/immersion/blob/main/crates/musit-core/src/backup_template.rs). Some project libraries, especially ones managed by an app or stored in the cloud, may not work like ordinary files. Check that your project appears and try restoring a test version before relying on its history.

## Media and companion files

Immersion saves the main project file or supported files inside a bundle, plus some related files where needed. It usually skips large media, renders, samples, and temporary files. For example, it skips Ableton's `Samples` and `Backup` folders and REAPER's `Media` and peak-cache folders. Back up those files separately.
