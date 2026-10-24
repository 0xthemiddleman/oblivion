#!/usr/bin/python3
"""Gateway component tests in disposable, swap-free Debian; no external NIC."""
import fcntl
import hashlib
import json
import os
import re
import signal
import socket
import subprocess
import time
from pathlib import Path

BUNDLE = Path("/run/oblivion-lab-bundle")
KEY = "synthetic-provider-key-from-private-channel"
TOKEN = "c" * 64


def check(label):
    print("PASS: " + label, flush=True)


def wait(pid):
    deadline = time.monotonic() + 10
    while time.monotonic() < deadline:
        result = os.waitpid(pid, os.WNOHANG)
        if result[0]:
            return os.waitstatus_to_exitcode(result[1])
        time.sleep(0.01)
    os.kill(pid, signal.SIGKILL)
    os.waitpid(pid, 0)
    raise AssertionError("broker did not exit")


def production(drop_identity):
    control, inherited = socket.socketpair()
    listener = socket.socket()
    listener.bind(("127.0.0.1", 0))
    listener.listen()
    address = listener.getsockname()
    channel = fcntl.fcntl(inherited, fcntl.F_DUPFD_CLOEXEC, 10)
    incoming = fcntl.fcntl(listener, fcntl.F_DUPFD_CLOEXEC, 10)
    output, write = os.pipe()
    pid = os.fork()
    if pid == 0:
        os.dup2(write, 1)
        os.dup2(write, 2)
        os.dup2(channel, 3)
        os.dup2(incoming, 4)
        os.closerange(5, 1024)
        if drop_identity:
            os.setgroups([])
            os.setgid(1000)
            os.setuid(1000)
        os.execve(str(BUNDLE / "oblivion-api"), [str(BUNDLE / "oblivion-api")], {})
    for fd in (write, channel, incoming):
        os.close(fd)
    inherited.close()
    listener.close()
    control.settimeout(10)
    if not drop_identity:
        assert wait(pid) == 1
        assert os.read(output, 8192) == b"oblivion-api: service_failed\n"
        os.close(output)
        control.close()
        check("production binary rejects execution as root")
        return
    try:
        control.sendall((json.dumps({"api_key": KEY, "session_token": TOKEN,
                                    "socks": "127.0.0.1:1"}) + "\n").encode())
        assert control.recv(128) == b'{"ready":true}\n'
        status = Path(f"/proc/{pid}/status").read_text()
        assert re.search(r"^Uid:\s+1000\s+1000\s+1000\s+1000$", status, re.M)
        assert re.search(r"^NoNewPrivs:\s+1$", status, re.M)
        assert re.search(r"^CapEff:\s+0000000000000000$", status, re.M)
        limits = Path(f"/proc/{pid}/limits").read_text()
        core = next(line for line in limits.splitlines() if line.startswith("Max core file size"))
        assert core.split()[-3:] == ["0", "0", "bytes"]
        assert KEY.encode() not in Path(f"/proc/{pid}/cmdline").read_bytes()
        check("production binary accepts a root-owned control channel as UID 1000 with no capabilities, no-new-privileges and core limit zero")
        for token, expected in [("invalid", b"HTTP/1.1 401"), (TOKEN, b"HTTP/1.1 502")]:
            body = json.dumps({"model": "abliterated-model", "messages": [
                {"role": "user", "content": "synthetic prompt"}], "stream": True})
            with socket.create_connection(address, timeout=10) as client:
                client.sendall(("POST /v1/chat/completions HTTP/1.1\r\nHost: gateway\r\n"
                    f"Authorization: Bearer {token}\r\nContent-Type: application/json\r\n"
                    f"Content-Length: {len(body)}\r\n\r\n{body}").encode())
                response = b""
                while chunk := client.recv(65536):
                    response += chunk
                assert response.startswith(expected)
                assert KEY.encode() not in response
        check("production API rejects a wrong token and fails closed with SOCKS unavailable")
        control.close()
        assert wait(pid) == 0
        pid = None
        assert os.read(output, 8192) == b""
        with socket.socket() as client:
            assert client.connect_ex(address) != 0
        check("production controller disconnect removes the listener and exits without stdout or stderr")
    finally:
        control.close()
        os.close(output)
        if pid is not None:
            try:
                os.kill(pid, signal.SIGKILL)
                os.waitpid(pid, 0)
            except ProcessLookupError:
                pass


try:
    assert os.getuid() == 0
    assert len(Path("/proc/swaps").read_text().splitlines()) == 1
    assert sorted(path.name for path in Path("/sys/class/net").iterdir()) == ["lo"]
    manifest = (BUNDLE / "SHA256SUMS").read_bytes()
    for line in manifest.decode().splitlines():
        digest, name = line.split()
        assert Path(name).name == name
        assert hashlib.sha256((BUNDLE / name).read_bytes()).hexdigest() == digest
    print("OBLIVION_GATEWAY_BUNDLE_SHA256 " + hashlib.sha256(manifest).hexdigest(), flush=True)
    print("OBLIVION_GATEWAY_FIXTURE_SHA256 " + hashlib.sha256(Path(__file__).read_bytes()).hexdigest(), flush=True)
    check("Debian ARM laboratory has no swap, no external network interface and verified gateway binaries")
    # This path is embedded by Cargo into the executable activation tests. The
    # symlink reaches a read-only executable RAM mount, not a shared filesystem.
    target = Path("/tmp/target/debug/oblivion-api")
    target.parent.mkdir(parents=True)
    target.symlink_to(BUNDLE / "oblivion-api-lab")
    for executable, expected in [("gateway-tests", 10), ("activation-tests", 4)]:
        result = subprocess.run(["setpriv", "--reuid=1000", "--regid=1000", "--clear-groups",
            "--no-new-privs", "--", str(BUNDLE / executable)],
            capture_output=True, text=True, timeout=120, env={"PATH": "/usr/bin:/bin", "HOME": "/tmp"})
        assert result.returncode == 0, result.stdout + result.stderr
        passed = re.findall(r"^test (.+) \.\.\. ok$", result.stdout, re.M)
        assert len(passed) == expected, result.stdout
        for test in passed:
            check(executable + ": " + test)
    production(False)
    production(True)
    print("OBLIVION_VM_LAB_PASS", flush=True)
except BaseException:
    print("OBLIVION_VM_LAB_FAIL", flush=True)
    raise
