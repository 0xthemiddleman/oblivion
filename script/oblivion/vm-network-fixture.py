#!/usr/bin/python3
"""Typed diagnostics in a lab-only image. No user documents or real API keys."""
import fcntl
import hashlib
import json
import os
import signal
import socket
import subprocess
import sys
import time
import tty
from pathlib import Path

ROLE = sys.argv[1]
API = ("10.88.0.1", 8111)
broker = None
canaries = []


def run(*command, **options):
    return subprocess.run(command, check=True, capture_output=True, timeout=30, **options).stdout


def reply(value):
    print("OBLIVION_NET " + json.dumps(value, separators=(",", ":")), flush=True)


def tor(command):
    with socket.socket(socket.AF_UNIX) as control:
        control.settimeout(10)
        control.connect("/run/tor/control")
        reader = control.makefile("rb")
        cookie = Path("/run/tor/control.authcookie").read_bytes().hex()
        control.sendall(("AUTHENTICATE " + cookie + "\r\n").encode())
        assert reader.readline() == b"250 OK\r\n"
        control.sendall((command + "\r\n").encode())
        lines = []
        while True:
            line = reader.readline().decode().strip()
            assert line.startswith("250"), line
            lines.append(line)
            if line.startswith("250 "):
                return "\n".join(lines)


def start_api(configuration):
    global broker
    assert broker is None
    control, inherited = socket.socketpair()
    listener = socket.socket()
    listener.setsockopt(socket.SOL_SOCKET, socket.SO_REUSEADDR, 1)
    listener.bind(API)
    listener.listen()
    channel = fcntl.fcntl(inherited, fcntl.F_DUPFD_CLOEXEC, 10)
    incoming = fcntl.fcntl(listener, fcntl.F_DUPFD_CLOEXEC, 10)
    pid = os.fork()
    if pid == 0:
        null = os.open("/dev/null", os.O_RDWR)
        for fd in (0, 1, 2):
            os.dup2(null, fd)
        os.dup2(channel, 3)
        os.dup2(incoming, 4)
        os.closerange(5, 1024)
        os.setgroups([])
        os.setgid(1300)
        os.setuid(1300)
        os.execve("/usr/bin/oblivion-api", ["/usr/bin/oblivion-api"], {})
    os.close(channel)
    os.close(incoming)
    inherited.close()
    listener.close()
    broker = (pid, control)
    control.settimeout(10)
    control.sendall((json.dumps({"api_key": configuration["api_key"],
        "session_token": configuration["token"], "socks": "127.0.0.1:9050"}) + "\n").encode())
    assert control.recv(128) == b'{"ready":true}\n'
    return {"ready": True, "uid": 1300}


def stop_api():
    global broker
    assert broker is not None
    pid, control = broker
    control.close()
    deadline = time.monotonic() + 10
    while time.monotonic() < deadline:
        result = os.waitpid(pid, os.WNOHANG)
        if result[0]:
            assert os.waitstatus_to_exitcode(result[1]) == 0
            broker = None
            return {"stopped": True}
        time.sleep(0.01)
    raise AssertionError("API revocation did not finish")


def request(token):
    body = json.dumps({"model": "abliterated-model", "messages": [
        {"role": "user", "content": "synthetic network fixture"}], "stream": True})
    with socket.create_connection(API, timeout=10) as connection:
        connection.sendall(("POST /v1/chat/completions HTTP/1.1\r\nHost: gateway\r\n"
            f"Authorization: Bearer {token}\r\nContent-Type: application/json\r\n"
            f"Content-Length: {len(body)}\r\n\r\n{body}").encode())
        data = b""
        while chunk := connection.recv(65536):
            data += chunk
            assert len(data) <= 65536
        return {"status": int(data.split(b" ", 2)[1]), "body": data.split(b"\r\n\r\n", 1)[1].decode()}


def probes():
    # Treat the chat VM as compromised: root may replace routes and its firewall.
    run("ip", "route", "replace", "default", "via", "10.88.0.1", "dev", "chat0")
    run("nft", "flush", "ruleset")
    denied = []
    for host, port in [("10.88.0.1", 9050), ("10.88.0.1", 9051), ("10.88.0.1", 22),
        ("10.88.0.1", 53), ("10.0.2.2", 80), ("10.0.2.3", 53), ("169.254.169.254", 80),
        ("1.1.1.1", 443)]:
        with socket.socket() as client:
            client.settimeout(0.5)
            assert client.connect_ex((host, port)) != 0, (host, port)
        denied.append(f"{host}:{port}")
    for host, port in [("10.88.0.1", 53), ("10.0.2.3", 53), ("1.1.1.1", 443)]:
        with socket.socket(socket.AF_INET, socket.SOCK_DGRAM) as client:
            client.sendto(b"OBLIVION_DENIED_UDP", (host, port))
    # Give the chat an IPv6 address and route; the gateway still drops IPv6.
    run("ip", "-6", "address", "replace", "fd00:88::2/64", "dev", "chat0")
    with socket.socket(socket.AF_INET6) as client:
        client.settimeout(0.5)
        assert client.connect_ex(("fd00:88::1", 9050)) != 0
    return {"tcp_denied": denied, "udp_probes_sent": 3, "ipv6_denied": True,
        "interfaces": sorted(p.name for p in Path("/sys/class/net").iterdir())}


def gateway(operation, configuration):
    if operation == "start-api":
        return start_api(configuration)
    if operation == "stop-api":
        return stop_api()
    if operation == "rules":
        return json.loads(run("nft", "-j", "list", "ruleset"))
    if operation == "forwarding-on":
        run("busybox", "sysctl", "-q", "-w", "net.ipv4.ip_forward=1")
        return {"forwarding": True}
    if operation == "forwarding-off":
        run("/usr/libexec/oblivion-gateway-network")
        return {"forwarding": False}
    if operation == "tor-enable":
        tor("SETCONF DisableNetwork=0")
        return {"enabled": True}
    if operation == "tor-status":
        return {"bootstrapped": "PROGRESS=100" in tor("GETINFO status/bootstrap-phase")}
    if operation == "tor-check":
        result = subprocess.run(["setpriv", "--reuid=1300", "--regid=1300", "--clear-groups", "--",
            "curl", "--silent", "--show-error", "--fail", "--connect-timeout", "20", "--max-time", "90",
            "--socks5-hostname", "127.0.0.1:9050", "--proxy-user", "oblivion-lab:synthetic-check",
            "https://check.torproject.org/api/ip"], capture_output=True, timeout=95,
            env={"PATH": "/usr/bin:/bin", "HOME": "/tmp"})
        assert result.returncode == 0, result.stderr.decode()
        assert json.loads(result.stdout)["IsTor"] is True
        return {"is_tor": True, "verified_tls": True}
    if operation == "tor-crash":
        run("systemctl", "kill", "--signal=SIGKILL", "--kill-whom=main", "oblivion-tor.service")
        deadline = time.monotonic() + 5
        while subprocess.run(["systemctl", "is-active", "--quiet", "oblivion-tor.service"], timeout=5).returncode == 0:
            assert time.monotonic() < deadline
            time.sleep(0.01)
        return {"stopped": True}
    if operation == "direct-denied":
        result = subprocess.run(["setpriv", "--reuid=1300", "--regid=1300", "--clear-groups", "--",
            "curl", "--silent", "--show-error", "--noproxy", "*", "--connect-timeout", "2", "--max-time", "3",
            "https://1.1.1.1/"], capture_output=True, timeout=5, env={"PATH": "/usr/bin:/bin", "HOME": "/tmp"})
        assert result.returncode != 0
        return {"api_uid_direct_denied": True}
    raise AssertionError("unknown gateway operation")


assert os.getuid() == 0
assert ROLE in ["gateway", "chat"]
assert len(Path("/proc/swaps").read_text().splitlines()) == 1
tty.setraw(0)
if ROLE == "gateway":
    deadline = time.monotonic() + 10
    while not all(Path(path).exists() for path in ["/run/tor/control", "/run/tor/control.authcookie"]):
        assert time.monotonic() < deadline, "Tor control socket readiness deadline"
        time.sleep(0.01)
    assert "DisableNetwork=1" in tor("GETCONF DisableNetwork")
    assert Path("/proc/sys/net/ipv4/ip_forward").read_text().strip() == "0"
    # Actual listening sockets make negative ingress tests meaningful even if
    # a future service accidentally binds to the inside address.
    for port in [22, 53, 9050, 9051]:
        listener = socket.socket()
        listener.bind((API[0], port))
        listener.listen(4)
        canaries.append(listener)
else:
    interface = next(path.name for path in Path("/sys/class/net").iterdir()
        if (path / "address").read_text().strip() == "52:54:00:6f:00:03")
    run("ip", "link", "set", interface, "name", "chat0")
    run("ip", "address", "replace", "10.88.0.2/30", "dev", "chat0")
    run("ip", "link", "set", "chat0", "up")
reply({"ready": ROLE, "fixture_sha256": hashlib.sha256(Path(__file__).read_bytes()).hexdigest()})
for line in sys.stdin:
    command = json.loads(line)
    assert set(command) <= {"id", "operation", "api_key", "token"}
    try:
        if command["operation"] == "poweroff":
            reply({"id": command["id"], "ok": True})
            break
        if ROLE == "gateway":
            result = gateway(command["operation"], command)
        elif command["operation"] == "request":
            result = request(command["token"])
        elif command["operation"] == "probes":
            result = probes()
        elif command["operation"] == "disconnected":
            with socket.socket() as connection:
                connection.settimeout(1)
                assert connection.connect_ex(API) != 0
            result = {"disconnected": True}
        else:
            raise AssertionError("unknown chat operation")
        reply({"id": command["id"], "ok": True, "result": result})
    except BaseException as error:
        reply({"id": command["id"], "ok": False, "error": type(error).__name__})
        raise
if broker:
    stop_api()
