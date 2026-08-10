#pragma once

#include <QObject>
#include <QString>

#include <functional>

class QLocalServer;

class SingleInstanceGuard final : public QObject {
    Q_OBJECT
public:
    explicit SingleInstanceGuard(const QString& serverName, QObject* parent = nullptr);
    ~SingleInstanceGuard() override;

    bool isPrimaryInstance() const;
    void notifyExistingInstance() const;

    void setRaiseHandler(std::function<void()> handler);

private:
    bool tryBecomePrimary();

    void handleConnection();

    QString m_serverName;
    QLocalServer* m_server {nullptr};
    void* m_platformInstanceLock {nullptr};
    bool m_isPrimary {false};
    std::function<void()> m_raiseHandler;
};
