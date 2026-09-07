# kitt-eye 🚗💨

> AI 코딩 에이전트의 상태를 MQTT로 수집해서, ESP32-C3 기반 K.I.T.T. 스캐너 LED(및 Home Assistant)로 시각화하는 홈랩 모니터링 시스템

`claude`, `codex`, `cursor-cli`, `antigravity-cli` 같은 AI 에이전트 CLI의 라이프사이클 훅을 감지해 현재 동작 상태(Thinking / Coding / Tool Execution / Waiting Input 등)를 수집하고, Go 데몬이 MQTT 브로커(Home Assistant Mosquitto)로 발행하면 ESP32-C3 펌웨어(Rust)가 이를 LED 패턴으로 보여줍니다.

## 왜 만들었나

여러 터미널/세션에서 AI 에이전트를 돌려놓으면 "지금 뭐 하고 있지? 끝나긴 한 건가? 나를 기다리고 있나?"를 확인할 방법이 없습니다. kitt-eye는 방 건너편에서도 한눈에 알 수 있게 해 줍니다.

| LED 패턴 (계획) | 상태 | 의미 |
|---|---|---|
| 🔴 K.I.T.T. 스캐너 (좌우 왕복) | `executing_tool` | 도구/쉘/파일 작업 실행 중 |
| 🔵 Breathing (호흡) | `idle` | 대기 |
| 🟣 Pulsing (점멸) | `thinking` | 추론 중 |
| 🔵 Fast Sweep | `generating` | 응답/코드 생성 중 |
| 🟠 점멸 | `waiting_input` | **사용자 확인/입력 대기** |
| 🟢 Solid / Rapid Green Flash | `done` | 완료 |
| 🔴 Blinking Red | `error` | 실패 |

## 시스템 아키텍처

```text
[ AI CLI Tools ]
 ├── claude          (~/.claude/settings.json hooks)
 ├── codex           (~/.codex/hooks.json + /hooks trust)
 ├── cursor-cli      (~/.cursor/hooks.json)
 └── antigravity-cli (hooks.json named bundle)
         │
         │  훅 스크립트 → Unix Domain Socket (/tmp/kitt-eye.sock)
         │  한 줄 JSON 데이터그램, fire-and-forget
         ▼
[ kitt-eye Publisher (Go 데몬) ]
  · 세션별 상태 추적 + TTL 만료 + 우선순위 집계
  · LWT: 데몬 사망 시 자동 offline 발행
         │
         │  MQTT (retained, QoS 1): kitt-eye/#
         ▼
[ HomeLab MQTT Broker (Home Assistant Mosquitto) ]
         │
         │  MQTT Subscribe: kitt-eye/#
         ▼
[ ESP32-C3 (Rust / esp-idf-svc) ]           [ Home Assistant ]
 └── WS2812B NeoPixel K.I.T.T. 스캐너       └── MQTT Discovery 센서 (옵션)
```

## 상태 스펙 & 집계 우선순위

수집되는 상태는 7종이며, 한 에이전트의 여러 세션(및 전체 에이전트) 중 **가장 높은 우선순위** 상태가 대표 상태(`active_state`)로 승격됩니다.

```
error > waiting_input > executing_tool > generating > thinking > done > idle
```

`done`/`error`는 TTL이 지나면 자동으로 `idle`로 복귀하고, events가 멈춘 세션(크래시 등)도 stale TTL로 회수됩니다.

## 저장소 구조

```text
kitt-eye/
├── Makefile                  # 빌드/실행/훅 설치/테스트 통합 관리
├── config.example.yaml       # Publisher 설정 템플릿
├── PLAN.md                   # 상세 설계: 훅 매핑 규격, MQTT 프로토콜, 로드맵
├── hooks/
│   ├── common/
│   │   ├── send_event.sh     # UDS 전송기 (v1.1: session_id/event, jq JSON 인코딩)
│   │   └── merge_hooks.sh    # CLI 설정 JSON 멱등 병합/제거 (jq 필수)
│   ├── claude/               # Claude Code: hook.sh + settings.snippet.json
│   ├── codex/                # Codex: hook.sh + hooks.snippet.json
│   ├── cursor-cli/           # Cursor CLI: hook.sh + hooks.snippet.json
│   └── antigravity-cli/      # Antigravity (agy): hook.sh + named-bundle 스니펫
├── pub-go/                   # Go Publisher 데몬  (module github.com/suapapa/kitt-eye/pub)
│   ├── cmd/kitteye/          # 진입점
│   └── internal/
│       ├── config/           # YAML 설정 로더 (기본값/검증/env 오버라이드)
│       ├── ipc/              # Unix Domain Socket 서버 (훅 이벤트 수신)
│       ├── model/            # 이벤트 와이어 포맷 · 상태 사전 · 우선순위
│       ├── state/            # 세션 상태 저장소 (TTL, 집계, janitor)
│       ├── publish/          # Sink 인터페이스 + dry-run(stdout) sink
│       └── mqtt/             # paho 기반 MQTT 클라이언트 (retained, LWT, HA Discovery)
└── mcu-rust/                 # ESP32-C3 Rust 펌웨어
    └── Cargo.toml            # esp-idf-svc / smart-leds / ws2812-esp32-rmt-driver
```

## 구현 현황

| 컴포넌트 | 상태 |
|---|---|
| Go Publisher (`pub-go`) | ✅ 구현 완료 — IPC 수신, 세션/TTL 상태 집계, MQTT retained 발행, LWT, HA Discovery, dry-run 모드 (단위 테스트 + 선택적 실브로커 스모크 테스트) |
| 설정 / Makefile | ✅ `config.example.yaml` + `make` 타깃 (`install-hooks-*` 포함) |
| 공통 훅 전송기 (`hooks/common/send_event.sh`) | ✅ v1.1 — `session_id`/`event`, `jq`로 JSON 이스케이프, stdout 무출력 |
| CLI별 훅 스크립트 (`hooks/{claude,codex,cursor-cli,antigravity-cli}/`) | ✅ bash+jq 관찰자 훅 + 스니펫 + `make install-hooks` 멱등 병합 |
| ESP32-C3 펌웨어 (`mcu-rust`) | 🚧 매니페스트(`Cargo.toml`)만 존재, `src/` 없음 — 현재 빌드 불가 |
| Home Assistant 연동 | ✅ 퍼블리셔 측 MQTT Discovery 발행 지원 (`homeassistant.discovery: true`) |

## 빠른 시작

### 요구 사항

- Go 1.25+
- **`jq`** — CLI 훅 파싱·설치(`make install-hooks`)·`send_event.sh` JSON 인코딩에 **필수**. macOS: `brew install jq` / Debian·Ubuntu: `apt install jq`
- `nc` (Unix domain socket 지원, macOS 기본 포함) — 훅 → 데몬 IPC 전송
- (MQTT 모드만) Mosquitto 등 MQTT 브로커 — Home Assistant의 Mosquitto 애드온 권장

### 1) 브로커 없이 dry-run으로 시작하기

브로커가 없어도 전체 파이프라인을 확인할 수 있습니다. `mqtt.broker`가 비어 있으면 데몬이 상태를 stdout으로 JSON 라인 출력합니다.

```sh
cp config.example.yaml config.yaml
# config.yaml에서 mqtt.broker를 ""(빈 값)으로 두면 dry-run

make run-pub
# → {"type":"status","status":"online"} 로 시작
```

다른 터미널에서 이벤트를 직접 던져 봅니다:

```sh
./hooks/common/send_event.sh claude executing_tool "Bash: npm test"
./hooks/common/send_event.sh claude done "task complete"
```

dry-run 터미널에 agent/active/status 레코드가 찍히면 동작 확인 완료입니다.

### 2) MQTT 브로커에 연결하기

`config.yaml`을 브로커 정보에 맞게 수정합니다.

```yaml
mqtt:
  broker: "homeassistant.local"   # 빈 값이면 dry-run
  port: 1883
  username: "mqtt_user"
  topic_prefix: "kitt-eye"

homeassistant:
  discovery: true                 # HA 센서 자동 등록
```

비밀번호는 파일에 쓰지 않고 환경변수로 전달하는 것을 권장합니다:

```sh
export KITTEYE_MQTT_PASSWORD='***'
make run-pub
```

발행되는 주제를 관찰해 확인합니다:

```sh
mosquitto_sub -h homeassistant.local -v -t 'kitt-eye/#'
```

### 3) CLI 훅 연동

`jq`가 PATH에 있어야 합니다. Publisher 데몬을 띄운 뒤 훅을 설치합니다:

```sh
make install-hooks              # 4 CLI 전부 (멱등 병합 + 백업)
# 또는 개별:
make install-hooks-claude
make install-hooks-codex
make install-hooks-cursor-cli
make install-hooks-antigravity-cli
```

설치 내용:
- `~/.kitt-eye/bin/kitt-eye-send` + CLI별 `kitt-eye-*-hook`
- 각 CLI 설정 JSON에 스니펫 병합 (기존 사용자 훅 유지, kitt-eye 항목만 교체)

수동 전송 테스트:

```sh
~/.kitt-eye/bin/kitt-eye-send claude thinking "analyzing prompt" sess1 UserPromptSubmit
```

| CLI | 설정 위치 | 설치 후 확인 |
|---|---|---|
| `claude` | `~/.claude/settings.json` | `/hooks` 메뉴 · workspace trust |
| `codex` | `~/.codex/hooks.json` | **`/hooks`에서 kitt-eye 승인 필수** (커맨드 해시 trust — 재설치마다 재승인) |
| `cursor-cli` | `~/.cursor/hooks.json` | file watcher 즉시 반영 (IDE와 설정 공유) |
| `antigravity-cli` | `~/.gemini/config/hooks.json` (`kitt-eye` bundle) | **trustedWorkspaces** 승인 |

제거 (kitt-eye 항목만 선별 삭제, 다른 훅 유지):

```sh
make uninstall-hooks
```

이벤트 매핑·stdout 계약 상세는 [PLAN.md](./PLAN.md) §4.4–4.7.

### 4) ESP32-C3 펌웨어 (예정)

`mcu-rust`는 `Cargo.toml`만 올라간 스캐폴딩 상태입니다. 구현되면 `espup`/`riscv32imc-esp-espidf` 툴체인 아래 다음 명령으로 빌드·플래시합니다:

```sh
make build-mcu   # cargo build --release (riscv32imc-esp-espidf)
make flash-mcu   # cargo espflash flash --release --monitor
```

## MQTT 토픽 규격

모두 retained QoS 1이며, prefix는 `mqtt.topic_prefix`(기본 `kitt-eye`)입니다. ESP32가 늦게 부팅해도 retained로 즉시 현재 상태를 얻습니다.

| 토픽 | 페이로드 | 용도 |
|---|---|---|
| `kitt-eye/agents/<agent>/state` | 상태 문자열 (`executing_tool` 등) | 에이전트 대표 상태 |
| `kitt-eye/agents/<agent>/sessions` | `AgentSnapshot` JSON (세션 배열) | 멀티 세션 구분 |
| `kitt-eye/active_state` | 상태 문자열 | 전체 대표 상태 — **MCU 구독용** |
| `kitt-eye/active_agent` | 에이전트 이름 | 어느 에이전트인지 |
| `kitt-eye/system/status` | `online` / `offline` | 가용성 · LWT (비정상 종료 시 broker가 `offline` 발행) |
| `homeassistant/sensor/kitt_eye_*/config` | Discovery 설정 JSON | HA 센서 자동 등록 (옵션) |

`AgentSnapshot` 예시:

```json
{
  "agent": "claude",
  "state": "executing_tool",
  "sessions": [
    {"session_id": "abc123", "state": "executing_tool", "detail": "Bash: npm test", "event": "PreToolUse", "updated_at": "2026-09-07T11:26:00Z"}
  ],
  "updated_at": "2026-09-07T11:26:00Z"
}
```

## 훅 → 데몬 IPC 이벤트 포맷

훅 스크립트는 `/tmp/kitt-eye.sock`(Unix stream socket)에 한 줄 JSON을 쓰고 바로.exit합니다(차단·백프레셔 없음).

```json
{"agent": "cursor-cli", "state": "executing_tool", "detail": "Running git diff", "timestamp": "2026-09-07T11:26:00Z"}
```

v1.1 확장(선택 필드, 멀티 세션용): `session_id`, `event` 추가.

```json
{"agent": "claude", "state": "executing_tool", "detail": "Bash: npm test", "timestamp": "...", "session_id": "abc123", "event": "PreToolUse"}
```

- `session_id` 생략 시 `default`로 간주
- `agent`/`session_id`는 토픽 주입 방지를 위해 화이트리스트 문자만 허용, `detail`은 제어문자 제거 후 120바이트로 truncate
- 잘못된 이벤트는 조용히 드롭(훅은 절대 에러를 보면 안 됨)

## 설정 레퍼런스

`config.yaml` 로더는 알려지지 않은 키를 거부(typo 방지)하고, 다음 환경변수가 빈 값을 채웁니다.

| 환경변수 | 설명 |
|---|---|
| `KITTEYE_CONFIG` | 설정 파일 경로 (기본 `./config.yaml`, `--config` 플래그 우선) |
| `KITTEYE_MQTT_BROKER` | `mqtt.broker`가 비어 있을 때만 적용 |
| `KITTEYE_MQTT_PASSWORD` | `mqtt.password`가 비어 있을 때만 적용 |
| `KITTEYE_SOCKET` | 훅 전송기(`send_event.sh`)가 쓰는 소켓 경로 (기본 `/tmp/kitt-eye.sock`) |

상태 TTL 기본값: `done` 15s, `error` 60s, stale 300s, session 3600s, 에이전트당 세션 8개.

## Makefile

```sh
make help                       # 대상 목록
make build-pub                  # Go Publisher 빌드 → pub-go/bin/kitt-eye-pub
make run-pub                    # 빌드 후 config.yaml로 실행
make build-mcu                  # ESP32-C3 펌웨어 빌드 (구현 예정)
make flash-mcu                  # espflash로 플래시 + 모니터링 (구현 예정)
make install-hooks              # 4 CLI 훅 설치 (jq 필수)
make install-hooks-claude       # Claude만
make uninstall-hooks            # CLI 설정에서 kitt-eye 훅 제거 + bin 삭제
make test                       # go test ./... + cargo test (Rust는 아직 no-op)
make clean                      # 산출물 제거
```

## 개발

```sh
cd pub-go && go test ./...    # 단위 테스트 (브로커 불요, hermetic)
cd pub-go && go vet ./...

# 실재 MQTT 브로커에 대한 스모크 테스트 (기본 skip)
KITTEYE_TEST_MQTT="homeassistant.local:1883" \
KITTEYE_TEST_MQTT_USER="..." KITTEYE_TEST_MQTT_PASSWORD="***" \
  go test -run TestLiveBrokerSmoke ./internal/mqtt/
```

> 알려진 문제: `internal/ipc`의 `TestDeliversDatagrams`는 간헐적으로 실패합니다. 서버가 동시 연결을 각각 별도 고루틴에서 처리해 연결 간 이벤트 순서가 보장되지 않는데, 테스트가 전송 순서를 가정하기 때문입니다 — 서버 동작(순서 비보장)이 의도한 설계이며, 테스트가 첫 이벤트 수신을 기다린 뒤 다음을 전송하도록 보완할 계획입니다.

상세 설계(훅 이벤트 매핑 표, hook.sh 계약, JSON 병합 절차, 주의사항)는 [PLAN.md](./PLAN.md)를 참고하세요.

## 라이선스

[MIT](./LICENSE) © 2026 Homin Lee <i@homin.dev>
