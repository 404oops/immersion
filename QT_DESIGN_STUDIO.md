# musit Qt Design Studio Files

This folder is a standalone Qt Design Studio scaffold for visual UI editing.

## Files
- `musit.qmlproject`: project descriptor to open in Qt Design Studio.
- `content/App.qml`: entry window.
- `content/MainView.ui.qml`: editable UI form that mirrors the current musit layout.

## Open in Qt Design Studio
1. Open `musit.qmlproject`.
2. Edit `MainView.ui.qml` in Design mode.
3. Use the generated QML as a visual blueprint for the QWidget implementation.

## Note
These files are not wired into the current CMake QWidget build. They are intended for design/prototyping.
