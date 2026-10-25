#!/bin/sh
set -eu
cd "$(dirname "$0")/../.."
root=$(pwd)
bun=${OBLIVION_BUN:-bun}
test "$("$bun" --version)" = 1.4.4 || { printf '%s\n' 'Bun 1.4.4 is required.' >&2; exit 1; }
out="$root/artifacts/oblivion-beta-iso"
mkdir -p "$out"
(
    cd packages/cli
    OPENCODE_CHANNEL=oblivion-beta TURBO_TELEMETRY_DISABLED=1 "$bun" run script/build.ts \
        --target=opencode-linux-x64-baseline --skip-install --skip-web-ui --oblivion --outdir="$out/client"
)
python3 - <<'PY'
import hashlib
import json
from pathlib import Path
import subprocess

root = Path('artifacts/oblivion-beta-iso')
with (root / 'client/cli-linux-x64-baseline/bin/opencode').open('rb') as binary:
    digest = hashlib.file_digest(binary, 'sha256').hexdigest()
files = subprocess.check_output(['git', 'ls-files', '-z', 'packages', 'script', 'patches', 'bun.lock', 'package.json']).decode().split('\0')
manifest = ''.join(hashlib.sha256(Path(name).read_bytes()).hexdigest() + '  ' + name + '\n'
    for name in sorted(files) if name and Path(name).is_file())
(root / 'client-sources.sha256').write_text(manifest)
(root / 'client-evidence.json').write_text(json.dumps({
    'source_commit': subprocess.check_output(['git', 'rev-parse', 'HEAD']).decode().strip(),
    'source_manifest_sha256': hashlib.sha256(manifest.encode()).hexdigest(),
    'source_manifest': 'client-sources.sha256',
    'binary_sha256': digest, 'architecture': 'x86_64', 'profile': 'oblivion',
    'scope': 'Development ISO build; not a signed release.'
}, indent=2) + '\n')
PY
docker buildx build --platform linux/amd64 --file script/oblivion/Dockerfile.vm-lab \
    --target beta-iso --output "type=local,dest=$out/image" .
cp "$out/client-sources.sha256" "$out/image/client-sources.sha256"
(cd "$out/image" && shasum -a 256 client-sources.sha256 >> SHA256SUMS)
printf 'ISO: %s/image/oblivion-development-amd64.iso\n' "$out"
