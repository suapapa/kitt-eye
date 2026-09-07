#!/usr/bin/env bash
# send_event.sh: Lightweight event dispatcher to kitt-eye IPC socket
# Usage: ./send_event.sh <agent_name> <state> [detail]
# Example: ./send_event.sh cursor-cli executing_tool "Running tests"

set -euo pipefail

AGENT="${1:-unknown}"
STATE="${2:-idle}"
DETAIL="${3:-}"
SOCKET_PATH="${KITTEYE_SOCKET:-/tmp/kitt-eye.sock}"
TIMESTAMP="$(date -u +"%Y-%m-%dT%H:%M:%SZ")"

PAYLOAD=$(cat <<EOF
{"agent":"$AGENT","state":"$STATE","detail":"$DETAIL","timestamp":"$TIMESTAMP"}
EOF
)

if [ -S "$SOCKET_PATH" ]; then
    echo "$PAYLOAD" | nc -U -w 1 "$SOCKET_PATH" 2>/dev/null || true
fi
