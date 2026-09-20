# `scripts/config-serial/` — USB Serial Configuration Utilities

This directory provides scripts to configure `kitt-eye` ESP32 hardware (ESP32-S2 USB-OTG CDC-ACM and ESP32-C3 USB-Serial-JTAG) **directly over USB Serial**, without requiring network access, an MQTT broker, or recompilation.

---

## Features

- **Direct Hardware Configuration**:
  - Motion Theme: `classic` or `colorful`
  - LED Direction: `reverse on` or `reverse off`
  - Wi-Fi Settings: SSID and Password
  - MQTT Settings: Broker URL/host, Topic Prefix, Client ID
  - Flash Persistence: `save`, `load`, and compile-time `reset`
  - Software Reboot: `reboot`
- **Zero External Dependencies**: Built using Python 3's POSIX standard library (`termios`, `fcntl`, `select`). No `pyserial` or `pip install` needed on macOS and Linux.
- **Auto Port Discovery**: Automatically detects `/dev/cu.usbmodem*`, `/dev/cu.usbserial*`, `/dev/ttyACM*`, `/dev/ttyUSB*`.
- **Interactive Menu**: Run with `-i` / `--interactive` for an easy-to-use terminal menu.

---

## Usage Examples

### 1. View Current Status and Configuration
```bash
./scripts/config-serial/config.sh --status
./scripts/config-serial/config.sh --config
```

### 2. Change Theme and LED Direction (and Save to Flash)
```bash
# Set theme to classic, reverse direction ON, and save to flash:
./scripts/config-serial/config.sh --theme classic --reverse on --save

# Set theme to colorful, reverse direction OFF, and save to flash:
./scripts/config-serial/config.sh --theme colorful --reverse off --save
```

### 3. Update Wi-Fi and MQTT Settings
```bash
# Update Wi-Fi credentials and save:
./scripts/config-serial/config.sh --wifi "MyWiFiSSID" "MyWiFiPassword" --save

# Update MQTT broker and topic, then reboot:
./scripts/config-serial/config.sh --mqtt-broker "192.168.1.50:1883" --mqtt-topic "kitt-eye" --save --reboot
```

### 4. Interactive Configuration Menu
```bash
./scripts/config-serial/config.sh -i
```

---

## Command-Line Options

| Option | Short | Description |
|---|---|---|
| `--theme` | | Motion theme (`classic` or `colorful`) |
| `--reverse` | | Invert LED direction (`on` or `off`) |
| `--wifi` | | Set Wi-Fi credentials (`SSID [PASS]`) |
| `--mqtt-broker` | | Set MQTT broker (`host[:port]` or URL) |
| `--mqtt-topic` | | Set MQTT topic prefix |
| `--mqtt-client-id` | | Set MQTT client ID |
| `--save` | | Save changes to persistent Flash storage |
| `--load` | | Reload settings from Flash storage |
| `--reset` | | Reset settings to compile-time defaults |
| `--reboot` | | Reboot MCU to apply network settings |
| `--status` | | Query device status |
| `--config` | | Query device runtime configuration |
| `--interactive` | `-i` | Launch interactive configuration menu |
| `--port` | `-p` | Specific serial port device |
| `--baud` | `-b` | Baud rate (default: `115200`) |
| `--list-ports` | | List detected serial ports |
