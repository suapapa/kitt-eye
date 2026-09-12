# AGENTS.md — AI Agent Guide for `kitt-eye`

Welcome to `kitt-eye`. This document provides essential architectural context, conventions, commands, and constraints for AI coding assistants (Claude Code, Cursor, Codex, Antigravity, etc.) working on this repository.

---

## 1. Project Overview

`kitt-eye` is a homelab monitoring system that captures lifecycle events from AI coding CLI agents (`claude`, `codex`, `cursor-cli`, `antigravity-cli`), aggregates their states via a Go daemon over Unix Domain Socket IPC, publishes the active state to an MQTT broker (Home Assistant Mosquitto), and visualizes the state on an ESP32-C3 or ESP32-S2 WS2812B NeoPixel LED bar (K.I.T.T. scanner from Knight Rider).

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
[ ESP32-C3 / ESP32-S2 Firmware (`mcu-rust-esp32c3` / `mcu-rust-esp32s2`) ]
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
├── docs/
│   ├── hardware_wiring.md   # ESP32-S2 / C3 & WS2812B wiring guide
│   └── voice_system_design.md # K.I.T.T. voice playback design
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
├── mcu-rust-esp32c3/        # ESP32-C3 Rust Firmware (RISC-V, nightly toolchain)
│   ├── Cargo.toml           # esp-hal (~1.1), esp-rtos, esp-radio, esp32c3 features
│   ├── .cargo/config.toml   # riscv32imc-unknown-none-elf target
│   └── src/                 # Embassy tasks, Wi-Fi, MQTT client, LED engine
└── mcu-rust-esp32s2/        # ESP32-S2 Rust Firmware (Xtensa, esp toolchain)
    ├── Cargo.toml           # esp-hal (~1.1), esp-rtos, esp-radio, esp32s2 features
    ├── .cargo/config.toml   # xtensa-esp32s2-none-elf target (--no-stub runner)
    └── src/                 # Embassy tasks, Wi-Fi, MQTT client, LED engine
```

---

## 3. Quick Reference Commands

Always run these from the repository root:

```bash
# Build components
make build-pub          # Compile Go publisher binary (pub-go/bin/kitt-eye-pub)
make build-sub          # Compile Go MQTT subscriber utility (pub-go/bin/kitt-eye-sub)
make build-mcu          # Compile both ESP32-C3 and ESP32-S2 firmwares
make build-mcu-c3       # Compile ESP32-C3 Rust firmware
make build-mcu-s2       # Compile ESP32-S2 Rust firmware

# Flash firmware (select by chip)
make flash-mcu-c3       # Flash ESP32-C3 firmware via cargo espflash
make flash-mcu-s2       # Flash ESP32-S2 firmware via cargo espflash (--no-stub)

# Run test suite
make test               # Runs:
                        # 1. cd pub-go && go test ./...
                        # 2. cd mcu-rust-esp32c3 && cargo check --release
                        # 3. cd mcu-rust-esp32s2 && cargo check --release

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

### 4.2. Embedded Firmwares (`mcu-rust-esp32c3/` & `mcu-rust-esp32s2/`)

> [!IMPORTANT]
> **MANDATORY DUAL-DIRECTORY MODIFICATION RULE**:
> Both ESP32-C3 and ESP32-S2 are actively supported. When modifying MCU firmware logic (e.g. `patterns.rs`, `theme.rs`, `state.rs`, `mqtt.rs`, `main.rs`, `.env` keys, or dependencies), **modifications MUST be made in BOTH directories**:
> 1. `mcu-rust-esp32c3/`
> 2. `mcu-rust-esp32s2/`
>
> Never apply a feature or bugfix to only one MCU directory. Always verify both builds using `make test` (which executes `cargo check --release` in both directories).

- **Architecture & Toolchain Differences**:
  | Property | `mcu-rust-esp32c3/` | `mcu-rust-esp32s2/` |
  |---|---|---|
  | **Target Triple** | `riscv32imc-unknown-none-elf` | `xtensa-esp32s2-none-elf` |
  | **Rust Toolchain** | `nightly` (upstream) | `esp` (via `espup`, path in `~/export-esp.sh`) |
  | **`Cargo.toml` Features** | `features = ["esp32c3", ...]` | `features = ["esp32s2", ...]` |
  | **Linker** | `rust-lld` (supports `linker_be_nice`) | `xtensa-esp32s2-elf-gcc` (no LLD scripts) |
  | **Flashing (`espflash`)** | Standard stub | **Must use `--no-stub`** (Native USB CDC drops with stub) |
  | **Runner in `.cargo`** | `espflash flash --monitor --chip esp32c3` | `espflash flash --monitor --chip esp32s2 --no-stub` |

- **CRITICAL Build Rule**:
  - **DO NOT run bare `cargo test`** inside either MCU directory or target the host platform (`aarch64-apple-darwin` / `x86_64`). `esp-hal` build scripts deliberately panic on non-ESP host platforms.
  - **Always verify Rust code using `make test`** or `cd <mcu-dir> && cargo check --release`.
- **Hardware Wiring (Both Boards)**:
  - WS2812 DIN: `GPIO6` (SPI2 MOSI)
  - SCK: `GPIO4` (Unused by WS2812, reserved for SPI master init)
  - Default strip length: `LED_COUNT = 8` (`patterns::LED_COUNT`)
  - Detailed wiring guide: see [`docs/hardware_wiring.md`](file:///Users/suapapa/ws_suapapa/kitt-eye/docs/hardware_wiring.md).
- **Compile-Time Configuration**:
  - Firmware configuration is statically injected at compile time via `.env` -> `build.rs` -> `env!()` macros (in `src/config.rs`).
- **Motion Themes (`src/theme.rs`)**:
  - `classic`: All patterns use classic Knight Rider red (`RGB8(255, 0, 0)`). States are distinguished solely by motion pattern.
  - `colorful`: States are mapped to distinct, vivid colors and expressive motions:
    - `idle` → Classic Red (`RGB8(255, 0, 0)`) Slow K.I.T.T. Scanner
    - `thinking` → Mystic Purple (`RGB8(170, 0, 255)`) Center-Out
    - `generating` → Mint Emerald (`RGB8(0, 240, 120)`) Fill-Sweep
    - `executing_tool` → Vivid Amber (`RGB8(255, 140, 0)`) **Fast K.I.T.T. Scanner**
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
