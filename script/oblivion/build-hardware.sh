#!/bin/sh
set -eu
cd "$(dirname "$0")/../.."
root=$(pwd)
out="$root/artifacts/oblivion-hardware-iso"
key=${OBLIVION_TEST_KEY:-"$HOME/.ssh/oblivion-hardware-test"}
mkdir -p "$out"
# The public key is the only credential copied into the build context. This
# dedicated private key stays on the operator's Mac, outside the repository.
if [ ! -f "$key" ]; then
    mkdir -p "$(dirname "$key")"
    (umask 077; ssh-keygen -q -t ed25519 -N '' -C oblivion-hardware-test -f "$key")
fi
ssh-keygen -y -f "$key" > "$out/authorized_key.pub"
base_sha=$(python3 - <<'PY'
import hashlib
import json
from pathlib import Path

evidence = json.loads(Path('docs/oblivion/live-iso-evidence.json').read_text())['iso']
with Path(evidence['path']).open('rb') as file:
    digest = hashlib.file_digest(file, 'sha256').hexdigest()
assert digest == evidence['sha256'], 'The verified base ISO is missing or changed'
print(digest)
PY
)
docker buildx build --platform linux/amd64 --file script/oblivion/Dockerfile.vm-lab \
    --target hardware-iso --build-arg "BASE_ISO_SHA256=$base_sha" \
    --output "type=local,dest=$out/image" .
printf 'ISO: %s/image/oblivion-hardware-test-amd64.iso\n' "$out"
printf 'Private SSH key (not in ISO): %s\n' "$key"
