#!/usr/bin/env python3
"""
kitt-eye USB Serial Configuration Tool
Configures ESP32-S2 and ESP32-C3 firmware runtime settings (theme, reverse, Wi-Fi, MQTT)
directly over USB Serial without requiring network or MQTT connectivity.

Zero external dependencies required (uses standard library termios/fcntl on POSIX).
"""

import argparse
import fcntl
import glob
import os
import select
import struct
import sys
import termios
import time

# ANSI Color codes
RESET = "\033[0m"
BOLD = "\033[1m"
DIM = "\033[2m"
RED = "\033[31m"
GREEN = "\033[32m"
YELLOW = "\033[33m"
MAGENTA = "\033[35m"
CYAN = "\033[36m"
BRIGHT_RED = "\033[91m"
BRIGHT_GREEN = "\033[92m"
BRIGHT_YELLOW = "\033[93m"
BRIGHT_WHITE = "\033[97m"


def find_serial_ports():
    """Find potential USB serial device ports on macOS and Linux."""
    ports = []
    # macOS CDC devices: /dev/cu.usbmodem* is preferred over /dev/tty.usbmodem*
    ports.extend(sorted(glob.glob("/dev/cu.usbmodem*")))
    ports.extend(sorted(glob.glob("/dev/cu.usbserial*")))
    # Linux ACM / USB devices
    ports.extend(sorted(glob.glob("/dev/ttyACM*")))
    ports.extend(sorted(glob.glob("/dev/ttyUSB*")))
    # Fallback to tty.usbmodem on macOS if cu.* not present
    if not ports:
        ports.extend(sorted(glob.glob("/dev/tty.usbmodem*")))
        ports.extend(sorted(glob.glob("/dev/tty.usbserial*")))
    return ports


class SerialPort:
    """Lightweight POSIX serial port communicator using Python standard library."""

    def __init__(self, port, baud=115200):
        self.port = port
        self.baud = baud
        self.fd = None
        self._saved_termios = None

    def open(self):
        try:
            self.fd = os.open(
                self.port, os.O_RDWR | os.O_NOCTTY | os.O_NONBLOCK
            )
        except OSError as e:
            raise RuntimeError(
                f"Failed to open port '{self.port}': {e}"
            ) from e

        # Configure termios
        attrs = termios.tcgetattr(self.fd)
        self._saved_termios = list(attrs)

        baud_const = getattr(termios, f"B{self.baud}", termios.B115200)
        attrs[4] = baud_const  # ispeed
        attrs[5] = baud_const  # ospeed

        # Raw mode 8N1
        attrs[0] = 0  # iflag
        attrs[1] = 0  # oflag
        attrs[2] = termios.CS8 | termios.CREAD | termios.CLOCAL  # cflag
        attrs[3] = 0  # lflag
        attrs[6][termios.VMIN] = 0
        attrs[6][termios.VTIME] = 0

        termios.tcsetattr(self.fd, termios.TCSANOW, attrs)

        # Assert DTR & RTS (essential for USB CDC-ACM on ESP32-S2)
        try:
            TIOCMBIS = 0x5416 if sys.platform.startswith("linux") else 0x2000746C
            TIOCM_DTR = 0x0002
            TIOCM_RTS = 0x0004
            fcntl.ioctl(self.fd, TIOCMBIS, struct.pack("I", TIOCM_DTR | TIOCM_RTS))
        except Exception:
            pass

        # Brief settle time and clear initial buffer
        time.sleep(0.15)
        self.flush_rx()

    def close(self):
        if self.fd is not None:
            try:
                if self._saved_termios:
                    termios.tcsetattr(self.fd, termios.TCSANOW, self._saved_termios)
            except Exception:
                pass
            try:
                os.close(self.fd)
            except Exception:
                pass
            self.fd = None

    def flush_rx(self):
        """Discard unread incoming bytes."""
        if self.fd is None:
            return
        while True:
            r, _, _ = select.select([self.fd], [], [], 0.05)
            if r:
                try:
                    os.read(self.fd, 1024)
                except Exception:
                    break
            else:
                break

    def write_raw(self, data: bytes):
        if self.fd is None:
            raise RuntimeError("Port not open")
        os.write(self.fd, data)

    def send_command(self, cmd: str, timeout: float = 0.8) -> str:
        """Send command string, wait for and return the response."""
        self.flush_rx()
        payload = (cmd.strip() + "\r\n").encode("utf-8")
        self.write_raw(payload)

        out = []
        start = time.time()
        while time.time() - start < timeout:
            r, _, _ = select.select([self.fd], [], [], 0.05)
            if r:
                try:
                    chunk = os.read(self.fd, 1024)
                    if not chunk:
                        break
                    out.append(chunk)
                    combined = b"".join(out).decode("utf-8", errors="replace")
                    # Check for prompt or key response prefixes
                    if (
                        "kitt-eye> " in combined
                        or "OK:" in combined
                        or "ERR:" in combined
                        or "STATUS:" in combined
                        or "CONFIG:" in combined
                        or "STATE:" in combined
                    ):
                        time.sleep(0.05)
                        r2, _, _ = select.select([self.fd], [], [], 0.05)
                        if r2:
                            out.append(os.read(self.fd, 1024))
                        break
                except Exception:
                    break

        raw_resp = b"".join(out).decode("utf-8", errors="replace")
        lines = raw_resp.replace("\r\n", "\n").replace("\r", "\n").split("\n")
        cleaned = []
        for line in lines:
            trimmed = line.strip()
            if not trimmed or trimmed == cmd.strip() or trimmed == "kitt-eye>":
                continue
            cleaned.append(trimmed)
        return "\n".join(cleaned)


def execute_commands(sp: SerialPort, commands: list[tuple[str, str]]) -> bool:
    """Execute a sequence of (description, command_str) pairs."""
    success = True
    for desc, cmd in commands:
        print(f"{CYAN}--> {desc}...{RESET} ({DIM}{cmd}{RESET})")
        resp = sp.send_command(cmd)
        if resp:
            for line in resp.split("\n"):
                if "ERR:" in line:
                    print(f"    {BRIGHT_RED}{line}{RESET}")
                    success = False
                elif "OK:" in line:
                    print(f"    {BRIGHT_GREEN}{line}{RESET}")
                else:
                    print(f"    {line}")
        else:
            print(f"    {DIM}(No response or command sent){RESET}")
        time.sleep(0.05)
    return success


def interactive_menu(sp: SerialPort):
    """Interactive console menu for querying and setting configurations."""
    while True:
        print(f"\n{BOLD}{'=' * 65}{RESET}")
        print(f"{BOLD}   K.I.T.T.-EYE USB Serial Configuration Console{RESET}")
        print(f"   Port: {CYAN}{sp.port}{RESET} (115200 baud)")
        print(f"{BOLD}{'=' * 65}{RESET}")
        print(f"  {BOLD}[1]{RESET} Status & Config  - Query current device status & config")
        print(f"  {BOLD}[2]{RESET} Theme            - Set LED motion theme (classic / colorful)")
        print(f"  {BOLD}[3]{RESET} Reverse          - Set LED direction (on / off)")
        print(f"  {BOLD}[4]{RESET} Wi-Fi            - Configure Wi-Fi SSID and password")
        print(f"  {BOLD}[5]{RESET} MQTT Broker      - Configure MQTT broker host/url")
        print(f"  {BOLD}[6]{RESET} MQTT Topic       - Configure MQTT topic prefix")
        print(f"  {BOLD}[7]{RESET} MQTT Client ID   - Configure MQTT client ID")
        print(f"  {BOLD}[8]{RESET} Save to Flash    - Persist current settings to flash")
        print(f"  {BOLD}[9]{RESET} Load from Flash  - Reload settings from flash")
        print(f"  {BOLD}[0]{RESET} Reset Defaults   - Reset configuration to compile-time defaults")
        print(f"  {BOLD}[r]{RESET} Reboot MCU       - Reboot device to apply network changes")
        print(f"  {BOLD}[q]{RESET} Quit")
        print(f"{BOLD}{'=' * 65}{RESET}")

        try:
            choice = input(f"{BOLD}Select an option [0-9, r, q]: {RESET}").strip().lower()
        except (EOFError, KeyboardInterrupt):
            print("\nExiting.")
            break

        if choice in ("q", "quit", "exit"):
            break
        elif choice == "1":
            print(f"\n{CYAN}--- Device Status ---{RESET}")
            resp_status = sp.send_command("status")
            print(resp_status if resp_status else "(No response)")
            print(f"\n{CYAN}--- Device Config ---{RESET}")
            resp_config = sp.send_command("config")
            print(resp_config if resp_config else "(No response)")
        elif choice == "2":
            theme = input("Enter theme (classic / colorful): ").strip().lower()
            if theme in ("classic", "colorful", "red", "color", "rgb"):
                execute_commands(sp, [(f"Set theme to {theme}", f"theme {theme}")])
            else:
                print(f"{RED}Invalid theme. Use 'classic' or 'colorful'.{RESET}")
        elif choice == "3":
            rev = input("Reverse LED direction? (on / off): ").strip().lower()
            if rev in ("on", "off", "true", "false", "1", "0"):
                execute_commands(sp, [(f"Set reverse to {rev}", f"reverse {rev}")])
            else:
                print(f"{RED}Invalid value. Use 'on' or 'off'.{RESET}")
        elif choice == "4":
            ssid = input("Enter Wi-Fi SSID: ").strip()
            if ssid:
                pwd = input("Enter Wi-Fi Password: ").strip()
                execute_commands(sp, [("Update Wi-Fi credentials", f"wifi {ssid} {pwd}")])
            else:
                print(f"{RED}SSID cannot be empty.{RESET}")
        elif choice == "5":
            broker = input("Enter MQTT Broker (e.g. 192.168.1.50 1883 or mqtt://user:pass@host:1883): ").strip()
            if broker:
                execute_commands(sp, [("Update MQTT Broker", f"mqtt_broker {broker}")])
            else:
                print(f"{RED}Broker cannot be empty.{RESET}")
        elif choice == "6":
            topic = input("Enter MQTT Topic Prefix (e.g. kitt-eye): ").strip()
            if topic:
                execute_commands(sp, [("Update MQTT Topic", f"mqtt_topic {topic}")])
        elif choice == "7":
            client_id = input("Enter MQTT Client ID: ").strip()
            if client_id:
                execute_commands(sp, [("Update MQTT Client ID", f"mqtt_client_id {client_id}")])
        elif choice == "8":
            execute_commands(sp, [("Save configuration to flash", "save")])
        elif choice == "9":
            execute_commands(sp, [("Reload configuration from flash", "load")])
        elif choice == "0":
            confirm = input("Reset all settings to defaults? [y/N]: ").strip().lower()
            if confirm == "y":
                execute_commands(sp, [("Reset configuration to defaults", "reset")])
        elif choice == "r":
            confirm = input("Reboot MCU now? [y/N]: ").strip().lower()
            if confirm == "y":
                execute_commands(sp, [("Reboot MCU", "reboot")])
                print(f"{YELLOW}MCU reboot triggered. Exiting console.{RESET}")
                break


def main():
    parser = argparse.ArgumentParser(
        description="K.I.T.T.-EYE USB Serial Configuration Tool",
        formatter_class=argparse.RawDescriptionHelpFormatter,
        epilog="""
Examples:
  # Show current status and configuration:
  %(prog)s --status
  %(prog)s --config

  # Set theme to classic and enable LED reverse, and save to flash:
  %(prog)s --theme classic --reverse on --save

  # Set colorful theme and disable reverse:
  %(prog)s --theme colorful --reverse off --save

  # Update Wi-Fi and MQTT settings then save and reboot:
  %(prog)s --wifi "MyHomeWiFi" "SecretPass" --mqtt-broker "192.168.1.50 1883" --save --reboot

  # Interactive configuration menu:
  %(prog)s -i
        """,
    )

    parser.add_argument(
        "-p",
        "--port",
        default=None,
        help="Serial port device (default: auto-detect)",
    )
    parser.add_argument(
        "-b",
        "--baud",
        type=int,
        default=115200,
        help="Baud rate (default: 115200)",
    )
    parser.add_argument(
        "-i",
        "--interactive",
        action="store_true",
        help="Run interactive configuration menu",
    )
    parser.add_argument(
        "--list-ports",
        action="store_true",
        help="List detected serial ports and exit",
    )
    parser.add_argument(
        "--status",
        action="store_true",
        help="Query device status (Wi-Fi, state, theme, reverse)",
    )
    parser.add_argument(
        "--config",
        action="store_true",
        help="Query device runtime configuration",
    )
    parser.add_argument(
        "--theme",
        choices=["classic", "colorful", "red", "color"],
        default=None,
        help="Set LED motion theme (classic / colorful)",
    )
    parser.add_argument(
        "--reverse",
        choices=["on", "off", "true", "false", "1", "0"],
        default=None,
        help="Invert left-right LED direction (on / off)",
    )
    parser.add_argument(
        "--wifi",
        nargs="+",
        metavar=("SSID", "PASS"),
        help="Set Wi-Fi credentials: SSID [PASSWORD]",
    )
    parser.add_argument(
        "--mqtt-broker",
        metavar="BROKER",
        help="Set MQTT broker (host[:port] or mqtt://user:pass@host:port)",
    )
    parser.add_argument(
        "--mqtt-topic",
        metavar="PREFIX",
        help="Set MQTT topic prefix (default: kitt-eye)",
    )
    parser.add_argument(
        "--mqtt-client-id",
        metavar="ID",
        help="Set MQTT client ID",
    )
    parser.add_argument(
        "--save",
        action="store_true",
        help="Save updated configuration to persistent flash",
    )
    parser.add_argument(
        "--load",
        action="store_true",
        help="Reload configuration from flash",
    )
    parser.add_argument(
        "--reset",
        action="store_true",
        help="Reset configuration to compile-time defaults",
    )
    parser.add_argument(
        "--reboot",
        action="store_true",
        help="Trigger MCU software reboot",
    )

    args = parser.parse_args()

    # List ports
    if args.list_ports:
        ports = find_serial_ports()
        if ports:
            print("Detected USB serial port(s):")
            for p in ports:
                print(f"  - {p}")
        else:
            print("No USB serial devices detected.")
        return 0

    # Auto-detect or validate port
    port = args.port
    if not port:
        candidates = find_serial_ports()
        if not candidates:
            print(
                f"{RED}[Error] No USB serial port detected.{RESET}\n"
                f"Please connect your ESP32-S2 or ESP32-C3 via USB, or specify with -p /dev/... .\n"
                f"Run with --list-ports to inspect."
            )
            return 1
        port = candidates[0]
        print(f"{DIM}[Auto-detected port: {port}]{RESET}")

    # Open serial port
    sp = SerialPort(port, baud=args.baud)
    try:
        sp.open()
    except Exception as e:
        print(f"{RED}[Error] Could not open serial port '{port}': {e}{RESET}")
        return 1

    try:
        if args.interactive:
            interactive_menu(sp)
            return 0

        # Build list of commands to run
        commands: list[tuple[str, str]] = []

        if args.reset:
            commands.append(("Reset configuration to defaults", "reset"))

        if args.theme:
            theme_val = "classic" if args.theme in ("classic", "red") else "colorful"
            commands.append((f"Set theme to {theme_val}", f"theme {theme_val}"))

        if args.reverse:
            rev_val = "on" if args.reverse in ("on", "true", "1") else "off"
            commands.append((f"Set reverse to {rev_val}", f"reverse {rev_val}"))

        if args.wifi:
            ssid = args.wifi[0]
            pwd = args.wifi[1] if len(args.wifi) > 1 else ""
            commands.append((f"Set Wi-Fi credentials (SSID: {ssid})", f"wifi {ssid} {pwd}"))

        if args.mqtt_broker:
            commands.append((f"Set MQTT broker ({args.mqtt_broker})", f"mqtt_broker {args.mqtt_broker}"))

        if args.mqtt_topic:
            commands.append((f"Set MQTT topic prefix ({args.mqtt_topic})", f"mqtt_topic {args.mqtt_topic}"))

        if args.mqtt_client_id:
            commands.append((f"Set MQTT client ID ({args.mqtt_client_id})", f"mqtt_client_id {args.mqtt_client_id}"))

        if args.save:
            commands.append(("Save configuration to flash", "save"))

        if args.load:
            commands.append(("Load configuration from flash", "load"))

        if args.reboot:
            commands.append(("Reboot MCU", "reboot"))

        # If specific queries requested
        if args.status:
            commands.append(("Query device status", "status"))

        if args.config:
            commands.append(("Query device configuration", "config"))

        # If no action flags were specified at all, default to showing status and config
        if not commands:
            print(f"\n{BOLD}No configuration arguments provided. Showing current status & config:{RESET}\n")
            commands.append(("Query device status", "status"))
            commands.append(("Query device configuration", "config"))

        success = execute_commands(sp, commands)
        return 0 if success else 1

    finally:
        sp.close()


if __name__ == "__main__":
    sys.exit(main())
