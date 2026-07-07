#pragma once

#include <QIcon>
#include <QObject>

class QMenu;
class QQmlApplicationEngine;
class QmlBackend;

#if !defined(Q_OS_MACOS) && !defined(Q_OS_MAC)
class QSystemTrayIcon;
#endif

#ifdef Q_OS_MACOS
class MacStatusBar;
#endif

class TrayController : public QObject {
    Q_OBJECT
public:
    explicit TrayController(QObject* parent = nullptr);
    ~TrayController() override;

    void attach(QmlBackend* backend, QQmlApplicationEngine* engine);

public slots:
    void showMainWindow();

private slots:
    void onProjectSaveRecorded(const QString& projectName,
                               const QString& versionLabel,
                               const QString& relativePath);
    void hideMainWindow();
    void quitApplication();

private:
    QObject* mainWindowObject() const;
    static QIcon createTrayIcon();

#ifdef Q_OS_MACOS
    MacStatusBar* m_macStatusBar {nullptr};
#else
    QSystemTrayIcon* m_tray {nullptr};
    QMenu* m_menu {nullptr};
#endif

    QmlBackend* m_backend {nullptr};
    QQmlApplicationEngine* m_engine {nullptr};
};
