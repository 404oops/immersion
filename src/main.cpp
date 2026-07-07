#include <QApplication>
#include <QQmlApplicationEngine>
#include <QQmlContext>
#include <QQuickStyle>
#include <QVariant>

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
    tray.attach(&backend, &engine);
    instanceGuard.setRaiseHandler([&tray]() {
        QMetaObject::invokeMethod(&tray, &TrayController::showMainWindow, Qt::QueuedConnection);
    });

    return app.exec();
}
