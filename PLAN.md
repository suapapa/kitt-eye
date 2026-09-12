# KITT-EYE (AI Agent Status Indicator via MQTT & ESP32-C3)

## 1. 프로젝트 개요
`kitt-eye`는 다양한 AI 에이전트 CLI(`antigravity-cli`, `claude`, `codex`, `cursor-cli`)의 라이프사이클 훅(Hook)을 감지하여 에이전트의 현재 동작 상태(Thinking, Coding, Tool Execution, Waiting Input 등)를 수집하고, Go 언어로 작성된 Publisher를 통해 MQTT 브로커(Home Assistant Mosquitto)로 발행(Publish)한 뒤, ESP32-C3 기반 MCU(Rust 작성)에서 구독(Subscribe)하여 LED(K.I.T.T. 스캐너 효과) 또는 디스플레이로 시각화하는 모니터링 시스템입니다.

---

## 2. 시스템 아키텍처

```text
[ AI CLI Tools ]
 ├── antigravity-cli (공식 Hooks: hooks.json named bundle, agy CLI)
 ├── claude          (공식 Hooks: ~/.claude/settings.json)
 ├── codex           (공식 Hooks: ~/.codex/hooks.json + /hooks trust)
 └── cursor-cli      (공식 Hooks: ~/.cursor/hooks.json, cursor-agent)
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
│   ├── claude/
│   │   ├── hook.sh                # stdin 훅 JSON → kitt-eye 이벤트 변환기 (항상 exit 0)
│   │   └── settings.snippet.json  # ~/.claude/settings.json 병합용 훅 정의 템플릿
│   ├── codex/
│   │   └── hook.sh
│   ├── cursor-cli/
│   │   └── hook.sh
│   └── common/
│       └── send_event.sh          # Go 데몬에 상태를 전송하는 공통 전송 유틸리티 (v1.1: 세션/이벤트 인자)
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
   - **v1.1 확장 (하위 호환, 선택 필드)**: `claude`는 여러 세션/서브에이전트가 동시에 동작할 수 있어 세션 식별 정보를 추가합니다. 미전송 시 데몬은 `session_id="default"`로 간주합니다.
   ```json
   {
     "agent": "claude",
     "state": "executing_tool",
     "detail": "Bash: npm test",
     "timestamp": "2026-09-07T11:26:00Z",
     "session_id": "abc123",
     "event": "PreToolUse"
   }
   ```
3. **Makefile 설치 자동화**
   - `make install-hooks`: 4개 CLI 모두 훅이 **설정 파일 JSON 병합** 방식으로 등록되므로, `jq` 기반 서브타깃으로 백업 + 멱등 병합을 통일한다.
     - `install-hooks-claude`: `jq`로 `~/.claude/settings.json`에 병합(4.4.5).
     - `install-hooks-antigravity-cli`: `hooks.json`에 `kitt-eye` named bundle 병합 + trustedWorkspaces trust 승인 안내(4.5.5).
     - `install-hooks-codex`: `~/.codex/hooks.json` 병합. trust가 hook 해시에 묶이므로 **설치/갱신마다 `/hooks` 재승인 필요**(4.6.5).
     - `install-hooks-cursor-cli`: `~/.cursor/hooks.json`(또는 프로젝트 `.cursor/hooks.json`) 병합, file watcher가 즉시 반영(4.7.5).
   - `make uninstall-hooks`: 각 설정 파일에서 kitt-eye 훅 항목만 선별 제거하고 나머지 사용자 훅은 유지, `~/.kitt-eye/bin/kitt-eye-*-hook` 삭제.

### 4.2. Go Publisher (`pub-go`)
1. **핵심 기능**
   - **상시 연결 유지**: Home Assistant의 Mosquitto 브로커와 지속적인 MQTT 세션 유지 (매 CLI 호출마다 발생하는 핸드셰이크 오버헤드 방지).
   - **IPC 서버**: `/tmp/kitt-eye.sock`을 열어 훅 스크립트의 빠른 비동기 신호 수신.
   - **세션 단위 상태 관리**: `claude`는 한 기기에서 여러 세션(`session_id`)과 서브에이전트(`agent_id`)가 동시에 살아있을 수 있으므로, 상태를 `agent + session_id` 키로 보관하고 세션별 TTL(`done`/`error` 만료 → `idle`)을 적용한 뒤, 활성 세션 중 가장 높은 우선순위의 상태만 `active_state`로 승격합니다.
     - 우선순위: `error` > `waiting_input` > `executing_tool` > `generating` > `thinking` > `done` > `idle`
   - **LWT (Last Will and Testament)**: 데몬 비정상 종료 시 `kitt-eye/system/status = offline` 자동 발행.
   - **토픽 구조**:
     - `kitt-eye/agents/<agent_name>/state`: 개별 에이전트 상태
     - `kitt-eye/agents/<agent_name>/sessions`: (선택 사항) 세션별 상태 배열 — `claude` 멀티 세션/서브에이전트 구분용
     - `kitt-eye/active_state`: 현재 가장 활발한 에이전트의 대표 상태 (MCU 연동용)
     - `homeassistant/sensor/kitt_eye_.../config`: (선택 사항) HA MQTT Discovery 엔티티 자동 등록.

### 4.3. MCU ESP32-C3 Rust 펌웨어 (`mcu-rust`)
1. **기술 스택**
   - **프레임워크**: `esp-hal` + `esp-rtos` / Embassy (`esp-radio` Wi-Fi, `embassy-net`)
   - **타깃 아키텍처**: `riscv32imc-unknown-none-elf` (ESP32-C3)
   - **LED 드라이버**: `smart-leds` + `ws2812-spi` (SPI MOSI → WS2812B)
   - **설정**: `mcu-rust/.env` (`SSID`, `PASS`, `MOTION_THEME`, `MQTT_BROKER`, …)
2. **동작 시나리오**
   - Wi-Fi 부팅 및 HA Mosquitto 브로커 접속 (`kitt-eye/#` 토픽 구독).
  - 수신된 에이전트 상태에 따라 LED 패턴 렌더링 (`MOTION_THEME=classic`, **전부 Red**):
     - **Breathing**: `idle`
     - **Center-Out**: `thinking`
     - **Fill Sweep**: `generating`
     - **K.I.T.T. Scanner (좌우 왕복)**: `executing_tool`
     - **Blink**: `waiting_input`
     - **Flash → Solid**: `done`
     - **Comet (한 방향 혜성)**: `error`
   - Wi-Fi / MQTT / 테마는 `mcu-rust/.env` → `build.rs` → `env!`로 컴파일 타임 주입.

### 4.4. Claude Code (`claude`) 훅 연동

`claude`는 PATH 래퍼 방식이 아니라 **공식 Hooks 기능**을 사용합니다. Claude Code는 라이프사이클 이벤트마다 설정된 shell command를 실행하고, 이벤트 컨텍스트 JSON을 **stdin**으로 넘겨줍니다. kitt-eye의 훅은 이 기능을 **순수 관찰자(observer)로만** 사용하여 에이전트의 동작을 절대 변경하지 않습니다.

#### 4.4.1. 설정 위치

| 위치 | 범위 | 용도 |
| --- | --- | --- |
| `~/.claude/settings.json` | 모든 프로젝트 | kitt-eye 기본 설치 위치 (전역 모니터) |
| `.claude/settings.json` | 단일 프로젝트 | 레포 커밋 가능, 팀 공유용 |
| `.claude/settings.local.json` | 단일 프로젝트 | gitignore, 개인용 |

- settings 파일 직접 수정은 file watcher로 즉시 반영되므로 **Claude Code 재시작이 필요 없습니다.**
- 등록 여부는 Claude Code에서 `/hooks` 메뉴(읽기 전용 브라우저)로 확인합니다.
- `config.example.yaml`의 `agents:` 목록에 `claude` 항목을 추가하고, 훅 동작 옵션을 함께 정의합니다.
  ```yaml
  agents:
    - name: "claude"
      enabled: true
      detail: "minimal"   # minimal: 도구명/파일명만 | full: 프롬프트·응답 앞 60자까지 전송
      max_sessions: 8     # 세션 단위 추적 상한
  ```
- **런타임 의존성**: `jq`(claude 훅의 stdin 파싱 및 settings 병합에 필요).

#### 4.4.2. 훅 이벤트 → kitt-eye 상태 매핑

| Claude Code 이벤트 | matcher | kitt-eye 상태 | `detail` 출처 |
| --- | --- | --- | --- |
| `SessionStart` | 생략 | `idle` | `source` (`startup`/`resume`/`clear`) |
| `UserPromptSubmit` | 없음 | `thinking` | `prompt` 앞 60자 |
| `PreToolUse` | 생략 (전 도구) | `executing_tool` | `tool_name` + `tool_input` 요약 |
| `PostToolUse` | 생략 | `executing_tool` | `<tool> 완료` |
| `PostToolBatch` | 없음 | `thinking` | `next model call` |
| `PostToolUseFailure` | 생략 | `error` | `tool_name` + `error` 앞 60자 |
| `PermissionRequest` | 생략 | `waiting_input` | 승인 대기 중인 `tool_name` |
| `Notification` | `permission_prompt\|agent_needs_input\|idle_prompt` | `waiting_input` | `notification_type` |
| `Stop` | 없음 | `done` | `last_assistant_message` 앞 60자 |
| `StopFailure` | 생략 (`rate_limit` 등) | `error` | `error` 종류 |
| `PreCompact` | 생략 (`manual\|auto`) | `thinking` | `compacting` |
| `SessionEnd` | 생략 (`clear`/`other` 등) | `idle` | `reason` |
| (옵션) `MessageDisplay` | 없음 | `generating` | `delta` 앞 60자 |
| (옵션) `SubagentStart` | agent type | `executing_tool` | `subagent: <agent_type>` |
| (옵션) `SubagentStop` | agent type | `thinking` | `subagent 완료` |

- `PreToolUse`의 matcher를 생략하면 내장 도구(`Bash`, `Edit`, `Write`, `Read`, `Glob`, `Grep`, `Agent`, `WebFetch`, `WebSearch` …)와 MCP 도구(`mcp__<server>__<tool>`)를 모두 잡아 `executing_tool`의 `detail`로 표시할 수 있습니다.
- 상태 스펙(4.1.1)의 `generating`은 `MessageDisplay`로만 관측 가능합니다. 이 이벤트는 스트리밍 텍스트 배치마다 훅 프로세스를 spawn 하므로 **기본 비활성**, 설정 플래그로 켜는 옵션으로 처리합니다.

#### 4.4.3. `settings.json` 스니펫 (기본 세트)

```json
{
  "hooks": {
    "SessionStart":       [{ "hooks": [{ "type": "command", "command": "$HOME/.kitt-eye/bin/kitt-eye-claude-hook", "timeout": 2 }] }],
    "UserPromptSubmit":   [{ "hooks": [{ "type": "command", "command": "$HOME/.kitt-eye/bin/kitt-eye-claude-hook", "timeout": 2 }] }],
    "PreToolUse":         [{ "hooks": [{ "type": "command", "command": "$HOME/.kitt-eye/bin/kitt-eye-claude-hook", "timeout": 2 }] }],
    "PostToolUse":        [{ "hooks": [{ "type": "command", "command": "$HOME/.kitt-eye/bin/kitt-eye-claude-hook", "timeout": 2 }] }],
    "PostToolBatch":      [{ "hooks": [{ "type": "command", "command": "$HOME/.kitt-eye/bin/kitt-eye-claude-hook", "timeout": 2 }] }],
    "PostToolUseFailure": [{ "hooks": [{ "type": "command", "command": "$HOME/.kitt-eye/bin/kitt-eye-claude-hook", "timeout": 2 }] }],
    "PermissionRequest":  [{ "hooks": [{ "type": "command", "command": "$HOME/.kitt-eye/bin/kitt-eye-claude-hook", "timeout": 2 }] }],
    "Notification":       [{ "matcher": "permission_prompt|agent_needs_input|idle_prompt", "hooks": [{ "type": "command", "command": "$HOME/.kitt-eye/bin/kitt-eye-claude-hook", "timeout": 2 }] }],
    "Stop":               [{ "hooks": [{ "type": "command", "command": "$HOME/.kitt-eye/bin/kitt-eye-claude-hook", "timeout": 2 }] }],
    "StopFailure":        [{ "hooks": [{ "type": "command", "command": "$HOME/.kitt-eye/bin/kitt-eye-claude-hook", "timeout": 2 }] }],
    "PreCompact":         [{ "hooks": [{ "type": "command", "command": "$HOME/.kitt-eye/bin/kitt-eye-claude-hook", "timeout": 2 }] }],
    "SessionEnd":         [{ "hooks": [{ "type": "command", "command": "$HOME/.kitt-eye/bin/kitt-eye-claude-hook", "timeout": 2 }] }]
  }
}
```

- `args`를 생략한 **shell form**이므로 `"$HOME/..."`은 `sh -c`가 해석합니다. 설치 스크립트에서 절대 경로를 박아 넣으려면 `args`를 채운 **exec form**(플레이스홀더/따옴표 안전)을 사용합니다.
- 모든 핸들러는 **동일한 스크립트 하나**를 이벤트별로만 다르게 붙이며, 실제 분기는 stdin JSON의 `hook_event_name`가 합니다(4.4.4).

#### 4.4.4. 훅 스크립트 계약 (`hooks/claude/hook.sh`)

**에이전트를 망가뜨리지 않기 위한 필수 규칙**

1. **stdout에 아무것도 출력하지 않는다.** `UserPromptSubmit`/`SessionStart`의 plain stdout은 Claude 컨텍스트에 주입되고, `{...}`로 시작하면 decision JSON으로 파싱됩니다. 자식으로 호출되는 `send_event.sh`의 stdout도 훅의 stdout으로 상속되므로 전송 유틸리티는 반드시 무출력이어야 합니다.
2. **항상 exit 0.** `exit 2`는 도구 호출·프롬프트·중단을 차단하고, 그 외 non-zero는 transcript에 `hook error` 공지를 남깁니다. `trap 'exit 0' EXIT`로 강제합니다.
3. **`jq`로 stdin 파싱** (필수 의존성). stdin은 끝까지 소비합니다. jq 파싱 실패의 stderr는 exit 0이므로 debug log로만 전달되어 세션에 흔적을 남기지 않습니다.
4. **`detail` 살균(sanitize).** `send_event.sh`가 문자열 보간으로 JSON을 조립하므로 `"`·개행·탭을 제거하고 60자로 자릅니다.
5. **빠른 종료.** `timeout: 2`를 지정하고, 전송 실패는 조용히 무시합니다(`|| true`).

```bash
#!/usr/bin/env bash
# hooks/claude/hook.sh : Claude Code 훅 stdin JSON -> kitt-eye 이벤트 변환기
# 관찰자 전용: stdout 출력 금지, 항상 exit 0
set -u
trap 'exit 0' EXIT

INPUT="$(cat)"; [ -n "$INPUT" ] || exit 0
command -v jq >/dev/null 2>&1 || exit 0
SEND="${HOME}/.kitt-eye/bin/kitt-eye-send"; [ -x "$SEND" ] || exit 0

# jq 프로그램을 작은따옴표로 감싸 셸 확장을 차단하고, 탭 구분자로 4개 필드를 받는다
IFS=$'\t' read -r EVENT STATE SID DETAIL < <(printf '%s' "$INPUT" | jq -r '
  def clean: tostring | gsub("[\r\n\t\"]"; " ") | .[0:60];
  .hook_event_name as $e
  | (if   $e == "SessionStart"        then "idle"
     elif $e == "SessionEnd"          then "idle"
     elif $e == "UserPromptSubmit"    then "thinking"
     elif $e == "MessageDisplay"      then "generating"
     elif $e == "PreCompact"          then "thinking"
     elif $e == "PostToolBatch"       then "thinking"
     elif $e == "SubagentStop"        then "thinking"
     elif $e == "PreToolUse"          then "executing_tool"
     elif $e == "PostToolUse"         then "executing_tool"
     elif $e == "SubagentStart"       then "executing_tool"
     elif $e == "PermissionRequest"   then "waiting_input"
     elif $e == "Notification"        then "waiting_input"
     elif $e == "Stop"                then "done"
     elif $e == "StopFailure"         then "error"
     elif $e == "PostToolUseFailure"  then "error"
     else "idle" end) as $st
  | (if .tool_name and .error
       then .tool_name + " failed: " + (.error | clean)
     elif .tool_name
       then .tool_name + " " + ((.tool_input.description // .tool_input.command
             // .tool_input.file_path // .tool_input.pattern // .tool_input.query
             // .tool_input.url // .tool_input.prompt // "") | clean)
     elif .notification_type      then (.notification_type | clean)
     elif .error                  then (.error | clean)
     elif .delta                  then (.delta | clean)
     elif .last_assistant_message then (.last_assistant_message | clean)
     elif .prompt                 then (.prompt | clean)
     else "" end | sub(" +$"; "")) as $det
  | [ $e, $st, (.session_id // "default"), $det ] | @tsv')

[ -n "$EVENT" ] || exit 0
"$SEND" claude "$STATE" "$DETAIL" "$SID" "$EVENT" || true
exit 0
```

> `send_event.sh`는 `kitt-eye-send <agent> <state> [detail] [session_id] [event]`로 인자를 확장하고, `detail`을 JSON 문자열로 안전하게 인코딩합니다(4.1.2 v1.1).

#### 4.4.5. 설치 / 제거 (JSON 병합)

래퍼와 달리 settings.json은 **병합**해야 하므로 `jq` 기반 서브타깃으로 구현합니다.

- **install**: `hooks/claude/hook.sh`를 `~/.kitt-eye/bin/kitt-eye-claude-hook`으로 복사 + `chmod +x` → 파일/디렉토리 보장 → `settings.json.bak.<ts>` 백업 → `command`에 `kitt-eye-claude-hook`을 포함한 항목만 제거(멱등성) → 스니펫의 `hooks`를 이벤트 키별로 병합 → 임시 파일에 써서 JSON 검증 후 원자적 교체.
- **uninstall**: 위와 동일한 “kitt-eye 항목만 선별 제거” 필터를 적용하고 나머지는 그대로 유지. `~/.kitt-eye/bin/kitt-eye-claude-hook` 삭제.
- **검증**: `jq '.hooks | keys' ~/.claude/settings.json`, Claude Code `/hooks` 메뉴.

#### 4.4.6. 주의사항 체크리스트

1. **중단 신호 누락 대비**: `Stop`은 사용자가 `Esc`로 멈추면 발화하지 않고, 크래시 시 `SessionEnd`도 오지 않습니다. publisher 쪽 **TTL 만료**가 기본 안전망입니다.
2. **Workspace trust**: 대화형 세션은 trust 다이얼로그를 승인하기 전까지 settings 파일 훅을 실행하지 않습니다(`-p`/SDK 세션은 trust로 간주).
3. **비활성화 경로**: `disableAllHooks: true`, 엔터프라이즈 `allowManagedHooksOnly` 환경에서는 kitt-eye 훅이 실행되지 않습니다. 설치 후 `/hooks` 확인이 필수입니다.
4. **Cloud/remote 세션**: Claude Code on the web은 로컬 `~/.claude/settings.json`을 읽지 않으므로 대상에서 제외합니다.
5. **`waiting_input` 타이밍**: `Notification`의 `permission_prompt`는 프롬프트가 약 6초 대기한 뒤(사용자가 입력 중인 동안은 지연) 발화합니다. 즉각적인 신호는 `PermissionRequest`가 담당합니다.
6. **`SessionEnd` 예산**: 모든 `SessionEnd` 훅은 1.5초 예산을 공유하므로 `send_event.sh`의 `nc -w 1` 타임아웃을 유지합니다.
7. **`MessageDisplay` 오버헤드**: 기본 timeout 10초, 메시지당 여러 번 호출 → 옵션 처리. `-p` 모드에서는 메시지당 1회 호출됩니다.
8. **훅 타임아웃은 fail-open**: `PreToolUse` 훅이 타임아웃돼도 도구 호출은 정상 진행되므로 관찰자 훅으로 안전하지만, 체감 지연을 막기 위해 `timeout: 2`를 유지합니다.
9. **실행 환경**: 훅은 현재 디렉터리에서 사용자 권한으로 도는 일반 셸 프로세스입니다. `set -u`, 변수 quote, 절대 경로를 지키고 sensitive 파일은 읽지 않습니다.
10. **서브에이전트**: main 스레드와 서브에이전트 훅이 동일하게 발화하며 `agent_id`/`agent_type`으로 구분됩니다. LED를 서브에이전트로 어지럽히지 않도록 publisher에서 세션 단위로 집계합니다.
11. **프라이버시**: `prompt`/`last_assistant_message`/`delta`/`tool_input.command`를 `detail`로 그대로 태우면 사용자 입력과 코드 경로가 MQTT 브로커(및 HA 히스토리)로 전송됩니다. 기본값은 `detail: minimal`(도구명 + 파일 basename/명령어 첫 토큰)로 하고, 원문 포함은 `detail: full` 명시 시에만 허용합니다. 위 샘플은 `full` 기준이며, `minimal`은 `.prompt`/`.last_assistant_message` 분기를 생략하면 됩니다.

### 4.5. Antigravity CLI (`antigravity-cli`) 훅 연동

`antigravity-cli`(실행 파일 `agy`)는 Gemini CLI 계열의 **공식 Hooks 기능**을 사용하며 설정 디렉터리도 Gemini(`~/.gemini/...`)를 상속합니다. Claude/Codex와 달리 라이프사이클 이벤트가 **5개뿐**이고, 최상위 스키마가 **named bundle** 형태이며, 이벤트별 출력 계약이 "에이전트 동작을 바꾸는" 방향으로 설계돼 있어 kitt-eye는 중립 관찰이 가능한 최소 세트만 등록합니다.

#### 4.5.1. 설정 위치

| 위치 | 범위 | 용도 |
| --- | --- | --- |
| `~/.gemini/config/hooks.json` | 모든 프로젝트 | kitt-eye 기본 설치 위치 (Gemini 전역 설정 디렉터리 공유) |
| `<workspace root>/.agents/hooks.json` | 단일 워크스페이스 | 레포 커밋 가능, 팀 공유용 |
| `~/.gemini/antigravity-cli/plugins/<name>/hooks.json` | 플러그인 | 플러그인 배포 시 동봉 |

- 훅은 **trusted folder에서 로드/실행**됩니다. `~/.gemini/antigravity-cli/settings.json`의 `trustedWorkspaces`에 대상 워크스페이스가 포함되어야 하며(또는 대화형 세션의 trust 다이얼로그 승인), 승인 전에는 모든 훅이 무시됩니다.
- agy CLI 안에서 `/hooks` 슬래시 커맨드로 등록 상태를 확인합니다.
- 최상위 스키마는 named bundle: `{ "<bundle name>": { "enabled": true, "<Event>": [...] } }`. 여러 bundle이 공존할 수 있으므로 kitt-eye는 `kitt-eye` bundle을 사용합니다.
- `config.example.yaml`의 `agents:` 목록에 `antigravity-cli` 항목을 추가합니다(필드는 4.4.1과 동일).
- **런타임 의존성**: `jq`.

#### 4.5.2. 훅 이벤트 → kitt-eye 상태 매핑

| Antigravity 이벤트 | matcher | kitt-eye 상태 | `detail` 출처 |
| --- | --- | --- | --- |
| `PreInvocation` | 없음(무시) | `thinking` | `invocationNum` |
| `PostInvocation` | 없음(무시) | `generating` | `invocationNum` |
| `PostToolUse` | 도구명 glob | `executing_tool` (`error` 비어있지 않으면 `error`) | `toolCall.name` + `toolCall.args` 요약 |
| `Stop` | 없음(무시) | `done` (`terminationReason=="error"` 또는 `error` → `error`) | `terminationReason` |
| `PreToolUse` | — | **기본 미등록** — decision 필수로 권한 동작이 변함(4.5.4) | — |

- 이벤트는 총 5개뿐입니다. **`SessionStart`/`SessionEnd`/`Notification`/`PermissionRequest`가 존재하지 않습니다** → `waiting_input`은 구조상 관측 불가이며 `idle`은 publisher TTL 만료에 의존합니다. 세션 시작은 `PreInvocation`이, 종료는 `Stop`/`PostInvocation`이 근사 대체합니다.
- payload는 camelCase이며 공통 필드는 `conversationId`(→ `session_id`), `workspacePaths`, `transcriptPath`(`~/.gemini/antigravity-cli/brain/<id>/.system_generated/logs/transcript.jsonl`), `artifactDirectoryPath`, `modelName`입니다.
- **CLI payload에는 이벤트명 자체를 식별하는 필드가 없습니다**(IDE 변형에만 `hookEventName`이 존재) → 이벤트명은 command의 argv로 주입합니다(4.5.3/4.5.4).
- `PostToolUse`의 `error`는 성공 시 빈 문자열입니다. `toolCall.args`의 키는 도구별 PascalCase(`run_command` → `CommandLine`)이므로 detail은 값들을 연결해 만듭니다.

#### 4.5.3. `hooks.json` 스니펫 (기본 세트)

```json
{
  "kitt-eye": {
    "enabled": true,
    "PreInvocation":  [{ "type": "command", "command": "$HOME/.kitt-eye/bin/kitt-eye-agy-hook PreInvocation",  "timeout": 2 }],
    "PostInvocation": [{ "type": "command", "command": "$HOME/.kitt-eye/bin/kitt-eye-agy-hook PostInvocation", "timeout": 2 }],
    "PostToolUse":    [{ "matcher": "*", "hooks": [{ "type": "command", "command": "$HOME/.kitt-eye/bin/kitt-eye-agy-hook PostToolUse", "timeout": 2 }] }],
    "Stop":           [{ "type": "command", "command": "$HOME/.kitt-eye/bin/kitt-eye-agy-hook Stop", "timeout": 2 }]
  }
}
```

- 핸들러 형태가 이벤트마다 다릅니다: `PreInvocation`/`PostInvocation`/`Stop`은 handler list가 **이벤트 키 바로 아래**(matcher 그룹 없음, matcher 무시), `Pre`/`PostToolUse`만 `[{ matcher, hooks: [...] }]` 그룹을 씁니다.
- handler type은 `"command"` 하나뿐이며 기본 timeout은 30초입니다. 훅은 **에이전트 루프를 동기 차단**하므로 `timeout: 2`로 통일합니다.
- `command` 문자열은 셸로 실행되므로 이벤트명을 인자로 붙일 수 있습니다. payload에 이벤트명이 없으므로 **argv 주입이 필수**입니다.
- `matcher`로 도구를 좁히려는 경우의 주요 대상: `run_command`, `view_file`, `write_to_file`, `replace_file_content`, `multi_replace_file_content`, `list_dir`, `find_by_name`, `grep_search`, `search_web`, `read_url_content`, `manage_task`, `schedule`, `ask_permission`, `invoke_subagent`, `ask_question`, `generate_image`, `browser_*` 등.

#### 4.5.4. 훅 스크립트 계약 (`hooks/antigravity-cli/hook.sh`)

**Claude(4.4.4)와 다른 규칙 — stdout 계약이 이벤트별**

1. **이벤트명은 argv로 받는다**: `"$1"`이 기준값이며, payload의 `hookEventName`은 백업 정도로만 참조합니다.
2. **이벤트별 stdout**:
   - `PostToolUse` → `{}` 출력 필수.
   - `Stop` → `decision` 필드 필수. 정지 차단("continue")이 아닌 값이면 정지가 허용되므로 항상 `{"decision":"allow"}`를 출력합니다. 무출력/invalid JSON은 훅 실패로 처리됩니다.
   - `PreInvocation`/`PostInvocation` → 무출력(`injectSteps` 기능은 사용하지 않습니다).
3. **`PreToolUse`는 기본 미등록**: decision(`allow`/`deny`/`ask`/`force_ask`/`deny_unless_prior_grant`)이 필수이고 모든 합법 값이 권한 동작을 바꾸므로 **중립 관찰 지점이 없습니다** → 도구 활동은 `PostToolUse`가 담당합니다.
4. **동기 차단**: 모든 훅이 에이전트 루프의 critical path(기본 30초)에 있으므로 `timeout: 2` + socket fire-and-forget이 필수입니다.
5. 나머지 규칙(plain stdout 금지, 항상 exit 0, `jq` 파싱, `detail` 살균, 빠른 종료)은 4.4.4와 동일합니다.

```bash
#!/usr/bin/env bash
# hooks/antigravity-cli/hook.sh : Antigravity CLI 훅 -> kitt-eye 이벤트 변환기
# 이벤트명은 argv[1] 주입(payload에 없음). stdout 계약: PostToolUse->"{}", Stop->decision JSON, 그 외 무출력.
set -u
EVENT="${1:-unknown}"
INPUT="$(cat)"

# stdout 계약은 관측 성공/실패(jq·SEND 부재 포함)와 무관하게 항상 유지
case "$EVENT" in
  PostToolUse) printf '{}\n' ;;
  Stop)        printf '{"decision":"allow"}\n' ;;
esac

[ -n "$INPUT" ] || exit 0
command -v jq >/dev/null 2>&1 || exit 0
SEND="${HOME}/.kitt-eye/bin/kitt-eye-send"; [ -x "$SEND" ] || exit 0

IFS=$'\t' read -r STATE SID DETAIL < <(printf '%s' "$INPUT" | jq -r --arg e "$EVENT" '
  def clean: tostring | gsub("[\r\n\t\"]"; " ") | .[0:60];
  (if   $e == "PreInvocation"  then "thinking"
   elif $e == "PostInvocation" then "generating"
   elif $e == "PostToolUse" and ((.error // "") != "") then "error"
   elif $e == "PostToolUse"    then "executing_tool"
   elif $e == "Stop" and (((.terminationReason // "") == "error") or ((.error // "") != "")) then "error"
   elif $e == "Stop"           then "done"
   else "idle" end) as $st
  | (if   .toolCall and .toolCall.name
       then .toolCall.name + " " + ((.toolCall.args // {} | [.[]] | map(tostring) | join(" ")) | clean)
     elif $e == "Stop" then ((.terminationReason // "stop") | clean)
     elif $e == "PreInvocation" or $e == "PostInvocation"
       then "invocation " + ((.invocationNum // 0) | tostring)
     else "" end | sub(" +$"; "")) as $det
  | [ $st, (.conversationId // "default"), $det ] | @tsv')

[ -n "$STATE" ] || exit 0
"$SEND" antigravity-cli "$STATE" "$DETAIL" "$SID" "$EVENT" || true
exit 0
```

> `toolCall.args`의 값 타입은 도구마다 달라서 `map(tostring)`로 방어했습니다. 상세 스키치는 Step 3 스모크 테스트로 확정합니다.

#### 4.5.5. 설치 / 제거 (JSON 병합)

- **install**: `hooks/antigravity-cli/hook.sh`를 `~/.kitt-eye/bin/kitt-eye-agy-hook`으로 복사 + `chmod +x` → 대상 `hooks.json` 보장(없으면 `{}` 생성) → `hooks.json.bak.<ts>` 백업 → 기존 `kitt-eye` bundle 통째 제거(멱등성) → 스니펫의 `kitt-eye` 키를 루트에 딥 병합 → JSON 검증 후 원자적 교체.
- **trust**: 설치 후 대상 워크스페이스가 `trustedWorkspaces`에 있는지 확인하고, 없으면 trust 승인을 안내합니다. 전역(global) `hooks.json`도 워크스페이스별 trust 판정을 받습니다.
- **uninstall**: `jq 'del(.["kitt-eye"])'`로 kitt-eye bundle만 선별 제거, `kitt-eye-agy-hook` 삭제.
- **검증**: `jq '.["kitt-eye"] | keys' <hooks.json>`, agy `/hooks`.

#### 4.5.6. 주의사항 체크리스트

1. **trustedWorkspaces 게이트**: trust되지 않은 폴더에서는 훅이 아예 실행되지 않습니다. LED가 안 켜지면 trust 상태부터 확인합니다(`/hooks` 또는 settings.json).
2. **동기 루프 차단**: 훅은 에이전트 루프 critical path입니다. `timeout: 2`와 fire-and-forget 전송을 반드시 유지합니다(기본 30초는 위험).
3. **이벤트명 필드 불신**: CLI payload에는 이벤트명 필드가 없습니다. `hookEventName` 의존 로직은 CLI에서 동작하지 않으며 argv 주입이 정답입니다.
4. **`PreToolUse` 미등록**: decision 계약상 중립 관찰이 불가능합니다. 등록해야 한다면 권한 동작을 바꾸지 않는 값이 존재하지 않음을 인지합니다.
5. **`Stop`의 decision JSON 유지**: stdout 실패는 훅 실패로 기록되고 정지를 막을 수 있습니다. 스크립트처럼 jq 파싱 실패와 무관하게 stdout 계약을 항상 유지합니다.
6. **세션/권한 이벤트 부재**: `waiting_input`은 관측 불가, `idle`은 TTL 의존이며 세션 종료 감지는 `Stop`/`PostInvocation` 근사치입니다.
7. **전역 설정 공유**: `~/.gemini/config/hooks.json`은 Gemini CLI와 공유 디렉터리입니다. 다른 도구의 bundle과 이름 충돌을 방지합니다.
8. **args 값 타입 다양성**: `toolCall.args` 값에 숫자/객체가 섞일 수 있어 detail 추출 로직은 Step 3에서 도구별로 검증합니다.
9. **프라이버시**: `toolCall.args`에 명령줄과 파일 경로가 그대로 들어 있습니다. 도구명 + args 앞 60자만 전송하고 `detail: minimal` 정책(4.4.6 #11)을 동일하게 적용합니다.

### 4.6. Codex CLI (`codex`) 훅 연동

`codex`는 **공식 Hooks 기능**을 사용합니다. 핸들러 그룹핑 스키마와 payload 모양이 Claude Code와 거의 동일해서 `hooks/claude/hook.sh` 계열을 대부분 재사용할 수 있지만, 두 가지가 다릅니다: **trust 승인된 훅만 실행**된다는 점, 이벤트별 stdout 계약이 한층 더 엄격하다는 점.

#### 4.6.1. 설정 위치

| 위치 | 범위 | 용도 |
| --- | --- | --- |
| `~/.codex/hooks.json` | 모든 프로젝트 | kitt-eye 기본 설치 위치 (전역 모니터) |
| `~/.codex/config.toml` 인라인 `[hooks]` | 모든 프로젝트 | TOML 파생 대안 — `hooks.json`과 공존(중복 등록 주의) |
| `<repo>/.codex/hooks.json`, `<repo>/.codex/config.toml` | 단일 프로젝트 | 레포 커밋 가능, 팀 공유용. 해당 프로젝트 레이어가 trusted일 때만 로드 |
| 플러그인 `hooks/hooks.json` | 플러그인 | 플러그인 배포 시 동봉 |

- **레이어는 오버라이드 değil 동시 실행**입니다 — 전역/프로젝트 중복 등록에 주의합니다.
- 훅은 기본 활성이며 `[features].hooks = false`로 끕니다(`codex_hooks`는 deprecated 별명).
- **trust 모델**: managed가 아닌 훅은 review승인 전까지 스킵됩니다. `/hooks` 메뉴에서 승인하며 trust는 **hook 커맨드 해시에 묶여** 스크립트를 편집하면 재승인이 필요합니다. 자동화는 `--dangerously-bypass-hook-trust`로 우회합니다.
- `config.example.yaml`의 `agents:` 목록에 `codex` 항목을 추가합니다(필드는 4.4.1과 동일).
- **런타임 의존성**: `jq`.

#### 4.6.2. 훅 이벤트 → kitt-eye 상태 매핑

| Codex 이벤트 | matcher | kitt-eye 상태 | `detail` 출처 |
| --- | --- | --- | --- |
| `SessionStart` | `startup\|resume\|clear\|compact` | `idle` | `source` |
| `UserPromptSubmit` | 무시 | `thinking` | `prompt` 앞 60자 |
| `PreToolUse` | `tool_name` | `executing_tool` | `tool_name` + `tool_input` 요약 |
| `PostToolUse` | `tool_name` | `executing_tool` (`tool_response`에 오류 포함 시 `error`) | `<tool> 완료/실패` |
| `PermissionRequest` | `tool_name` | `waiting_input` | 승인 대기 중인 `tool_name` |
| `PreCompact` | `manual\|auto` | `thinking` | `compacting` |
| `PostCompact` | `manual\|auto` | `thinking` | `compacted` |
| `SubagentStart` | agent_type | `executing_tool` | `subagent: <agent_type>` |
| `SubagentStop` | agent_type | `thinking` | `subagent 완료` |
| `Stop` | 무시 | `done` | `last_assistant_message` 앞 60자 |
| `Interrupt` | 무시 | `idle` | `interrupted` |
| `SessionEnd` | reason(현재 `other`만 확인) | `idle` | `reason` |

- payload는 Claude Code 호환입니다: `session_id`/`transcript_path`/`cwd`/`hook_event_name`/`model` + `turn_id`/`permission_mode`/`tool_name`/`tool_use_id`/`tool_input`/`tool_response`. **`Interrupt`가 신규 이벤트** — `Esc` 중단을 관측하므로 4.4.6 #1의 공백을 메워줍니다.
- matcher 커버리지: `Bash`(shell + unified exec 모두), `apply_patch`(`Edit`/`Write` 별칭 포함), MCP 도구(`mcp__<server>__<tool>`), 로컬 function tool(`update_plan`, `spawn_agent`→`Agent`)까지 잡습니다. **hosted 도구(WebSearch 등)는 tool 훅에 잡히지 않습니다** → `executing_tool`의 블라인드 스팟.
- 실패한 Bash도 `PostToolUse`가 발화하며 오류는 `tool_response` 안에 실려 옵니다(도구별 스키마 상이 → Step 3 검증).
- 레거시 `notify` 설정은 deprecated이므로 사용하지 않습니다.

#### 4.6.3. `hooks.json` 스니펫 (기본 세트)

```json
{
  "hooks": {
    "SessionStart":      [{ "hooks": [{ "type": "command", "command": "$HOME/.kitt-eye/bin/kitt-eye-codex-hook", "timeout": 2 }] }],
    "UserPromptSubmit":  [{ "hooks": [{ "type": "command", "command": "$HOME/.kitt-eye/bin/kitt-eye-codex-hook", "timeout": 2 }] }],
    "PreToolUse":        [{ "hooks": [{ "type": "command", "command": "$HOME/.kitt-eye/bin/kitt-eye-codex-hook", "timeout": 2 }] }],
    "PostToolUse":       [{ "hooks": [{ "type": "command", "command": "$HOME/.kitt-eye/bin/kitt-eye-codex-hook", "timeout": 2 }] }],
    "PermissionRequest": [{ "hooks": [{ "type": "command", "command": "$HOME/.kitt-eye/bin/kitt-eye-codex-hook", "timeout": 2 }] }],
    "PreCompact":        [{ "hooks": [{ "type": "command", "command": "$HOME/.kitt-eye/bin/kitt-eye-codex-hook", "timeout": 2 }] }],
    "PostCompact":       [{ "hooks": [{ "type": "command", "command": "$HOME/.kitt-eye/bin/kitt-eye-codex-hook", "timeout": 2 }] }],
    "SubagentStart":     [{ "hooks": [{ "type": "command", "command": "$HOME/.kitt-eye/bin/kitt-eye-codex-hook", "timeout": 2 }] }],
    "SubagentStop":      [{ "hooks": [{ "type": "command", "command": "$HOME/.kitt-eye/bin/kitt-eye-codex-hook", "timeout": 2 }] }],
    "Stop":              [{ "hooks": [{ "type": "command", "command": "$HOME/.kitt-eye/bin/kitt-eye-codex-hook", "timeout": 2 }] }],
    "Interrupt":         [{ "hooks": [{ "type": "command", "command": "$HOME/.kitt-eye/bin/kitt-eye-codex-hook", "timeout": 2 }] }],
    "SessionEnd":        [{ "hooks": [{ "type": "command", "command": "$HOME/.kitt-eye/bin/kitt-eye-codex-hook", "timeout": 2 }] }]
  }
}
```

- 모양은 Claude와 동일한 `[{ matcher?, hooks: [{ type: "command", ... }] }]`입니다. 핸들러에 `commandWindows`, `async`, `statusMessage`, `additionalContextLimit` 추가 필드가 있지만 kitt-eye는 사용하지 않습니다(`mcp_tool` 핸들러도 미사용).
- 기본 timeout은 600초 → `timeout: 2`로 통일합니다. `SessionEnd`/`Interrupt`는 기본 1초에 **최대 3초** 상한이라 2초가 적합합니다.

#### 4.6.4. 훅 스크립트 계약 (`hooks/codex/hook.sh`)

**Claude(4.4.4)와 다른 규칙 — stdout 계약이 더 엄격**

1. **plain stdout의 두 얼굴**:
   - `SessionStart`/`UserPromptSubmit`의 plain stdout은 **developer 컨텍스트로 모델에 주입**됩니다 → plain 텍스트 출력 절대 금지.
   - `Stop`/`SubagentStop`은 exit 0에서 **JSON stdout을 요구**합니다 — plain 텍스트는 invalid(훅 실패) → `{}`만 출력합니다.
   - 그 외 이벤트는 exit 0의 plain stdout을 무시하지만 kitt-eye는 원칙적으로 무출력입니다.
2. **exit 2 = 차단**(도구 호출/프롬프트/중단) → 관찰자에게는 금기, `trap 'exit 0' EXIT`로 강제. `PreToolUse`가 미지원 필드(`continue`/`stopReason`/`suppressOutput`)를 반환하면 도구는 fail-open으로 진행하지만 "hook failed"로 기록됩니다 → **Stop/SubagentStop 외에는 decision JSON을 출력하지 않습니다**.
3. **`async: true`**는 훅을 백그라운드 실행(동시 8개上限, 차단 불가, `SessionEnd`는 항상 sync)하지만 kitt-eye는 이미 socket fire-and-forget이므로 불필요합니다.
4. 나머지 규칙(`jq` 파싱, `detail` 살균, 빠른 종료)은 4.4.4와 동일합니다.

```bash
#!/usr/bin/env bash
# hooks/codex/hook.sh : Codex CLI 훅 stdin JSON -> kitt-eye 이벤트 변환기
# 관찰자 전용: 항상 exit 0. plain stdout 금지, Stop/SubagentStop만 JSON stdout("{}") 필수.
set -u
trap 'exit 0' EXIT

INPUT="$(cat)"; [ -n "$INPUT" ] || exit 0
command -v jq >/dev/null 2>&1 || exit 0
SEND="${HOME}/.kitt-eye/bin/kitt-eye-send"; [ -x "$SEND" ] || exit 0

IFS=$'\t' read -r EVENT STATE SID DETAIL < <(printf '%s' "$INPUT" | jq -r '
  def clean: tostring | gsub("[\r\n\t\"]"; " ") | .[0:60];
  .hook_event_name as $e
  | (if   $e == "SessionStart"      then "idle"
     elif $e == "SessionEnd"        then "idle"
     elif $e == "Interrupt"         then "idle"
     elif $e == "UserPromptSubmit"  then "thinking"
     elif $e == "PreCompact"        then "thinking"
     elif $e == "PostCompact"       then "thinking"
     elif $e == "SubagentStop"      then "thinking"
     elif $e == "PreToolUse"        then "executing_tool"
     elif $e == "SubagentStart"     then "executing_tool"
     elif $e == "PostToolUse" and ((.tool_response | tostring) | test("error")) then "error"
     elif $e == "PostToolUse"       then "executing_tool"
     elif $e == "PermissionRequest" then "waiting_input"
     elif $e == "Stop"              then "done"
     else "idle" end) as $st
  | (if   .tool_name                 then .tool_name + " " + ((.tool_input.command
             // .tool_input.file_path // .tool_input.pattern // .tool_input.query
             // .tool_input.url // "") | clean)
     elif .notification_type         then (.notification_type | clean)
     elif .last_assistant_message    then (.last_assistant_message | clean)
     elif .prompt                    then (.prompt | clean)
     elif .reason                    then (.reason | clean)
     else "" end | sub(" +$"; "")) as $det
  | [ $e, $st, (.session_id // "default"), $det ] | @tsv')

[ -n "$EVENT" ] || exit 0
"$SEND" codex "$STATE" "$DETAIL" "$SID" "$EVENT" || true

# Stop/SubagentStop은 exit 0에서 plain 텍스트 stdout이 invalid → JSON 계약 유지
case "$EVENT" in Stop|SubagentStop) printf '{}\n' ;; esac
exit 0
```

> `PostToolUse`의 실패 감지 `test("error")`는 휴리스틱입니다 — `tool_response` 스키마가 도구마다 달라서 Step 3 검증 후 정확한 필드 참조로 교체합니다.

#### 4.6.5. 설치 / 제거 (JSON 병합 + trust)

- **install**: bin 복사/백업/멱등 병합 흐름은 4.4.5와 동일하고, 병합 대상이 `~/.codex/hooks.json`의 `hooks` 키(없으면 `{}` 생성)라는 점만 다릅니다.
- **trust (필수 단계)**: 설치 후 codex 세션을 열어 `/hooks` → kitt-eye 훅을 review/승인합니다. **command 문자열을 바꿔 재설치하면 해시 불일치로 trust가 해제되므로 매번 재승인**이 필요합니다. CI/자동화는 `--dangerously-bypass-hook-trust`를 사용합니다.
- **uninstall**: `kitt-eye-codex-hook`을 `command`에 포함한 항목만 선별 제거, bin 삭제. 남은 trust 항목은 해시 불일치로 무해화됩니다.
- **검증**: `jq '.hooks | keys' ~/.codex/hooks.json`, codex `/hooks`에서 trusted 상태 확인.

#### 4.6.6. 주의사항 체크리스트

1. **trust review이 1순위 디버깅 대상**: 설치/스크립트 편집 후 `/hooks` 승인을 안 하면 훅이 조용히 스킵됩니다. 프로젝트 레이어도 별도 trust가 필요합니다.
2. **plain 텍스트 출력 금지**: `SessionStart`/`UserPromptSubmit`은 stdout이 모델 컨텍스트를 오염시키고, `Stop`/`SubagentStop`은 invalid JSON으로 실패 취급됩니다. `{}` 외에는 무출력 계약을 지킵니다.
3. **exit 2 금기**, Stop/SubagentStop 외 decision JSON 미출력(fail-open이지만 hook-failed 기록이 남습니다).
4. **timeout 예산**: `timeout: 2` 통일. `SessionEnd`/`Interrupt`는 3초 상한, 기본 600초 방치는 위험합니다.
5. **hosted 도구 블라인드**: WebSearch 등은 tool 훅이 발화하지 않아 `executing_tool`이 켜지지 않을 수 있습니다.
6. **레이어 공존**: `hooks.json`과 `config.toml`의 `[hooks]`가 동시에 실행되므로 설치 스크립트에서 중복을 방지합니다.
7. **`Interrupt` 가치**: 4개 CLI 중 유일하게 `Esc` 중단을 명시적 이벤트로 관측합니다 — publisher의 TTL 단축 판단에 활용합니다.
8. **레거시 `notify` 금지**: deprecated된 notify 설정과 혼용하지 않습니다.
9. **프라이버시**: `prompt`/`last_assistant_message`/`tool_input.command`는 4.4.6 #11과 동일한 `detail: minimal` 정책을 적용합니다.
10. **팀 공유 설치**: `<repo>/.codex/` 훅은 그 프로젝트 레이어가 trusted일 때만 로드됩니다 — 클론 직후 멤버는 `/hooks` 승인이 필요합니다.

### 4.7. Cursor CLI (`cursor-cli` / `cursor-agent`) 훅 연동

`cursor-agent`는 Cursor IDE와 **공식 Hooks를 공유**합니다(`~/.cursor/hooks.json`). 스키마는 버전화(`"version": 1`)되고 이벤트명은 camelCase이며, exit code 의미가 Claude와 다릅니다(0=성공/2=차단/그 외=fail-open). **로컬 CLI에서 모든 이벤트가 발화하지는 않는 것**이 핵심 리스크이며, 문서와 공식 답변에서 확인된 최소 세트가 기본값입니다(4.7.2).

#### 4.7.1. 설정 위치

| 위치 | 범위 | 용도 |
| --- | --- | --- |
| `~/.cursor/hooks.json` | 사용자 전역 (**Cursor IDE와 공유**) | kitt-eye 기본 설치 위치. 훅은 cwd `~/.cursor/`에서 실행 |
| `<project root>/.cursor/hooks.json` | 단일 프로젝트 | cwd=프로젝트 루트. 스크립트 참조는 루트 기준 `.cursor/hooks/x.sh` 방식 |
| `/Library/Application Support/Cursor/hooks.json` | Enterprise (macOS) | 관리 정책 레이어(최우선, kitt-eye 설치 대상 아님) |

- 우선순위: Enterprise → Team → Project → User.
- **file watcher가 자동 재로드**하므로 재시작이 필요 없습니다.
- 사용자 전역 파일은 IDE와 공유 → **IDE 세션에서도 같은 훅이 발화**해 LED에 노이즈가 됩니다. CLI만 모니터링하려면 프로젝트 레이어 설치 또는 publisher 측 구분을 권장합니다.
- `config.example.yaml`의 `agents:` 목록에 `cursor-cli` 항목을 추가합니다(필드는 4.4.1과 동일).
- **런타임 의존성**: `jq`.

#### 4.7.2. 훅 이벤트 → kitt-eye 상태 매핑

| Cursor 이벤트 (camelCase) | matcher (regex) | kitt-eye 상태 | `detail` 출처 | 로컬 CLI 발화 |
| --- | --- | --- | --- | --- |
| `sessionStart` | — | `idle` | `source` | 확인됨 |
| `beforeShellExecution` | command 텍스트 | `executing_tool` | `command` 앞 60자 | 확인됨 |
| `afterShellExecution` | — | `executing_tool` | `command 완료` | 확인됨 |
| `afterFileEdit` | — | `executing_tool` | `file_path` basename | 확인됨 |
| `postToolUse` | 도구명 | `executing_tool` | `tool_name` 완료 | 확인됨 |
| `stop` | — | `completed`→`done` / `aborted`→`idle` / `error`→`error` | `status` (+`loop_count`) | 확인됨 |
| `beforeSubmitPrompt` | — | `thinking` | `prompt` 앞 60자 | 검증 필요 |
| `preToolUse` | 도구명 (`Shell\|Read\|Write\|Grep\|Delete\|Task\|MCP:*`) | `executing_tool` | `tool_name` + `tool_input` | 검증 필요 |
| `postToolUseFailure` | 도구명 | `error` | `error_message` + `failure_type` | 검증 필요 |
| `beforeMCPExecution` / `afterMCPExecution` | MCP 도구명 | `executing_tool` | MCP 도구명 | 검증 필요 |
| `preCompact` | — | `thinking` | `compacting` | 검증 필요 |
| `sessionEnd` | reason | `idle` | `reason` (`completed\|aborted\|error\|window_close\|user_close`) | 검증 필요 |
| `afterAgentThought` / `afterAgentResponse` | — | (`generating` 후보) | `text` 앞 60자 | **로컬 CLI 미발화** |

- "확인됨" 기준: 공식 문서 + Cursor 측 공식 답변(forum, 2026-06). "검증 필요" 세트는 `cursor-agent` 버전을 대상으로 스모크 테스트해 확장 세트를 확정합니다(Step 3). 발화하지 않는 이벤트는 신호만 없을 뿐 에이전트에 해가 없습니다.
- **권한 요청 직전 이벤트가 없습니다** → `waiting_input`은 사실상 관측 불가. `postToolUseFailure`의 `failure_type: permission_denied`는 사후 정보입니다.
- `afterAgentThought`/`afterAgentResponse`가 로컬에서 발화하지 않으므로 `generating`은 cursor-cli에서 관측 불가합니다.
- 공통 payload: `conversation_id`(→ `session_id`), `generation_id`, `model`, `hook_event_name`, `cursor_version`, `workspace_roots`, `user_email`, `transcript_path` + `CURSOR_PROJECT_DIR` 계열 환경변수. payload 필드명은 snake_case, 이벤트 값은 camelCase입니다.
- Tab 계열/`workspaceOpen` 등 추가 이벤트가 있지만 IDE 중심이라 상태 매핑에서 제외합니다.

#### 4.7.3. `hooks.json` 스니펫 (기본 세트 — 확인된 6개 이벤트)

```json
{
  "version": 1,
  "hooks": {
    "sessionStart":         [{ "command": "$HOME/.kitt-eye/bin/kitt-eye-cursor-hook", "type": "command", "timeout": 2 }],
    "beforeShellExecution": [{ "command": "$HOME/.kitt-eye/bin/kitt-eye-cursor-hook", "type": "command", "timeout": 2 }],
    "afterShellExecution":  [{ "command": "$HOME/.kitt-eye/bin/kitt-eye-cursor-hook", "type": "command", "timeout": 2 }],
    "afterFileEdit":        [{ "command": "$HOME/.kitt-eye/bin/kitt-eye-cursor-hook", "type": "command", "timeout": 2 }],
    "postToolUse":          [{ "command": "$HOME/.kitt-eye/bin/kitt-eye-cursor-hook", "type": "command", "timeout": 2 }],
    "stop":                 [{ "command": "$HOME/.kitt-eye/bin/kitt-eye-cursor-hook", "type": "command", "timeout": 2 }]
  }
}
```

- **matcher 그룹 중첩이 없습니다** — claude/codex와 달리 handler가 flat하고 `matcher`가 handler 안의 **regex 문자열**입니다(생략 시 전체 매칭). `beforeShellExecution`은 command 텍스트, `pre`/`postToolUse`는 도구명에 적용됩니다.
- 확장 세트(`beforeSubmitPrompt`/`preToolUse`/`postToolUseFailure`/`preCompact`/`sessionEnd`/`beforeMCPExecution`/`afterMCPExecution`)는 동일 형태로 추가하되, Step 3의 버전별 검증 후 활성화합니다.
- `afterAgentThought`/`afterAgentResponse`는 등록하지 않습니다(로컬 CLI 미발화).

#### 4.7.4. 훅 스크립트 계약 (`hooks/cursor-cli/hook.sh`)

**Claude(4.4.4)와 다른 규칙**

1. **exit code 의미**: 0=성공(stdout JSON 파싱), 2=차단, 그 외=fail-open 경고. `trap 'exit 0' EXIT`로 0을 강제합니다.
2. **stdout**: 무출력 또는 `{}`만. permission/decision 계열 키는 출력하지 않습니다(Cursor는別 메커니즘).
3. **이벤트명은 payload의 `hook_event_name`에 존재**합니다 — cursor는 argv 주입이 불필요(4.5와 대비).
4. **cwd 함정**: 사용자 전역 훅은 cwd `~/.cursor/`에서 실행 → command는 절대 경로만 사용합니다(kitt-eye bin이 절대 경로라 이미 충족).
5. 나머지 규칙(`jq`, `detail` 살균, 빠른 종료)은 4.4.4와 동일합니다.

```bash
#!/usr/bin/env bash
# hooks/cursor-cli/hook.sh : Cursor CLI 훅 stdin JSON -> kitt-eye 이벤트 변환기
# exit: 항상 0 (2=차단, 그 외=fail-open 경고). stdout: 무출력 또는 "{}"만.
set -u
trap 'exit 0' EXIT

INPUT="$(cat)"; [ -n "$INPUT" ] || exit 0
command -v jq >/dev/null 2>&1 || exit 0
SEND="${HOME}/.kitt-eye/bin/kitt-eye-send"; [ -x "$SEND" ] || exit 0

IFS=$'\t' read -r EVENT STATE SID DETAIL < <(printf '%s' "$INPUT" | jq -r '
  def clean: tostring | gsub("[\r\n\t\"]"; " ") | .[0:60];
  .hook_event_name as $e
  | (if   $e == "sessionStart"         then "idle"
     elif $e == "sessionEnd"           then "idle"
     elif $e == "beforeSubmitPrompt"   then "thinking"
     elif $e == "preCompact"           then "thinking"
     elif $e == "postToolUseFailure"   then "error"
     elif $e == "stop" and ((.status // "") == "aborted") then "idle"
     elif $e == "stop" and ((.status // "") == "error")   then "error"
     elif $e == "stop"                 then "done"
     else "executing_tool" end) as $st
  | (if   .command       then ("$ " + (.command | clean))
     elif .file_path     then ((.file_path | split("/") | last) | clean)
     elif .error_message then ((.failure_type // "error") | clean) + ": " + (.error_message | clean)
     elif .status        then (.status | clean)
     elif .reason        then (.reason | clean)
     elif .prompt        then (.prompt | clean)
     elif .tool_name     then .tool_name
     else "" end | sub(" +$"; "")) as $det
  | [ $e, $st, (.conversation_id // "default"), $det ] | @tsv')

[ -n "$EVENT" ] || exit 0
"$SEND" cursor-cli "$STATE" "$DETAIL" "$SID" "$EVENT" || true
exit 0
```

> `else "executing_tool"` 분기는 tool/MCP/shell 계열 이벤트를 잡습니다. 확장 세트 등록 전 발화 미확인 이벤트에는 도달하지 않으므로 안전합니다.

#### 4.7.5. 설치 / 제거 (JSON 병합)

- **install**: `hooks/cursor-cli/hook.sh`를 `~/.kitt-eye/bin/kitt-eye-cursor-hook`으로 복사 + `chmod +x` → 대상 `~/.cursor/hooks.json`(또는 프로젝트 `.cursor/hooks.json`) 보장(없으면 `{"version":1,"hooks":{}}` 생성) → 백업 → `command`에 `kitt-eye-cursor-hook`을 포함한 항목 제거(멱등) → 이벤트 키별 병합(`"version"` 보존) → JSON 검증 후 원자적 교체. file watcher가 즉시 반영합니다.
- **uninstall**: 위 필터로 kitt-eye 항목만 선별 제거하고 `kitt-eye-cursor-hook` 삭제, 사용자 훅은 유지.
- **검증**: `jq '.hooks | keys' ~/.cursor/hooks.json` + `cursor-agent -p "..."` 실행과 `mosquitto_sub`으로 등록 이벤트별 발화 확인.

#### 4.7.6. 주의사항 체크리스트

1. **부분적 CLI 지원이 1순위 리스크**: 확인된 기본 6개 외에는 `cursor-agent` 버전마다 다릅니다. 공식 CLI 전용 문서가 없으므로 업그레이드마다 스모크 테스트로 확장 세트를 갱신합니다(4.7.2).
2. **`generating`/`waiting_input` 관측 불가**: thought 계열 미발화 + 권한 사전 이벤트 부재 → publisher TTL과 `stop`의 `status`로 보완합니다.
3. **exit code 계약**: 비(0) exit은 fail-open 경고로 기록되고 2는 차단을 걸므로 항상 0을 유지합니다. stdout은 무출력 또는 `{}`.
4. **IDE 파일 공유**: `~/.cursor/hooks.json`은 IDE에서도 발화합니다. CLI 전용 원리면 프로젝트 레이어 설치 또는 publisher 측 `workspace_roots`/`cursor_version` 구분 처리가 필요합니다.
5. **matcher는 regex**: `beforeShellExecution`은 command 텍스트에 regex를 적용하므로 탈자 처리를 잘못하면 매칭이 깨집니다. kitt-eye 기본은 matcher 생략(전체 매칭)입니다.
6. **invalid JSON stdout은 실패 취급**(fail-open) — 파싱 실패 시에도 `{}`만 출력하는 계약이 안전합니다.
7. **`failClosed` 미설정 유지**: 기본 false. true로 켜면 훅 실패가 에이전트 기능을 차단합니다. `type: "prompt"`/`loop_limit` 계열 리뷰 훅도 사용하지 않습니다.
8. **프라이버시**: `command`/`prompt`/`text`는 4.4.6 #11의 `detail: minimal` 정책을 따르고, payload의 `user_email`은 어떤 상태에도 태우지 않습니다.

---

## 5. 단계별 개발 로드맵

1. **Step 1: 환경 구성 및 기본 디렉토리 셋업**
   - 저장소 구조 초기화 및 설정 파일 포맷 확정
   - 훅 런타임 의존성(`jq`) 및 대상 CLI 목록(`antigravity-cli`, `claude`, `codex`, `cursor-cli`) 확정
2. **Step 2: Go Publisher 구현 및 MQTT 연동 테스트**
   - IPC 소켓 리스너, MQTT 퍼블리셔 구현
   - Mosquitto 브로커 연동 및 `mosquitto_sub`로 발행 검증
3. **Step 3: CLI Hooks & Makefile 작성**
   - `claude` 훅 구현: `hooks/claude/hook.sh`(stdin JSON → 상태 매핑) + `settings.snippet.json`(4.4.2 매핑, 4.4.4 계약 준수)
   - `codex` 훅 구현: `hooks/codex/hook.sh` + `hooks.json` 스니펫(4.6). `Stop`/`SubagentStop`은 stdout `{}` 필수, 설치 후 `/hooks` trust review 검증
   - `antigravity-cli` 훅 구현: `hooks/antigravity-cli/hook.sh` + `kitt-eye` named bundle(4.5). 이벤트명 argv 주입, `PreToolUse` 기본 미등록, trustedWorkspaces trust 확인
   - `cursor-cli` 훅 구현: `hooks/cursor-cli/hook.sh` + `hooks.json` 스니펫(4.7). 검증된 6개 이벤트 우선, `cursor-agent` 버전별 이벤트 스모크 테스트로 확장 세트 확정
   - `make install-hooks` 타깃 구현: 4 CLI 모두 `install-hooks-<agent>` 서브타깃으로 `jq` JSON 병합/백업/멱등 설치(4.4.5·4.5.5·4.6.5·4.7.5)
   - `send_event.sh` v1.1: `session_id`/`event` 인자 추가 및 `detail` JSON 이스케이프 처리
4. **Step 4: ESP32-C3 Rust 펌웨어 작성**
   - `esp-idf-template` 기반 프로젝트 구성
   - Wi-Fi 및 MQTT 클라이언트 구독 로직 작성
   - WS2812B LED 애니메이션 제어 루프 구현
5. **Step 5: 통합 테스트 & Home Assistant 연동**
   - CLI 실행 -> Hook -> Go Pub -> Mosquitto -> ESP32-C3 LED 반응 실시간 검증
   - `claude` 검증: `claude -p "..."` 실행 중 `mosquitto_sub -v -t 'kitt-eye/#'`로 이벤트 흐름 확인 → `PreToolUse`(빨간 스캐너) → `Stop`(초록) 순서 관측
   - `claude` 엣지 케이스: 동시 세션 2개(멀티 세션 집계), `Esc` 중단(TTL 만료), 권한 요청(`waiting_input`), 훅 stdout 무출력 및 exit 0 확인
   - Home Assistant 대시보드 상태 카드 연동
