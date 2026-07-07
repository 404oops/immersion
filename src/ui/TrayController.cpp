#include "TrayController.h"

#include "PlatformAgent.h"
#include "QmlBackend.h"

#include "../core/BackupTemplate.h"

#include <QAction>
#include <QApplication>
#include <QFileInfo>
#include <QIcon>
#include <QMenu>
#include <QPainter>
#include <QPixmap>
#include <QQmlApplicationEngine>
#include <QQuickWindow>

#if !defined(Q_OS_MACOS) && !defined(Q_OS_MAC) && !defined(__APPLE__)
#include <QSystemTrayIcon>
#endif

#if defined(Q_OS_MACOS) || defined(Q_OS_MAC) || defined(__APPLE__)
#include "MacStatusBar.h"
#endif

namespace {

QQuickWindow* quickWindowForObject(QObject* object) {
    if (!object) {
        return nullptr;
    }

    if (auto* window = qobject_cast<QQuickWindow*>(object)) {
        return window;
    }

    return qobject_cast<QQuickWindow*>(object->findChild<QQuickWindow*>());
}

} // namespace

TrayController::TrayController(QObject* parent)
    : QObject(parent) {}

TrayController::~TrayController() {
#if !defined(Q_OS_MACOS) && !defined(Q_OS_MAC) && !defined(__APPLE__)
    if (m_tray) {
        m_tray->hide();
    }
#endif
}

void TrayController::attach(QmlBackend* backend, QQmlApplicationEngine* engine) {
    m_backend = backend;
    m_engine = engine;

    if (m_backend) {
        connect(m_backend, &QmlBackend::projectSaveRecorded,
                this, &TrayController::onProjectSaveRecorded);
    }

#ifdef Q_OS_MACOS
    m_macStatusBar = new MacStatusBar(this);
    connect(m_macStatusBar, &MacStatusBar::openRequested, this, &TrayController::showMainWindow);
    connect(m_macStatusBar, &MacStatusBar::quitRequested, this, &TrayController::quitApplication);
    m_macStatusBar->install();

    if (QQuickWindow* window = quickWindowForObject(mainWindowObject())) {
        connect(window, &QQuickWindow::visibleChanged, window, [window]() {
            // Menu-bar agent when hidden; normal Dock app while the window is open.
            PlatformAgent::setBackgroundAgentMode(!window->isVisible());
        });
    }

    if (m_backend && !m_backend->hasProjectsFolder()) {
        showMainWindow();
    } else {
        PlatformAgent::setBackgroundAgentMode(true);
    }
    return;
#else
    if (!QSystemTrayIcon::isSystemTrayAvailable()) {
        showMainWindow();
        return;
    }

    m_menu = new QMenu;
    m_menu->addAction(QStringLiteral("Open Immersion"), this, &TrayController::showMainWindow);
    m_menu->addSeparator();
    m_menu->addAction(QStringLiteral("Quit"), this, &TrayController::quitApplication);

    m_tray = new QSystemTrayIcon(createTrayIcon(), this);
    m_tray->setToolTip(QStringLiteral("Immersion"));
    m_tray->setContextMenu(m_menu);

    connect(m_tray, &QSystemTrayIcon::activated, this, [this](QSystemTrayIcon::ActivationReason reason) {
        if (reason == QSystemTrayIcon::Trigger || reason == QSystemTrayIcon::DoubleClick) {
            if (QQuickWindow* window = quickWindowForObject(mainWindowObject())) {
                if (window->isVisible()) {
                    hideMainWindow();
                } else {
                    showMainWindow();
                }
            } else {
                showMainWindow();
            }
        }
    });

    m_tray->show();

    if (m_backend && !m_backend->hasProjectsFolder()) {
        showMainWindow();
    }
#endif
}

void TrayController::onProjectSaveRecorded(const QString& projectName,
                                           const QString& versionLabel,
                                           const QString& relativePath) {
    const QString artifactPath = BackupTemplates::artifactForPath(relativePath);
    const QString artifactName = QFileInfo(artifactPath).fileName();
    const QString body = artifactName.isEmpty()
        ? QStringLiteral("%1 saved %2").arg(projectName, versionLabel)
        : QStringLiteral("%1 — %2 saved %3").arg(projectName, artifactName, versionLabel);

#ifdef Q_OS_MACOS
    if (m_macStatusBar) {
        m_macStatusBar->showNotification(QStringLiteral("Snapshot saved"), body);
    }
#else
    if (m_tray) {
        m_tray->showMessage(QStringLiteral("Snapshot saved"), body, QSystemTrayIcon::Information, 5000);
    }
#endif
}

void TrayController::showMainWindow() {
    QObject* root = mainWindowObject();
    if (!root) {
        return;
    }

#ifdef Q_OS_MACOS
    PlatformAgent::setBackgroundAgentMode(false);
#endif
    PlatformAgent::activateApplication();

    if (auto* window = quickWindowForObject(root)) {
        window->show();
        window->raise();
        window->requestActivate();
        return;
    }

    root->setProperty("visible", true);
}

void TrayController::hideMainWindow() {
    QObject* root = mainWindowObject();
    if (!root) {
        return;
    }

    if (auto* window = quickWindowForObject(root)) {
        window->hide();
        return;
    }

    root->setProperty("visible", false);
}

void TrayController::quitApplication() {
    QApplication::quit();
}

QObject* TrayController::mainWindowObject() const {
    if (!m_engine) {
        return nullptr;
    }

    const QList<QObject*> roots = m_engine->rootObjects();
    return roots.isEmpty() ? nullptr : roots.first();
}

QIcon TrayController::createTrayIcon() {
    QPixmap pixmap(64, 64);
    pixmap.fill(Qt::transparent);

    QPainter painter(&pixmap);
    painter.setRenderHint(QPainter::Antialiasing, true);
    painter.setPen(Qt::NoPen);
    painter.setBrush(QColor(72, 120, 220));
    painter.drawEllipse(8, 8, 48, 48);

    QFont font = painter.font();
    font.setBold(true);
    font.setPixelSize(34);
    painter.setFont(font);
    painter.setPen(Qt::white);
    painter.drawText(pixmap.rect(), Qt::AlignCenter, QStringLiteral("M"));

    return QIcon(pixmap);
}
