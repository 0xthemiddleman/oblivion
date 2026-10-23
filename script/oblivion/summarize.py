#!/usr/bin/env python3
"""Publish compact synthetic evidence; keep full traces in ignored artifacts."""

import hashlib
import json
import re
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
RAW = ROOT / "artifacts/oblivion-phase0"
OUT = ROOT / "docs/oblivion/phase-0"


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


scenarios = []
for report in json.loads((RAW / "runtime-results.json").read_text()):
    directory = RAW / report["artifact_directory"]
    trace = directory / "syscalls.log"
    endpoints = set()
    for line in trace.read_text().splitlines():
        match = re.search(
            r'(bind|connect)\(.*sa_family=AF_INET, sin_port=htons\((\d+)\), sin_addr=inet_addr\("([^"]+)"\)',
            line,
        )
        if match:
            operation, port, address = match.groups()
            # EINPROGRESS is not evidence of a successful connection. Preserve
            # the syscall result, including unfinished calls, without guessing.
            result = line.split(" = ", 1)[-1] if " = " in line else "unfinished; consult full trace"
            endpoints.add((operation, address, int(port), result))
    item = {key: value for key, value in report.items() if key != "files"}
    item["created_file_count"] = len(report["files"])
    item["canary_file_metadata"] = [file for file in report["files"] if file["path"] in report["canary_files"]]
    item["raw_sha256"] = {name: digest(directory / name) for name in ("syscalls.log", "stdout.txt", "stderr.txt")}
    item["ipv4_bind_connect_attempts"] = [
        {"operation": op, "address": ip, "port": port, "syscall_result": result}
        for op, ip, port, result in sorted(endpoints)
    ]
    scenarios.append(item)

summary = {
    "upstream_commit": "68bc26b7da1990ce10ffecfbc1e09eb13aa5d824",
    "environment": "Linux arm64, Bun 1.4.4, Docker Desktop on macOS; network none; synthetic fixtures only",
    "scope": "CLI standalone conversation and shell; production TUI editor helper and browser file helper",
    "not_exercised": ["interactive TUI end-to-end", "live Abliteration", "Tor", "LUKS/VM isolation", "native PTY daemon lifecycle"],
    "network_note": "This compact parser reports IPv4 bind/connect only. Raw strace includes other network syscalls. No external network was available.",
    "scenarios": scenarios,
    "temporary_helpers": json.loads((RAW / "temp-results.json").read_text()),
    "temporary_helpers_trace_sha256": digest(RAW / "temp-syscalls.log"),
}
(OUT / "runtime-summary.json").write_text(json.dumps(summary, indent=2) + "\n")

review = []
for package, directory, files in [
    ("@opentui/core", "packages/tui/node_modules/@opentui/core", [
        "chunk-bun-4e5bpeqh.js", "chunk-bun-ehr9v0n8.js", "parser.worker.js",
    ]),
    ("@opentunnel/client", "packages/cli/node_modules/@opentunnel/client", ["dist/effect/storage.js", "dist/effect/client.js"]),
    ("@npmcli/config", "packages/util/node_modules/@npmcli/config", ["lib/definitions/definitions.js"]),
]:
    root = ROOT / directory
    data = json.loads((root / "package.json").read_text())
    review.append({
        "package": package,
        "version": data["version"],
        "scope": "Targeted review of filesystem/network helpers only; not a complete dependency audit",
        "files": [{"path": str((root / file).relative_to(ROOT)), "sha256": digest(root / file)} for file in files],
    })
(OUT / "dependency-review.json").write_text(json.dumps(review, indent=2) + "\n")
print("Wrote runtime-summary.json and dependency-review.json")
