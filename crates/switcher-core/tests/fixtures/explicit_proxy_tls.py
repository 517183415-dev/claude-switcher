"""Bounded loopback CONNECT/TLS recorders. All certificates and traffic are synthetic."""
import json
import os
import select
import socket
import socketserver
import ssl
import sys
import threading
from pathlib import Path

root = Path(sys.argv[1])
assert sys.flags.isolated == 1, "fixture requires isolated Python"
# macOS may synthesize this encoding hint even after Command.env_clear().
assert set(os.environ) <= {"HOME", "TMPDIR", "LC_ALL", "LC_CTYPE", "__CF_USER_TEXT_ENCODING"}
assert os.environ["HOME"] == str(root / "home")
assert os.environ["TMPDIR"] == str(root / "tmp")
lock = threading.Lock()
context = ssl.SSLContext(ssl.PROTOCOL_TLS_SERVER)
context.load_cert_chain(root / "server.pem", root / "server.key")


def record(endpoint, event, **fields):
    with lock, (root / "capture.jsonl").open("a") as stream:
        stream.write(json.dumps(dict(endpoint=endpoint, event=event, **fields)) + "\n")


def header(stream):
    data = bytearray()
    while not data.endswith(b"\r\n\r\n"):
        chunk = stream.recv(1)
        if not chunk:
            raise ValueError("incomplete synthetic request")
        data.extend(chunk)
        if len(data) > 4096:
            raise ValueError("synthetic header limit")
    lines = data.decode("ascii").split("\r\n")
    method, target, _ = lines[0].split(" ")
    headers = dict(line.split(": ", 1) for line in lines[1:] if line)
    return method, target, headers


def reply(stream):
    stream.sendall(b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\nConnection: close\r\n\r\nok")


class Server(socketserver.ThreadingTCPServer):
    daemon_threads = True

    def handle_error(self, request, address):
        # A fixture failure remains a machine-readable event, without console addresses.
        record(self.endpoint, "fixture_error")


class Sink(socketserver.BaseRequestHandler):
    def handle(self):
        self.request.settimeout(3)
        method, target, _ = header(self.request)
        record(self.server.endpoint, "request", method=method, target=target)
        reply(self.request)


class Tls(socketserver.BaseRequestHandler):
    def handle(self):
        self.request.settimeout(3)
        record("tls", "accepted")
        try:
            stream = context.wrap_socket(self.request, server_side=True)
        except ssl.SSLError:
            record("tls", "handshake_rejected")
            return
        with stream:
            method, target, headers = header(stream)
            record("tls", "request", method=method, target=target,
                   user_agent=headers.get("User-Agent", headers.get("user-agent")))
            reply(stream)


class Proxy(socketserver.BaseRequestHandler):
    def handle(self):
        self.request.settimeout(3)
        method, target, _ = header(self.request)
        record("explicit", "request", method=method, target=target)
        if method == "GET":
            if target != "/positive-control":
                raise ValueError("unexpected proxy request")
            reply(self.request)
            return
        allowed = {f"127.0.0.1:{direct.server_address[1]}",
                   f"localhost:{direct.server_address[1]}"}
        if method != "CONNECT" or target not in allowed:
            raise ValueError("unexpected CONNECT authority")
        # Forward only to this process's loopback TLS server, never resolve a target.
        with socket.create_connection(tls.server_address, timeout=3) as upstream:
            self.request.sendall(b"HTTP/1.1 200 Connection Established\r\n\r\n")
            peers = [self.request, upstream]
            while True:
                ready, _, _ = select.select(peers, [], [], 5)
                if not ready:
                    raise TimeoutError("synthetic tunnel timeout")
                for source in ready:
                    data = source.recv(16384)
                    if not data:
                        return
                    destination = upstream if source is self.request else self.request
                    destination.sendall(data)


servers = []
for name, handler in [("tls", Tls), ("environment", Sink), ("direct", Sink),
                      ("explicit", Proxy)]:
    server = Server(("127.0.0.1", 0), handler)
    server.endpoint = name
    servers.append(server)
tls, environment, direct, explicit = servers
for server in servers:
    threading.Thread(target=server.serve_forever, daemon=True).start()
print(json.dumps({server.endpoint: server.server_address[1] for server in servers}), flush=True)
threading.Event().wait()
