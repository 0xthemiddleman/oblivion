#!/usr/bin/env python3
"""Disposable Debian host + nested session VMs with synthetic data; never physical disks."""
import argparse
import hashlib
import json
import platform
import subprocess
import time
from pathlib import Path

parser = argparse.ArgumentParser()
parser.add_argument("--image", type=Path, required=True)
parser.add_argument("--output", type=Path, required=True)
args = parser.parse_args()
if (platform.system(), platform.machine()) != ("Darwin", "arm64"):
    raise SystemExit("This laboratory runner currently requires macOS ARM64/HVF.")
image = args.image.resolve()
output = args.output.resolve()
if any("," in str(path) for path in (image, output)):
    raise SystemExit("Paths containing QEMU option separators are unsupported.")
output.mkdir(parents=True, exist_ok=False)
disk = output / "data.raw"
with disk.open("xb") as stream:
    stream.truncate(1024 * 1024 * 1024)
started = time.monotonic()
with (output / "serial.log").open("xb") as log:
    result = subprocess.run([
        "qemu-system-aarch64", "-machine", "virt", "-accel", "hvf", "-cpu", "host",
        "-m", "4096", "-smp", "4", "-nodefaults", "-no-reboot", "-display", "none",
        "-monitor", "none", "-serial", "stdio", "-nic", "none",
        "-kernel", str(image / "kernel"), "-initrd", str(image / "initrd"),
        "-append", "console=ttyAMA0 root=/dev/vda rootfstype=squashfs ro panic=-1 systemd.getty_auto=no systemd.show_status=false",
        "-drive", f"file={image / 'host.squashfs'},format=raw,if=none,id=root,readonly=on",
        "-device", "virtio-blk-pci,drive=root",
        "-drive", f"file={disk},format=raw,if=none,id=data,cache=none",
        "-device", "virtio-blk-pci,drive=data",
    ], stdin=subprocess.DEVNULL, stdout=log, stderr=subprocess.STDOUT, timeout=900)
serial = (output / "serial.log").read_text(errors="replace")
markers = [b"OBLIVION_VM_A_CANARY_4d02", b"OBLIVION_VM_B_CANARY_b0ca", b"OBLIVION_VM_TEMP_CANARY_d9ae",
           b"synthetic-general-password", b"synthetic-session-password-a", b"synthetic-session-password-b"]
if (image / "opencode.sha256").exists():
    markers.append(b"OBLIVION_GUEST_CHAT_CANARY_7eaf")
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
    "workload": "compiled OpenCode CLI" if (image / "opencode.sha256").exists() else "synthetic shell",
    "outer_accelerator": "hvf", "inner_accelerator": "tcg", "production_kvm_tested": False,
    "seconds": round(time.monotonic() - started, 2), "qemu_exit": result.returncode,
    "checks": [line.strip() for line in serial.splitlines() if line.startswith("PASS:")],
    "guest_pass": "OBLIVION_VM_LAB_PASS" in serial, "plaintext_markers_on_disk": sorted(found),
    "disk_sha256": digest.hexdigest(), "image_manifest": (image / "SHA256SUMS").read_text(),
    "compiled_sources": (image / "sources.sha256").read_text(),
    "opencode_sha256": (image / "opencode.sha256").read_text().split()[0] if (image / "opencode.sha256").exists() else None,
}
(output / "summary.json").write_text(json.dumps(summary, indent=2) + "\n")
print(json.dumps(summary, indent=2))
if result.returncode or not summary["guest_pass"] or found:
    raise SystemExit(1)
