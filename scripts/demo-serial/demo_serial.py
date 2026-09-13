#!/usr/bin/env python3
"""
kitt-eye USB Serial Demo Tool
Controls ESP32-S2 (USB-OTG CDC-ACM) and ESP32-C3 (USB-Serial-JTAG) LED states
and device configuration directly via USB Serial Port.

Zero external dependencies required (uses standard library termios/fcntl on POSIX).
"""

import argparse
import fcntl
import glob
import os
import select
import signal
import struct
import sys
import termios
import time
import tty

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

STATES = [
    {
        "name": "idle",
        "key": "1",
        "color": RED,
        "classic": "Classic Red (Slow Scanner)",
        "colorful": "Classic Red (Slow Scanner)",
        "detail": "Standby / waiting for user prompt",
    },
    {
        "name": "thinking",
        "key": "2",
        "color": MAGENTA,
        "classic": "Classic Red (Center-Out)",
        "colorful": "Mystic Purple (Center-Out)",
        "detail": "Analyzing prompt & repository architecture",
    },
    {
        "name": "generating",
        "key": "3",
        "color": GREEN,
        "classic": "Classic Red (Fill-Sweep)",
        "colorful": "Mint Emerald (Fill-Sweep)",
        "detail": "Streaming response & drafting changes",
    },
    {
        "name": "executing_tool",
        "alt_name": "tool",
        "key": "4",
        "color": YELLOW,
        "classic": "Classic Red (Fast Scanner)",
        "colorful": "Vivid Amber (Fast Scanner)",
        "detail": "Executing tool: cargo test / make test",
    },
    {
        "name": "waiting_input",
        "alt_name": "input",
        "key": "5",
        "color": BRIGHT_YELLOW,
        "classic": "Classic Red (Blink)",
        "colorful": "Warning Yellow (Blink)",
        "detail": "Waiting for user confirmation / choice",
    },
    {
        "name": "done",
        "key": "6",
        "color": BRIGHT_GREEN,
        "classic": "Classic Red (Flash -> Solid)",
        "colorful": "Spring Lime (Flash -> Solid)",
        "detail": "Task completed successfully",
    },
    {
        "name": "error",
        "key": "7",
        "color": BRIGHT_RED,
        "classic": "Classic Red (Comet)",
        "colorful": "Crimson Red (Comet)",
        "detail": "Error: compilation or test failure",
    },
]

WORKFLOW_STEPS = [
    ("idle", 2.0, "Ready for user prompt"),
    ("thinking", 3.5, "Analyzing prompt & repository architecture"),
    ("generating", 3.5, "Generating code modifications"),
    ("executing_tool", 4.0, "Executing test suite: make test"),
    ("waiting_input", 3.0, "Prompting user for commit confirmation"),
    ("done", 3.0, "Task completed successfully!"),
    ("idle", 2.0, "Returning to standby"),
]


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

    def send_command(self, cmd: str, timeout: float = 0.6) -> str:
        """Send command string, wait for and return the response."""
        self.flush_rx()
        payload = (cmd.strip() + "\r\n").encode("utf-8")
        self.write_raw(payload)

        out = []
        start = time.time()
        while time.time() - start < timeout:
            r, _, _ = select.select([self.fd], [], [], 0.1)
            if r:
                try:
                    chunk = os.read(self.fd, 1024)
                    if not chunk:
                        break
                    out.append(chunk)
                    # Check if prompt or response terminator arrived
                    combined = b"".join(out).decode("utf-8", errors="replace")
                    if "kitt-eye> " in combined or "OK:" in combined or "STATUS:" in combined or "STATE:" in combined:
                        # Give a tiny slice to catch remaining bytes of line
                        time.sleep(0.05)
                        r2, _, _ = select.select([self.fd], [], [], 0.05)
                        if r2:
                            out.append(os.read(self.fd, 1024))
                        break
                except Exception:
                    break

        raw_resp = b"".join(out).decode("utf-8", errors="replace")
        # Clean up echo of command and prompt
        lines = raw_resp.replace("\r\n", "\n").replace("\r", "\n").split("\n")
        cleaned = []
        for line in lines:
            trimmed = line.strip()
            if not trimmed or trimmed == cmd.strip() or trimmed == "kitt-eye>":
                continue
            cleaned.append(trimmed)
        return "\n".join(cleaned)


def get_state_meta(name: str):
    name_lower = name.lower()
    for s in STATES:
        if s["name"] == name_lower or s.get("alt_name") == name_lower:
            return s
    return None


def run_single(sp: SerialPort, cmd_or_state: str):
    """Execute single state or command and print response."""
    state_meta = get_state_meta(cmd_or_state)
    if state_meta:
        cmd = f"state {state_meta['name']}"
        print(f"Setting state to '{state_meta['name']}'...")
    else:
        cmd = cmd_or_state

    resp = sp.send_command(cmd)
    if resp:
        print(resp)
    else:
        print(f"Command '{cmd}' sent.")


def run_loop(sp: SerialPort, interval: float, once: bool, theme_opt: str = None):
    """Cycle through all 7 states continuously."""
    if theme_opt:
        sp.send_command(f"theme {theme_opt}")
        print(f"[Theme set to {theme_opt}]")

    print(f"\n{BOLD}=== K.I.T.T.-EYE USB Serial Demo — State Loop ==={RESET}")
    print(f"Cycling through 7 states every {interval:.1f}s. Press {BOLD}Ctrl+C{RESET} to stop.\n")

    round_num = 1
    try:
        while True:
            for s in STATES:
                state_name = s["name"]
                cmd = f"state {state_name}"
                sp.send_command(cmd)

                timestamp = time.strftime("%H:%M:%S")
                color = s["color"]
                desc = s["colorful"]
                print(
                    f"{DIM}{timestamp}{RESET}  "
                    f"{color}[{state_name.upper():<14}]{RESET}  "
                    f"{desc:<28} | {DIM}{s['detail']}{RESET}"
                )

                time.sleep(interval)

            if once:
                break
            round_num += 1
            print(f"\n{DIM}--- Completed Cycle #{round_num - 1} ---{RESET}\n")

    except KeyboardInterrupt:
        print(f"\n\n{YELLOW}Stopping demo... Releasing manual override to MQTT...{RESET}")
        resp = sp.send_command("state auto")
        if resp:
            print(f"MCU: {resp}")


def run_workflow(sp: SerialPort, scale: float, once: bool, theme_opt: str = None):
    """Simulate a realistic AI coding workflow."""
    if theme_opt:
        sp.send_command(f"theme {theme_opt}")
        print(f"[Theme set to {theme_opt}]")

    print(f"\n{BOLD}=== K.I.T.T.-EYE USB Serial Demo — AI Workflow Simulation ==={RESET}")
    print(f"Simulating realistic developer + AI agent pairing. Press {BOLD}Ctrl+C{RESET} to stop.\n")

    cycle = 1
    try:
        while True:
            print(f"{CYAN}{BOLD}--- Workflow Scenario #{cycle} ---{RESET}")
            for state_name, dur, note in WORKFLOW_STEPS:
                s = get_state_meta(state_name)
                sp.send_command(f"state {state_name}")

                timestamp = time.strftime("%H:%M:%S")
                color = s["color"] if s else RESET
                actual_dur = dur * scale
                print(
                    f"{DIM}{timestamp}{RESET}  "
                    f"{color}[{state_name.upper():<14}]{RESET}  "
                    f"{s['colorful']:<28} | {BOLD}{note}{RESET} ({actual_dur:.1f}s)"
                )
                time.sleep(actual_dur)

            if once:
                break
            cycle += 1
            print()

    except KeyboardInterrupt:
        print(f"\n\n{YELLOW}Stopping workflow... Releasing manual override...{RESET}")
        resp = sp.send_command("state auto")
        if resp:
            print(f"MCU: {resp}")


def read_single_key():
    """Read a single keypress without waiting for Enter."""
    fd = sys.stdin.fileno()
    old_settings = termios.tcgetattr(fd)
    try:
        tty.setraw(fd)
        ch = sys.stdin.read(1)
        if ch == "\x1b":  # Escape sequence
            # Try to read extra characters if any
            r, _, _ = select.select([sys.stdin], [], [], 0.05)
            if r:
                extra = sys.stdin.read(2)
                return "\x1b" + extra
        return ch
    finally:
        termios.tcsetattr(fd, termios.TCSADRAIN, old_settings)


def run_interactive(sp: SerialPort, theme_opt: str = None):
    """Interactive menu where pressing a key immediately sends the command."""
    current_theme = theme_opt or "colorful"
    if theme_opt:
        sp.send_command(f"theme {theme_opt}")

    def print_menu():
        print(f"\n{BOLD}{'=' * 70}{RESET}")
        print(f"{BOLD}  K.I.T.T.-EYE USB Serial Demo — Interactive Mode{RESET}")
        print(f"  Port: {CYAN}{sp.port}{RESET} (115200 baud) | Theme: {MAGENTA}{current_theme}{RESET}")
        print(f"{BOLD}{'=' * 70}{RESET}")
        print(f"  {RED}[1] IDLE{RESET}           - Slow Scanner (Standby)")
        print(f"  {MAGENTA}[2] THINKING{RESET}       - Mystic Purple (Center-Out)")
        print(f"  {GREEN}[3] GENERATING{RESET}     - Mint Emerald (Fill-Sweep)")
        print(f"  {YELLOW}[4] EXECUTING_TOOL{RESET} - Vivid Amber (Fast Scanner)")
        print(f"  {BRIGHT_YELLOW}[5] WAITING_INPUT{RESET}  - Warning Yellow (Blink)")
        print(f"  {BRIGHT_GREEN}[6] DONE{RESET}           - Spring Lime (Flash -> Solid)")
        print(f"  {BRIGHT_RED}[7] ERROR{RESET}          - Crimson Red (Comet)")
        print(f"  {DIM}{'-' * 68}{RESET}")
        print(f"  {CYAN}[a]{RESET} state auto     - Release override, resume MQTT")
        print(f"  {CYAN}[t]{RESET} toggle theme   - Switch between 'classic' and 'colorful'")
        print(f"  {CYAN}[s]{RESET} status         - Query device status")
        print(f"  {CYAN}[c]{RESET} config         - Query device runtime configuration")
        print(f"  {CYAN}[?]{RESET} help           - Show MCU firmware CLI help")
        print(f"  {CYAN}[q]{RESET} quit           - Exit demo (resumes auto state)")
        print(f"{BOLD}{'=' * 70}{RESET}")
        print(f"Press key [1-7, a, t, s, c, ?, q]: ", end="", flush=True)

    print_menu()
    try:
        while True:
            key = read_single_key()
            if not key:
                continue

            if key in ("q", "Q", "\x03", "\x1b"):  # q, Ctrl+C, Esc
                print(f"\n{YELLOW}Exiting interactive mode...{RESET}")
                resp = sp.send_command("state auto")
                if resp:
                    print(f"MCU: {resp}")
                break

            # Handle state keys
            handled = False
            for s in STATES:
                if key == s["key"]:
                    state_name = s["name"]
                    print(f"\n--> Setting state: {s['color']}{state_name.upper()}{RESET}")
                    resp = sp.send_command(f"state {state_name}")
                    if resp:
                        print(f"    {resp}")
                    handled = True
                    break

            if not handled:
                if key in ("a", "A"):
                    print(f"\n--> Setting mode: {CYAN}AUTO (MQTT){RESET}")
                    resp = sp.send_command("state auto")
                    if resp:
                        print(f"    {resp}")
                elif key in ("t", "T"):
                    current_theme = "classic" if current_theme == "colorful" else "colorful"
                    print(f"\n--> Switching theme: {MAGENTA}{current_theme}{RESET}")
                    resp = sp.send_command(f"theme {current_theme}")
                    if resp:
                        print(f"    {resp}")
                elif key in ("s", "S"):
                    print(f"\n--> Querying Status:")
                    resp = sp.send_command("status")
                    if resp:
                        print(f"{CYAN}{resp}{RESET}")
                elif key in ("c", "C"):
                    print(f"\n--> Querying Config:")
                    resp = sp.send_command("config")
                    if resp:
                        print(f"{CYAN}{resp}{RESET}")
                elif key in ("?", "h", "H"):
                    print(f"\n--> MCU Help:")
                    resp = sp.send_command("help")
                    if resp:
                        print(resp)
                else:
                    print(f"\n[Unknown key: '{key}']")

            print_menu()

    except KeyboardInterrupt:
        print(f"\n{YELLOW}Interrupted. Restoring auto state...{RESET}")
        sp.send_command("state auto")


def run_console(sp: SerialPort):
    """Direct interactive passthrough console to MCU CLI."""
    print(f"\n{BOLD}=== K.I.T.T.-EYE Direct Serial Console ==={RESET}")
    print(f"Connected to {CYAN}{sp.port}{RESET} at {sp.baud} baud.")
    print(f"Type commands directly ('help', 'status', 'state thinking', 'exit' to quit).\n")

    # Send newline to trigger prompt
    sp.write_raw(b"\r\n")

    fd_stdin = sys.stdin.fileno()
    old_settings = termios.tcgetattr(fd_stdin)

    try:
        tty.setraw(fd_stdin)
        while True:
            r, _, _ = select.select([fd_stdin, sp.fd], [], [], 0.05)
            if fd_stdin in r:
                ch = os.read(fd_stdin, 1)
                if ch in (b"\x1d", b"\x03"):  # Ctrl+] or Ctrl+C
                    break
                sp.write_raw(ch)

            if sp.fd in r:
                incoming = os.read(sp.fd, 1024)
                if incoming:
                    # Write to stdout
                    os.write(sys.stdout.fileno(), incoming)
    finally:
        termios.tcsetattr(fd_stdin, termios.TCSADRAIN, old_settings)
        print(f"\n{YELLOW}Closed serial console.{RESET}")


def main():
    parser = argparse.ArgumentParser(
        description="K.I.T.T.-EYE USB Serial Demo & Control Tool",
        formatter_class=argparse.RawDescriptionHelpFormatter,
        epilog="""
Modes:
  loop          Cycle through all 7 states continuously (default)
  workflow      Simulate realistic AI pair programming workflow
  interactive   Interactive keyboard menu to switch states on the fly
  console       Direct interactive terminal console connected to MCU CLI
  single        Send single state or CLI command and exit

Examples:
  %(prog)s                              # Continuous loop across all 7 states
  %(prog)s -m workflow                  # AI pair programming workflow demo
  %(prog)s -m interactive               # Interactive keyboard menu [1-7, a, t, s, q]
  %(prog)s thinking                     # Set state 'thinking' once and exit
  %(prog)s executing_tool               # Set state 'executing_tool' (Amber scanner)
  %(prog)s status                       # Query system status from MCU
  %(prog)s theme colorful               # Set colorful theme
  %(prog)s state auto                   # Release manual override back to MQTT
  %(prog)s -m console                   # Direct interactive CLI shell
        """,
    )

    parser.add_argument(
        "target",
        nargs="?",
        default=None,
        help="Single state name (e.g. 'thinking') or command (e.g. 'status', 'state auto')",
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
        "-m",
        "--mode",
        choices=["loop", "workflow", "interactive", "console", "single"],
        default=None,
        help="Execution mode (default: loop, or single if target specified)",
    )
    parser.add_argument(
        "-i",
        "--interval",
        type=float,
        default=3.0,
        help="State duration interval in seconds (default: 3.0)",
    )
    parser.add_argument(
        "-t",
        "--theme",
        choices=["classic", "colorful"],
        default=None,
        help="Pre-set motion theme ('classic' or 'colorful')",
    )
    parser.add_argument(
        "--once",
        action="store_true",
        help="Run loop or workflow scenario once and exit",
    )
    parser.add_argument(
        "--list-ports",
        action="store_true",
        help="List detected serial ports and exit",
    )

    args = parser.parse_args()

    # List ports mode
    if args.list_ports:
        ports = find_serial_ports()
        if ports:
            print(f"Detected USB serial port(s):")
            for p in ports:
                print(f"  - {p}")
        else:
            print("No USB serial devices detected.")
            print("Please ensure your ESP32 board is connected with a data-capable USB cable.")
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

    # Determine mode
    mode = args.mode
    if mode is None:
        if args.target is not None:
            mode = "single"
        else:
            mode = "loop"

    # Open serial port
    sp = SerialPort(port, baud=args.baud)
    try:
        sp.open()
    except Exception as e:
        print(f"{RED}[Error] Could not open serial port '{port}': {e}{RESET}")
        return 1

    try:
        if mode == "single":
            target = args.target if args.target else "idle"
            run_single(sp, target)
        elif mode == "loop":
            run_loop(sp, args.interval, args.once, args.theme)
        elif mode == "workflow":
            run_workflow(sp, args.interval / 3.0, args.once, args.theme)
        elif mode == "interactive":
            run_interactive(sp, args.theme)
        elif mode == "console":
            run_console(sp)
    finally:
        sp.close()

    return 0


if __name__ == "__main__":
    sys.exit(main())
