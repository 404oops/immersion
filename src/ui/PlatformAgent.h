#pragma once

class QWindow;

namespace PlatformAgent {

// Hide the dock/taskbar presence until the user opens the main window (macOS).
void setBackgroundAgentMode(bool enabled);

// Bring the app forward when opening the window from the tray.
void activateApplication();

} // namespace PlatformAgent
