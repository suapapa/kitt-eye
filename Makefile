.PHONY: help build-pub run-pub build-sub run-sub build-mock run-mock demo demo-mqtt demo-serial config-serial \
	install-service install-user-service uninstall-service status-service restart-service logs-service log-service \
	build-mcu build-mcu-c3 build-mcu-s2 \
	flash-mcu flash-mcu-c3 flash-mcu-s2 \
	install-hooks uninstall-hooks \
	install-hooks-claude install-hooks-codex install-hooks-cursor-cli install-hooks-antigravity-cli install-hooks-hermes \
	test clean

UNAME_S := $(shell uname -s)

PREFIX ?= $(HOME)/.local
BINDIR ?= $(PREFIX)/bin
XDG_CONFIG_HOME ?= $(HOME)/.config
CONFIG_DIR ?= $(XDG_CONFIG_HOME)/kitt-eye
SYSTEMD_USER_DIR ?= $(XDG_CONFIG_HOME)/systemd/user
SERVICE_NAME := kitt-eye-pub.service

# macOS launchd configuration
LAUNCHD_USER_DIR ?= $(HOME)/Library/LaunchAgents
LAUNCHD_LABEL := com.suapapa.kitt-eye-pub
LAUNCHD_PLIST := $(LAUNCHD_LABEL).plist
LOG_DIR ?= $(HOME)/Library/Logs

HELP_MSG = "Usage: make [target]\n\n\
Targets:\n\
  build-pub                 Build Go publisher daemon\n\
  run-pub                   Run Go publisher daemon locally\n\
  install-service           Install & start Go publisher user service (Linux systemd or macOS launchd)\n\
  uninstall-service         Stop & remove Go publisher user service\n\
  status-service            Check user service status\n\
  restart-service           Restart user service\n\
  logs-service              Follow user service logs\n\
  build-sub                 Build Go MQTT subscriber example (cmd/kitteye_sub_example)\n\
  run-sub                   Run MQTT subscriber example (shares config.yaml)\n\
  build-mock                Build Go mock state generator (cmd/kitteye_mock)\n\
  run-mock                  Run MQTT mock state demo (shares config.yaml)\n\
  demo                      Shortcut for demo-mqtt (runs loop demo)\n\
  demo-mqtt                 Run MQTT broker mock state demo\n\
  demo-serial               Run USB serial port direct hardware demo\n\
  config-serial             Run USB serial configuration tool (interactive menu)\n\
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

install-service: build-pub
	@echo "==> Installing kitt-eye-pub binary to $(BINDIR)..."
	@mkdir -p "$(BINDIR)"
	@rm -f "$(BINDIR)/kitt-eye-pub"
	@cp -f pub-go/bin/kitt-eye-pub "$(BINDIR)/kitt-eye-pub"
	@chmod 0755 "$(BINDIR)/kitt-eye-pub"
	@echo "==> Setting up configuration in $(CONFIG_DIR)..."
	@mkdir -p "$(CONFIG_DIR)"
	@if [ ! -f "$(CONFIG_DIR)/config.yaml" ]; then \
		if [ -f config.yaml ]; then \
			echo "    Copied config.yaml -> $(CONFIG_DIR)/config.yaml"; \
			cp config.yaml "$(CONFIG_DIR)/config.yaml"; \
		else \
			echo "    Copied config.example.yaml -> $(CONFIG_DIR)/config.yaml"; \
			cp config.example.yaml "$(CONFIG_DIR)/config.yaml"; \
		fi; \
		chmod 0600 "$(CONFIG_DIR)/config.yaml"; \
	else \
		echo "    Preserved existing $(CONFIG_DIR)/config.yaml"; \
	fi
	@if [ ! -f "$(CONFIG_DIR)/env" ]; then \
		echo "# kitt-eye service environment variables" > "$(CONFIG_DIR)/env"; \
		echo "# KITTEYE_MQTT_PASSWORD=your_secret_password" >> "$(CONFIG_DIR)/env"; \
		echo "# KITTEYE_MQTT_BROKER=homeassistant.local" >> "$(CONFIG_DIR)/env"; \
		chmod 0600 "$(CONFIG_DIR)/env"; \
		echo "    Created template $(CONFIG_DIR)/env"; \
	fi
ifeq ($(UNAME_S),Darwin)
	@echo "==> Installing LaunchAgent to $(LAUNCHD_USER_DIR)..."
	@mkdir -p "$(LAUNCHD_USER_DIR)"
	@mkdir -p "$(LOG_DIR)"
	@sed -e 's|__BINDIR__|$(BINDIR)|g' \
	     -e 's|__CONFIG_DIR__|$(CONFIG_DIR)|g' \
	     -e 's|__LOG_DIR__|$(LOG_DIR)|g' \
	     -e 's|__HOME__|$(HOME)|g' \
	     launchd/$(LAUNCHD_PLIST) > "$(LAUNCHD_USER_DIR)/$(LAUNCHD_PLIST)"
	@echo "==> Loading $(LAUNCHD_LABEL) with launchctl..."
	@launchctl unload "$(LAUNCHD_USER_DIR)/$(LAUNCHD_PLIST)" 2>/dev/null || true
	@launchctl load -w "$(LAUNCHD_USER_DIR)/$(LAUNCHD_PLIST)"
	@echo ""
	@echo "kitt-eye publisher LaunchAgent successfully installed and started!"
	@echo "  Status : make status-service   (or launchctl list $(LAUNCHD_LABEL))"
	@echo "  Logs   : make logs-service     (or tail -f $(LOG_DIR)/kitt-eye-pub.log)"
	@echo "  Config : $(CONFIG_DIR)/config.yaml"
	@echo "  Env    : $(CONFIG_DIR)/env"
else ifeq ($(UNAME_S),Linux)
	@command -v systemctl >/dev/null 2>&1 || { \
		echo "Error: systemctl not found. This target requires Linux systemd." >&2; exit 1; \
	}
	@echo "==> Installing systemd user unit to $(SYSTEMD_USER_DIR)..."
	@mkdir -p "$(SYSTEMD_USER_DIR)"
	@sed -e 's|%h/.local/bin/kitt-eye-pub|$(subst $(HOME),%h,$(BINDIR))/kitt-eye-pub|g' \
	     -e 's|%h/.config/kitt-eye|$(subst $(HOME),%h,$(CONFIG_DIR))|g' \
	     systemd/$(SERVICE_NAME) > "$(SYSTEMD_USER_DIR)/$(SERVICE_NAME)"
	@echo "==> Reloading systemd daemon..."
	@systemctl --user daemon-reload
	@echo "==> Enabling and starting $(SERVICE_NAME)..."
	@systemctl --user enable $(SERVICE_NAME)
	@systemctl --user restart $(SERVICE_NAME)
	@echo ""
	@echo "kitt-eye publisher user service successfully installed and started!"
	@echo "  Status : make status-service   (or systemctl --user status $(SERVICE_NAME))"
	@echo "  Logs   : make logs-service     (or journalctl --user -u $(SERVICE_NAME) -f)"
	@echo "  Config : $(CONFIG_DIR)/config.yaml"
	@echo "  Env    : $(CONFIG_DIR)/env"
	@echo ""
	@echo "Tip: To keep user services running after logout / on boot without interactive login:"
	@echo "     loginctl enable-linger"
else
	@echo "Error: Unsupported operating system: $(UNAME_S)" >&2; exit 1;
endif

install-user-service: install-service

uninstall-service:
ifeq ($(UNAME_S),Darwin)
	@echo "==> Stopping and unloading $(LAUNCHD_LABEL)..."
	@launchctl unload -w "$(LAUNCHD_USER_DIR)/$(LAUNCHD_PLIST)" 2>/dev/null || true
	@echo "==> Removing LaunchAgent plist..."
	@rm -f "$(LAUNCHD_USER_DIR)/$(LAUNCHD_PLIST)"
	@echo "==> Removing binary $(BINDIR)/kitt-eye-pub..."
	@rm -f "$(BINDIR)/kitt-eye-pub"
	@echo "==> Service uninstalled."
	@echo "Note: Configuration at $(CONFIG_DIR) was preserved."
else ifeq ($(UNAME_S),Linux)
	@command -v systemctl >/dev/null 2>&1 || { \
		echo "Error: systemctl not found. This target requires Linux systemd." >&2; exit 1; \
	}
	@echo "==> Stopping and disabling $(SERVICE_NAME)..."
	@systemctl --user disable --now $(SERVICE_NAME) 2>/dev/null || true
	@echo "==> Removing systemd unit..."
	@rm -f "$(SYSTEMD_USER_DIR)/$(SERVICE_NAME)"
	@systemctl --user daemon-reload
	@echo "==> Removing binary $(BINDIR)/kitt-eye-pub..."
	@rm -f "$(BINDIR)/kitt-eye-pub"
	@echo "==> Service uninstalled."
	@echo "Note: Configuration at $(CONFIG_DIR) was preserved."
else
	@echo "Error: Unsupported operating system: $(UNAME_S)" >&2; exit 1;
endif

status-service:
ifeq ($(UNAME_S),Darwin)
	@echo "==> Checking LaunchAgent status for $(LAUNCHD_LABEL)..."
	@if launchctl list $(LAUNCHD_LABEL) >/dev/null 2>&1; then \
		launchctl list $(LAUNCHD_LABEL); \
		echo ""; \
		PID=$$(launchctl list $(LAUNCHD_LABEL) | awk '/"PID"/ {gsub(/[^0-9]/, ""); print}'); \
		if [ -n "$$PID" ]; then \
			echo "Status: Running (PID $$PID)"; \
		else \
			echo "Status: Loaded (Not currently running)"; \
		fi; \
	else \
		echo "Service $(LAUNCHD_LABEL) is not loaded."; \
		echo "Run 'make install-service' to install and start it."; \
	fi
else ifeq ($(UNAME_S),Linux)
	@systemctl --user status $(SERVICE_NAME)
else
	@echo "Error: Unsupported operating system: $(UNAME_S)" >&2; exit 1;
endif

restart-service:
ifeq ($(UNAME_S),Darwin)
	@echo "==> Restarting $(LAUNCHD_LABEL)..."
	@launchctl kickstart -k gui/$$(id -u)/$(LAUNCHD_LABEL) 2>/dev/null || { \
		launchctl unload "$(LAUNCHD_USER_DIR)/$(LAUNCHD_PLIST)" 2>/dev/null || true; \
		launchctl load -w "$(LAUNCHD_USER_DIR)/$(LAUNCHD_PLIST)"; \
	}
	@echo "Service restarted."
else ifeq ($(UNAME_S),Linux)
	@systemctl --user restart $(SERVICE_NAME)
else
	@echo "Error: Unsupported operating system: $(UNAME_S)" >&2; exit 1;
endif

logs-service:
ifeq ($(UNAME_S),Darwin)
	@if [ ! -f "$(LOG_DIR)/kitt-eye-pub.log" ]; then \
		echo "Creating $(LOG_DIR)/kitt-eye-pub.log..."; \
		mkdir -p "$(LOG_DIR)"; \
		touch "$(LOG_DIR)/kitt-eye-pub.log"; \
	fi
	@echo "==> Following $(LOG_DIR)/kitt-eye-pub.log (Ctrl+C to exit)..."
	@tail -f "$(LOG_DIR)/kitt-eye-pub.log"
else ifeq ($(UNAME_S),Linux)
	@journalctl --user -u $(SERVICE_NAME) -f
else
	@echo "Error: Unsupported operating system: $(UNAME_S)" >&2; exit 1;
endif

log-service: logs-service

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

config-serial:
	@echo "==> Starting USB serial configuration tool..."
	./scripts/config-serial/config.sh -i

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
