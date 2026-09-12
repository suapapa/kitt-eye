# kitt-eye 🚗💨

[English](README.md) | [한국어](README_ko.md)

> AI 코딩 에이전트의 상태를 MQTT로 수집하여, ESP32-C3 기반 **K.I.T.T. 스캐너 LED 바**(및 Home Assistant)로 시각화하는 홈랩 모니터링 시스템

여러 터미널에서 `claude`, `codex`, `cursor-cli`, `antigravity-cli` 같은 AI 에이전트를 동시에 실행해 두면, 지금 모델이 생각 중인지, 도구를 실행 중인지, 혹은 사용자의 입력/승인을 기다리고 있는지 터미널 창을 일일이 열어보지 않고는 알기 어렵습니다.

**kitt-eye**는 에이전트의 라이프사이클 훅(Hook)을 감지하여 방 건너편에서도 데스크 위의 LED 바 하나로 현재 상태를 한눈에 파악할 수 있게 해줍니다.

---

## 🌟 주요 특징

- **다양한 AI CLI 지원**: Claude Code, OpenAI Codex CLI, Cursor CLI, Antigravity CLI(`agy`) 공식 훅 지원.
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
  - ESP32-C3 베어메탈 Rust (`esp-hal` + `embassy`), WS2812B NeoPixel 8구 LED 바 제어.

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
 └── antigravity-cli (~/.gemini/config/hooks.json 훅)
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
[ ESP32-C3 LED 바 (Rust / Embassy) ]   [ Home Assistant ]
 └── WS2812B NeoPixel 8구 K.I.T.T. 스캐너    └── 센서 대시보드 (자동 Discovery)
```

---

## 🔌 하드웨어 구성 & 배선

### 준비물
- **ESP32-C3 개발 보드** (SuperMini, NodeMCU 등 RISC-V 코어 보드)
- **WS2812B NeoPixel 8구 LED 바** (또는 LED 스트립/링, 기본 8구 설정)
- 점퍼 케이블 및 USB 케이블

### 배선도

| ESP32-C3 핀 | WS2812B LED 바 핀 | 비고 |
|---|---|---|
| **5V** (또는 3V3) | **VCC / 5V** | 8구 기준 3.3V로도 동작 가능하나 5V 권장 |
| **GND** | **GND** | 공통 접지 |
| **GPIO6** | **DIN (Data In)** | SPI2 MOSI 신호선으로 고속 데이터 전송 |

*(참고: GPIO4는 SPI Master 초기화용 SCK로 예약되어 있으나 배선은 필요하지 않습니다.)*

---

## 🚀 빠른 시작

### 1. 요구 사항

- **Go 1.25+** (데몬 빌드용)
- **`jq`** (CLI 훅 JSON 처리 및 설치용 — **필수**)
  - macOS: `brew install jq`
  - Ubuntu/Debian: `sudo apt install jq`
- **`nc` (netcat)** (Unix 소켓 전송용, macOS/Linux 기본 포함)
- **Rust toolchain** (ESP32-C3 펌웨어 빌드용)
  - `rustup target add riscv32imc-unknown-none-elf`
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

### 4. CLI 훅 설치

단 한 번의 명령어로 사용 중인 모든 AI 코딩 도구에 kitt-eye 훅을 멱등(Idempotent)하게 등록합니다:

```bash
make install-hooks
```
*(개별 설치: `make install-hooks-claude`, `make install-hooks-codex`, `make install-hooks-cursor-cli`, `make install-hooks-antigravity-cli`)*

#### 도구별 설정 파일 및 확인 사항
| CLI 도구 | 설정 파일 위치 | 설치 후 확인 방법 |
|---|---|---|
| **Claude Code** | `~/.claude/settings.json` | 세션에서 `/hooks` 실행하여 등록 확인 |
| **OpenAI Codex** | `~/.codex/hooks.json` | 세션에서 `/hooks` 열고 **kitt-eye 훅 승인(Trust)** |
| **Cursor CLI** | `~/.cursor/hooks.json` | 파일 저장 즉시 반영 (Cursor IDE와 설정 공유) |
| **Antigravity CLI** | `~/.gemini/config/hooks.json` | 워크스페이스 신뢰(trust) 상태 확인 |

> 훅 제거 시: `make uninstall-hooks` (다른 개인 훅은 안전하게 보존하고 kitt-eye 훅만 깔끔하게 제거합니다).

---

### 5. ESP32-C3 펌웨어 빌드 & 플래시

```bash
cd mcu-rust
cp .env.example .env

# .env 파일을 열고 Wi-Fi 및 MQTT 정보를 입력합니다:
# SSID="MyWiFi"
# PASS="MyPassword"
# MQTT_BROKER="homeassistant.local"
# MOTION_THEME="colorful"   # 'classic' 또는 'colorful'

# 펌웨어 빌드 및 플래시 (ESP32-C3를 USB로 연결한 후)
make flash-mcu
```

이제 AI 에이전트에게 코딩을 시키면 데스크 위의 LED 바가 실시간으로 화려하게 반응합니다!

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
make run-pub                    # Go Publisher 데몬 실행
make build-sub                  # MQTT 테스트 구독자 빌드 -> pub-go/bin/kitt-eye-sub
make run-sub                    # MQTT 토픽 실시간 모니터링
make build-mcu                  # ESP32-C3 Rust 펌웨어 빌드
make flash-mcu                  # ESP32-C3 펌웨어 플래시 및 시리얼 모니터링
make install-hooks              # 4개 CLI 훅 일괄 설치
make uninstall-hooks            # 4개 CLI 훅 일괄 제거
make test                       # 전체 테스트 (Go 단위 테스트 + Rust 체크)
make clean                      # 빌드 산출물 정리
```

---

## 📖 추가 문서

- [English Documentation](README.md)
- [PLAN.md](PLAN.md): 아키텍처 및 통신 프로토콜 상세 규격서
- [AGENTS.md](AGENTS.md): AI 코딩 어시스턴트를 위한 개발 가이드 및 제약 사항

---

## 📄 라이선스

[MIT License](./LICENSE) © 2026 Homin Lee <i@homin.dev>
