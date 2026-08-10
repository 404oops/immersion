#pragma once

#include <QObject>
#include <QString>

// Native macOS menu bar icon. Qt's QSystemTrayIcon crashes when opening a
// context menu while NSApplicationActivationPolicyAccessory is active.
class MacStatusBar final : public QObject {
    Q_OBJECT
public:
    explicit MacStatusBar(QObject* parent = nullptr);
    ~MacStatusBar() override;

    void install();
    void showNotification(const QString& title, const QString& body);

public slots:
    void openFromMenu();
    void quitFromMenu();
    void toggleFromMenu();

signals:
    void openRequested();
    void quitRequested();
    void toggleWindowRequested();

private:
    struct Private;
    Private* d {nullptr};
};
