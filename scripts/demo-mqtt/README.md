# `scripts/demo-mqtt/` — MQTT Mock State Demo Utilities

This directory provides scripts to simulate AI coding agent lifecycle states for demoing `kitt-eye` hardware (ESP32-C3 / ESP32-S2 NeoPixel LED bar) and Home Assistant integrations via MQTT **without requiring real AI agent hook events** (Claude, Codex, Cursor, Antigravity).

---

## Features

- **Shares `config.yaml`**: Automatically reads `mqtt.broker`, `mqtt.port`, credentials, and `mqtt.topic_prefix` directly from `config.yaml` (or `$KITTEYE_CONFIG`).
- **No Hook / AI CLI Required**: Directly publishes state payloads to the MQTT broker (retained, QoS 1).
- **Multiple Demo Modes**:
  - **`loop`** (default): Cycles through all 7 states continuously with configurable transition intervals.
  - **`workflow`**: Simulates a realistic developer + agent pair programming workflow (Standby → Analyze → Generate → Test → Wait for input → Commit → Done).
  - **`interactive`**: Live interactive menu where pressing number keys [1-7] immediately triggers that state on the LED bar.
  - **`single`**: One-shot trigger of any specific state (e.g. `thinking` or `executing_tool`).
- **Graceful Reset**: Automatically restores the `idle` state when stopping (Ctrl+C).
- **Dry-run Mode**: Test state animations in terminal without an active MQTT broker using `--dry-run`.

---

## State & Motion Mapping

| State | Classic Theme | Colorful Theme | Terminal ANSI | Description |
|---|---|---|---|---|
| `idle` | Classic Red | Classic Red | Red | Standby / Slow K.I.T.T. scanner |
| `thinking` | Classic Red | Mystic Purple | Magenta | Center-Out breathing motion |
| `generating` | Classic Red | Mint Emerald | Green | Fill-Sweep streaming motion |
| `executing_tool` | Classic Red | Vivid Amber | Yellow | **Fast K.I.T.T. scanner** |
| `waiting_input` | Classic Red | Warning Yellow | Bright Yellow | Blink prompt |
| `done` | Classic Red | Spring Lime | Bright Green | Flash → Solid green |
| `error` | Classic Red | Crimson Red | Bright Red | Fast Comet warning |

---

## Usage Examples

```bash
# 1. Continuous state cycling (default, 3s interval)
./scripts/demo-mqtt/mock_state.sh
# or using shortcut:
./scripts/demo-mqtt/demo.sh

# 2. Change interval duration (e.g. 5s)
./scripts/demo-mqtt/mock_state.sh -i 5s

# 3. Realistic agent coding workflow simulation
./scripts/demo-mqtt/mock_state.sh -m workflow

# 4. Interactive mode (switch states on the fly via keyboard)
./scripts/demo-mqtt/mock_state.sh -m interactive

# 5. One-shot state trigger (convenient for quick testing)
./scripts/demo-mqtt/mock_state.sh thinking
./scripts/demo-mqtt/mock_state.sh executing_tool
./scripts/demo-mqtt/mock_state.sh done
./scripts/demo-mqtt/mock_state.sh error
./scripts/demo-mqtt/mock_state.sh idle

# 6. Dry-run mode (output to terminal without MQTT connection)
./scripts/demo-mqtt/mock_state.sh --dry-run
```

---

## Command Options

| Flag | Short | Default | Description |
|---|---|---|---|
| `--mode` | `-m` | `loop` | Mode: `loop`, `workflow`, `interactive`, `single` |
| `--state` | `-s` | `""` | State name for single mode (`idle`, `thinking`, etc.) |
| `--interval`| `-i` | `3s` | Interval duration between state transitions |
| `--config` | `-c` | `config.yaml` | Path to custom YAML configuration file |
| `--agent` | `-a` | `mock-agent` | Agent identifier embedded in MQTT topic/payload |
| `--detail` | `-d` | `""` | Custom detail message |
| `--once` | | `false` | Run loop or workflow scenario once and exit |
| `--dry-run` | | `false` | Print to stdout without connecting to MQTT broker |
| `--ipc` | | `""` | Path to UDS socket (e.g. `/tmp/kitt-eye.sock`) to also feed `pub-go` |
| `--help` | `-h` | | Show usage help |
