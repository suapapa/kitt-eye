.PHONY: help build-pub run-pub build-mcu flash-mcu install-hooks uninstall-hooks test clean

HELP_MSG = "Usage: make [target]\n\n\
Targets:\n\
  build-pub        Build Go publisher daemon\n\
  run-pub          Run Go publisher daemon locally\n\
  build-mcu        Build ESP32-C3 Rust firmware\n\
  flash-mcu        Flash ESP32-C3 firmware via cargo espflash\n\
  install-hooks    Install CLI hooks for antigravity-cli, codex, cursor-cli\n\
  uninstall-hooks  Remove installed CLI hooks\n\
  test             Run test suite across Go and Rust components\n\
  clean            Clean build artifacts\n"

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

install-hooks:
	@echo "==> Installing CLI hooks..."
	@mkdir -p $(HOME)/.kitt-eye/bin
	@cp -f hooks/common/send_event.sh $(HOME)/.kitt-eye/bin/kitt-eye-send
	@chmod +x $(HOME)/.kitt-eye/bin/kitt-eye-send
	@echo "Hooks installed. Ensure $(HOME)/.kitt-eye/bin is in your PATH or configured in your CLI tools."

uninstall-hooks:
	@echo "==> Uninstalling CLI hooks..."
	@rm -rf $(HOME)/.kitt-eye
	@echo "Hooks removed."

test:
	@echo "==> Testing Go publisher..."
	cd pub-go && go test ./...
	@echo "==> Testing Rust firmware..."
	cd mcu-rust && cargo test --lib || true

clean:
	rm -rf pub-go/bin mcu-rust/target
