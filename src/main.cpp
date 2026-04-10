#include <QApplication>
#include <QQmlApplicationEngine>
#include <QVariant>

#include "core/FileEvent.h"
#include "ui/QmlBackend.h"


int main(int argc, char *argv[]) {
    QApplication app(argc, argv);
    app.setOrganizationName("musit");
    app.setOrganizationDomain("musit.app");
    app.setApplicationName("musit");

    qRegisterMetaType<FileEvent>("FileEvent");

    QQmlApplicationEngine engine;
    QmlBackend backend;
    engine.setInitialProperties({
        {"backend", QVariant::fromValue(&backend)}
    });
    engine.load(QUrl(QStringLiteral("qrc:/src/qml/App.qml")));

    // Compatibility fallback in case resource prefix/path changes.
    if (engine.rootObjects().isEmpty()) {
        engine.load(QUrl(QStringLiteral("qrc:/src/qml/Main.qml")));
    }

    if (engine.rootObjects().isEmpty()) {
        return -1;
    }

    return app.exec();
}
