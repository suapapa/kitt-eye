.PHONY: help build-pub run-pub build-sub run-sub build-mock run-mock demo demo-mqtt demo-serial \
	build-mcu build-mcu-c3 build-mcu-s2 \
	flash-mcu flash-mcu-c3 flash-mcu-s2 \
	install-hooks uninstall-hooks \
	install-hooks-claude install-hooks-codex install-hooks-cursor-cli install-hooks-antigravity-cli install-hooks-hermes \
	test clean

HELP_MSG = "Usage: make [target]\n\n\
Targets:\n\
  build-pub                 Build Go publisher daemon\n\
  run-pub                   Run Go publisher daemon locally\n\
  build-sub                 Build Go MQTT subscriber example (cmd/kitteye_sub_example)\n\
  run-sub                   Run MQTT subscriber example (shares config.yaml)\n\
  build-mock                Build Go mock state generator (cmd/kitteye_mock)\n\
  run-mock                  Run MQTT mock state demo (shares config.yaml)\n\
  demo                      Shortcut for demo-mqtt (runs loop demo)\n\
  demo-mqtt                 Run MQTT broker mock state demo\n\
  demo-serial               Run USB serial port direct hardware demo\n\
  build-mcu                 Build both ESP32-C3 and ESP32-S2 Rust firmwares\n\
  build-mcu-c3              Build ESP32-C3 Rust firmware\n\
  build-mcu-s2              Build ESP32-S2 Rust firmware\n\
  flash-mcu-c3              Flash ESP32-C3 firmware via cargo espflash\n\
  flash-mcu-s2              Flash ESP32-S2 firmware via cargo espflash (--no-stub)\n\
  flash-mcu                 Show usage for flashing by chip\n\
  install-hooks             Install hooks for all CLIs (requires jq)\n\
  install-hooks-claude      Install Claude Code hooks only\n\
  install-hooks-codex       Install Codex hooks only\n\
  install-hooks-cursor-cli  Install Cursor CLI hooks only\n\
  install-hooks-antigravity-cli  Install Antigravity CLI hooks only\n\
  install-hooks-hermes      Install Hermes Agent hooks only\n\
  uninstall-hooks           Remove kitt-eye hooks from CLI settings + bins\n\
  test                      Run test suite across Go and both Rust components\n\
  clean                     Clean build artifacts\n"

MERGE := ./hooks/common/merge_hooks.sh

help:
	@echo $(HELP_MSG)

build-pub:
	@echo "==> Building Go publisher..."
	cd pub-go && go build -o bin/kitt-eye-pub ./cmd/kitteye

run-pub: build-pub
	@echo "==> Starting Go publisher daemon..."
	./pub-go/bin/kitt-eye-pub --config config.yaml

build-sub:
	@echo "==> Building Go MQTT subscriber example..."
	cd pub-go && go build -o bin/kitt-eye-sub ./cmd/kitteye_sub_example

run-sub: build-sub
	@echo "==> Starting Go MQTT subscriber example..."
	./pub-go/bin/kitt-eye-sub --config config.yaml

build-mock:
	@echo "==> Building Go mock state generator..."
	cd pub-go && go build -o bin/kitt-eye-mock ./cmd/kitteye_mock

run-mock: build-mock
	@echo "==> Starting MQTT mock state demo..."
	./scripts/demo-mqtt/mock_state.sh

demo: run-mock
demo-mqtt: run-mock

demo-serial:
	@echo "==> Starting USB serial direct demo..."
	./scripts/demo-serial/demo.sh

build-mcu: build-mcu-c3 build-mcu-s2

build-mcu-c3:
	@echo "==> Building ESP32-C3 Rust firmware..."
	cd mcu-rust-esp32c3 && cargo build --release

build-mcu-s2:
	@echo "==> Building ESP32-S2 Rust firmware..."
	cd mcu-rust-esp32s2 && ([ -f $(HOME)/export-esp.sh ] && . $(HOME)/export-esp.sh; cargo build --release)

flash-mcu:
	@echo "Please specify target chip:"
	@echo "  make flash-mcu-c3   # Flash ESP32-C3 firmware"
	@echo "  make flash-mcu-s2   # Flash ESP32-S2 firmware (--no-stub)"

flash-mcu-c3:
	@echo "==> Flashing ESP32-C3 Rust firmware..."
	cd mcu-rust-esp32c3 && cargo espflash flash --release --monitor

flash-mcu-s2:
	@echo "==> Flashing ESP32-S2 Rust firmware..."
	cd mcu-rust-esp32s2 && ([ -f $(HOME)/export-esp.sh ] && . $(HOME)/export-esp.sh; cargo espflash flash --release --monitor --no-stub)

install-hooks: install-hooks-claude install-hooks-codex install-hooks-cursor-cli install-hooks-antigravity-cli install-hooks-hermes
	@echo "==> All CLI hooks installed."

install-hooks-claude:
	@echo "==> Installing claude hooks..."
	@$(MERGE) install claude

install-hooks-codex:
	@echo "==> Installing codex hooks..."
	@$(MERGE) install codex

install-hooks-cursor-cli:
	@echo "==> Installing cursor-cli hooks..."
	@$(MERGE) install cursor-cli

install-hooks-antigravity-cli:
	@echo "==> Installing antigravity-cli hooks..."
	@$(MERGE) install antigravity-cli

install-hooks-hermes:
	@echo "==> Installing hermes hooks..."
	@$(MERGE) install hermes

uninstall-hooks:
	@echo "==> Uninstalling CLI hooks..."
	@$(MERGE) uninstall all

test:
	@echo "==> Testing Go publisher..."
	cd pub-go && go test ./...
	@echo "==> Testing MCU common logic..."
	cd mcu-rust-common && cargo test
	@echo "==> Checking ESP32-C3 Rust firmware..."
	cd mcu-rust-esp32c3 && cargo check --release
	@echo "==> Checking ESP32-S2 Rust firmware..."
	cd mcu-rust-esp32s2 && cargo check --release

clean:
	rm -rf pub-go/bin mcu-rust-common/target mcu-rust-esp32c3/target mcu-rust-esp32s2/target
