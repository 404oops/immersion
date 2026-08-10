#pragma once

#include <functional>

class QWindow;

namespace PlatformAgent {

// Hide the dock/taskbar presence until the user opens the main window (macOS).
void setBackgroundAgentMode(bool enabled);

// Bring the app forward when opening the window from the tray.
void activateApplication();

// Show and key the main window (platform-specific foreground handling).
void presentMainWindow(QWindow* window);

// Temporarily block present-on-activate while a status-bar menu is open (macOS).
void setPresentationSuppressed(bool suppressed);
bool isPresentationSuppressed();
void setSkipNextActivationPresent(bool skip);

// macOS: handle Dock/Spotlight reopen in-process (no second process is spawned).
// Windows/Linux: second-process IPC remains the primary relaunch path.
void initActivationHandling();
void setShowWindowHandler(std::function<void()> handler);

// Convenience wrapper that installs native handlers and stores the callback.
void installShowWindowHandler(std::function<void()> handler);

// Register or remove the app from the OS login / startup items.
void setLaunchAtStartup(bool enabled);
bool isLaunchAtStartupEnabled();
bool isLaunchAtStartupSupported();

} // namespace PlatformAgent
