#!/usr/bin/env bash
# ==============================================================================
# K.I.T.T.-EYE USB Serial Demo Launcher
#
# Connects directly to ESP32-S2 or ESP32-C3 USB serial port to control LED
# states, themes, and configuration without requiring an MQTT broker or AI CLI hooks.
# ==============================================================================
set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PYTHON_BIN="$(command -v python3 || command -v python || true)"

if [ -z "$PYTHON_BIN" ]; then
    echo "[Error] python3 is required to run the serial demo." >&2
    exit 1
fi

exec "$PYTHON_BIN" "$SCRIPT_DIR/demo_serial.py" "$@"
