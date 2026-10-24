#!/usr/bin/env python3
"""Actual systemd units, manager PTY, LUKS and QEMU; disposable synthetic disks only."""
import codecs
import fcntl
import hashlib
import json
import os
import pwd
import select
import shutil
import signal
import struct
import subprocess
import termios
import time
from pathlib import Path

import pyte

CONFIG = "/run/oblivion-services-test.json"
GENERAL = b"synthetic-general-password"
PASSWORD = b"synthetic-session-password-a"
CONTROLLER = "oblivion-controller.service"
MANAGER = "oblivion-manager.service"
RECOVER = "oblivion-recover.service"
terminal = pyte.Screen(100, 30)
stream = pyte.Stream(terminal)
decoder = codecs.getincrementaldecoder("utf-8")("replace")
output = bytearray()


def check(label):
    print("PASS: " + label, flush=True)


def systemctl(*arguments):
    result = subprocess.run(["systemctl", *arguments], capture_output=True, timeout=180)
    assert result.returncode == 0, (arguments, result.stderr)
    return result.stdout.decode().strip()


def property(unit, name):
    return systemctl("show", unit, "--property=" + name, "--value")


def command(operation, *arguments, secret=b"", success=True):
    result = subprocess.run(["/usr/bin/oblivion-storage-lab", "--config", CONFIG, operation, *arguments],
        input=secret, capture_output=True, timeout=90)
    assert (result.returncode == 0) == success, (operation, result.stderr)
    return result.stdout.decode().strip() if success else result.stderr.decode().strip()


def status():
    return json.loads(command("status"))


def read_terminal():
    if select.select([master], [], [], 0.1)[0]:
        data = os.read(master, 65536)
        output.extend(data)
        stream.feed(decoder.decode(data))


def wait_screen(marker):
    deadline = time.monotonic() + 180
    while time.monotonic() < deadline:
        read_terminal()
        if marker in "\n".join(terminal.display):
            return
    raise AssertionError(("screen timeout", marker, "\n".join(terminal.display),
        systemctl("status", MANAGER) if property(MANAGER, "ActiveState") == "active" else property(MANAGER, "Result")))


def send(value, marker):
    os.write(master, value)
    wait_screen(marker)


def wait_restart(unit, previous):
    deadline = time.monotonic() + 180
    while time.monotonic() < deadline:
        pid = int(property(unit, "MainPID"))
        if pid > 0 and pid != previous and property(unit, "ActiveState") == "active":
            return pid
        read_terminal()
    raise AssertionError(("restart timeout", unit, property(unit, "Result")))


def locked():
    current = status()
    assert not any(current[k] for k in ["general_open", "general_mounted", "session_open", "recovery_required"]), current
    assert current["vm_state"] in ["inactive", "failed"], current


def reopen():
    send(b"\r", "Password del volume generale")
    send(GENERAL + b"\r", "Sessioni protette")
    assert not status()["session_open"]
    send(b"\r", "Password della sessione")
    send(PASSWORD + b"\r", "Conversazioni e documenti")
    wait_screen("/exit blocca")


print("OBLIVION_SERVICES_FIXTURE_SHA256 " + hashlib.sha256(Path(__file__).read_bytes()).hexdigest(), flush=True)
os.umask(0o077)
bundle = Path("/run/oblivion-lab-bundle")
manifest = (bundle / "SHA256SUMS").read_text()
for line in manifest.splitlines():
    digest, name = line.split()
    assert hashlib.sha256((bundle / name).read_bytes()).hexdigest() == digest, name
    if name in ["oblivion-storage", "oblivion-storage-lab", "oblivion-manager", "oblivion-control"]:
        assert hashlib.sha256((Path("/usr/bin") / name).read_bytes()).hexdigest() == digest, name
print("OBLIVION_HOST_BUNDLE_SHA256 " + hashlib.sha256(manifest.encode()).hexdigest(), flush=True)
subprocess.run(["mkfs.ext4", "-q", "-m", "0", "/dev/vdb"], check=True)
subprocess.run(["mount", "/dev/vdb", "/data"], check=True)
Path(CONFIG).write_text(json.dumps({"general_image": "/data/general.luks", "general_mib": 768,
    "session_mib": 128, "pbkdf_memory_kib": 65536, "manager_uid": 1000,
    "vm": {"root": "/usr/lib/oblivion/vm/qemu-root", "kernel": "/usr/lib/oblivion/vm/kernel",
           "initrd": "/usr/lib/oblivion/vm/initrd", "filesystem": "/usr/lib/oblivion/vm/guest.squashfs",
           "memory_mib": 1536, "cpus": 2, "accelerator": "tcg"}}))
command("init-general", secret=GENERAL)
with Path("/run/oblivion/storage.lock").open("rb") as operation_lock:
    fcntl.flock(operation_lock, fcntl.LOCK_EX | fcntl.LOCK_NB)
    assert command("recover", success=False) == "controller_busy"
assert status()["recovery_required"]
assert command("unlock", secret=GENERAL, success=False) == "recovery_required"
command("lock")
locked()
check("recovery latches before a busy operation lock and only verified closure clears it")
# Older immutable lab images predate the dedicated manager account. Supply the
# same public account records as the current Dockerfile, exclusively in RAM.
if not any(user.pw_uid == 1000 for user in pwd.getpwall()):
    for name, entry in [("passwd", "oblivion-manager:x:1000:1000::/run/oblivion-manager:/usr/sbin/nologin\n"),
            ("group", "oblivion-manager:x:1000:\n")]:
        overlay = Path("/run/oblivion-lab-" + name)
        overlay.write_text((Path("/etc") / name).read_text() + entry)
        overlay.chmod(0o644)
        subprocess.run(["mount", "--bind", str(overlay), "/etc/" + name], check=True)
assert pwd.getpwuid(1000).pw_name == "oblivion-manager"
master, slave = os.openpty()
os.chown(os.ttyname(slave), 1000, 1000)
fcntl.ioctl(slave, termios.TIOCSWINSZ, struct.pack("HHHH", 30, 100, 0, 0))
for file in bundle.glob("*.service"):
    shutil.copyfile(file, Path("/run/systemd/system") / file.name)
    directory = Path("/run/systemd/system") / (file.name + ".d")
    directory.mkdir()
    # Only the binary/config/console paths change. Lifecycle, dependencies and
    # manager confinement are the exact units intended for the live image.
    if file.name == MANAGER:
        override = f"TTYPath={os.ttyname(slave)}\nTTYRows=30\nTTYColumns=100\n"
    else:
        operation = "serve" if file.name == CONTROLLER else "recover"
        override = f"ExecStart=\nExecStart=/usr/bin/oblivion-storage-lab --config {CONFIG} {operation}\n"
        if file.name == CONTROLLER:
            override += f"ExecStartPre=\nExecStartPre=-/usr/bin/oblivion-storage-lab --config {CONFIG} recover\n"
            override += f"ExecStopPost=\nExecStopPost=/usr/bin/oblivion-storage-lab --config {CONFIG} recover\n"
    (directory / "lab.conf").write_text("[Service]\n" + override)
systemctl("daemon-reload")
try:
    systemctl("start", CONTROLLER)
    wait_screen("Archivio generale bloccato")
    manager_pid = int(property(MANAGER, "MainPID"))
    process = Path(f"/proc/{manager_pid}/status").read_text()
    assert "Uid:\t1000\t1000\t1000\t1000" in process and "CapEff:\t0000000000000000" in process
    assert "NoNewPrivs:\t1" in process
    assert os.readlink(f"/proc/{manager_pid}/ns/net") != os.readlink("/proc/1/ns/net")
    assert property(CONTROLLER, "Type") == "notify"
    locked()
    check("systemd waits for controller readiness and runs the confined manager as UID 1000")
    send(b"\r", "Password del volume generale")
    send(GENERAL + b"\r", "Nessuna sessione")
    send(b"n", "Nuova sessione - password")
    send(PASSWORD + b"\r", "Ripeti la password")
    send(PASSWORD + b"\r", "Conversazioni e documenti")
    wait_screen("/exit blocca")
    session_id = status()["sessions"][0]
    terminal.reset()
    os.kill(manager_pid, signal.SIGKILL)
    manager_pid = wait_restart(MANAGER, manager_pid)
    wait_screen("Archivio generale bloccato")
    locked()
    check("SIGKILL of manager powers down the VM and closes both LUKS mappings before restarting")
    reopen()
    controller_pid = int(property(CONTROLLER, "MainPID"))
    terminal.reset()
    os.kill(controller_pid, signal.SIGKILL)
    wait_restart(CONTROLLER, controller_pid)
    manager_pid = wait_restart(MANAGER, manager_pid)
    wait_screen("Archivio generale bloccato")
    locked()
    check("SIGKILL of controller stops its dependent manager and recovers before both restart")
    reopen()
    vm_pid = int(property("oblivion-session.service", "MainPID"))
    os.kill(vm_pid, signal.SIGSTOP)
    terminal.reset()
    os.kill(manager_pid, signal.SIGKILL)
    manager_pid = wait_restart(MANAGER, manager_pid)
    wait_screen("RECUPERO NECESSARIO")
    current = status()
    assert current["recovery_required"] and current["session_open"] and current["general_open"], current
    for operation, arguments, secret in [("create", [], PASSWORD), ("enter", [session_id], PASSWORD),
            ("unlock", [], GENERAL), ("unlock-ephemeral", [], GENERAL)]:
        assert command(operation, *arguments, secret=secret, success=False) == "recovery_required"
    rejected = subprocess.run(["/usr/bin/oblivion-control", "create"], input=PASSWORD,
        capture_output=True, user=1000, group=1000, extra_groups=[], timeout=30)
    assert rejected.returncode != 0 and rejected.stderr == b"control_request_failed\n"
    check("unresponsive VM leaves recovery latched and rejects every new opening")
    # Restart once more while recovery is failing: the error must survive UI restarts.
    terminal.reset()
    os.kill(manager_pid, signal.SIGKILL)
    manager_pid = wait_restart(MANAGER, manager_pid)
    wait_screen("RECUPERO NECESSARIO")
    assert status()["recovery_required"]
    send(b"f", "Premi F di nuovo")
    send(b"x", "RECUPERO NECESSARIO")
    assert status()["session_open"] and int(property("oblivion-session.service", "MainPID")) == vm_pid
    send(b"f", "Premi F di nuovo")
    send(b"f", "Archivio generale bloccato")
    locked()
    check("recovery survives restart; cancelled force-stop preserves VM; explicit confirmation closes both volumes")
    reopen()
    # An explicit service stop must complete without ordering deadlocks or orphan VMs.
    systemctl("stop", CONTROLLER)
    locked()
    assert property(MANAGER, "ActiveState") in ["inactive", "failed"]
    assert not Path(f"/proc/{manager_pid}").exists()
    check("controller stop drains dependent services and leaves no active VM or open volume")
    assert GENERAL not in output and PASSWORD not in output
    check("synthetic passwords never appear on the managed console")
    subprocess.run(["umount", "/data"], check=True)
    print("OBLIVION_VM_LAB_PASS", flush=True)
except BaseException:
    print("OBLIVION_VM_LAB_FAIL", flush=True)
    # This disposable image journals only synthetic diagnostics in RAM.
    subprocess.run(["journalctl", "-b", "-u", CONTROLLER, "-u", MANAGER, "-u", RECOVER, "--no-pager", "-n", "80"])
    raise
finally:
    os.close(slave)
    os.close(master)
