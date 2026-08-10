#include "SingleInstanceGuard.h"

#include <QLocalServer>
#include <QLocalSocket>
#include <QAbstractSocket>
#include <QThread>

#ifdef Q_OS_WIN
#ifndef NOMINMAX
#define NOMINMAX
#endif
#include <windows.h>
#endif

namespace {

constexpr char kRaiseCommand[] = "raise";
constexpr int kProbeTimeoutMs = 500;

#ifdef Q_OS_WIN
constexpr wchar_t kInstanceMutexName[] = L"Local\\musit-immersion-v1";

bool acquireWindowsInstanceMutex(void*& lock) {
    HANDLE mutex = CreateMutexW(nullptr, FALSE, kInstanceMutexName);
    if (!mutex) {
        return true;
    }

    if (GetLastError() == ERROR_ALREADY_EXISTS) {
        CloseHandle(mutex);
        lock = nullptr;
        return false;
    }

    lock = mutex;
    return true;
}

void releaseWindowsInstanceMutex(void* lock) {
    if (lock) {
        CloseHandle(static_cast<HANDLE>(lock));
    }
}
#endif

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

bool startLocalServer(QLocalServer* server, const QString& serverName) {
    if (server->listen(serverName)) {
        return true;
    }

    if (server->serverError() == QAbstractSocket::AddressInUseError
        && isAnotherInstanceRunning(serverName)) {
        return false;
    }

    QLocalServer::removeServer(serverName);
    return server->listen(serverName);
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
#ifdef Q_OS_WIN
    releaseWindowsInstanceMutex(m_platformInstanceLock);
    m_platformInstanceLock = nullptr;
#endif
}

bool SingleInstanceGuard::tryBecomePrimary() {
#ifdef Q_OS_WIN
    if (!acquireWindowsInstanceMutex(m_platformInstanceLock)) {
        return false;
    }

    if (!startLocalServer(m_server, m_serverName)) {
        // We own the mutex but the pipe failed; still run as primary so the app
        // launches, just without cross-process raise until the next restart.
        return true;
    }

    return true;
#else
    return startLocalServer(m_server, m_serverName)
        || !isAnotherInstanceRunning(m_serverName);
#endif
}

bool SingleInstanceGuard::isPrimaryInstance() const {
    return m_isPrimary;
}

void SingleInstanceGuard::notifyExistingInstance() const {
    for (int attempt = 0; attempt < 20; ++attempt) {
        QLocalSocket socket;
        socket.connectToServer(m_serverName);
        if (!socket.waitForConnected(250)) {
            QThread::msleep(50);
            continue;
        }

        socket.write(kRaiseCommand);
        socket.flush();
        socket.waitForBytesWritten(1000);
        socket.disconnectFromServer();
        if (socket.state() != QLocalSocket::UnconnectedState) {
            socket.waitForDisconnected(500);
        }
        return;
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
