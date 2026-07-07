#include "SingleInstanceGuard.h"

#include <QLocalServer>
#include <QLocalSocket>

SingleInstanceGuard::SingleInstanceGuard(const QString& serverName, QObject* parent)
    : QObject(parent)
    , m_serverName(serverName) {
    QLocalServer::removeServer(m_serverName);

    m_server = new QLocalServer(this);
    connect(m_server, &QLocalServer::newConnection, this, &SingleInstanceGuard::handleConnection);

    m_isPrimary = m_server->listen(m_serverName);
}

SingleInstanceGuard::~SingleInstanceGuard() {
    if (m_server && m_server->isListening()) {
        m_server->close();
    }
    QLocalServer::removeServer(m_serverName);
}

bool SingleInstanceGuard::isPrimaryInstance() const {
    return m_isPrimary;
}

void SingleInstanceGuard::notifyExistingInstance() const {
    QLocalSocket socket;
    socket.connectToServer(m_serverName);
    if (!socket.waitForConnected(1500)) {
        return;
    }

    socket.write("raise");
    socket.flush();
    socket.waitForBytesWritten(1500);
}

void SingleInstanceGuard::setRaiseHandler(std::function<void()> handler) {
    m_raiseHandler = std::move(handler);
}

void SingleInstanceGuard::handleConnection() {
    if (!m_server) {
        return;
    }

    QLocalSocket* socket = m_server->nextPendingConnection();
    if (!socket) {
        return;
    }

    connect(socket, &QLocalSocket::readyRead, this, [this, socket]() {
        const QByteArray message = socket->readAll();
        if (message.contains("raise") && m_raiseHandler) {
            m_raiseHandler();
        }
        socket->deleteLater();
    });
    connect(socket, &QLocalSocket::disconnected, socket, &QObject::deleteLater);
}
