#!/usr/bin/env python3
"""Disposable Debian host + nested session VMs with synthetic data; never physical disks."""
import argparse
import base64
import gzip
import hashlib
import json
import platform
import re
import subprocess
import tempfile
import time
import zlib
from pathlib import Path

parser = argparse.ArgumentParser()
parser.add_argument("--image", type=Path, required=True)
parser.add_argument("--output", type=Path, required=True)
parser.add_argument("--refresh-tui-test", action="store_true",
    help="Load the current synthetic TUI test into the lab initramfs, without rebuilding Debian")
args = parser.parse_args()
if (platform.system(), platform.machine()) != ("Darwin", "arm64"):
    raise SystemExit("This laboratory runner currently requires macOS ARM64/HVF.")
image = args.image.resolve()
output = args.output.resolve()
if any("," in str(path) for path in (image, output)):
    raise SystemExit("Paths containing QEMU option separators are unsupported.")
output.mkdir(parents=True, exist_ok=False)
initrd = image / "initrd"
fixture = None
if args.refresh_tui_test:
    if not (image / "tui-lab").exists():
        raise SystemExit("The fixture override requires a tui-host laboratory image.")
    fixture = Path(__file__).with_name("vm-tui-test.py").read_bytes()
    # Debian's initramfs-tools executes its generated ORDER file, not every
    # executable in init-bottom. Preserve that order and append the lab hook.
    order = None
    for header in re.finditer(b"\x1f\x8b\x08", initrd.read_bytes()):
        try:
            archive = gzip.decompress(initrd.read_bytes()[header.start():])
        except (zlib.error, EOFError, gzip.BadGzipFile):
            continue
        result = subprocess.run(["bsdtar", "-xOf", "-", "scripts/init-bottom/ORDER"],
            input=archive, capture_output=True, check=True)
        order = result.stdout
        break
    if order is None:
        raise SystemExit("Cannot find the Debian initramfs hook order; refusing fixture override.")
    with tempfile.TemporaryDirectory(prefix="oblivion-lab-initrd-") as temporary:
        root = Path(temporary)
        (root / "scripts/init-bottom").mkdir(parents=True)
        (root / "scripts/oblivion-tui-test.py").write_bytes(fixture)
        hook = root / "scripts/init-bottom/zz-oblivion-lab-fixture"
        hook.write_text('#!/bin/sh\nset -eu\ncase "${1:-}" in prereqs) exit 0;; esac\n'
            'mount --bind /scripts/oblivion-tui-test.py "${rootmnt:-/root}/usr/libexec/oblivion-vm-test"\n')
        hook.chmod(0o755)
        (hook.parent / "ORDER").write_bytes(order + b'\n/scripts/init-bottom/zz-oblivion-lab-fixture "$@"\n')
        archive = subprocess.check_output(["bsdtar", "--format=newc", "--uid=0", "--gid=0",
            "-cf", "-", "scripts"], cwd=root)
    initrd = output / "lab-initrd"
    # Linux accepts concatenated compressed initramfs archives. Only the
    # synthetic host test is overlaid; guest roots and controller binaries stay fixed.
    initrd.write_bytes((image / "initrd").read_bytes() + gzip.compress(archive, mtime=0))
disk = output / "data.raw"
with disk.open("xb") as stream:
    stream.truncate(1024 * 1024 * 1024)
started = time.monotonic()
with (output / "serial.log").open("xb") as log:
    result = subprocess.run([
        "qemu-system-aarch64", "-machine", "virt", "-accel", "hvf", "-cpu", "host",
        "-m", "4096", "-smp", "4", "-nodefaults", "-no-reboot", "-display", "none",
        "-monitor", "none", "-serial", "stdio", "-nic", "none",
        "-kernel", str(image / "kernel"), "-initrd", str(initrd),
        "-append", "console=ttyAMA0 root=/dev/vda rootfstype=squashfs ro panic=-1 systemd.getty_auto=no systemd.show_status=false",
        "-drive", f"file={image / 'host.squashfs'},format=raw,if=none,id=root,readonly=on",
        "-device", "virtio-blk-pci,drive=root",
        "-drive", f"file={disk},format=raw,if=none,id=data,cache=none",
        "-device", "virtio-blk-pci,drive=data",
    ], stdin=subprocess.DEVNULL, stdout=log, stderr=subprocess.STDOUT, timeout=900)
serial = (output / "serial.log").read_text(errors="replace")
fixture_verified = fixture is None or ("OBLIVION_TUI_FIXTURE_SHA256 " + hashlib.sha256(fixture).hexdigest()) in serial.splitlines()
markers = [b"OBLIVION_VM_A_CANARY_4d02", b"OBLIVION_VM_B_CANARY_b0ca", b"OBLIVION_VM_TEMP_CANARY_d9ae",
           b"synthetic-general-password", b"synthetic-session-password-a", b"synthetic-session-password-b"]
if (image / "opencode.sha256").exists():
    markers.append(b"OBLIVION_GUEST_CHAT_CANARY_7eaf")
if (image / "tui-lab").exists():
    markers.append(b"OBLIVION_TUI_CANARY_8ac4")
    for line in serial.splitlines():
        if not line.startswith("OBLIVION_SCREENSHOT "):
            continue
        _, name, data = line.split()
        assert name in {"home", "conversation-a", "trusted-confirmation", "resumed-a"}
        (output / f"{name}.png").write_bytes(base64.b64decode(data, validate=True))
found = set()
digest = hashlib.sha256()
with disk.open("rb") as stream:
    overlap = b""
    while chunk := stream.read(4 * 1024 * 1024):
        digest.update(chunk)
        found.update(marker.decode() for marker in markers if marker in overlap + chunk)
        overlap = chunk[-128:]
summary = {
    "scope": "real LUKS/systemd/QEMU lifecycle with synthetic data; nested TCG is NOT an isolation guarantee",
    "workload": ("trusted manager TUI + " if (image / "manager.sha256").exists() else "") + ("compiled interactive OpenCode TUI" if (image / "tui-lab").exists() else "compiled OpenCode CLI" if (image / "opencode.sha256").exists() else "synthetic shell"),
    "outer_accelerator": "hvf", "inner_accelerator": "tcg", "production_kvm_tested": False,
    "seconds": round(time.monotonic() - started, 2), "qemu_exit": result.returncode,
    "checks": [line.strip() for line in serial.splitlines() if line.startswith("PASS:")],
    "guest_pass": "OBLIVION_VM_LAB_PASS" in serial, "plaintext_markers_on_disk": sorted(found),
    "disk_sha256": digest.hexdigest(), "image_manifest": (image / "SHA256SUMS").read_text(),
    "compiled_sources": (image / "sources.sha256").read_text(),
    "opencode_sha256": (image / "opencode.sha256").read_text().split()[0] if (image / "opencode.sha256").exists() else None,
    "lab_initrd_sha256": hashlib.sha256(initrd.read_bytes()).hexdigest(),
    "runtime_fixture_override": {"file": "vm-tui-test.py", "sha256": hashlib.sha256(fixture).hexdigest()} if fixture is not None else None,
    "fixture_verified": fixture_verified,
}
(output / "summary.json").write_text(json.dumps(summary, indent=2) + "\n")
print(json.dumps(summary, indent=2))
if result.returncode or not summary["guest_pass"] or found or not fixture_verified:
    raise SystemExit(1)
