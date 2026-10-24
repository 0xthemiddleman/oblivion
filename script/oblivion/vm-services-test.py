#!/usr/bin/env python3
"""Actual systemd units, manager PTY, LUKS and QEMU; disposable synthetic disks only."""
import codecs
import ctypes
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


def file_digest(path):
    with Path(path).open("rb") as file:
        return hashlib.file_digest(file, "sha256").hexdigest()


def interrupt_creation(operation, secret, executable):
    # Trace real exec/syscall events so even a fast mkfs can be stopped after a
    # successful write to the actual mapped block device.
    # No fake cryptsetup, shortened production timeout or filesystem hook is used.
    libc = ctypes.CDLL(None, use_errno=True)
    libc.ptrace.argtypes = [ctypes.c_ulong, ctypes.c_int, ctypes.c_void_p, ctypes.c_void_p]
    libc.ptrace.restype = ctypes.c_long

    def trace(request, pid, data=0, address=0):
        result = libc.ptrace(request, pid, address, data)
        assert result != -1, (request, pid, ctypes.get_errno())
        return result

    child = subprocess.Popen(["/usr/bin/oblivion-storage-lab", "--config", CONFIG, operation],
        stdin=subprocess.PIPE, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL,
        preexec_fn=lambda: trace(0, 0))  # PTRACE_TRACEME, before controller exec
    child.stdin.write(secret)
    child.stdin.close()
    live = {child.pid}
    deadline = time.monotonic() + 45
    target = None
    try:
        pid, state = os.waitpid(child.pid, 0)
        assert os.WIFSTOPPED(state)
        trace(0x4200, pid, 1 | 2 | 4 | 8 | 16 | 0x100000)  # TRACESYSGOOD, fork/vfork/clone/exec + EXITKILL
        trace(7, pid)  # CONT
        while target is None:
            assert time.monotonic() < deadline, ("exec boundary timeout", executable)
            pid, state = os.waitpid(-1, os.WNOHANG | 0x40000000)  # __WALL
            if pid == 0:
                time.sleep(0.001)
                continue
            if os.WIFEXITED(state) or os.WIFSIGNALED(state):
                live.discard(pid)
                assert pid != child.pid, "creation completed before target exec"
                continue
            event = state >> 16
            if event in [1, 2, 3]:
                forked = ctypes.c_ulong()
                trace(0x4201, pid, ctypes.byref(forked))  # GETEVENTMSG
                live.add(forked.value)
            if event == 4 and Path(f"/proc/{pid}/exe").resolve().name == executable:
                target = pid
                break
            trace(7, pid, 0 if os.WSTOPSIG(state) in [signal.SIGTRAP, signal.SIGSTOP] else os.WSTOPSIG(state))
        block = Path("/dev/mapper/oblivion-general" if operation == "init-general" else "/dev/mapper/oblivion-session").resolve()
        writing_block = False
        while True:
            trace(24, target)  # PTRACE_SYSCALL
            while True:
                assert time.monotonic() < deadline, "block write timeout"
                pid, state = os.waitpid(target, os.WNOHANG | 0x40000000)
                if pid:
                    break
                time.sleep(0.001)
            assert os.WIFSTOPPED(state) and os.WSTOPSIG(state) == signal.SIGTRAP | 0x80
            info = ctypes.create_string_buffer(88)
            assert trace(0x420e, target, info, len(info)) >= 33  # GET_SYSCALL_INFO
            assert struct.unpack_from("=I", info.raw, 4)[0] == 0xc00000b7  # lab ABI: AArch64
            if info.raw[0] == 1:  # syscall entry: union at byte 24, then nr and fd
                number, fd = struct.unpack_from("=QQ", info.raw, 24)
                writing_block = number in [64, 68] and Path(f"/proc/{target}/fd/{fd}").resolve() == block
            if info.raw[0] == 2 and writing_block and struct.unpack_from("=q", info.raw, 24)[0] > 0:
                break  # successful write/pwrite64 exit; mkfs has not completed
        print("OBLIVION_FAULT_AFTER_BLOCK_WRITE " + operation, flush=True)
        os.kill(child.pid, signal.SIGKILL)
        assert os.WIFSIGNALED(os.waitpid(child.pid, 0)[1])
        live.remove(child.pid)
        child.returncode = -signal.SIGKILL
        assert command("status", success=False) == "controller_busy"
        assert command("recover", success=False) == "controller_busy"
    finally:
        for pid in live:
            os.kill(pid, signal.SIGKILL)
        for pid in live:
            os.waitpid(pid, 0)
    command("recover")
    locked()


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
interrupt_creation("init-general", GENERAL, "mke2fs")
assert status()["general_incomplete"] and not status()["general_present"]
before = file_digest("/data/general.luks.pending")
assert command("init-general", secret=GENERAL, success=False) == "image_exists_or_unavailable"
assert file_digest("/data/general.luks.pending") == before
check("interrupted general filesystem formatting retains an unpublished file and retry never overwrites it")
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
    wait_screen("Preparazione generale interrotta")
    manager_pid = int(property(MANAGER, "MainPID"))
    process = Path(f"/proc/{manager_pid}/status").read_text()
    assert "Uid:\t1000\t1000\t1000\t1000" in process and "CapEff:\t0000000000000000" in process
    assert "NoNewPrivs:\t1" in process
    assert os.readlink(f"/proc/{manager_pid}/ns/net") != os.readlink("/proc/1/ns/net")
    assert property(CONTROLLER, "Type") == "notify"
    locked()
    check("systemd waits for controller readiness and runs the confined manager as UID 1000")
    send(b"d", "Premi D di nuovo")
    send(b"x", "Preparazione generale interrotta")
    assert file_digest("/data/general.luks.pending") == before
    send(b"d", "Premi D di nuovo")
    send(b"d", "Prima preparazione")
    assert not status()["general_incomplete"] and not status()["general_present"]
    outer_filler = Path("/data/space-fixture")
    space = os.statvfs("/data")
    with outer_filler.open("xb") as file:
        os.posix_fallocate(file.fileno(), 0, space.f_bavail * space.f_frsize - 4 * 1024 * 1024)
    send(b"\r", "Nuovo volume generale - password")
    send(GENERAL + b"\r", "Ripeti la password del volume generale")
    send(GENERAL + b"\r", "Spazio insufficiente")
    assert status()["general_incomplete"] and not status()["general_present"]
    outer_filler.unlink()
    send(b"d", "Premi D di nuovo")
    send(b"d", "Prima preparazione")
    check("outer disk exhaustion cannot publish a partial general volume; explicit cleanup permits retry")
    send(b"\r", "Nuovo volume generale - password")
    send(GENERAL + b"\r", "Ripeti la password del volume generale")
    send(GENERAL + b"\r", "Archivio generale bloccato")
    assert status()["general_present"] and not status()["general_incomplete"]
    before = file_digest("/data/general.luks")
    assert command("init-general", secret=GENERAL, success=False) == "general_already_exists"
    assert command("discard-general-pending", success=False) == "general_already_exists"
    assert file_digest("/data/general.luks") == before
    check("trusted setup confirms discard, recreates a complete general volume and protects existing archives")
    send(b"\r", "Password del volume generale")
    send(GENERAL + b"\r", "Nessuna sessione")
    send(b"n", "Nuova sessione - password")
    send(PASSWORD + b"\r", "Ripeti la password")
    send(PASSWORD + b"\r", "Conversazioni e documenti")
    wait_screen("/exit blocca")
    session_id = status()["sessions"][0]
    send(b"\x1d", "Sessioni protette")
    archive = Path(f"/run/oblivion/general/sessions/{session_id}.luks")
    before = file_digest(archive)
    Path("/run/oblivion/general/profile/canary").write_bytes(b"OBLIVION_VM_A_CANARY_4d02")
    interrupt_creation("create", PASSWORD, "mke2fs")
    encrypted_before = file_digest("/data/general.luks")
    command("unlock-ephemeral", secret=GENERAL)
    pending_id = next(Path("/run/oblivion/general/sessions").glob("*.pending")).stem
    assert command("discard-session-pending", pending_id, success=False) == "general_not_writable"
    assert status()["incomplete_sessions"] == []
    command("lock")
    assert file_digest("/data/general.luks") == encrypted_before
    check("read-only profile rejects pending deletion and remains byte-identical")
    terminal.reset()
    os.kill(manager_pid, signal.SIGKILL)
    manager_pid = wait_restart(MANAGER, manager_pid)
    wait_screen("Archivio generale bloccato")
    send(b"\r", "Password del volume generale")
    send(GENERAL + b"\r", "Sessioni protette")
    current = status()
    assert current["sessions"] == [session_id] and len(current["incomplete_sessions"]) == 1
    pending_id = current["incomplete_sessions"][0]
    assert command("open", pending_id, secret=PASSWORD, success=False) == "file_unavailable"
    assert file_digest(archive) == before
    pending = Path(f"/run/oblivion/general/sessions/{pending_id}.pending")
    partial = file_digest(pending)
    send(b"i", "Creazioni interrotte - mai aperte")
    send(b"d", "Premi D di nuovo")
    send(b"x", "Creazioni interrotte - mai aperte")
    assert file_digest(pending) == partial
    send(b"d", "Premi D di nuovo")
    send(b"d", "Nessuna creazione interrotta")
    assert not pending.exists() and file_digest(archive) == before
    assert command("discard-session-pending", session_id, success=False) == "file_unavailable"
    assert command("discard-session-pending", "../profile", success=False) == "invalid_session_id"
    alias_id = "f" * 32
    alias = archive.parent / (alias_id + ".pending")
    alias.symlink_to(archive)
    assert command("discard-session-pending", alias_id, success=False) == "file_unavailable"
    alias.unlink()
    os.link(archive, alias)
    assert command("discard-session-pending", alias_id, success=False) == "unsafe_file"
    alias.unlink()
    assert file_digest(archive) == before
    check("interrupted session formatting survives restart, cannot open, and is discarded only after confirmation")
    # Fill the general filesystem with a synthetic preallocation. The next
    # creation must fail without touching the already published session.
    filler = Path("/run/oblivion/general/profile/space-fixture")
    with filler.open("xb") as file:
        os.posix_fallocate(file.fileno(), 0, status()["available_bytes"] - 4 * 1024 * 1024)
    send(b"i", "Sessioni protette")
    send(b"n", "Nuova sessione - password")
    send(PASSWORD + b"\r", "Ripeti la password")
    send(PASSWORD + b"\r", "Spazio insufficiente")
    assert status()["sessions"] == [session_id] and not status()["session_open"]
    assert len(status()["incomplete_sessions"]) == 1 and file_digest(archive) == before
    assert Path("/run/oblivion/general/profile/canary").read_bytes() == b"OBLIVION_VM_A_CANARY_4d02"
    filler.unlink()
    send(b"i", "Creazioni interrotte - mai aperte")
    send(b"d", "Premi D di nuovo")
    send(b"d", "Nessuna creazione interrotta")
    assert not status()["incomplete_sessions"] and status()["available_bytes"] >= status()["session_bytes"]
    send(b"i", "Sessioni protette")
    send(b"\r", "Password della sessione")
    send(PASSWORD + b"\r", "Conversazioni e documenti")
    wait_screen("/exit blocca")
    check("disk exhaustion is explicit, preserves existing archives and permits reopening after pending cleanup")
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
