# KITT-EYE (Architecture & Protocol Specification)

## 1. 프로젝트 개요

`kitt-eye`는 다양한 AI 에이전트 CLI(`claude`, `codex`, `cursor-cli`, `antigravity-cli`)의 라이프사이클 훅(Hook)을 감지하여 에이전트의 현재 동작 상태(Thinking, Tool Execution, Waiting Input 등)를 수집하고, Go 언어로 작성된 Publisher를 통해 MQTT 브로커(Home Assistant Mosquitto)로 발행한 뒤, ESP32-C3 기반 MCU(Rust 작성)에서 구독하여 WS2812B LED(K.I.T.T. 스캐너 효과)로 시각화하는 홈랩 모니터링 시스템입니다.

---

## 2. 시스템 아키텍처

```text
[ AI CLI Tools ]
 ├── claude          (공식 Hooks: ~/.claude/settings.json)
 ├── codex           (공식 Hooks: ~/.codex/hooks.json + /hooks trust)
 ├── cursor-cli      (공식 Hooks: ~/.cursor/hooks.json)
 └── antigravity-cli (공식 Hooks: ~/.gemini/config/hooks.json named bundle)
         │
         │  Unix Domain Socket (/tmp/kitt-eye.sock)
         │  JSON 단일 데이터그램, fire-and-forget, stdout 무출력
         ▼
[ kitt-eye Publisher (Go Daemon) ]
   · 멀티 세션 추적 + TTL 만료 + 우선순위 집계
   · LWT (Last Will and Testament): 비정상 종료 시 offline 자동 발행
         │
         │  MQTT QoS 1 Retained: kitt-eye/#
         ▼
[ HomeLab MQTT Broker (Home Assistant Mosquitto) ]
         │
         ├──────────────────────────────────────┐
         ▼                                      ▼
[ ESP32-C3 Device (Rust / Embassy) ]   [ Home Assistant ]
 └── WS2812B NeoPixel K.I.T.T. 스캐너        └── MQTT Discovery 센서 (옵션)
```

---

## 3. 디렉토리 구조

```text
kitt-eye/
├── Makefile                 # 훅 설치, Go 빌드, Rust 체크/플래시 통합 관리
├── config.example.yaml      # MQTT 브로커 연결 정보 및 토픽 설정 템플릿
├── config.yaml              # 로컬 설정 (git-ignored 또는 사용자 설정)
├── PLAN.md                  # 아키텍처 및 상세 프로토콜 사양서 (본 문서)
├── README.md                # 영문 사용자 가이드
├── README_ko.md             # 한글 사용자 가이드
├── AGENTS.md                # AI 에이전트 작업 가이드 및 제약사항
├── hooks/                   # 각 CLI별 훅 스크립트 및 래퍼
│   ├── common/
│   │   ├── send_event.sh    # Go 데몬에 상태를 전송하는 공통 UDS 전송 유틸리티
│   │   └── merge_hooks.sh   # CLI 설정 JSON 멱등 병합/제거 유틸리티
│   ├── claude/              # Claude Code 훅 (hook.sh, settings.snippet.json)
│   ├── codex/               # Codex CLI 훅 (hook.sh, hooks.snippet.json)
│   ├── cursor-cli/          # Cursor CLI 훅 (hook.sh, hooks.snippet.json)
│   └── antigravity-cli/     # Antigravity CLI 훅 (hook.sh, hooks.snippet.json)
├── pub-go/                  # Go Publisher 데몬 (module: github.com/suapapa/kitt-eye/pub)
│   ├── cmd/kitteye/         # 진입점 (main.go)
│   ├── cmd/kitteye_sub_example/ # MQTT 구독 테스트 유틸리티
│   └── internal/
│       ├── config/          # YAML/환경변수 설정 로더
│       ├── ipc/             # Unix Domain Socket 서버 (/tmp/kitt-eye.sock)
│       ├── model/           # 에이전트 상태 이벤트 데이터 구조체 & 우선순위
│       ├── state/           # 세션 상태 저장소, TTL 관리, 대표 상태 집계기
│       ├── publish/         # 싱크 인터페이스 (MQTT & stdout dry-run)
│       └── mqtt/            # paho 기반 MQTT 클라이언트 (Retained, LWT, HA Discovery)
└── mcu-rust/                # ESP32-C3 Rust 펌웨어 (no_std, esp-hal + embassy)
    ├── Cargo.toml           # esp-hal, embassy, smart-leds, ws2812-spi
    ├── build.rs             # 컴파일 타임 .env 로더 및 링커 설정
    ├── .env.example         # Wi-Fi / MQTT / 테마 환경변수 템플릿
    └── src/
        ├── bin/main.rs      # Embassy 런타임 진입점, Wi-Fi/MQTT 태스크, LED 루프
        ├── config.rs        # build.rs에서 주입된 컴파일 타임 상수
        ├── mqtt.rs          # TcpSocket 기반 최소 비동기 MQTT 3.1.1 클라이언트
        ├── patterns.rs      # LedEngine 및 모션 패턴 렌더러
        ├── state.rs         # AgentState 열거형 및 MQTT 페이로드 파서
        └── theme.rs         # MotionTheme (classic vs colorful) 및 색상 매핑
```

---

## 4. 세부 컴포넌트 설계

### 4.1. Hook 인터페이스

1. **상태 정의 (Agent State Spec)**
   - `idle`: 대기 상태
   - `thinking`: 프롬프트 분석 및 추론 중
   - `generating`: 코드/응답 스트리밍 생성 중
   - `executing_tool`: 쉘 명령, 파일 읽기/쓰기, MCP 도구 실행 중
   - `waiting_input`: 사용자 확인/입력 대기 중
   - `done`: 태스크 완료 (TTL 후 idle 복귀)
   - `error`: 오류 발생 (TTL 후 idle 복귀)

2. **이벤트 전송 프로토콜 (IPC)**
   - 각 훅 스크립트는 가볍고 지연 시간이 없도록 로컬 Unix Domain Socket (`/tmp/kitt-eye.sock`)에 JSON 데이터그램을 한 줄로 전송합니다.
   - `session_id`, `event`는 멀티 세션 추적용 선택 필드입니다 (미전송 시 `default` 간주).
   ```json
   {
     "agent": "claude",
     "state": "executing_tool",
     "detail": "Bash: npm test",
     "timestamp": "2026-09-12T17:00:00Z",
     "session_id": "abc12345",
     "event": "PreToolUse"
   }
   ```

3. **Makefile 설치 자동화**
   - `make install-hooks`: 4개 CLI 모두 설정 파일 JSON 병합 방식으로 등록 (백업 + 멱등 병합).
     - `install-hooks-claude`: `~/.claude/settings.json` 병합.
     - `install-hooks-codex`: `~/.codex/hooks.json` 병합 (설치 후 `/hooks` 승인 필요).
     - `install-hooks-cursor-cli`: `~/.cursor/hooks.json` 병합.
     - `install-hooks-antigravity-cli`: `~/.gemini/config/hooks.json`에 `kitt-eye` named bundle 병합.
   - `make uninstall-hooks`: 각 설정 파일에서 kitt-eye 훅만 선별 제거하고 기존 사용자 훅은 보존.

---

### 4.2. Go Publisher (`pub-go`)

1. **핵심 기능**
   - **상시 연결 유지**: Mosquitto 브로커와 지속적인 MQTT 세션 유지 (매 CLI 호출 핸드셰이크 오버헤드 방지).
   - **IPC 서버**: `/tmp/kitt-eye.sock`에서 비동기 신호 수신 (1024바이트 제한, 300ms 소켓 타임아웃).
   - **세션 단위 상태 관리**: `agent + session_id` 복합 키로 상태를 관리하고 활성 세션 중 가장 높은 우선순위 상태를 `active_state`로 승격.
     - 우선순위: `error` > `waiting_input` > `executing_tool` > `generating` > `thinking` > `done` > `idle`
   - **TTL 만료**:
     - `done`: 기본 15초 후 자동 `idle` 복귀
     - `error`: 기본 60초 후 자동 `idle` 복귀
     - `stale`: 300초간 이벤트가 없는 비정상 세션 회수
   - **LWT (Last Will and Testament)**: 데몬 비정상 종료 시 `kitt-eye/system/status = offline` 자동 발행.
   - **Dry-run 지원**: `mqtt.broker`가 빈 문자열일 때 stdout으로 NDJSON 출력.

2. **MQTT 토픽 구조**
   | 토픽 | QoS / Retain | 페이로드 | 용도 |
   |---|---|---|---|
   | `{prefix}/active_state` | 1 / Retained | 문자열 (`executing_tool` 등) | 전체 대표 상태 (MCU 구독용) |
   | `{prefix}/active_agent` | 1 / Retained | 문자열 (`claude` 등) | 대표 활성 에이전트 이름 |
   | `{prefix}/agents/{agent}/state` | 1 / Retained | 문자열 | 개별 에이전트 상태 |
   | `{prefix}/agents/{agent}/sessions` | 1 / Retained | `AgentSnapshot` JSON | 에이전트의 세션 배열 스냅샷 |
   | `{prefix}/system/status` | 1 / Retained | `online` / `offline` | 데몬 생존 상태 (LWT) |
   | `homeassistant/sensor/kitt_eye_*/config` | 1 / Retained | JSON | HA Discovery 설정 |

---

### 4.3. MCU ESP32-C3 Rust 펌웨어 (`mcu-rust`)

1. **기술 스택**
   - **프레임워크**: `esp-hal` + Embassy async (`esp-rtos`, `esp-radio`, `embassy-net`)
   - **타깃**: `riscv32imc-unknown-none-elf` (ESP32-C3, `no_std`, `alloc`)
   - **LED 제어**: `smart-leds` + `ws2812-spi` (SPI2 MOSI GPIO6 이용)
   - **설정 주입**: `.env` -> `build.rs` -> `env!` 매크로 (컴파일 타임 상수 주입)

2. **모션 패턴 & 테마**
   - **`classic` (전부 Red 🔴)**:
     - `idle`: Slow K.I.T.T. Scanner (느린 좌우 왕복)
     - `thinking`: Center-Out (중앙→양끝 확산)
     - `generating`: Fill Sweep (채움 웨이브)
     - `executing_tool`: Fast K.I.T.T. Scanner (빠른 좌우 왕복 스캐너)
     - `waiting_input`: Blink (동기 점멸)
     - `done`: Flash → Solid (플래시 후 점등)
     - `error`: Comet (한 방향 혜성)
   - **`colorful` (상태별 생생한 컬러 & 모션)**:
     - `idle`: Classic Red (`RGB8(255, 0, 0)`) Slow K.I.T.T. Scanner
     - `thinking`: Mystic Purple (`RGB8(170, 0, 255)`) Center-Out
     - `generating`: Mint Emerald (`RGB8(0, 240, 120)`) Fill Sweep
     - `executing_tool`: Vivid Amber (`RGB8(255, 140, 0)`) **Fast K.I.T.T. Scanner**
     - `waiting_input`: Warning Yellow (`RGB8(255, 215, 0)`) Blink
     - `done`: Spring Lime (`RGB8(0, 255, 50)`) Flash → Solid
     - `error`: Crimson Red (`RGB8(255, 20, 20)`) Comet

---

### 4.4. Claude Code (`claude`) 훅 연동

Claude Code는 라이프사이클 이벤트마다 설정된 셸 커맨드를 실행하며, 컨텍스트 JSON을 **stdin**으로 넘깁니다.

#### 4.4.1. 설정 위치
- 기본 위치: `~/.claude/settings.json` (전역 모니터링)
- file watcher가 변경을 자동 감지하므로 CLI 재시작이 필요 없습니다.

#### 4.4.2. 훅 이벤트 → kitt-eye 상태 매핑
| Claude Code 이벤트 | matcher | kitt-eye 상태 | detail 내용 |
|---|---|---|---|
| `SessionStart` | 생략 | `idle` | `source` (`startup`/`resume`/`clear`) |
| `UserPromptSubmit` | 없음 | `thinking` | `prompt` 요약 |
| `PreToolUse` | 생략 (전 도구) | `executing_tool` | `tool_name` + `tool_input` 요약 |
| `PostToolUse` | 생략 | `executing_tool` | `<tool> 완료` |
| `PostToolBatch` | 없음 | `thinking` | `next model call` |
| `PostToolUseFailure` | 생략 | `error` | `tool_name` + 에러 요약 |
| `PermissionRequest` | 생략 | `waiting_input` | 승인 대기 중인 `tool_name` |
| `Notification` | `permission_prompt\|agent_needs_input\|idle_prompt` | `waiting_input` | `notification_type` |
| `Stop` | 없음 | `done` | `last_assistant_message` 요약 |
| `StopFailure` | 생략 | `error` | `error` 종류 |
| `PreCompact` | 생략 | `thinking` | `compacting` |
| `SessionEnd` | 생략 | `idle` | `reason` |

#### 4.4.3. 훅 스크립트 계약 (`hooks/claude/hook.sh`)
- **stdout 출력 절대 금지**: plain stdout은 모델 프롬프트로 주입됩니다.
- **항상 exit 0**: 비정상 종료 코드는 Claude 세션을 차단하거나 에러를 유발합니다 (`trap 'exit 0' EXIT`).
- **타임아웃 2초**: 훅 실행 지연이 에이전트 응답성에 영향을 주지 않도록 빠른 fire-and-forget 유지.

---

### 4.5. Antigravity CLI (`antigravity-cli`) 훅 연동

Antigravity CLI(`agy`)는 Gemini CLI 계열의 훅 시스템을 사용합니다. 설정은 `~/.gemini/config/hooks.json`의 **named bundle** 형태를 취합니다.

#### 4.5.1. 설정 위치 & 스키마
- 기본 위치: `~/.gemini/config/hooks.json`
- bundle 키: `"kitt-eye"`
- **신뢰(Trust) 필수**: 대상 워크스페이스가 `trustedWorkspaces`에 포함되어 있어야 훅이 활성화됩니다.

#### 4.5.2. 훅 이벤트 → kitt-eye 상태 매핑
| Antigravity 이벤트 | kitt-eye 상태 | detail 내용 |
|---|---|---|
| `PreInvocation` | `thinking` | `invocationNum` |
| `PostInvocation` | `generating` | `invocationNum` |
| `PostToolUse` | `executing_tool` (error 존재 시 `error`) | `toolCall.name` + args 요약 |
| `Stop` | `done` (terminationReason=="error" 시 `error`) | `terminationReason` |

#### 4.5.3. 훅 스크립트 계약 (`hooks/antigravity-cli/hook.sh`)
- **이벤트명 argv 전달**: payload 자체에 이벤트명이 없으므로 command 인자로 전달 (`$1`).
- **이벤트별 stdout 계약**:
  - `PostToolUse`: `{}` 출력 필수.
  - `Stop`: `{"decision":"allow"}` JSON 출력 필수.
  - `PreInvocation`/`PostInvocation`: 무출력.
- **`PreToolUse` 미사용**: decision 강제로 에이전트 권한 정책을 변경하므로 순수 관찰을 위해 등록하지 않음.

---

### 4.6. OpenAI Codex CLI (`codex`) 훅 연동

Codex CLI는 Claude Code와 유사한 핸들러 스키마를 사용하지만 엄격한 trust 승인 절차를 요구합니다.

#### 4.6.1. 설정 위치 & Trust
- 기본 위치: `~/.codex/hooks.json`
- **Trust 승인 필수**: 설치 후 세션에서 `/hooks`를 열어 kitt-eye 훅을 검토 및 승인해야 합니다. 스크립트 커맨드가 변경되면 해시가 바뀌어 재승인이 필요합니다.

#### 4.6.2. 훅 이벤트 → kitt-eye 상태 매핑
| Codex 이벤트 | matcher | kitt-eye 상태 | detail 내용 |
|---|---|---|---|
| `SessionStart` | `startup\|resume\|clear\|compact` | `idle` | `source` |
| `UserPromptSubmit` | 무시 | `thinking` | `prompt` 요약 |
| `PreToolUse` | `tool_name` | `executing_tool` | `tool_name` + 입력 요약 |
| `PostToolUse` | `tool_name` | `executing_tool` (오류 시 `error`) | 도구 완료/실패 요약 |
| `PermissionRequest` | `tool_name` | `waiting_input` | 승인 대기 중인 도구 |
| `PreCompact` / `PostCompact` | `manual\|auto` | `thinking` | compacting |
| `SubagentStart` / `SubagentStop` | `agent_type` | `executing_tool` / `thinking` | 서브에이전트 상태 |
| `Stop` | 무시 | `done` | `last_assistant_message` 요약 |
| `Interrupt` | 무시 | `idle` | 사용자의 `Esc` 중단 감지 |
| `SessionEnd` | reason | `idle` | `reason` |

#### 4.6.3. 훅 스크립트 계약 (`hooks/codex/hook.sh`)
- `Stop`/`SubagentStop`은 exit 0에서 **JSON stdout(`{}`) 필수** (plain 텍스트는 invalid).
- 그 외 이벤트는 무출력 유지.

---

### 4.7. Cursor CLI (`cursor-cli`) 훅 연동

Cursor CLI(`cursor-agent`)는 Cursor IDE와 훅 설정을 공유합니다(`~/.cursor/hooks.json`).

#### 4.7.1. 설정 위치
- 기본 위치: `~/.cursor/hooks.json` (`"version": 1`)
- file watcher에 의해 즉시 반영됩니다.

#### 4.7.2. 훅 이벤트 → kitt-eye 상태 매핑
| Cursor 이벤트 | kitt-eye 상태 | detail 내용 |
|---|---|---|
| `sessionStart` | `idle` | `source` |
| `beforeShellExecution` | `executing_tool` | `command` 요약 |
| `afterShellExecution` | `executing_tool` | `command 완료` |
| `afterFileEdit` | `executing_tool` | `file_path` 파일명 |
| `postToolUse` | `executing_tool` | `tool_name` 완료 |
| `stop` | `done` (`aborted`시 `idle`, `error`시 `error`) | `status` |

#### 4.7.3. 훅 스크립트 계약 (`hooks/cursor-cli/hook.sh`)
- 항상 exit 0 유지 (비0 코드는 경고/차단 유발).
- stdout은 무출력 또는 `{}`.

---

## 5. 보안 및 관찰자 원칙

1. **순수 관찰자 (Read-Only Observer)**
   - 훅은 에이전트의 결정을 조작하거나 stdin/stdout 파이프라인을 간섭하지 않습니다.
2. **Fail-Safe & Non-Blocking**
   - 데몬이 꺼져 있거나 소켓이 응답하지 않아도 CLI에 어떠한 지연이나 오류를 주지 않고 즉시 리턴합니다.
3. **개인정보 보호 (Privacy)**
   - `detail` 필드는 기본적으로 도구명과 파일 경로 요약만 전송하며, 긴 프롬프트 원문은 마스킹/절삭됩니다.
