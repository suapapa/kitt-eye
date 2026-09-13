# `scripts/demo-serial/` — USB Serial Demo & Control Utilities

This directory provides scripts to control the `kitt-eye` ESP32 hardware (ESP32-S2 USB-OTG CDC-ACM and ESP32-C3 USB-Serial-JTAG) **directly over USB Serial**, without requiring an MQTT broker, Wi-Fi network, Go publisher daemon, or AI CLI hooks.

---

## Key Features

- **Direct Hardware Control**: Directly sets the active LED state (`idle`, `thinking`, `generating`, `executing_tool`, `waiting_input`, `done`, `error`) and motion theme (`classic`, `colorful`) via the MCU firmware's USB Serial CLI.
- **Zero External Dependencies**: Built with Python 3's POSIX standard library (`termios`, `fcntl`, `select`). No `pip install` or `pyserial` required on macOS and Linux.
- **Auto Port Discovery**: Automatically scans and detects active USB serial devices:
  - macOS: `/dev/cu.usbmodem*`, `/dev/cu.usbserial*`
  - Linux: `/dev/ttyACM*`, `/dev/ttyUSB*`
- **Multiple Demo Modes**:
  - **`loop`** (default): Cycles through all 7 states continuously with configurable transition intervals.
  - **`workflow`**: Simulates a realistic developer + AI agent pair programming workflow (Standby → Analyze → Generate → Test → Wait for input → Done).
  - **`interactive`**: Live interactive terminal dashboard where pressing a single key (`1`–`7`, `a`, `t`, `s`, `c`, `q`) immediately switches states or toggles settings.
  - **`single`**: One-shot execution of any state or CLI command (e.g. `thinking`, `status`, `theme colorful`).
  - **`console`**: Direct two-way interactive console connected to the firmware's `kitt-eye> ` shell prompt.
- **Automatic Clean-up**: Automatically releases manual override (`state auto`) when interrupted or exiting, returning the device to normal MQTT monitoring.

---

## State & Motion Reference

| State | Key | Classic Theme | Colorful Theme | Terminal ANSI | Description |
|---|---|---|---|---|---|
| `idle` | `1` | Classic Red | Classic Red | Red | Standby / Slow K.I.T.T. scanner |
| `thinking` | `2` | Classic Red | Mystic Purple | Magenta | Center-Out breathing motion |
| `generating` | `3` | Classic Red | Mint Emerald | Green | Fill-Sweep streaming motion |
| `executing_tool` | `4` | Classic Red | Vivid Amber | Yellow | **Fast K.I.T.T. scanner** |
| `waiting_input` | `5` | Classic Red | Warning Yellow | Bright Yellow | Blink prompt |
| `done` | `6` | Classic Red | Spring Lime | Bright Green | Flash → Solid green |
| `error` | `7` | Classic Red | Crimson Red | Bright Red | Fast Comet warning |

---

## Usage Examples

### 1. Interactive Keyboard Dashboard
Control states in real time by pressing single keys without hitting Enter:
```bash
./scripts/demo-serial/demo.sh -m interactive
```

```text
======================================================================
  K.I.T.T.-EYE USB Serial Demo — Interactive Mode
  Port: /dev/cu.usbmodem12301 (115200 baud) | Theme: colorful
======================================================================
  [1] IDLE           - Slow Scanner (Standby)
  [2] THINKING       - Mystic Purple (Center-Out)
  [3] GENERATING     - Mint Emerald (Fill-Sweep)
  [4] EXECUTING_TOOL - Vivid Amber (Fast Scanner)
  [5] WAITING_INPUT  - Warning Yellow (Blink)
  [6] DONE           - Spring Lime (Flash -> Solid)
  [7] ERROR          - Crimson Red (Comet)
  --------------------------------------------------------------------
  [a] state auto     - Release override, resume MQTT
  [t] toggle theme   - Switch between 'classic' and 'colorful'
  [s] status         - Query device status
  [c] config         - Query device runtime configuration
  [?] help           - Show MCU firmware CLI help
  [q] quit           - Exit demo (resumes auto state)
======================================================================
Press key [1-7, a, t, s, c, ?, q]:
```

### 2. Automated State Cycling (Loop Mode)
Continuously cycle through all 7 states:
```bash
# Default 3s interval
./scripts/demo-serial/demo.sh

# Custom 5s interval with colorful theme
./scripts/demo-serial/demo.sh -i 5.0 -t colorful

# Run all 7 states once and exit
./scripts/demo-serial/demo.sh --once
```

### 3. AI Coding Workflow Simulation
Simulates realistic pair-programming transitions:
```bash
./scripts/demo-serial/demo.sh -m workflow
```

### 4. One-Shot State or Command Execution
Set a specific state directly from the command line:
```bash
# Set specific LED states
./scripts/demo-serial/demo.sh thinking
./scripts/demo-serial/demo.sh executing_tool
./scripts/demo-serial/demo.sh done
./scripts/demo-serial/demo.sh error

# Resume MQTT listening mode
./scripts/demo-serial/demo.sh "state auto"

# Switch LED theme
./scripts/demo-serial/demo.sh "theme classic"
./scripts/demo-serial/demo.sh "theme colorful"

# Query device status
./scripts/demo-serial/demo.sh status
```

### 5. Direct Firmware Console Shell
Open a raw terminal session directly connected to the MCU CLI:
```bash
./scripts/demo-serial/demo.sh -m console
```
*(Press `Ctrl+C` or `Ctrl+]` to exit).*

---

## Command-Line Options

| Option | Short | Default | Description |
|---|---|---|---|
| `--mode` | `-m` | `loop` (or `single`) | Execution mode: `loop`, `workflow`, `interactive`, `console`, `single` |
| `--port` | `-p` | Auto-detect | Serial port device path (e.g. `/dev/cu.usbmodem12301`) |
| `--baud` | `-b` | `115200` | Serial baud rate |
| `--interval` | `-i` | `3.0` | Duration in seconds per state in loop/workflow |
| `--theme` | `-t` | `None` | Pre-set motion theme (`classic` or `colorful`) |
| `--once` | | `false` | Run loop or workflow once then exit |
| `--list-ports` | | | List detected serial ports and exit |
| `--help` | `-h` | | Show usage help |

---

## Troubleshooting

1. **Port Busy / Access Denied (`PermissionError` / `EBUSY`)**:
   - Ensure another terminal (`screen`, `picocom`, `cargo espflash monitor`, `minicom`) is not currently using the port.
   - On Linux, ensure your user belongs to the `dialout` group: `sudo usermod -a -G dialout $USER`.
2. **Device Not Found**:
   - Verify USB cable supports data transfer (some cables are charge-only).
   - Run `./scripts/demo-serial/demo.sh --list-ports` to inspect available serial ports.
   - For ESP32-S2 boards, ensure firmware was flashed with USB-CDC enabled.
