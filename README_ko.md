# kitt-eye 🚗💨

[English](README.md) | [한국어](README_ko.md)

> AI 코딩 에이전트의 상태를 MQTT로 수집하여, ESP32-S2 기반 **K.I.T.T. 스캐너 LED 바**(및 Home Assistant)로 시각화하는 홈랩 모니터링 시스템

여러 터미널에서 `claude`, `codex`, `cursor-cli`, `antigravity-cli`, `hermes` 같은 AI 에이전트를 동시에 실행해 두면, 지금 모델이 생각 중인지, 도구를 실행 중인지, 혹은 사용자의 입력/승인을 기다리고 있는지 터미널 창을 일일이 열어보지 않고는 알기 어렵습니다.

**kitt-eye**는 에이전트의 라이프사이클 훅(Hook)을 감지하여 방 건너편에서도 데스크 위의 LED 바 하나로 현재 상태를 한눈에 파악할 수 있게 해줍니다.

---

## 🌟 주요 특징

- **다양한 AI CLI 지원**: Claude Code, OpenAI Codex CLI, Cursor CLI, Antigravity CLI(`agy`), Hermes Agent 공식 훅 지원.
- **2가지 LED 모션 테마**:
  - **`classic`**: 전설적인 '전격 Z작전(Knight Rider)'의 K.I.T.T. 오리지널 레드(🔴) 단색 테마. 모션 패턴으로 상태를 구분합니다.
  - **`colorful`**: 상태별 고유한 생생한 색상(앰버 K.I.T.T. 스캐너, 사이언 호흡, 옐로우 점멸 등)으로 직관적인 상태 식별.
- **경량 & 제로 오버헤드**:
  - CLI 훅은 Unix Domain Socket (`/tmp/kitt-eye.sock`)으로 한 줄 JSON만 던지고 즉시 종료 (비차단 fire-and-forget, CLI 동작에 영향 제로).
- **스마트 세션 집계**:
  - 멀티 터미널/멀티 에이전트 동시 실행 시 엄격한 우선순위(`error` > `waiting_input` > `executing_tool` > ...)로 대표 상태 선별.
  - 자동 TTL 만료(`done`, `error` 완료 후 `idle` 복귀, 비정상 종료 세션 자동 정리).
- **Home Assistant & MQTT 친화적**:
  - MQTT QoS 1 Retained 메시지 발행 (MCU가 늦게 켜져도 즉시 현재 상태 수신).
  - LWT(Last Will and Testament)로 데몬 오프라인 상태 자동 감지.
  - Home Assistant MQTT Discovery 자동 센서 등록 지원.
- **초경량 임베디드 펌웨어**:
  - ESP32-S2 및 ESP32-C3 베어메탈 Rust (`esp-hal` + `embassy`), WS2812B NeoPixel 8구 LED 바 제어.
  - 온칩 USB 시리얼 CLI(USB-OTG CDC-ACM / USB-Serial-JTAG)를 통한 실시간 Wi-Fi/MQTT 설정, 테마 전환, 직접 LED 상태 제어 지원.

---

## 🚦 상태 스펙 & LED 모션 테마

총 7가지 상태를 표현하며, 여러 세션이 동작 중일 때는 다음 우선순위에 따라 가장 중요한 상태가 LED에 표시됩니다:
```text
error > waiting_input > executing_tool > generating > thinking > done > idle
```

| 상태 | 의미 | `classic` (오리지널 🔴 단색) | `colorful` (생생한 컬러 & 모션) |
|---|---|---|---|
| `idle` | 대기 중 | 느린 K.I.T.T. 좌우 왕복 (Slow Scanner) | 🔴 Classic Red 느린 좌우 왕복 |
| `thinking` | 프롬프트 분석 / 추론 중 | 중앙→양끝 확산 (Center-Out) | 🟣 Mystic Purple 중앙 확산 |
| `generating` | 응답 / 코드 스트리밍 중 | 채움 웨이브 (Fill Sweep) | 🟢 Mint Emerald 채움 웨이브 |
| `executing_tool` | 쉘/파일/MCP 도구 실행 중 | **빠른 K.I.T.T. 좌우 왕복 스캐너** | 🟠 Vivid Amber **빠른 K.I.T.T. 스캐너** |
| `waiting_input` | **사용자 확인 / 입력 대기** | 전체 동기 점멸 (Blink) | 🟡 Warning Yellow 동기 점멸 |
| `done` | 태스크 완료 | 플래시 후 점등 (Flash → Solid) | 🟢 Spring Lime 플래시 후 점등 |
| `error` | 도구 실패 / 오류 | 한 방향 혜성 (Comet) | 🔴 Crimson Red 한 방향 혜성 |

> `done`과 `error`는 설정된 TTL(기본: 15초, 60초)이 지나면 자동으로 `idle`로 복귀합니다.

---

## 🏗️ 시스템 아키텍처

```text
[ AI CLI Tools ]
 ├── claude          (~/.claude/settings.json 훅)
 ├── codex           (~/.codex/hooks.json 훅)
 ├── cursor-cli      (~/.cursor/hooks.json 훅)
 ├── antigravity-cli (~/.gemini/config/hooks.json 훅)
 └── hermes          (~/.hermes/config.yaml 훅)
         │
         │  Unix Domain Socket (/tmp/kitt-eye.sock)
         │  JSON 단일 데이터그램 (Fire-and-Forget, stdout 무출력)
         ▼
[ kitt-eye Publisher (Go 데몬) ]
   · 멀티 세션 추적 + TTL 자동 회수 + 우선순위 집계
   · MQTT QoS 1 Retained 발행 + LWT
         │
         │  MQTT (kitt-eye/active_state 등)
         ▼
[ HomeLab MQTT Broker (Home Assistant Mosquitto) ]
         │
         ├──────────────────────────────────────┐
         ▼                                      ▼
[ ESP32-S2 LED 바 (Rust / Embassy) ]   [ Home Assistant ]
 └── WS2812B NeoPixel 8구 K.I.T.T. 스캐너    └── 센서 대시보드 (자동 Discovery)
```

---

## 🔌 하드웨어 구성 & 배선

> 보드별 상세 핀아웃 다이어그램과 트러블슈팅은 [하드웨어 연결 가이드](docs/hardware_wiring.md)를 참고하세요.

### 준비물
- **ESP32-S2 또는 ESP32-C3 개발 보드** (S2 Mini, ESP32-C3-DevKit, NodeMCU 등)
- **WS2812B NeoPixel 8구 LED 바** (또는 LED 스트립/링, 기본 8구 설정)
- 점퍼 케이블 및 USB-C 데이터 케이블

### 배선도

| 핀 | ESP32-S2 | ESP32-C3 | WS2812B LED 바 핀 | 비고 |
|---|---|---|---|---|
| **전원** | **5V** (또는 3V3) | **5V** (또는 3V3) | **VCC / 5V** | 8구 기준 3.3V로도 동작 가능하나 5V 권장 |
| **GND** | **GND** | **GND** | **GND** | 공통 접지 |
| **DIN** | **GPIO6** | **GPIO6** | **DIN (Data In)** | SPI2 MOSI 신호선으로 고속 데이터 전송 |

*(참고: GPIO4는 내부 SPI Master 클록 초기화용으로 예약되어 있으나 배선은 필요하지 않습니다.)*

---

## 🚀 빠른 시작

### 1. 요구 사항

- **Go 1.25+** (데몬 빌드용)
- **`jq`** (CLI 훅 JSON 처리 및 설치용 — **필수**)
  - macOS: `brew install jq`
  - Ubuntu/Debian: `sudo apt install jq`
- **`nc` (netcat)** (Unix 소켓 전송용, macOS/Linux 기본 포함)
- **Rust toolchain** (ESP32-S2 펌웨어 빌드용)
  - `espup install` (또는 `esp` 툴체인)
  - `cargo install espflash`

---

### 2. 브로커 없이 로컬에서 바로 체험하기 (Dry-Run 모드)

MQTT 브로커나 ESP32 하드웨어가 없어도 터미널에서 전체 파이프라인의 동작을 확인할 수 있습니다:

```bash
# 1. 설정 템플릿 복사
cp config.example.yaml config.yaml
# config.yaml의 mqtt.broker가 ""(빈 값)이면 자동으로 Dry-Run(stdout 출력) 모드로 동작합니다.

# 2. Go Publisher 데몬 실행
make run-pub
# -> {"type":"status","status":"online"} 출력과 함께 UDS 소켓 대기 시작
```

새 터미널을 열고 직접 이벤트를 보내봅니다:
```bash
./hooks/common/send_event.sh claude executing_tool "Bash: go test ./..."
./hooks/common/send_event.sh claude done "All tests passed"
```
데몬이 실행 중인 터미널에 실시간으로 집계된 상태가 JSON으로 출력되는 것을 확인할 수 있습니다.

---

### 3. MQTT 브로커 연결 (Home Assistant 등)

`config.yaml`에 MQTT 브로커 정보를 설정합니다:

```yaml
mqtt:
  broker: "homeassistant.local"   # 브로커 주소 (IP 또는 도메인)
  port: 1883
  username: "mqtt_user"
  topic_prefix: "kitt-eye"

homeassistant:
  discovery: true                 # Home Assistant 센서 자동 등록
```

비밀번호는 환경변수로 지정하는 것을 권장합니다:
```bash
export KITTEYE_MQTT_PASSWORD='your_password'
make run-pub
```

발행되는 토픽은 내장 구독 유틸리티로 즉시 확인할 수 있습니다:
```bash
make run-sub
# 또는 mosquitto_sub 사용:
# mosquitto_sub -h homeassistant.local -v -t 'kitt-eye/#'
```

---

### 3a. 백그라운드 사용자 서비스로 상시 실행 (Linux systemd 및 macOS launchd)

터미널 창을 계속 띄워둘 필요 없이, `kitt-eye-pub`을 백그라운드 사용자 서비스로 등록하여 상시 실행할 수 있습니다. Makefile이 **macOS**(`launchd`)와 **Linux**(`systemd`)를 자동으로 감지하여 알맞게 등록합니다:

```bash
# 1. 서비스 빌드, 설치 및 시작
make install-service

# 2. 서비스 상태 확인
make status-service    # macOS: launchctl list com.suapapa.kitt-eye-pub
                       # Linux: systemctl --user status kitt-eye-pub.service

# 3. 실시간 로그 확인
make logs-service      # macOS: tail -f ~/Library/Logs/kitt-eye-pub.log
                       # Linux: journalctl --user -u kitt-eye-pub.service -f

# 4. 서비스 재시작
make restart-service
```

- 실행 바이너리: `~/.local/bin/kitt-eye-pub`
- 설정 파일: `~/.config/kitt-eye/config.yaml`
- 환경변수 설정: `~/.config/kitt-eye/env` (예: `KITTEYE_MQTT_PASSWORD=...`)
- 서비스 정의 파일:
  - macOS: `~/Library/LaunchAgents/com.suapapa.kitt-eye-pub.plist`
  - Linux: `~/.config/systemd/user/kitt-eye-pub.service`

> [!TIP]
> **Linux 팁**: SSH 세션을 종료하거나 시스템 부팅 후 로그인하지 않아도 데몬이 계속 실행되도록 하려면 링거(linger)를 활성화하세요:
> ```bash
> loginctl enable-linger
> ```

서비스를 중지하고 삭제하려면:
```bash
make uninstall-service
```

---

### 3b. 하드웨어 및 상태 데모 (AI Agent 훅 연동 없이 테스트)

실제 AI CLI 에이전트 작업을 기다리지 않고 ESP32 LED 바의 애니메이션이나 Home Assistant 연동을 즉시 시연/테스트할 수 있는 두 가지 데모 환경을 제공합니다:

```bash
# 1. USB 시리얼 직접 제어 데모 (브로커나 Wi-Fi 불필요!)
make demo-serial                         # USB 시리얼 포트 직접 데모
./scripts/demo-serial/demo.sh -m interactive # 대화형 키보드 메뉴 [1-7, a, t, s, q]
./scripts/demo-serial/demo.sh thinking   # 특정 상태 단독 1회 즉시 지정

# 2. MQTT 브로커 모의 상태 데모 (전체 네트워크 스택 테스트)
make demo-mqtt                           # 7개 전체 상태 자동 순환 데모
./scripts/demo-mqtt/demo.sh -m workflow  # 실제 AI 페어 프로그래밍 시나리오 데모
./scripts/demo-mqtt/demo.sh -m interactive # 숫자 키(1~7)로 실시간 상태 변경 메뉴
./scripts/demo-mqtt/demo.sh thinking     # 특정 상태 단독 1회 발행
```

자세한 옵션과 사용법은 [`scripts/README.md`](scripts/README.md), [`scripts/demo-serial/README.md`](scripts/demo-serial/README.md), 및 [`scripts/demo-mqtt/README.md`](scripts/demo-mqtt/README.md)를 참고하세요.

---

### 4. CLI 훅 설치

단 한 번의 명령어로 사용 중인 모든 AI 코딩 도구에 kitt-eye 훅을 멱등(Idempotent)하게 등록합니다:

```bash
make install-hooks
```
*(개별 설치: `make install-hooks-claude`, `make install-hooks-codex`, `make install-hooks-cursor-cli`, `make install-hooks-antigravity-cli`, `make install-hooks-hermes`)*

#### 도구별 설정 파일 및 확인 사항
| CLI 도구 | 설정 파일 위치 | 설치 후 확인 방법 |
|---|---|---|
| **Claude Code** | `~/.claude/settings.json` | 세션에서 `/hooks` 실행하여 등록 확인 |
| **OpenAI Codex** | `~/.codex/hooks.json` | 세션에서 `/hooks` 열고 **kitt-eye 훅 승인(Trust)** |
| **Cursor CLI** | `~/.cursor/hooks.json` | 파일 저장 즉시 반영 (Cursor IDE와 설정 공유) |
| **Antigravity CLI** | `~/.gemini/config/hooks.json` | 워크스페이스 신뢰(trust) 상태 확인 |
| **Hermes Agent** | `~/.hermes/config.yaml` | `hermes hooks list` 실행하여 등록 확인 |

> 훅 제거 시: `make uninstall-hooks` (다른 개인 훅은 안전하게 보존하고 kitt-eye 훅만 깔끔하게 제거합니다).

---

### 5. MCU 펌웨어 빌드 & 플래시 (ESP32-C3 / ESP32-S2)

사용 중인 MCU 보드 디렉터리에서 `.env` 설정을 완료합니다:

```bash
# ESP32-C3 사용 시:
cd mcu-rust-esp32c3 && cp .env.example .env

# ESP32-S2 사용 시:
cd mcu-rust-esp32s2 && cp .env.example .env

# .env 파일을 열고 Wi-Fi 및 MQTT 정보를 입력합니다:
# SSID="MyWiFi"
# PASS="MyPassword"
# MQTT_BROKER="homeassistant.local"
# MOTION_THEME="colorful"   # 'classic' 또는 'colorful'
# LED_REVERSE="false"       # 'true' 설정 시 LED 좌우 방향 반전

# 칩에 맞는 타깃으로 빌드 및 플래시:
make flash-mcu-c3   # ESP32-C3 보드 플래시
# 또는
make flash-mcu-s2   # ESP32-S2 보드 플래시 (--no-stub 자동 적용)
```

이제 AI 에이전트에게 코딩을 시키면 데스크 위의 LED 바가 실시간으로 화려하게 반응합니다!

---

### 6. USB 시리얼 CLI 설정 및 직접 LED 제어

**ESP32-S2**(내장 USB-OTG Full-Speed CDC-ACM) 및 **ESP32-C3**(내장 USB-Serial-JTAG)는 온칩 USB 포트를 통한 대화형 시리얼 CLI를 제공합니다. 펌웨어를 다시 빌드하거나 플래시하지 않고도 Wi-Fi 및 MQTT 브로커 설정을 런타임에 변경하고, 테마를 전환하거나 LED 상태를 직접 수동으로 테스트할 수 있습니다.

#### 시리얼 터미널 연결

보드를 USB 데이터 케이블로 PC에 연결한 후, **115200 baud** 속도로 시리얼 터미널을 엽니다:

```bash
# macOS
picocom -b 115200 /dev/tty.usbmodem*
# 또는 screen 사용:
screen /dev/tty.usbmodem* 115200

# Linux
picocom -b 115200 /dev/ttyACM0

# 또는 cargo espflash 모니터 사용 (두 보드 공통)
cargo espflash monitor
```

`Enter` 키를 누르면 대화형 프롬프트가 표시됩니다:
```text
kitt-eye> 
```

#### CLI 명령어 목록

| 명령어 | 설명 | 사용 예시 |
|---|---|---|
| `help` 또는 `?` | 전체 명령어 도움말 및 사용법 출력 | `help` |
| `status` | 시스템 상태 종합 확인 (Wi-Fi, IP, MQTT 연결, 현재 상태 및 모드, 테마) | `status` |
| `config` | 현재 런타임 설정값 확인 (SSID, 브로커, 토픽, 클라이언트 ID, 테마) | `config` |
| `wifi <SSID> <PASS>` | Wi-Fi 접속 정보 변경 (RAM 임시 반영) | `wifi MyHomeWiFi SecretPass` |
| `mqtt_broker <URL\|HOST> [PORT]` | MQTT 브로커 설정 (URL 포맷으로 ID/PW/포트 일괄 지정 또는 호스트/포트 지정) | `mqtt_broker mqtt://mqtt:896351@172.16.0.100`<br>`mqtt_broker 192.168.1.50 1883` |
| `mqtt_topic <PREFIX>` | MQTT 구독 토픽 접두어 변경 (기본값: `kitt-eye`) | `mqtt_topic my-kitt` |
| `mqtt-client_id <ID>` | MQTT 클라이언트 ID 변경 | `mqtt-client_id kitt-desk` |
| `theme <classic\|colorful>` | LED 모션 테마 **실시간 전환** (재부팅 없이 다음 프레임에 즉시 반영) | `theme colorful` |
| `reverse <on\|off>` | LED 바 좌우 방향 **실시간 반전** (재부팅 없이 다음 프레임에 즉시 반영) | `reverse on` |
| `state <NAME>` | **LED 상태 직접 수동 지정** (`state auto` 실행 전까지 무기한 고정 유지) | `state executing_tool` |
| `state auto` | 수동 오버라이드 해제 및 MQTT 브로커 실시간 이벤트 추적 복귀 | `state auto` |
| `save` | 현재 RAM 설정을 비휘발성 Flash 메모리에 영구 저장 | `save` |
| `load` | Flash에 저장된 설정을 다시 불러와 RAM에 적용 | `load` |
| `reset` | 컴파일 타임(`.env`) 기본값으로 설정 초기화 | `reset` |
| `reboot` | MCU 재부팅 (새로 저장된 Wi-Fi / MQTT 설정 적용) | `reboot` |

> [!TIP]
> **네트워크 및 MQTT 설정 적용 흐름**:
> Wi-Fi 및 브로커 변경 사항은 먼저 RAM에 임시 반영됩니다. 영구 저장 및 적용하려면:
> 1. 정보 설정: `wifi MySSID MyPass` 또는 `mqtt_broker mqtt://user:pass@192.168.1.100:1883`
> 2. Flash 저장: `save`
> 3. 재부팅: `reboot` (MCU가 재기동되며 새 설정으로 자동 연결됩니다)
>
> **직접 LED 애니메이션 테스트**:
> `state <NAME>` (`idle`, `thinking`, `generating`, `executing_tool`, `waiting_input`, `done`, `error`) 명령으로 AI 에이전트 작업 없이도 7가지 모션 애니메이션을 즉시 확인할 수 있습니다. 테스트가 끝나면 `state auto`를 입력하여 정상 모드로 복귀하세요.

---

## 📡 MQTT 토픽 규격

모든 상태 토픽은 **QoS 1, Retain** 플래그로 발행되어 네트워크가 재연결되거나 MCU가 늦게 켜져도 즉시 마지막 유효 상태를 유지합니다.

| 토픽 (기본 Prefix: `kitt-eye`) | 페이로드 예시 | 설명 |
|---|---|---|
| `{prefix}/active_state` | `executing_tool` | **전체 대표 상태 (MCU 구독용)** |
| `{prefix}/active_agent` | `claude` | 현재 가장 활발한 에이전트 이름 |
| `{prefix}/agents/{agent}/state` | `thinking` | 특정 에이전트의 대표 상태 |
| `{prefix}/agents/{agent}/sessions` | `{"agent":"claude",...}` | 세션별 상세 현황 (JSON) |
| `{prefix}/system/status` | `online` / `offline` | 데몬 생존 상태 (LWT 지원) |
| `homeassistant/sensor/kitt_eye_*/config` | `{...}` | Home Assistant 자동 센서 디스커버리 |

---

## 🛠️ 주요 Makefile 명령어

```bash
make help                       # 전체 타깃 도움말 출력
make build-pub                  # Go Publisher 빌드 -> pub-go/bin/kitt-eye-pub
make run-pub                    # Go Publisher 데몬 로컬 실행
make install-service            # Go publisher 사용자 서비스 등록 및 시작 (Linux systemd 또는 macOS launchd)
make uninstall-service          # Go publisher 사용자 서비스 중지 및 제거
make status-service             # 사용자 서비스 상태 확인
make restart-service            # 사용자 서비스 재시작
make logs-service               # 사용자 서비스 실시간 로그 확인
make build-sub                  # MQTT 테스트 구독자 빌드 -> pub-go/bin/kitt-eye-sub
make run-sub                    # MQTT 토픽 실시간 모니터링
make build-mcu                  # ESP32-C3 & S2 펌웨어 일괄 빌드
make build-mcu-c3               # ESP32-C3 Rust 펌웨어 빌드
make build-mcu-s2               # ESP32-S2 Rust 펌웨어 빌드
make flash-mcu-c3               # ESP32-C3 펌웨어 플래시 및 시리얼 모니터링
make flash-mcu-s2               # ESP32-S2 펌웨어 플래시 (--no-stub) 및 모니터링
make install-hooks              # 5개 CLI 훅 일괄 설치
make uninstall-hooks            # 5개 CLI 훅 일괄 제거
make test                       # 전체 테스트 (Go 단위 테스트 + C3/S2 Rust 체크)
make clean                      # 빌드 산출물 정리
```

---

## 📖 추가 문서

- [English Documentation](README.md)
- [PLAN.md](PLAN.md): 아키텍처 및 통신 프로토콜 상세 규격서
- [AGENTS.md](AGENTS.md): AI 코딩 어시스턴트를 위한 개발 가이드 및 제약 사항
- [docs/hardware_wiring.md](docs/hardware_wiring.md): 하드웨어 배선도 및 트러블슈팅 가이드
- [docs/esp32s2_usb_serial_plan.md](docs/esp32s2_usb_serial_plan.md): USB 시리얼 CLI 설계 및 아키텍처

---

## 📄 라이선스

[MIT License](./LICENSE) © 2026 Homin Lee <i@homin.dev>
