"""Loopback-only SOCKS and TLS fixture. Credentials and certificates are synthetic."""
import json
import select
import socket
import ssl
import sys
import threading

root = sys.argv[1]
lock = threading.Lock()


def log(**event):
    with lock, open(root + '/wire.jsonl', 'a', encoding='utf-8') as out:
        out.write(json.dumps(event) + '\n')


def listener():
    sock = socket.socket()
    sock.bind(('127.0.0.1', 0))
    sock.listen(20)
    return sock


def read_exact(conn, count):
    data = b''
    while len(data) < count:
        part = conn.recv(count - len(data))
        if not part:
            raise EOFError()
        data += part
    return data


def serve_tls(conn):
    try:
        with context.wrap_socket(conn, server_side=True) as stream:
            data = stream.recv(4096)
            log(endpoint='tls', event='http', valid=data.startswith(b'GET /controlled '))
            stream.sendall(b'HTTP/1.1 200 OK\r\nContent-Length: 2\r\nConnection: close\r\n\r\nOK')
    except (ssl.SSLError, OSError):
        log(endpoint='tls', event='handshake_rejected')


def socks(conn):
    try:
        with conn:
            conn.settimeout(5)
            version, length = read_exact(conn, 2)
            methods = read_exact(conn, length)
            assert version == 5
            method = 2 if 2 in methods else 0 if 0 in methods else 255
            conn.sendall(bytes((5, method)))
            if method == 255:
                return
            if method == 2:
                version, length = read_exact(conn, 2)
                user = read_exact(conn, length)
                length = read_exact(conn, 1)[0]
                password = read_exact(conn, length)
                accepted = version == 1 and user == b'fixture-user' and password == b'fixture-pass'
                log(endpoint='socks', event='auth', accepted=accepted)
                conn.sendall(bytes((1, 0 if accepted else 1)))
                if not accepted:
                    return
            version, command, reserved, atyp = read_exact(conn, 4)
            assert (version, command, reserved) == (5, 1, 0)
            if atyp == 1:
                address = socket.inet_ntop(socket.AF_INET, read_exact(conn, 4))
            elif atyp == 3:
                address = read_exact(conn, read_exact(conn, 1)[0]).decode('ascii')
            elif atyp == 4:
                address = socket.inet_ntop(socket.AF_INET6, read_exact(conn, 16))
            else:
                raise ValueError('unsupported address type')
            port = int.from_bytes(read_exact(conn, 2), 'big')
            log(endpoint='socks', event='connect', atyp=atyp, address=address, port=port, method=method)
            # Never resolve or connect to a requested host. Only the fixture TLS port is reachable.
            if port != direct.getsockname()[1] or address not in ('127.0.0.1', '::1', 'localhost'):
                conn.sendall(b'\x05\x02\x00\x01\x00\x00\x00\x00\x00\x00')
                return
            with socket.create_connection(('127.0.0.1', tls.getsockname()[1]), 3) as target:
                conn.sendall(b'\x05\x00\x00\x01\x7f\x00\x00\x01\x00\x00')
                while True:
                    ready, _, _ = select.select([conn, target], [], [], 5)
                    if not ready:
                        break
                    for source in ready:
                        data = source.recv(16384)
                        if not data:
                            return
                        (target if source is conn else conn).sendall(data)
    except (OSError, EOFError, AssertionError, ValueError) as exc:
        log(endpoint='socks', event='error', kind=type(exc).__name__)


def trap(name, conn):
    with conn:
        log(endpoint=name, event='unexpected_connection')


def run(sock, handler):
    while True:
        conn, _ = sock.accept()
        threading.Thread(target=handler, args=(conn,), daemon=True).start()


context = ssl.SSLContext(ssl.PROTOCOL_TLS_SERVER)
context.load_cert_chain(root + '/server.pem', root + '/server.key')
tls, proxy, direct, poison = (listener() for _ in range(4))
for sock, handler in ((tls, serve_tls), (proxy, socks), (direct, lambda c: trap('direct', c)), (poison, lambda c: trap('environment', c))):
    threading.Thread(target=run, args=(sock, handler), daemon=True).start()
print(json.dumps({name: sock.getsockname()[1] for name, sock in (('tls', tls), ('proxy', proxy), ('direct', direct), ('poison', poison))}), flush=True)
threading.Event().wait()
