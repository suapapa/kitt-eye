# kitt-eye 🚗💨

[English](README.md) | [한국어](README_ko.md)

> A homelab status monitoring system that captures lifecycle events from AI coding agents, aggregates their states via a Go daemon over MQTT, and visualizes them on an ESP32-C3 **K.I.T.T. scanner LED bar** (and Home Assistant).

When running multiple AI coding agents across terminals (`claude`, `codex`, `cursor-cli`, `antigravity-cli`), it is difficult to tell what they are doing without constantly checking each window: *Are they thinking? Running a build? Finished? Or waiting for your input?*

**kitt-eye** bridges this gap by turning an ESP32-C3 and an 8-LED NeoPixel strip into an iconic K.I.T.T. scanner on your desk, giving you peripheral awareness of your AI agents from across the room.

---

## 🌟 Key Features

- **Multi-Agent CLI Support**: Native hook integration for **Claude Code**, **OpenAI Codex CLI**, **Cursor CLI**, and **Antigravity CLI** (`agy`).
- **2 LED Motion Themes**:
  - **`classic`**: Pure Knight Rider monochrome red (🔴) scanner. States are conveyed purely through distinct motion dynamics.
  - **`colorful`**: High-contrast, vivid color palette (Amber K.I.T.T. scanner, Cyan breathing, Yellow blink, etc.) for instant glanceability.
- **Zero CLI Overhead**:
  - Hooks send fire-and-forget single-line JSON datagrams to a local Unix Domain Socket (`/tmp/kitt-eye.sock`).
  - No stdout pollution, fail-safe exits, and sub-millisecond execution.
- **Intelligent State Aggregation**:
  - Multi-session, multi-agent priority resolution: `error` > `waiting_input` > `executing_tool` > `generating` > `thinking` > `done` > `idle`.
  - Configurable state TTLs (`done` and `error` auto-revert to `idle`, crashed sessions are cleaned up).
- **Home Assistant & MQTT First-Class Support**:
  - Retained QoS 1 messages (ESP32 immediately picks up the current state upon boot).
  - Last Will and Testament (LWT) for daemon online/offline presence.
  - Optional Home Assistant MQTT Discovery for instant entity setup.
- **Ultra-Lightweight Embedded Firmware**:
  - Bare-metal Rust (`no_std`, `esp-hal` + `embassy`) for ESP32-C3 controlling WS2812B NeoPixel LEDs.

---

## 🚦 Agent States & LED Motion Patterns

When multiple agents or sessions run simultaneously, the highest priority state is promoted to `active_state`:

```text
error > waiting_input > executing_tool > generating > thinking > done > idle
```

| State | Agent Activity | `classic` (All Red 🔴) | `colorful` (Expressive Colors & Motion) |
|---|---|---|---|
| `idle` | Standing by | Slow K.I.T.T. Scanner | 🔴 Classic Red Slow K.I.T.T. Scanner |
| `thinking` | Prompt analysis / reasoning | Center-Out | 🟣 Mystic Purple Center-Out |
| `generating` | Generating text or code | Fill Sweep | 🟢 Mint Emerald Fill Sweep |
| `executing_tool` | Executing bash, file, or MCP tool | **K.I.T.T. Scanner (Bouncing)** | 🟠 Vivid Amber **K.I.T.T. Scanner** |
| `waiting_input` | **Awaiting user approval / input** | Synchronous Blink | 🟡 Warning Yellow Blink |
| `done` | Task completed | Flash → Solid | 🟢 Spring Lime Flash → Solid |
| `error` | Tool failed or command error | One-way Comet | 🔴 Crimson Red Comet |

> `done` and `error` states automatically decay back to `idle` after their configured TTLs (defaults: 15s and 60s).

---

## 🏗️ Architecture

```text
[ AI CLI Tools ]
 ├── claude          (~/.claude/settings.json hooks)
 ├── codex           (~/.codex/hooks.json hooks)
 ├── cursor-cli      (~/.cursor/hooks.json hooks)
 └── antigravity-cli (~/.gemini/config/hooks.json hooks)
         │
         │  Unix Domain Socket (/tmp/kitt-eye.sock)
         │  Single-line JSON datagram (Fire-and-forget, zero stdout)
         ▼
[ kitt-eye Publisher Daemon (`pub-go`) ]
   · Multi-session tracking + TTL cleanup + priority aggregation
   · MQTT QoS 1 Retained publish + LWT
         │
         │  MQTT topics (`kitt-eye/active_state`, etc.)
         ▼
[ HomeLab MQTT Broker (Home Assistant Mosquitto) ]
         │
         ├──────────────────────────────────────┐
         ▼                                      ▼
[ ESP32-C3 Firmware (`mcu-rust`) ]     [ Home Assistant ]
 └── WS2812B NeoPixel 8-LED Bar             └── Sensor Dashboard (Auto-Discovery)
```

---

## 🔌 Hardware Setup & Wiring

### Bill of Materials
- **ESP32-C3 Development Board** (SuperMini, NodeMCU, or any RISC-V C3 board)
- **WS2812B NeoPixel 8-LED Bar** (strip or stick, default count: 8 LEDs)
- Jumper wires and a USB-C cable

### Wiring Diagram

| ESP32-C3 Pin | WS2812B LED Bar Pin | Notes |
|---|---|---|
| **5V** (or 3V3) | **VCC / 5V** | 5V recommended for full brightness |
| **GND** | **GND** | Common ground |
| **GPIO6** | **DIN (Data In)** | SPI2 MOSI hardware line for high-speed WS2812 timing |

*(Note: GPIO4 is reserved internally for SPI master clock initialization, but no physical wiring to the LED bar is needed.)*

---

## 🚀 Quick Start

### 1. Prerequisites

- **Go 1.25+** (for building the publisher daemon)
- **`jq`** (**Required** for CLI hook JSON filtering and automated installation)
  - macOS: `brew install jq`
  - Debian/Ubuntu: `sudo apt install jq`
- **`nc` (netcat)** with Unix domain socket support (`nc -U`, built-in on macOS/Linux)
- **Rust toolchain** (for ESP32-C3 firmware)
  - `rustup target add riscv32imc-unknown-none-elf`
  - `cargo install espflash`

---

### 2. Test Locally Without Hardware (Dry-Run Mode)

You can test the entire pipeline right in your terminal without an MQTT broker or hardware:

```bash
# 1. Create local config
cp config.example.yaml config.yaml
# When mqtt.broker is "" (empty), the daemon runs in dry-run mode and prints NDJSON to stdout.

# 2. Run publisher daemon
make run-pub
# Output: {"type":"status","status":"online"}
```

In a second terminal, inject simulated agent events:
```bash
./hooks/common/send_event.sh claude executing_tool "Bash: go test ./..."
./hooks/common/send_event.sh claude done "All tests passed"
```
Watch the publisher daemon instantly compute and output the aggregated status!

---

### 3. Connect to MQTT Broker (Home Assistant)

Edit `config.yaml` with your broker details:

```yaml
mqtt:
  broker: "homeassistant.local"   # IP or hostname (empty string enables dry-run)
  port: 1883
  username: "mqtt_user"
  topic_prefix: "kitt-eye"

homeassistant:
  discovery: true                 # Automatically registers sensors in Home Assistant
```

Set the password securely via environment variable:
```bash
export KITTEYE_MQTT_PASSWORD='your_password'
make run-pub
```

Verify published topics using the built-in subscriber utility:
```bash
make run-sub
# Or using mosquitto_sub:
# mosquitto_sub -h homeassistant.local -v -t 'kitt-eye/#'
```

---

### 4. Install CLI Lifecycle Hooks

Install hooks into all supported AI CLI configurations with one idempotent command:

```bash
make install-hooks
```
*(Or individually: `make install-hooks-claude`, `make install-hooks-codex`, `make install-hooks-cursor-cli`, `make install-hooks-antigravity-cli`)*

#### Configuration Locations & Post-Install Verification
| CLI Agent | Config File | Post-Install Verification |
|---|---|---|
| **Claude Code** | `~/.claude/settings.json` | Run `/hooks` in session to inspect active hooks |
| **OpenAI Codex** | `~/.codex/hooks.json` | Run `/hooks` in session to **trust/approve** kitt-eye |
| **Cursor CLI** | `~/.cursor/hooks.json` | Reloaded automatically by Cursor file watcher |
| **Antigravity CLI** | `~/.gemini/config/hooks.json` | Verify workspace trust status |

> To remove hooks: `make uninstall-hooks` (safely removes kitt-eye entries while preserving other custom user hooks).

---

### 5. Flash ESP32-C3 Firmware

```bash
cd mcu-rust
cp .env.example .env

# Edit .env with your Wi-Fi and MQTT credentials:
# SSID="MyHomeWiFi"
# PASS="SecretWiFiPassword"
# MQTT_BROKER="homeassistant.local"
# MOTION_THEME="colorful"   # 'classic' or 'colorful'

# Build and flash to connected ESP32-C3 board
make flash-mcu
```

Now start coding with your favorite AI agent and watch your desk LED bar pulse and scan in real time!

---

## 📡 MQTT Topic Reference

All state topics are published with **QoS 1 and Retain flag** so late-joining devices (like the ESP32) receive the current state immediately.

| Topic (Default Prefix: `kitt-eye`) | Sample Payload | Purpose |
|---|---|---|
| `{prefix}/active_state` | `executing_tool` | **Highest-priority active state (subscribed by MCU)** |
| `{prefix}/active_agent` | `claude` | Name of the representative active agent |
| `{prefix}/agents/{agent}/state` | `thinking` | Representative state for a specific agent |
| `{prefix}/agents/{agent}/sessions` | `{"agent":"claude",...}` | Multi-session array and detail snapshot (JSON) |
| `{prefix}/system/status` | `online` / `offline` | Daemon availability & LWT message |
| `homeassistant/sensor/kitt_eye_*/config` | `{...}` | Home Assistant MQTT Discovery configuration |

---

## 🛠️ Makefile Commands

```bash
make help                       # Display all available targets
make build-pub                  # Compile Go publisher -> pub-go/bin/kitt-eye-pub
make run-pub                    # Run Go publisher daemon with config.yaml
make build-sub                  # Compile MQTT subscriber example -> pub-go/bin/kitt-eye-sub
make run-sub                    # Run subscriber example against config.yaml broker
make build-mcu                  # Build ESP32-C3 Rust firmware
make flash-mcu                  # Flash firmware via cargo espflash and monitor logs
make install-hooks              # Install hooks for all 4 AI CLIs
make uninstall-hooks            # Uninstall hooks and clean up binaries
make test                       # Run full test suite (Go unit tests + Rust check)
make clean                      # Clean build artifacts
```

---

## 📖 Further Documentation

- [Korean Documentation (한국어)](README_ko.md)
- [PLAN.md](PLAN.md): Complete architecture specification & hook protocol details
- [AGENTS.md](AGENTS.md): Developer & AI agent technical constraints and guidelines

---

## 📄 License

[MIT License](./LICENSE) © 2026 Homin Lee <i@homin.dev>
