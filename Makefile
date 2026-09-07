.PHONY: help build-pub run-pub build-mcu flash-mcu \
	install-hooks uninstall-hooks \
	install-hooks-claude install-hooks-codex install-hooks-cursor-cli install-hooks-antigravity-cli \
	test clean

HELP_MSG = "Usage: make [target]\n\n\
Targets:\n\
  build-pub                 Build Go publisher daemon\n\
  run-pub                   Run Go publisher daemon locally\n\
  build-mcu                 Build ESP32-C3 Rust firmware\n\
  flash-mcu                 Flash ESP32-C3 firmware via cargo espflash\n\
  install-hooks             Install hooks for all CLIs (requires jq)\n\
  install-hooks-claude      Install Claude Code hooks only\n\
  install-hooks-codex       Install Codex hooks only\n\
  install-hooks-cursor-cli  Install Cursor CLI hooks only\n\
  install-hooks-antigravity-cli  Install Antigravity CLI hooks only\n\
  uninstall-hooks           Remove kitt-eye hooks from CLI settings + bins\n\
  test                      Run test suite across Go and Rust components\n\
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

build-mcu:
	@echo "==> Building ESP32-C3 Rust firmware..."
	cd mcu-rust && cargo build --release

flash-mcu:
	@echo "==> Flashing ESP32-C3 Rust firmware..."
	cd mcu-rust && cargo espflash flash --release --monitor

install-hooks: install-hooks-claude install-hooks-codex install-hooks-cursor-cli install-hooks-antigravity-cli
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

uninstall-hooks:
	@echo "==> Uninstalling CLI hooks..."
	@$(MERGE) uninstall all

test:
	@echo "==> Testing Go publisher..."
	cd pub-go && go test ./...
	@echo "==> Testing Rust firmware..."
	cd mcu-rust && cargo test --lib || true

clean:
	rm -rf pub-go/bin mcu-rust/target
