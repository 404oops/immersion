# The main window

The window has folder tabs across the top, a project list on the left, a version graph on the right, and a details panel below. Drag the dividers to give the list, graph, or details more room.

![A second folder with illustrative Blender and Affinity projects](images/immersion-visual.png)

## Folder tabs

- Select a tab to see that folder's projects. Immersion keeps watching your other folders too. A tab may show when its folder is being scanned or is waiting to be scanned.
- Select **+** to add a folder. A new folder asks for its layout; an existing configured folder reuses its saved layout.
- Double-click a tab name to rename it. Press **Enter** to save or **Escape** to cancel. Drag tabs to reorder them.
- Use a tab's remove control to stop watching it. Confirming removal leaves its `.musit` version history on disk, so you can add the folder again later.
- Select **Settings** for folder and app preferences.

## Project list

- Click a project to select it; double-click to open its main file in the usual app.
- Type in **Search projects…** to narrow the visible list.
- Use **Sort by** to choose **Name** or **Last Opened**. If you opened a project through Immersion since starting the app, Last Opened uses that time. Otherwise, it uses the last time the main project file changed. You can also set the sort order in Settings.
- The list shows the project type and main file. If a project is missing, check its file type and folder layout in [Supported projects](Supported-Projects.md).

## Version graph

- Click a version in the graph to select it. Lines show how versions are connected, including new paths made after you restore an older version. **Current** means the files saved in that version match what is on disk now. It does not check every file or linked media item in the project folder.
- Double-click a version to restore it. The **Open version** button in the details panel does the same thing.
- Drag or scroll to move around the graph. Use **−** and **+** to zoom, or select the percentage to return to 100%. Command/Control plus the mouse wheel also zooms.
- A dashed border means Immersion has removed that version's quick copy to save space. You can still restore it if its compressed copy is available.

## Details panel

- **Project** shows the project's name, main file, location, and Last Opened time. In Bundles mode, use **File** to choose which project file opens and whose history you see.
- **Project note** is an editable note for the project. Select **Save note** to keep it.
- **Version** shows when the selected version was saved, whether it is current, and its note. Select **Save note** after editing. **Delete** asks before permanently removing that version. **Open version** restores it.

Project notes and version notes are different: project notes follow the project, while version notes identify one saved point in its history.
