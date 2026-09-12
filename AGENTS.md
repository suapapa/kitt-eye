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
│   ├── cmd/kitteye_mock/    # Mock state generator for hardware demos
│   ├── cmd/kitteye_sub_example/ # MQTT subscriber test utility
│   └── internal/
│       ├── config/          # YAML config parser & env overrides
│       ├── ipc/             # UDS server (/tmp/kitt-eye.sock)
│       ├── model/           # Data models, states, and priority hierarchy
│       ├── state/           # Session state store, TTLs, and aggregator
│       ├── publish/         # Sink interface (MQTT & dry-run stdout sinks)
│       └── mqtt/            # Paho MQTT v3 client, LWT, and HA discovery
├── script/                  # Demo & mock state scripts
│   ├── mock_state.sh        # Executable mock state generator (reads config.yaml)
│   ├── demo.sh              # Shortcut symlink to mock_state.sh
│   └── README.md            # Demo script documentation
├── mcu-rust-common/        # Shared MCU logic (no_std library: state, theme, patterns, mqtt, app_main)
│   ├── Cargo.toml           # Core dependencies (smart-leds, embedded-io-async, heapless)
│   ├── build.rs             # Unified build script (loads .env, sets cargo:rustc-env, linker flags)
│   └── src/                 # lib.rs, state.rs, theme.rs, patterns.rs, mqtt.rs, config.rs, app_main.rs
├── mcu-rust-esp32c3/        # ESP32-C3 Firmware Package (RISC-V, nightly toolchain)
│   ├── Cargo.toml           # References mcu-rust-common, esp-hal (~1.1) with esp32c3 feature
│   └── .cargo/config.toml   # riscv32imc-unknown-none-elf target, espflash runner
└── mcu-rust-esp32s2/        # ESP32-S2 Firmware Package (Xtensa, esp toolchain)
    ├── Cargo.toml           # References mcu-rust-common, esp-hal (~1.1) with esp32s2 feature
    └── .cargo/config.toml   # xtensa-esp32s2-none-elf target, espflash --no-stub runner
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
                        # 2. cd mcu-rust-common && cargo test
                        # 3. cd mcu-rust-esp32c3 && cargo check --release
                        # 4. cd mcu-rust-esp32s2 && cargo check --release

# Local testing without MQTT broker (Dry-run mode)
make run-pub            # Runs publisher using config.yaml (stdout JSON sink if broker is "")
# In another terminal:
./hooks/common/send_event.sh claude executing_tool "Running tests"
./hooks/common/send_event.sh claude done "All tests passed"

# Mock state demo (simulates states without AI CLI agents or hooks)
make demo               # Run automated state cycling demo using config.yaml
./script/mock_state.sh -m workflow     # Run realistic agent pair-programming demo
./script/mock_state.sh -m interactive  # Interactive keyboard menu to switch states
./script/mock_state.sh thinking        # Publish single state directly to MQTT

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

### 4.2. Embedded Firmwares (`mcu-rust-common/`, `mcu-rust-esp32c3/` & `mcu-rust-esp32s2/`)

> [!TIP]
> **SINGLE SOURCE OF TRUTH IN `mcu-rust-common/`**:
> All MCU application logic, patterns, themes, state models, MQTT client, and main firmware loop (`app_main.rs`) are centralized in **`mcu-rust-common/`**.
> `mcu-rust-esp32c3/` and `mcu-rust-esp32s2/` are thin board packages that specify chip features (`esp32c3` vs `esp32s2`), target triples, and flash runners.
>
> When making changes to firmware logic, edit only files in `mcu-rust-common/`!
> Always verify all builds using `make test` (which executes `cargo test` in `mcu-rust-common` and `cargo check --release` in both board directories).

- **Architecture & Toolchain Differences**:
  | Property | `mcu-rust-common/` | `mcu-rust-esp32c3/` | `mcu-rust-esp32s2/` |
  |---|---|---|---|
  | **Role** | Core logic & app runner | Thin ESP32-C3 binary wrapper | Thin ESP32-S2 binary wrapper |
  | **Target Triple** | Host / any `no_std` target | `riscv32imc-unknown-none-elf` | `xtensa-esp32s2-none-elf` |
  | **Rust Toolchain** | `stable` / `nightly` | `nightly` (upstream) | `esp` (via `espup`, path in `~/export-esp.sh`) |
  | **`Cargo.toml` Features** | `no_std` dependencies only | `features = ["esp32c3", ...]` | `features = ["esp32s2", ...]` |
  | **Linker** | Host linker | `rust-lld` (supports `linker_be_nice`) | `xtensa-esp32s2-elf-gcc` (no LLD scripts) |
  | **Flashing (`espflash`)** | N/A (library) | Standard stub | **Must use `--no-stub`** (Native USB CDC drops with stub) |
  | **Runner in `.cargo`** | N/A | `espflash flash --monitor --chip esp32c3` | `espflash flash --monitor --chip esp32s2 --no-stub` |

- **Testing Rules**:
  - `mcu-rust-common/` has **no direct HAL dependencies**, so its unit tests run natively on the host via `cd mcu-rust-common && cargo test`.
  - In `mcu-rust-esp32c3/` and `mcu-rust-esp32s2/`, do NOT run bare host tests; verify with `cargo check --release` or `make test`.
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
