#!/usr/bin/python3
"""Lab-only SOCKS/TLS provider. Synthetic credentials, counters on private serial."""
import hashlib
import json
import os
import socket
import ssl
import struct
import sys
import threading
import tty
from pathlib import Path

ROOT = Path("/usr/lib/oblivion-stream-fixture")
KEYS = {b"Bearer OBLIVION_API_KEY_CANARY_5c91": "initial", b"Bearer OBLIVION_API_NEXT_CANARY_86ab": "alternate"}
lock = threading.Lock()
active = set()
closed = set()
errors = []
sequence = 0
credentials = {}
context = ssl.SSLContext(ssl.PROTOCOL_TLS_SERVER)
context.load_cert_chain(ROOT / "cert.pem", ROOT / "key.pem")


def exact(connection, size):
    data = bytearray()
    while len(data) < size:
        chunk = connection.recv(size - len(data))
        assert chunk, "incomplete fixture input"
        data.extend(chunk)
    return bytes(data)


def serve(connection):
    global sequence
    identifier = None
    try:
        with connection:
            connection.settimeout(180)
            assert exact(connection, 1) == b"\x05"
            methods = exact(connection, exact(connection, 1)[0])
            assert 2 in methods
            connection.sendall(b"\x05\x02")
            assert exact(connection, 1) == b"\x01"
            assert exact(connection, exact(connection, 1)[0]) == b"oblivion-api"
            token = exact(connection, exact(connection, 1)[0])
            assert len(token) == 64 and all(byte in b"0123456789abcdef" for byte in token)
            connection.sendall(b"\x01\x00")
            assert exact(connection, 4) == b"\x05\x01\x00\x03"
            assert exact(connection, exact(connection, 1)[0]) == b"api.abliteration.ai"
            assert struct.unpack(">H", exact(connection, 2))[0] == 443
            connection.sendall(b"\x05\x00\x00\x01\x7f\x00\x00\x01\x00\x00")
            with context.wrap_socket(connection, server_side=True) as tls:
                with tls.makefile("rb") as reader:
                    assert reader.readline(8192) == b"POST /v1/chat/completions HTTP/1.1\r\n"
                    headers = {}
                    while True:
                        line = reader.readline(8192)
                        if line == b"\r\n":
                            break
                        assert line.endswith(b"\r\n") and len(headers) < 32
                        name, value = line.split(b":", 1)
                        assert name.lower() not in headers
                        headers[name.lower()] = value.strip()
                    assert headers[b"authorization"] in KEYS
                    assert headers[b"host"] == b"api.abliteration.ai"
                    size = int(headers[b"content-length"])
                    assert 0 < size <= 16 * 1024 * 1024
                    body = json.loads(reader.read(size))
                    assert body["model"] == "abliterated-model" and body["stream"] is True
                    if "OBLIVION_STREAM_REQUEST" not in json.dumps(body["messages"]):
                        return  # Existing offline probes still receive provider_unavailable.
                    chat = bool(body.get("tools"))
                    tls.sendall(b"HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nTransfer-Encoding: chunked\r\nConnection: close\r\n\r\n")
                    value = {"id": "chatcmpl-fixture", "object": "chat.completion.chunk", "created": 1,
                        "model": "abliterated-model", "choices": [{"index": 0,
                        "delta": {"role": "assistant", "content": "Synthetic open stream"}, "finish_reason": None}]}
                    payload = ("data: " + json.dumps(value) + "\n\n").encode()
                    if not chat:
                        value["choices"][0] = {"index": 0, "delta": {}, "finish_reason": "stop"}
                        payload += ("data: " + json.dumps(value) + "\n\ndata: [DONE]\n\n").encode()
                    if chat:
                        with lock:
                            sequence += 1
                            identifier = sequence
                            active.add(identifier)
                            credentials[identifier] = KEYS[headers[b"authorization"]]
                    tls.sendall(f"{len(payload):x}\r\n".encode() + payload + b"\r\n")
                    if not chat:
                        tls.sendall(b"0\r\n\r\n")
                        return
                    # The response remains open until the real broker closes its
                    # upstream TLS socket. A fixture timeout is a failure, not EOF.
                    try:
                        assert tls.recv(1) == b"", "unexpected request after streaming"
                    except (ssl.SSLEOFError, ConnectionResetError):
                        pass
                    with lock:
                        active.remove(identifier)
                        closed.add(identifier)
    except (BrokenPipeError, ConnectionResetError, ssl.SSLEOFError):
        with lock:
            if identifier is not None:
                active.discard(identifier)
                closed.add(identifier)
    except BaseException as error:
        with lock:
            errors.append(type(error).__name__)


def accept(listener):
    while True:
        connection, _ = listener.accept()
        threading.Thread(target=serve, args=(connection,), daemon=True).start()


assert os.getuid() == 0
assert len(Path("/proc/swaps").read_text().splitlines()) == 1
listener = socket.socket()
listener.setsockopt(socket.SOL_SOCKET, socket.SO_REUSEADDR, 1)
listener.bind(("127.0.0.1", 9050))
listener.listen(16)
threading.Thread(target=accept, args=(listener,), daemon=True).start()
tty.setraw(0)
for line in sys.stdin:
    request = json.loads(line)
    assert set(request) == {"operation", "id"} and request["operation"] == "status"
    with lock:
        value = {"id": request["id"], "active": sorted(active), "closed": sorted(closed), "errors": errors.copy(), "credentials": credentials.copy()}
    value["fixture_sha256"] = hashlib.sha256(Path(__file__).read_bytes()).hexdigest()
    value["api_sha256"] = hashlib.sha256(Path("/usr/bin/oblivion-api").read_bytes()).hexdigest()
    print("OBLIVION_STREAM " + json.dumps(value, separators=(",", ":")), flush=True)
