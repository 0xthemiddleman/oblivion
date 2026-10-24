#!/bin/sh
set -eu
test "$(id -u)" = 1000
# A revocable local capability is intentionally available to this session's
# tools. No infrastructure credential or arbitrary endpoint is supplied here.
ABLIT_KEY=$(cat /run/oblivion-api/token)
export ABLIT_KEY
export OPENCODE_CONFIG_CONTENT='{"model":"abliteration-ai/abliterated-model","snapshots":false,"providers":{"abliteration-ai":{"settings":{"baseURL":"http://10.88.0.1:8111/v1"}}}}'
exec /usr/bin/oblivion "$@" --standalone
