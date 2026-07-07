#include "SingleInstanceGuard.h"

#include <QLocalServer>
#include <QLocalSocket>
#include <QAbstractSocket>

namespace {

constexpr char kRaiseCommand[] = "raise";
constexpr int kProbeTimeoutMs = 500;

void processRaiseMessage(const QByteArray& message, const std::function<void()>& raiseHandler) {
    if (message.contains(kRaiseCommand) && raiseHandler) {
        raiseHandler();
    }
}

void handleIncomingSocket(QLocalSocket* socket, const std::function<void()>& raiseHandler) {
    if (!socket) {
        return;
    }

    const auto readAndHandle = [socket, raiseHandler]() {
        if (socket->bytesAvailable() <= 0) {
            return;
        }
        processRaiseMessage(socket->readAll(), raiseHandler);
    };

    QObject::connect(socket, &QLocalSocket::readyRead, socket, readAndHandle);
    QObject::connect(socket, &QLocalSocket::disconnected, socket, &QObject::deleteLater);

    readAndHandle();
}

bool isAnotherInstanceRunning(const QString& serverName) {
    QLocalSocket probe;
    probe.connectToServer(serverName);
    if (!probe.waitForConnected(kProbeTimeoutMs)) {
        return false;
    }

    probe.disconnectFromServer();
    return true;
}

} // namespace

SingleInstanceGuard::SingleInstanceGuard(const QString& serverName, QObject* parent)
    : QObject(parent)
    , m_serverName(serverName) {
    m_server = new QLocalServer(this);
    connect(m_server, &QLocalServer::newConnection, this, &SingleInstanceGuard::handleConnection);

    m_isPrimary = tryBecomePrimary();
}

SingleInstanceGuard::~SingleInstanceGuard() {
    if (m_server && m_server->isListening()) {
        m_server->close();
    }
    if (m_isPrimary) {
        QLocalServer::removeServer(m_serverName);
    }
}

bool SingleInstanceGuard::tryBecomePrimary() {
    if (m_server->listen(m_serverName)) {
        return true;
    }

    if (m_server->serverError() == QAbstractSocket::AddressInUseError
        && isAnotherInstanceRunning(m_serverName)) {
        return false;
    }

    QLocalServer::removeServer(m_serverName);
    if (m_server->listen(m_serverName)) {
        return true;
    }

    // Could not bind even after cleanup. Only defer to an existing instance if one
    // is actually reachable; otherwise launch anyway rather than exiting silently.
    return !isAnotherInstanceRunning(m_serverName);
}

bool SingleInstanceGuard::isPrimaryInstance() const {
    return m_isPrimary;
}

void SingleInstanceGuard::notifyExistingInstance() const {
    QLocalSocket socket;
    socket.connectToServer(m_serverName);
    if (!socket.waitForConnected(3000)) {
        return;
    }

    socket.write(kRaiseCommand);
    socket.flush();
    socket.waitForBytesWritten(3000);
    socket.disconnectFromServer();
    if (socket.state() != QLocalSocket::UnconnectedState) {
        socket.waitForDisconnected(1000);
    }
}

void SingleInstanceGuard::setRaiseHandler(std::function<void()> handler) {
    m_raiseHandler = std::move(handler);
}

void SingleInstanceGuard::handleConnection() {
    if (!m_server) {
        return;
    }

    while (QLocalSocket* socket = m_server->nextPendingConnection()) {
        handleIncomingSocket(socket, m_raiseHandler);
    }
}
