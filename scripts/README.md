# `scripts/` — Demo & Mock State Utilities

This directory provides demo and test utilities to simulate AI coding agent lifecycle states and test `kitt-eye` hardware without requiring real AI agent hook events (Claude, Codex, Cursor, Antigravity).

Two demo environments are provided depending on your testing setup:

```text
scripts/
├── demo-mqtt/               # Demo 1: MQTT broker-based simulation (Full network stack)
│   ├── mock_state.sh        # Executable mock state publisher (reads config.yaml)
│   ├── demo.sh              # Shortcut symlink to mock_state.sh
│   └── README.md            # Detailed MQTT demo documentation
└── demo-serial/             # Demo 2: USB Serial port-based direct control (No broker/Wi-Fi needed)
    ├── demo_serial.py       # Standalone Python 3 serial CLI & interactive controller (no deps)
    ├── demo.sh              # Shell launcher for demo_serial.py
    └── README.md            # Detailed Serial demo documentation
```

---

## Comparison: Which Demo Should You Use?

| Feature | `scripts/demo-mqtt/` | `scripts/demo-serial/` |
|---|---|---|
| **Connection Method** | Wi-Fi / Ethernet via MQTT Broker | USB-C cable directly to PC/Mac |
| **Requirements** | MQTT broker running, Wi-Fi configured | USB cable only |
| **Best Used For** | End-to-end integration test (Go pub, MQTT, Home Assistant, ESP32) | Hardware bench testing, LED animation previews, offline demos |
| **Firmware Mode** | Normal AUTO mode (subscribed to MQTT topics) | Direct manual override via USB Serial CLI |
| **Interactive Menu** | Supported (keyboard keys 1–7) | Supported (keyboard keys 1–7, theme, status) |
| **Direct Console Shell**| N/A | Supported (`-m console`) |

---

## Quick Reference

### 1. Direct USB Serial Demo (`scripts/demo-serial/`)
Directly control the LED bar over USB cable without needing Wi-Fi or MQTT broker:

```bash
# Interactive keyboard menu (press 1-7 to change state immediately)
./scripts/demo-serial/demo.sh -m interactive

# Continuous state cycle
./scripts/demo-serial/demo.sh

# Simulated AI coding workflow
./scripts/demo-serial/demo.sh -m workflow

# Trigger single state
./scripts/demo-serial/demo.sh thinking
./scripts/demo-serial/demo.sh executing_tool

# Direct MCU CLI terminal shell
./scripts/demo-serial/demo.sh -m console
```

See the [Serial Demo Guide](demo-serial/README.md) for full options.

---

### 2. MQTT Broker Mock State Demo (`scripts/demo-mqtt/`)
Simulate AI agent lifecycle states published to your Home Assistant / Mosquitto MQTT broker:

```bash
# Continuous state loop (reads broker from config.yaml)
./scripts/demo-mqtt/demo.sh

# Realistic agent pair-programming workflow simulation
./scripts/demo-mqtt/demo.sh -m workflow

# Interactive keyboard menu [1-7]
./scripts/demo-mqtt/demo.sh -m interactive

# Publish single state directly
./scripts/demo-mqtt/demo.sh thinking

# Terminal-only test without MQTT broker
./scripts/demo-mqtt/demo.sh --dry-run
```

See the [MQTT Demo Guide](demo-mqtt/README.md) for full options.
