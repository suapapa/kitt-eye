# KITT-EYE (AI Agent Status Indicator via MQTT & ESP32-C3)

## 1. 프로젝트 개요
`kitt-eye`는 다양한 AI 에이전트 CLI(`antigravity-cli`, `codex`, `cursor-cli`)의 라이프사이클 훅(Hook)을 감지하여 에이전트의 현재 동작 상태(Thinking, Coding, Tool Execution, Waiting Input 등)를 수집하고, Go 언어로 작성된 Publisher를 통해 MQTT 브로커(Home Assistant Mosquitto)로 발행(Publish)한 뒤, ESP32-C3 기반 MCU(Rust 작성)에서 구독(Subscribe)하여 LED(K.I.T.T. 스캐너 효과) 또는 디스플레이로 시각화하는 모니터링 시스템입니다.

---

## 2. 시스템 아키텍처

```text
[ AI CLI Tools ]
 ├── antigravity-cli (Hook / Wrapper)
 ├── codex           (Hook / Wrapper)
 └── cursor-cli      (Hook / Wrapper)
         │
         │ (Unix Domain Socket / IPC / CLI call)
         ▼
[ kitt-eye Publisher (Go Daemon) ]
         │
         │ (MQTT Pub: kitt-eye/agents/<agent>/state)
         ▼
[ HomeLab MQTT Broker (Home Assistant Mosquitto) ]
         │
         │ (MQTT Sub: kitt-eye/#)
         ▼
[ ESP32-C3 Device (Rust / esp-idf-svc) ]
 ├── WS2812B NeoPixel RGB LED (K.I.T.T. 스캐너 / 상태 애니메이션)
 └── (옵션) I2C OLED / LCD 디스플레이 (에이전트 이름 및 상태 텍스트 표시)
```

---

## 3. 디렉토리 구조 계획

```text
kitt-eye/
├── Makefile                       # 훅 설치, Go 빌드, Rust 플래시 통합 관리
├── config.example.yaml            # MQTT 브로커 연결 정보 및 토픽 설정
├── PLAN.md                        # 상세 구현 로드맵 및 프로토콜 규격
├── hooks/                         # 각 CLI별 훅 스크립트 및 래퍼
│   ├── antigravity-cli/
│   │   └── hook.sh
│   ├── codex/
│   │   └── hook.sh
│   ├── cursor-cli/
│   │   └── hook.sh
│   └── common/
│       └── send_event.sh          # Go 데몬에 상태를 전송하는 공통 전송 유틸리티
├── pub-go/                        # Go Publisher 데몬
│   ├── go.mod
│   ├── go.sum
│   ├── cmd/
│   │   └── kitteye/
│   │       └── main.go
│   └── internal/
│       ├── config/                # YAML/환경변수 설정 로더
│       ├── ipc/                   # Unix Domain Socket 서버 (훅 이벤트 수신)
│       ├── mqtt/                  # paho.mqtt.golang 클라이언트 (재연결, LWT 지원)
│       └── model/                 # 에이전트 상태 이벤트 데이터 구조체
└── mcu-rust/                      # ESP32-C3 Rust 펌웨어
    ├── Cargo.toml
    ├── .cargo/
    │   └── config.toml            # riscv32imc-esp-espidf 타깃 빌드 설정
    ├── sdkconfig.defaults         # ESP-IDF Wi-Fi/NVS/FreeRTOS 기본 설정
    └── src/
        ├── main.rs                # 앱 진입점 및 메인 이벤트 루프
        ├── wifi.rs                # Wi-Fi 연결 매니저
        ├── mqtt.rs                # esp-idf-svc 기반 MQTT 구독 핸들러
        ├── state.rs               # 에이전트 상태 파싱 및 우선순위 큐
        └── led/
            ├── mod.rs
            ├── ws2812.rs          # RMT 기반 WS2812B NeoPixel 제어
            └── patterns.rs        # K.I.T.T. 스캐너, 호흡(Breathing), 점멸 효과
```

---

## 4. 세부 컴포넌트 설계

### 4.1. Hook 인터페이스 & Makefile
1. **상태 정의 (Agent State Spec)**
   - `idle`: 대기 상태
   - `thinking`: 프롬프트 분석 및 추론 중
   - `generating`: 코드/응답 스트리밍 생성 중
   - `executing_tool`: 쉘 명령, 파일 읽기/쓰기, MCP 도구 실행 중
   - `waiting_input`: 사용자 확인/입력 대기 중
   - `error`: 오류 발생
   - `done`: 태스크 완료 (일정 시간 후 idle 복귀)

2. **이벤트 전송 프로토콜**
   - 각 훅 스크립트는 가볍고 지연 시간이 없도록 로컬 Unix Domain Socket (`/tmp/kitt-eye.sock`)에 JSON 데이터그램을 전송합니다.
   ```json
   {
     "agent": "cursor-cli",
     "state": "executing_tool",
     "detail": "Running git diff",
     "timestamp": "2026-09-07T11:26:00Z"
   }
   ```
3. **Makefile 설치 자동화**
   - `make install-hooks`:
     - CLI별 훅 디렉토리(또는 래퍼 스크립트 경로)에 심볼릭 링크 또는 래퍼를 구성.
     - PATH 우선순위를 이용한 래퍼 스크립트 등록 또는 공식 hook 확장 메커니즘 연동.
   - `make uninstall-hooks`: 설치된 훅/래퍼 원복.

### 4.2. Go Publisher (`pub-go`)
1. **핵심 기능**
   - **상시 연결 유지**: Home Assistant의 Mosquitto 브로커와 지속적인 MQTT 세션 유지 (매 CLI 호출마다 발생하는 핸드셰이크 오버헤드 방지).
   - **IPC 서버**: `/tmp/kitt-eye.sock`을 열어 훅 스크립트의 빠른 비동기 신호 수신.
   - **LWT (Last Will and Testament)**: 데몬 비정상 종료 시 `kitt-eye/system/status = offline` 자동 발행.
   - **토픽 구조**:
     - `kitt-eye/agents/<agent_name>/state`: 개별 에이전트 상태
     - `kitt-eye/active_state`: 현재 가장 활발한 에이전트의 대표 상태 (MCU 연동용)
     - `homeassistant/sensor/kitt_eye_.../config`: (선택 사항) HA MQTT Discovery 엔티티 자동 등록.

### 4.3. MCU ESP32-C3 Rust 펌웨어 (`mcu-rust`)
1. **기술 스택**
   - **프레임워크**: `esp-idf-svc` (안정적인 Wi-Fi, TLS/TCP 소켓 및 `EspMqttClient` 제공)
   - **타깃 아키텍처**: `riscv32imc-esp-espidf` (ESP32-C3 RISC-V 코어)
   - **LED 드라이버**: `smart-leds` + `esp-idf-hal::rmt` (WS2812B 스트립/단일 LED 구동)
2. **동작 시나리오**
   - Wi-Fi 부팅 및 HA Mosquitto 브로커 접속 (`kitt-eye/#` 토픽 구독).
   - 수신된 에이전트 상태에 따라 비동기 FreeRTOS 태스크에서 LED 패턴 렌더링:
     - **K.I.T.T. Scanner (Red 좌우 왕복)**: `executing_tool` (도구 실행 중)
     - **Breathing (Cyan/Blue 서서히 밝아졌다 어두워짐)**: `idle` (대기 상태)
     - **Pulsing (Amber/Purple 점멸)**: `thinking` (추론 중)
     - **Solid Green / Rapid Green Flash**: `done` (완료 알림)
     - **Blinking Red**: `error` (실패 알림)

---

## 5. 단계별 개발 로드맵

1. **Step 1: 환경 구성 및 기본 디렉토리 셋업**
   - 저장소 구조 초기화 및 설정 파일 포맷 확정
2. **Step 2: Go Publisher 구현 및 MQTT 연동 테스트**
   - IPC 소켓 리스너, MQTT 퍼블리셔 구현
   - Mosquitto 브로커 연동 및 `mosquitto_sub`로 발행 검증
3. **Step 3: CLI Hooks & Makefile 작성**
   - 각 CLI (`antigravity-cli`, `codex`, `cursor-cli`)의 후킹 방식 분석 및 스크립트 작성
   - `make install-hooks` 타깃 구현
4. **Step 4: ESP32-C3 Rust 펌웨어 작성**
   - `esp-idf-template` 기반 프로젝트 구성
   - Wi-Fi 및 MQTT 클라이언트 구독 로직 작성
   - WS2812B LED 애니메이션 제어 루프 구현
5. **Step 5: 통합 테스트 & Home Assistant 연동**
   - CLI 실행 -> Hook -> Go Pub -> Mosquitto -> ESP32-C3 LED 반응 실시간 검증
   - Home Assistant 대시보드 상태 카드 연동
