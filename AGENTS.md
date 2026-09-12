# AGENTS.md — AI Agent Guide for `kitt-eye`

Welcome to `kitt-eye`. This document provides essential architectural context, conventions, commands, and constraints for AI coding assistants (Claude Code, Cursor, Codex, Antigravity, etc.) working on this repository.

---

## 1. Project Overview

`kitt-eye` is a homelab monitoring system that captures lifecycle events from AI coding CLI agents (`claude`, `codex`, `cursor-cli`, `antigravity-cli`), aggregates their states via a Go daemon over Unix Domain Socket IPC, publishes the active state to an MQTT broker (Home Assistant Mosquitto), and visualizes the state on an ESP32-C3 WS2812B NeoPixel LED bar (K.I.T.T. scanner from Knight Rider).

```text
[ AI CLI Tools (claude / codex / cursor / agy) ]
        │  Lifecycle hooks (JSON over Unix Domain Socket /tmp/kitt-eye.sock)
        ▼
[ Go Publisher Daemon (`pub-go`) ]
        │  Multi-session tracking, TTL expiration, priority aggregation
        │  MQTT QoS 1 retained topics, LWT availability, Home Assistant discovery
        ▼
[ MQTT Broker (Mosquitto / Home Assistant) ]
        │  Subscribed topic: kitt-eye/active_state
        ▼
[ ESP32-C3 Firmware (`mcu-rust`) ]
        └── WS2812B LED Bar: Classic Red K.I.T.T. scanner or Colorful motion theme
```

---

## 2. Repository Layout

```text
kitt-eye/
├── Makefile                 # Unified targets for build, run, test, and hook installation
├── config.example.yaml      # Configuration template for Go publisher
├── config.yaml              # Local publisher config (git-ignored or user-configured)
├── PLAN.md                  # Comprehensive architectural specification & protocol design
├── README.md                # User-facing guide (English)
├── README_ko.md             # User-facing guide (Korean)
├── AGENTS.md                # Developer & agent guidelines (this file)
├── hooks/                   # Lifecycle hooks for AI CLI tools
│   ├── common/
│   │   ├── send_event.sh    # UDS sender (uses nc -U, encodes JSON via jq)
│   │   └── merge_hooks.sh   # Idempotent snippet injector for CLI config JSONs
│   ├── claude/              # Claude Code (~/.claude/settings.json)
│   ├── codex/               # Codex CLI (~/.codex/hooks.json)
│   ├── cursor-cli/          # Cursor CLI (~/.cursor/hooks.json)
│   └── antigravity-cli/     # Antigravity (~/.gemini/config/hooks.json)
├── pub-go/                  # Go Publisher Daemon (module: github.com/suapapa/kitt-eye/pub)
│   ├── cmd/kitteye/         # Daemon entry point
│   ├── cmd/kitteye_sub_example/ # MQTT subscriber test utility
│   └── internal/
│       ├── config/          # YAML config parser & env overrides
│       ├── ipc/             # UDS server (/tmp/kitt-eye.sock)
│       ├── model/           # Data models, states, and priority hierarchy
│       ├── state/           # Session state store, TTLs, and aggregator
│       ├── publish/         # Sink interface (MQTT & dry-run stdout sinks)
│       └── mqtt/            # Paho MQTT v3 client, LWT, and HA discovery
└── mcu-rust/                # ESP32-C3 Rust Firmware
    ├── Cargo.toml           # esp-hal, esp-rtos, embassy, smart-leds, ws2812-spi
    ├── build.rs             # Compile-time .env loader & linker configuration
    ├── .env.example         # Template for Wi-Fi, MQTT broker, and MOTION_THEME
    └── src/
        ├── bin/main.rs      # Embassy runtime entry, Wi-Fi/MQTT tasks, LED loop
        ├── config.rs        # Compile-time constants injected via build.rs
        ├── mqtt.rs          # Minimal async MQTT 3.1.1 client over TcpSocket
        ├── patterns.rs      # LedEngine and motion pattern renderers
        ├── state.rs         # AgentState enum and MQTT payload parser
        └── theme.rs         # MotionTheme (Classic vs. Colorful) & color maps
```

---

## 3. Quick Reference Commands

Always run these from the repository root:

```bash
# Build components
make build-pub          # Compile Go publisher binary (pub-go/bin/kitt-eye-pub)
make build-sub          # Compile Go MQTT subscriber utility (pub-go/bin/kitt-eye-sub)
make build-mcu          # Compile ESP32-C3 Rust firmware (release)
make flash-mcu          # Flash ESP32-C3 firmware via cargo espflash

# Run test suite
make test               # Runs:
                        # 1. cd pub-go && go test ./...
                        # 2. cd mcu-rust && cargo check --release

# Local testing without MQTT broker (Dry-run mode)
make run-pub            # Runs publisher using config.yaml (stdout JSON sink if broker is "")
# In another terminal:
./hooks/common/send_event.sh claude executing_tool "Running tests"
./hooks/common/send_event.sh claude done "All tests passed"

# CLI Hooks management (requires jq and nc)
make install-hooks      # Idempotently inject hooks for all 4 supported CLIs
make uninstall-hooks    # Cleanly remove kitt-eye hooks while preserving other user hooks
```

---

## 4. Component Rules & Technical Constraints

### 4.1. Go Publisher (`pub-go/`)

- **Language & Runtime**: Go 1.25+.
- **Coding Style**:
  - Keep control flow clear and early-return on errors.
  - Avoid global state; inject dependencies via constructors (`New*`).
  - Rely on standard library before introducing external modules.
- **State Priority Hierarchy**:
  When multiple sessions or agents are active, the representative state (`active_state`) is strictly resolved by priority:
  ```text
  error > waiting_input > executing_tool > generating > thinking > done > idle
  ```
- **TTL & Expiration**:
  - `done` and `error` states automatically revert to `idle` after their configured TTLs (defaults: 15s for done, 60s for error).
  - Stale sessions that stop emitting events are pruned by the janitor routine (default: 300s).
- **Dry-Run Mode**:
  - When `mqtt.broker` in `config.yaml` is empty (`""`), the daemon routes events through `DryRunSink`, printing NDJSON events to `stdout`. Do not break this fallback path.
- **Testing**:
  - Run tests with `cd pub-go && go test ./...`. Ensure all packages in `internal/*` pass.

---

### 4.2. ESP32-C3 Firmware (`mcu-rust/`)

- **Target Architecture**: `riscv32imc-unknown-none-elf` (`no_std`, `build-std = ["alloc", "core"]`).
- **Frameworks**: `esp-hal` (v1.1+), Embassy async tasks (`esp-rtos`, `esp-radio`, `embassy-net`), `smart-leds`, `ws2812-spi`.
- **CRITICAL Build Rule**:
  - **DO NOT run bare `cargo test`** inside `mcu-rust/` or target the host platform (`aarch64-apple-darwin` / `x86_64`). `esp-hal` build scripts deliberately panic on non-ESP host platforms.
  - **Always verify Rust code using `cargo check --release`** (or `make test`).
- **Hardware Wiring**:
  - WS2812 DIN: `GPIO6` (SPI2 MOSI)
  - SCK: `GPIO4` (Unused by WS2812, reserved for SPI master init)
  - Default strip length: `LED_COUNT = 8` (`patterns::LED_COUNT`)
- **Compile-Time Configuration**:
  - Firmware configuration is statically injected at compile time via `.env` -> `build.rs` -> `env!()` macros (in `src/config.rs`).
- **Motion Themes (`src/theme.rs`)**:
  - `classic`: All patterns use classic Knight Rider red (`RGB8(255, 0, 0)`). States are distinguished solely by motion pattern.
  - `colorful`: States are mapped to distinct, vivid colors and expressive motions:
    - `idle` → Cool Cyan (`RGB8(0, 180, 255)`) Breathing
    - `thinking` → Mystic Purple (`RGB8(170, 0, 255)`) Center-Out
    - `generating` → Mint Emerald (`RGB8(0, 240, 120)`) Fill-Sweep
    - `executing_tool` → Vivid Amber (`RGB8(255, 140, 0)`) **Classic K.I.T.T. Scanner**
    - `waiting_input` → Warning Yellow (`RGB8(255, 215, 0)`) Blink
    - `done` → Spring Lime (`RGB8(0, 255, 50)`) Flash → Solid
    - `error` → Crimson Red (`RGB8(255, 20, 20)`) Comet
- **Embedded Safety**:
  - Never use `mem::forget` on `esp-hal` peripheral or buffer types.
  - Keep stack frames bounded; large buffers must use `StaticCell` or static arrays.

---

### 4.3. CLI Lifecycle Hooks (`hooks/`)

- **Observer Contract**:
  - Hooks are **strictly read-only observers**. They must NEVER alter CLI arguments, return non-zero exit codes to the parent process, or disrupt agent execution.
- **Standard Output (stdout) Rule**:
  - **NEVER output arbitrary text to `stdout` in hooks**. Many CLIs (e.g. Claude Code, Codex) capture stdout for LLM context or require specific JSON schemas.
- **Non-blocking Execution**:
  - Hook datagrams must be fire-and-forget over `/tmp/kitt-eye.sock`. If the daemon is not running, `nc -U` should fail silently without blocking the CLI.
- **Dependencies**:
  - Requires `jq` and `nc` with Unix Domain Socket support (`nc -U`).

---

## 5. Wire Protocols & Formats

### 5.1. IPC Datagram (Hook → Daemon)

Newline-delimited JSON sent to Unix stream socket `/tmp/kitt-eye.sock`:

```json
{
  "agent": "claude",
  "state": "executing_tool",
  "detail": "Bash: npm test",
  "session_id": "abc12345",
  "event": "PreToolUse",
  "timestamp": "2026-09-12T17:00:00Z"
}
```

### 5.2. MQTT Topics & Payloads

Default topic prefix is `kitt-eye` (all published with QoS 1 and Retain flag):

| Topic | Payload | Description |
|---|---|---|
| `{prefix}/active_state` | Plain text (`idle`, `thinking`, `generating`, `executing_tool`, `waiting_input`, `done`, `error`) | **Subscribed by MCU** |
| `{prefix}/active_agent` | Plain text (`claude`, `codex`, `cursor-cli`, `antigravity-cli`) | Name of current active agent |
| `{prefix}/agents/{agent}/state` | Plain text | Representative state for specific agent |
| `{prefix}/agents/{agent}/sessions` | JSON (`AgentSnapshot`) | Active sessions array with detail & timestamps |
| `{prefix}/system/status` | Plain text (`online` / `offline`) | Daemon availability & LWT message |
| `homeassistant/sensor/kitt_eye_*/config` | JSON | MQTT discovery configuration for Home Assistant |

---

## 6. Development & Verification Checklist

When implementing changes:

1. **Go changes**:
   - Run `cd pub-go && go test ./...`
   - Test dry-run with `./pub-go/bin/kitt-eye-pub --config config.yaml`
2. **Rust changes**:
   - Run `cd mcu-rust && cargo check --release`
   - Do NOT run `cargo test` directly in `mcu-rust/`
3. **Hook changes**:
   - Verify `hooks/common/send_event.sh` suppresses stdout completely
   - Verify JSON generation is properly escaped with `jq`
4. **Integration**:
   - Run `make test` from repository root to verify all components simultaneously.
