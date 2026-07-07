#include <QApplication>
#include <QQmlApplicationEngine>
#include <QQmlContext>
#include <QQuickStyle>
#include <QVariant>

#include "ui/PlatformAgent.h"

#include "core/FileEvent.h"
#include "ui/QmlBackend.h"
#include "ui/SingleInstanceGuard.h"
#include "ui/TrayController.h"

int main(int argc, char *argv[]) {
    QQuickStyle::setStyle(QStringLiteral("Basic"));

    QApplication app(argc, argv);
    QApplication::setQuitOnLastWindowClosed(false);

    app.setOrganizationName("musit");
    app.setOrganizationDomain("musit.app");
    app.setApplicationName("musit");

    PlatformAgent::initActivationHandling();

    qRegisterMetaType<FileEvent>("FileEvent");

    SingleInstanceGuard instanceGuard(QStringLiteral("musit-immersion-v1"));
    if (!instanceGuard.isPrimaryInstance()) {
        instanceGuard.notifyExistingInstance();
        return 0;
    }

    QQmlApplicationEngine engine;
    QmlBackend backend;
    engine.rootContext()->setContextProperty(QStringLiteral("MusitThemeBridge"), &backend);
    engine.setInitialProperties({
        {"backend", QVariant::fromValue(&backend)}
    });
    engine.load(QUrl(QStringLiteral("qrc:/src/qml/App.qml")));

    if (engine.rootObjects().isEmpty()) {
        engine.load(QUrl(QStringLiteral("qrc:/src/qml/Main.qml")));
    }

    if (engine.rootObjects().isEmpty()) {
        return -1;
    }

    TrayController tray;
    const auto showMainWindow = [&tray]() {
        QMetaObject::invokeMethod(&tray, &TrayController::showMainWindow, Qt::QueuedConnection);
    };
    instanceGuard.setRaiseHandler(showMainWindow);
    PlatformAgent::setShowWindowHandler(showMainWindow);
    tray.attach(&backend, &engine);

    // QML/engine setup can replace NSApp's delegate; wrap again before the event loop.
    PlatformAgent::initActivationHandling();

    return app.exec();
}
