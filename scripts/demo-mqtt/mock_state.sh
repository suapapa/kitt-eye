#!/usr/bin/env bash
# ==============================================================================
# kitt-eye Mock State Generator (Demo Script)
#
# Generates and publishes simulated AI agent lifecycle states to MQTT without
# requiring actual AI coding CLI agents (Claude, Codex, Cursor, Antigravity)
# or active hooks.
#
# Reads MQTT broker, port, credentials, and topic prefix directly from
# config.yaml (or $KITTEYE_CONFIG).
# ==============================================================================
set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(git -C "$SCRIPT_DIR" rev-parse --show-toplevel 2>/dev/null || (cd "$SCRIPT_DIR/../.." && pwd))"

# Configuration file resolution
CONFIG_FILE="${KITTEYE_CONFIG:-$REPO_ROOT/config.yaml}"
if [ ! -f "$CONFIG_FILE" ] && [ -f "$PWD/config.yaml" ]; then
    CONFIG_FILE="$PWD/config.yaml"
fi

show_help() {
    cat <<EOF
kitt-eye Mock State Generator (Demo Tool)

Usage:
  $(basename "$0") [options] [state]

States:
  idle           Classic Red (Slow K.I.T.T. Scanner)
  thinking       Mystic Purple (Center-Out)
  generating     Mint Emerald (Fill-Sweep)
  executing_tool Vivid Amber (Fast K.I.T.T. Scanner)
  waiting_input  Warning Yellow (Blink)
  done           Spring Lime (Flash -> Solid)
  error          Crimson Red (Comet)

Modes:
  loop           Cycle through all 7 states continuously (default)
  workflow       Simulate a realistic AI coding workflow (think -> code -> tool -> done)
  interactive    Interactive CLI to switch states on the fly using number keys
  single         Publish a single state and exit immediately

Options:
  -s, --state <state>      Set single state (idle, thinking, generating, tool, wait, done, error)
  -m, --mode <mode>        Select mode: loop, workflow, interactive, single
  -i, --interval <sec>     Interval duration between transitions (default: 3s)
  -c, --config <path>      Path to config.yaml (default: $CONFIG_FILE)
  -b, --broker <host>      Override MQTT broker IP/host (default: from config.yaml)
  -p, --port <port>        Override MQTT broker port (default: from config.yaml)
  --retries <count>        Max connection attempts before giving up (default: 5)
  -a, --agent <name>       Agent name to report (default: mock-agent)
  -d, --detail <text>      Custom detail message
  --once                   Run loop or workflow scenario once and exit
  --dry-run                Print states to stdout without connecting to MQTT broker
  --ipc <path>             Also send JSON datagrams to UDS socket (e.g. /tmp/kitt-eye.sock)
  -h, --help               Show this help message

Examples:
  $(basename "$0")                          # Continuous loop across all states
  $(basename "$0") thinking                 # Publish 'thinking' state once and exit
  $(basename "$0") executing_tool           # Publish 'executing_tool' (Amber scanner)
  $(basename "$0") -m workflow              # Realistic agent pair-programming demo
  $(basename "$0") -m interactive           # Interactive keyboard menu
  $(basename "$0") -i 5s                    # 5-second interval loop
  $(basename "$0") --dry-run                # Terminal-only demo without MQTT broker
EOF
}

# Check for help flag
for arg in "$@"; do
    if [ "$arg" = "-h" ] || [ "$arg" = "--help" ]; then
        show_help
        exit 0
    fi
done

# If Go is installed, use the compiled Go binary or go run
BIN_PATH="$REPO_ROOT/pub-go/bin/kitt-eye-mock"
CMD_DIR="$REPO_ROOT/pub-go/cmd/kitteye_mock"

if [ -x "$BIN_PATH" ]; then
    exec "$BIN_PATH" -config "$CONFIG_FILE" "$@"
elif command -v go >/dev/null 2>&1; then
    # Auto-compile for faster subsequent runs if writable
    if [ -w "$REPO_ROOT/pub-go" ]; then
        mkdir -p "$REPO_ROOT/pub-go/bin"
        (cd "$REPO_ROOT/pub-go" && go build -o bin/kitt-eye-mock ./cmd/kitteye_mock >/dev/null 2>&1) || true
        if [ -x "$BIN_PATH" ]; then
            exec "$BIN_PATH" -config "$CONFIG_FILE" "$@"
        fi
    fi
    exec go run "$CMD_DIR" -config "$CONFIG_FILE" "$@"
fi

# ------------------------------------------------------------------------------
# Fallback: Lightweight bash publisher if Go is not available
# ------------------------------------------------------------------------------
echo "[Notice] Go not detected, using shell fallback..."

# Parse MQTT settings from config.yaml using awk
parse_yaml() {
    awk -v key="$1" '
    BEGIN { in_mqtt = 0 }
    /^mqtt:/ { in_mqtt = 1; next }
    /^[a-zA-Z0-9_-]+:/ && !/^mqtt:/ { in_mqtt = 0 }
    in_mqtt && $1 ~ ("^" key ":") {
        val = $2; gsub(/^["'\''"]+|["'\''"]+$/, "", val); print val; exit
    }
    ' "$CONFIG_FILE"
}

BROKER="$(parse_yaml "broker")"
PORT="$(parse_yaml "port")"
PORT="${PORT:-1883}"
USER="$(parse_yaml "username")"
PASS="$(parse_yaml "password")"
PREFIX="$(parse_yaml "topic_prefix")"
PREFIX="${PREFIX:-kitt-eye}"

TARGET_STATE="${1:-idle}"
TOPIC="$PREFIX/active_state"

echo "MQTT Broker: $BROKER:$PORT | Topic: $TOPIC | State: $TARGET_STATE"

if command -v mosquitto_pub >/dev/null 2>&1 && [ -n "$BROKER" ]; then
    AUTH_ARGS=()
    [ -n "$USER" ] && AUTH_ARGS+=("-u" "$USER")
    [ -n "$PASS" ] && AUTH_ARGS+=("-P" "$PASS")
    mosquitto_pub -h "$BROKER" -p "$PORT" "${AUTH_ARGS[@]}" -t "$TOPIC" -m "$TARGET_STATE" -r -q 1
    echo "Published state '$TARGET_STATE' via mosquitto_pub."
else
    echo "Published state (dry-run): $TARGET_STATE"
fi
