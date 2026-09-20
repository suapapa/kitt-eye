# kitt-eye 🚗💨

[English](README.md) | [한국어](README_ko.md)

> A homelab status monitoring system that captures lifecycle events from AI coding agents, aggregates their states via a Go daemon over MQTT, and visualizes them on an ESP32-S2 **K.I.T.T. scanner LED bar** (and Home Assistant).

Running AI coding assistants (`claude`, `codex`, `cursor-cli`, `antigravity-cli`, `hermes`) across multiple terminal tabs leaves you guessing: is the agent still thinking, executing a long bash tool, or blocked waiting for user approval?

**kitt-eye** bridges this gap by turning an ESP32-S2 and an 8-LED NeoPixel strip into an iconic K.I.T.T. scanner on your desk, giving you peripheral awareness of your AI agents from across the room.

---

## 🌟 Highlights

- **Multi-Agent CLI Support**: Native lifecycle hooks for Claude Code, OpenAI Codex CLI, Cursor CLI, Antigravity CLI (`agy`), and Hermes Agent.
- **2 Expressive Motion Themes**:
  - **`classic`**: Pure Knight Rider K.I.T.T. scanner in iconic red (🔴). States are conveyed exclusively through movement patterns.
  - **`colorful`**: Vivid state color codes (e.g. Amber scanner, Mystic Purple breathing, Mint emerald waves, Warning yellow blink) for instant peripheral recognition.
- **Zero CLI Overhead**: Hooks send fire-and-forget JSON over Unix Domain Socket (`/tmp/kitt-eye.sock`) with zero stdout pollution.
- **Smart Session Aggregation**: Strict priority hierarchy (`error` > `waiting_input` > `executing_tool` > ...) resolving multiple active agents and terminal windows into a single representative state.
- **Automatic TTL Expiration**: Transient states (`done`, `error`) revert back to `idle` cleanly; abandoned sessions auto-expire.
- **Home Assistant & MQTT Native**:
  - Retained QoS 1 messages (ESP32 immediately picks up the current state upon boot).
  - LWT (Last Will and Testament) offline detection.
  - Home Assistant MQTT Discovery for instant entity configuration.
- **Ultra-lightweight Embedded Firmware**:
  - Bare-metal Rust (`no_std`, `esp-hal` + `embassy`) for ESP32-S2 and ESP32-C3 controlling WS2812B NeoPixel LEDs.
  - Interactive on-chip USB Serial CLI (USB-OTG CDC-ACM / USB-Serial-JTAG) for runtime Wi-Fi & MQTT configuration, theme switching, and direct manual state control.

---

## 🚦 State Hierarchy & LED Motion Patterns

When multiple agents or terminal sessions run concurrently, **kitt-eye** evaluates their states against a strict priority hierarchy:
```text
error > waiting_input > executing_tool > generating > thinking > done > idle
```

| State | Lifecycle Stage | `classic` Motion (Monochrome 🔴) | `colorful` Theme (Color & Motion) |
|---|---|---|---|
| `idle` | Idle / awaiting user prompt | Slow K.I.T.T. Scanner (back-and-forth) | 🔴 Classic Red Slow Scanner |
| `thinking` | Prompt analysis / reasoning | Center-Out Expansion | 🟣 Mystic Purple Center-Out |
| `generating` | Response / code streaming | Fill Sweep (wave-like fill) | 🟢 Mint Emerald Fill Sweep |
| `executing_tool` | Running Bash / Edit / MCP tool | **Fast K.I.T.T. Scanner** | 🟠 Vivid Amber **Fast Scanner** |
| `waiting_input` | **Blocked: needs user approval** | Synchronized Blink (all LEDs flash) | 🟡 Warning Yellow Blink |
| `done` | Task finished | Flash 3x then hold Solid (auto-reverts to idle) | 🟢 Spring Lime Flash → Solid |
| `error` | Tool failure or agent error | Comet (trailing tail, auto-reverts to idle) | 🔴 Crimson Red Comet |

---

## 🏗️ Architecture

```text
[ AI CLI Tools ]
 ├── claude          (~/.claude/settings.json hook)
 ├── codex           (~/.codex/hooks.json hook)
 ├── cursor-cli      (~/.cursor/hooks.json hook)
 ├── antigravity-cli (~/.gemini/config/hooks.json hook)
 └── hermes          (~/.hermes/config.yaml hook)
         │
         │  Unix Domain Socket (/tmp/kitt-eye.sock)
         │  NDJSON datagram (Fire-and-forget, silent stdout)
         ▼
[ kitt-eye Publisher Daemon (Go) ]
   · Multi-session state store + TTL janitor + Priority aggregator
   · MQTT QoS 1 Retained publisher + LWT
         │
         │  MQTT ({prefix}/active_state, etc.)
         ▼
[ HomeLab MQTT Broker (Home Assistant Mosquitto) ]
         │
         ├──────────────────────────────────────┐
         ▼                                      ▼
[ ESP32-S2 Firmware (`mcu-rust`) ]     [ Home Assistant ]
 └── WS2812B NeoPixel 8-LED Bar             └── Sensor Dashboard (Auto-Discovery)
```

---

## 🔌 Hardware Setup & Wiring

> For detailed pinout diagrams and troubleshooting, see the [Hardware Wiring Guide](docs/hardware_wiring.md).

### Bill of Materials
- **ESP32-S2 or ESP32-C3 Development Board** (S2 Mini, ESP32-C3-DevKit, NodeMCU, etc.)
- **WS2812B NeoPixel 8-LED Bar** (strip or stick, default count: 8 LEDs)
- Jumper wires and a USB-C data cable

### Wiring Diagram

| Pin | ESP32-S2 | ESP32-C3 | WS2812B LED Bar Pin | Notes |
|---|---|---|---|---|
| **Power** | **5V** (or 3V3) | **5V** (or 3V3) | **VCC / 5V** | 5V recommended for full brightness |
| **GND** | **GND** | **GND** | **GND** | Common ground |
| **DIN** | **GPIO6** | **GPIO6** | **DIN (Data In)** | SPI2 MOSI hardware line for high-speed WS2812 timing |

*(Note: GPIO4 is reserved internally for SPI master clock initialization on both chips, but no physical wiring to the LED bar is needed.)*

---

## 🚀 Quick Start

### 1. Prerequisites

- **Go 1.25+** (for building the publisher daemon)
- **`jq`** (**Required** for CLI hook JSON filtering and automated installation)
  - macOS: `brew install jq`
  - Debian/Ubuntu: `sudo apt install jq`
- **`nc` (netcat)** with Unix domain socket support (`nc -U`, built-in on macOS/Linux)
- **Rust toolchain** (for ESP32-S2 firmware)
  - `espup install` (or `esp` toolchain)
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

### 3a. Run as a Background User Service (Linux systemd or macOS launchd)

Instead of keeping a terminal window open with `make run-pub`, you can install `kitt-eye-pub` as a persistent background user service. The Makefile automatically detects whether you are on **macOS** (`launchd`) or **Linux** (`systemd`):

```bash
# 1. Build, install, and start the user service
make install-service

# 2. Check service status
make status-service    # macOS: launchctl list com.suapapa.kitt-eye-pub
                       # Linux: systemctl --user status kitt-eye-pub.service

# 3. View live logs
make logs-service      # macOS: tail -f ~/Library/Logs/kitt-eye-pub.log
                       # Linux: journalctl --user -u kitt-eye-pub.service -f

# 4. Restart service
make restart-service
```

- Service binary: `~/.local/bin/kitt-eye-pub`
- Configuration file: `~/.config/kitt-eye/config.yaml`
- Environment overrides: `~/.config/kitt-eye/env` (e.g. `KITTEYE_MQTT_PASSWORD=...`)
- Service definitions:
  - macOS: `~/Library/LaunchAgents/com.suapapa.kitt-eye-pub.plist`
  - Linux: `~/.config/systemd/user/kitt-eye-pub.service`

> [!TIP]
> **Linux Tip**: To keep the publisher running even after logging out of SSH or on system boot without interactive login:
> ```bash
> loginctl enable-linger
> ```

To stop and remove the service:
```bash
make uninstall-service
```

---

### 3b. Hardware & State Demo (Without AI Agent Hooks)

Want to test your ESP32 LED bar animations without waiting for real AI coding agents? We provide both MQTT and direct USB Serial demos:

```bash
# 1. Direct USB Serial Demo (No broker or Wi-Fi needed!)
make demo-serial                         # Direct USB serial hardware demo
./scripts/demo-serial/demo.sh -m interactive # Interactive keyboard menu [1-7, a, t, s, q]
./scripts/demo-serial/demo.sh thinking   # Set single state directly

# 2. MQTT Broker Simulation Demo (Full network stack)
make demo-mqtt                           # Automatic cycling across all 7 states
./scripts/demo-mqtt/demo.sh -m workflow  # Realistic agent workflow demo
./scripts/demo-mqtt/demo.sh -m interactive # Interactive keyboard menu [1-7]
./scripts/demo-mqtt/demo.sh thinking     # Publish single state directly
```

See the [`scripts/README.md`](scripts/README.md), [`scripts/demo-serial/README.md`](scripts/demo-serial/README.md), and [`scripts/demo-mqtt/README.md`](scripts/demo-mqtt/README.md) for full options.

---

### 4. Install CLI Lifecycle Hooks

Install hooks into all supported AI CLI configurations with one idempotent command:

```bash
make install-hooks
```
*(Or individually: `make install-hooks-claude`, `make install-hooks-codex`, `make install-hooks-cursor-cli`, `make install-hooks-antigravity-cli`, `make install-hooks-hermes`)*

#### Configuration Locations & Post-Install Verification
| CLI Agent | Config File | Post-Install Verification |
|---|---|---|
| **Claude Code** | `~/.claude/settings.json` | Run `/hooks` in session to inspect active hooks |
| **OpenAI Codex** | `~/.codex/hooks.json` | Run `/hooks` in session to **trust/approve** kitt-eye |
| **Cursor CLI** | `~/.cursor/hooks.json` | Reloaded automatically by Cursor file watcher |
| **Antigravity CLI** | `~/.gemini/config/hooks.json` | Verify workspace trust status |
| **Hermes Agent** | `~/.hermes/config.yaml` | Run `hermes hooks list` to inspect active hooks |

> To remove hooks: `make uninstall-hooks` (safely removes kitt-eye entries while preserving other custom user hooks).

---

### 5. Flash MCU Firmware (ESP32-C3 / ESP32-S2)

Navigate to the appropriate directory for your hardware and set up credentials:

```bash
# For ESP32-C3:
cd mcu-rust-esp32c3 && cp .env.example .env

# For ESP32-S2:
cd mcu-rust-esp32s2 && cp .env.example .env

# Edit .env with your Wi-Fi and MQTT credentials:
# SSID="MyHomeWiFi"
# PASS="SecretWiFiPassword"
# MQTT_BROKER="homeassistant.local"
# MOTION_THEME="colorful"   # 'classic' or 'colorful'
# LED_REVERSE="false"       # 'true' to invert left-right direction

# Flash to connected board:
make flash-mcu-c3   # Flash connected ESP32-C3 board
# or
make flash-mcu-s2   # Flash connected ESP32-S2 board (with --no-stub)
```

Now start coding with your favorite AI agent and watch your desk LED bar pulse and scan in real time!

---

### 6. USB Serial Configuration & Manual LED Control

Both **ESP32-S2** (via on-chip USB-OTG CDC-ACM) and **ESP32-C3** (via on-chip USB-Serial-JTAG) provide an interactive serial CLI over their USB-C port. You can configure Wi-Fi and MQTT credentials at runtime without editing `.env` or reflashing, switch motion themes in real time, and manually force LED states for testing.

#### Connecting to the CLI

Connect the board to your computer using a USB-C data cable and open a serial terminal at **115200 baud**:

```bash
# macOS
picocom -b 115200 /dev/tty.usbmodem*
# or using screen:
screen /dev/tty.usbmodem* 115200

# Linux
picocom -b 115200 /dev/ttyACM0

# Or via cargo espflash monitor (either board)
cargo espflash monitor
```

Press `Enter` to display the interactive prompt:
```text
kitt-eye> 
```

#### Available CLI Commands

| Command | Description | Example |
|---|---|---|
| `help` or `?` | Display available commands and usage | `help` |
| `status` | Display Wi-Fi, IP, MQTT connection status, current state & mode, and active theme | `status` |
| `config` | Show current runtime configuration (SSID, Broker, Topic, Client ID, Theme) | `config` |
| `wifi <SSID> <PASS>` | Change Wi-Fi credentials (staged in RAM) | `wifi MyHomeWiFi SecretPass` |
| `mqtt_broker <URL\|HOST> [PORT]` | Set MQTT broker (supports `mqtt://user:pass@host:port` or `host [port]`) | `mqtt_broker mqtt://mqtt:896351@172.16.0.100`<br>`mqtt_broker 192.168.1.50 1883` |
| `mqtt_topic <PREFIX>` | Set MQTT subscription topic prefix (default: `kitt-eye`) | `mqtt_topic my-kitt` |
| `mqtt-client_id <ID>` | Set MQTT client ID | `mqtt-client_id kitt-desk` |
| `theme <classic\|colorful>` | Switch motion theme **instantly** (applied on next frame without rebooting) | `theme colorful` |
| `reverse <on\|off>` | Invert left-right LED direction **instantly** (applied on next frame) | `reverse on` |
| `state <NAME>` | **Manually force LED state** (held indefinitely until `state auto`) | `state executing_tool` |
| `state auto` | Release manual override and return to tracking MQTT broker events | `state auto` |
| `save` | Persist current RAM configuration to non-volatile Flash memory | `save` |
| `load` | Reload saved configuration from Flash memory into RAM | `load` |
| `reset` | Reset configuration back to compile-time (`.env`) defaults | `reset` |
| `reboot` | Reboot MCU (re-initializes Wi-Fi and MQTT with saved settings) | `reboot` |

> [!TIP]
> **Applying Network Changes**:
> Network and broker changes are staged in RAM first. To persist and apply:
> 1. Update settings: `wifi MySSID MyPass` and/or `mqtt_broker mqtt://user:pass@192.168.1.100`
> 2. Persist to flash: `save`
> 3. Reboot: `reboot`
>
> **Testing LED Animations Directly**:
> Run `state <NAME>` (`idle`, `thinking`, `generating`, `executing_tool`, `waiting_input`, `done`, `error`) to display any animation immediately without waiting for AI agent actions. Enter `state auto` to return to live MQTT monitoring.

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
make run-pub                    # Run Go publisher daemon locally with config.yaml
make install-service            # Install & start Go publisher user service (Linux systemd or macOS launchd)
make uninstall-service          # Stop & remove Go publisher user service
make status-service             # Check user service status
make restart-service            # Restart user service
make logs-service               # Follow user service logs
make build-sub                  # Compile MQTT subscriber example -> pub-go/bin/kitt-eye-sub
make run-sub                    # Run subscriber example against config.yaml broker
make build-mcu                  # Build both ESP32-C3 and ESP32-S2 firmwares
make build-mcu-c3               # Build ESP32-C3 Rust firmware
make build-mcu-s2               # Build ESP32-S2 Rust firmware
make flash-mcu-c3               # Flash ESP32-C3 firmware and monitor logs
make flash-mcu-s2               # Flash ESP32-S2 firmware (--no-stub) and monitor logs
make install-hooks              # Install hooks for all 5 AI CLIs
make uninstall-hooks            # Uninstall hooks and clean up binaries
make test                       # Run full test suite (Go unit tests + both Rust checks)
make clean                      # Clean build artifacts
```

---

## 📖 Further Documentation

- [Korean Documentation (한국어)](README_ko.md)
- [PLAN.md](PLAN.md): Complete architecture specification & hook protocol details
- [AGENTS.md](AGENTS.md): Developer & AI agent technical constraints and guidelines
- [docs/hardware_wiring.md](docs/hardware_wiring.md): Hardware wiring diagram & troubleshooting
- [docs/esp32s2_usb_serial_plan.md](docs/esp32s2_usb_serial_plan.md): USB Serial CLI design & architecture

---

## 📄 License

[MIT License](./LICENSE) © 2026 Homin Lee <i@homin.dev>
