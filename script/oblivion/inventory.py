#!/usr/bin/env python3
"""Reproducible candidate inventory, not a proof of runtime containment.

Scan tracked upstream sources only. Patterns deliberately overapproximate I/O;
the review in docs/oblivion/phase-0/AUDIT.md owns reachability and interpretation.
"""

import csv
import hashlib
import json
import re
import subprocess
from collections import Counter
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
OUT = ROOT / "docs/oblivion/phase-0"
BASE = "68bc26b7da1990ce10ffecfbc1e09eb13aa5d824"
PATTERNS = {
    "filesystem": r"\b(?:writeFile\w*|appendFile\w*|appendText\w*|createWriteStream|writeWithDirs|writeJson\w*|writeText\w*|mkdir\w*|makeDirectory\w*|mkdtemp\w*|makeTemp\w*|copyFile\w*|rename\w*|unlink\w*|truncate\w*|chmod\w*|utimes\w*|symlink\w*|openSync)\s*\(|\b(?:Bun|fs|filesystem|file|handle|writer)\.(?:write|writeAll|open|remove|rm|copy|sink)\s*\(",
    "database": r"\b(?:db|tx|database|kv|storage|sql|native)\.(?:insert|update|delete|set|remove|run|exec|execute|transaction|prepare|unsafe)\s*\(|\b(?:new Database\w*|sqliteLayer|SqliteClient\.layer)|\b(?:INSERT INTO|UPDATE |DELETE FROM|PRAGMA|CREATE TABLE|ALTER TABLE)",
    "network": r"\bfetch\s*\(|\b(?:HttpClient\w*|WebSocket|EventSource|Socket|net|https?|dns|tls|dgram|Bonjour|WebSocketServer)\b|\.(?:listen|connect|createConnection|sendBeacon)\s*\(|\bBun\.serve\s*\(",
    "process": r"\b(?:ChildProcess\w*|spawn\w*|execFile\w*|execSync|fork|Worker|importModule|resolveModule)\b|\b(?:Npm|npm)\.(?:add|install)\s*\(|\$`",
    "logging-dumps": r"\b(?:logInfo|logDebug|logWarning|logError|logFatal|fileLogger|writeHeapSnapshot|generateHeapSnapshot|startCpuProfiling|OtlpLogger|OTLPTraceExporter|Logger\.toFile)\b|console\.(?:log|error|warn)|\.cpuprofile|\.heapsnapshot",
    "environment": r"process\.env|\b(?:Env|env)\.(?:all|get)|OTEL_|XDG_|TMPDIR|os\.tmpdir|os\.homedir",
}
compiled = {name: re.compile(pattern) for name, pattern in PATTERNS.items()}
tracked = subprocess.check_output(["git", "ls-tree", "-r", "--name-only", BASE], cwd=ROOT, text=True).splitlines()
extensions = {".ts", ".tsx", ".js", ".mjs", ".cjs", ".sh", ".rs", ".c", ".h", ".cpp", ".zig", ".py"}
inputs = {name for name in tracked if Path(name).suffix in extensions or name.endswith("/package.json")}
inputs.update({"package.json", "bun.lock", "install", "packages/core/src/models-dev/snapshot.txt"})
changed = set(subprocess.check_output(["git", "diff", "--name-only", BASE], cwd=ROOT, text=True).splitlines())
if inputs & changed:
    raise SystemExit("Pinned upstream inputs changed: " + ", ".join(sorted(inputs & changed)))
packages = {}
for name in tracked:
    if re.fullmatch(r"packages/[^/]+/package.json", name):
        data = json.loads((ROOT / name).read_text())
        packages[data["name"]] = (str(Path(name).parent), data)

closure = set()
pending = ["@opencode/cli"]
while pending:
    name = pending.pop()
    if name in closure or name not in packages:
        continue
    closure.add(name)
    pending.extend(packages[name][1].get("dependencies", {}))
runtime = {packages[name][0] for name in closure}

def scope(name):
    if re.search(r"(?:^|/)(?:test|tests|fixtures?|e2e|__tests__)/|\.(?:test|spec)\.", name):
        return "tests-excluded"
    if any(name.startswith(directory + "/src/") for directory in runtime):
        return "cli-runtime"
    if name.startswith("script/") or "/script/" in name or "/bin/" in name or name == "install":
        return "build-install"
    return "other-product"

manifest, candidates, endpoints = [], [], []
for name in tracked:
    if Path(name).suffix not in extensions and name != "install":
        continue
    path = ROOT / name
    content = path.read_text(errors="replace")
    area = scope(name)
    digest = hashlib.sha256(path.read_bytes()).hexdigest()
    manifest.append({"path": name, "scope": area, "lines": len(content.splitlines()), "sha256": digest})
    if area == "tests-excluded":
        continue
    for number, line in enumerate(content.splitlines(), 1):
        for category, pattern in compiled.items():
            if pattern.search(line):
                candidates.append({"scope": area, "category": category, "path": name, "line": number, "source": line.strip()})
        for match in re.finditer(r"(?:https?|wss?)://[^\s\"'`<>\\)]+", line):
            endpoints.append({"scope": area, "path": name, "line": number, "literal": match.group(0)})

dependencies = []
for name in sorted(closure):
    for dependency, spec in sorted(packages[name][1].get("dependencies", {}).items()):
        dependencies.append({"workspace": name, "dependency": dependency, "spec": spec,
                             "kind": "workspace" if dependency in packages else "external-unreviewed"})

OUT.mkdir(parents=True, exist_ok=True)
for name, rows in [("source-manifest.csv", manifest), ("io-candidates.csv", candidates),
                   ("endpoint-literals.csv", endpoints), ("runtime-dependencies.csv", dependencies)]:
    with (OUT / name).open("w", newline="") as stream:
        writer = csv.DictWriter(stream, fieldnames=list(rows[0]), lineterminator="\n")
        writer.writeheader()
        writer.writerows(rows)

catalog_path = ROOT / "packages/core/src/models-dev/snapshot.txt"
catalog = json.loads(catalog_path.read_text())
(OUT / "abliteration-catalog.json").write_text(json.dumps(catalog["abliteration-ai"], indent=2) + "\n")
summary = {
    "upstream": "https://github.com/anomalyco/opencode",
    "tag": "v2.1.4", "commit": BASE, "license": "MIT", "packageManager": "bun@1.4.4",
    "lockfile_sha256": hashlib.sha256((ROOT / "bun.lock").read_bytes()).hexdigest(),
    "catalog_sha256": hashlib.sha256(catalog_path.read_bytes()).hexdigest(),
    "runtime_workspaces": sorted(closure),
    "files_by_scope": dict(sorted(Counter(row["scope"] for row in manifest).items())),
    "runtime_candidates_by_category": dict(sorted(Counter(row["category"] for row in candidates if row["scope"] == "cli-runtime").items())),
    "patterns": PATTERNS,
    "limitations": [
        "Static lexical candidates include false positives; line presence is not runtime reachability.",
        "Arbitrary tools, plugins, native code, dynamic endpoints and external dependencies require runtime tracing and separate review.",
        "No claim that an absent pattern proves absence of I/O. All selected source files have a manifest entry.",
        "Tracked source, package manifests, lockfile and model catalog are required to match the pinned base; untracked files are not inventoried.",
    ],
}
(OUT / "upstream.json").write_text(json.dumps(summary, indent=2) + "\n")
print(json.dumps({key: value for key, value in summary.items() if key in {"tag", "commit", "files_by_scope", "runtime_candidates_by_category", "runtime_workspaces"}}, indent=2))
