# kitt-eye Architecture

ASCII 아키텍처 설명을 Mermaid 다이어그램으로 시각화한 문서입니다.  
상세 프로토콜·상태 규칙은 [PLAN.md](../PLAN.md), 개발 제약은 [AGENTS.md](../AGENTS.md)를 참고하세요.



## 1. System Overview

AI CLI 훅 → Go Publisher → MQTT → ESP32 LED 바까지의 전체 파이프라인입니다.

```mermaid
flowchart TB
    subgraph Agents["AI CLI Tools"]
        CLAUDE["claude<br/>~/.claude/settings.json"]
        CODEX["codex<br/>~/.codex/hooks.json"]
        CURSOR["cursor-cli<br/>~/.cursor/hooks.json"]
        AGY["antigravity-cli<br/>~/.gemini/config/hooks.json"]
        HERMES["hermes<br/>~/.hermes/config.yaml"]
    end

    subgraph Host["Host Machine"]
        HOOKS["hooks/*/hook.sh<br/>+ common/send_event.sh"]
        UDS[("/tmp/kitt-eye.sock<br/>Unix Domain Socket")]
        PUB["Go Publisher Daemon<br/>(pub-go)"]
    end

    subgraph Broker["HomeLab MQTT"]
        MQTT["Mosquitto<br/>(Home Assistant)"]
    end

    subgraph Device["ESP32 Device"]
        MCU["mcu-rust-esp32c3 / mcu-rust-esp32s2<br/>+ mcu-rust-common"]
        LED["WS2812B NeoPixel Bar<br/>GPIO6 · 8 LEDs"]
    end

    HA["Home Assistant<br/>MQTT Discovery Sensors"]

    CLAUDE & CODEX & CURSOR & AGY & HERMES --> HOOKS
    HOOKS -->|"NDJSON · fire-and-forget<br/>stdout silent"| UDS
    UDS --> PUB
    PUB -->|"QoS 1 · Retained · LWT"| MQTT
    MQTT -->|"kitt-eye/active_state"| MCU
    MQTT --> HA
    MCU --> LED
```



## 2. Data Flow (Publisher Internals)

훅 이벤트가 데몬 내부에서 세션 집계·TTL·발행까지 흘러가는 경로입니다.

```mermaid
flowchart LR
    H["hook.sh<br/>send_event.sh"] -->|"JSON 1 line"| IPC["ipc.Server<br/>/tmp/kitt-eye.sock"]
    IPC --> PARSE["model.ParseEvent<br/>whitelist"]
    PARSE --> STORE["state.Store<br/>agent · session · TTL"]
    JANITOR["janitor ticker"] --> STORE
    STORE -->|"change notify<br/>coalesced"| WORKER["publish worker"]
    WORKER --> SINK{"Sink"}
    SINK -->|"mqtt.broker set"| MQ["mqtt.Client<br/>QoS1 · retain · LWT"]
    SINK -->|"mqtt.broker empty"| DRY["stdout DryRun<br/>NDJSON"]
    MQ --> BR[(Mosquitto)]
    BR --> MCU["ESP32 MCU"]
    BR --> HA["Home Assistant"]
```



## 3. State Priority & TTL

여러 세션/에이전트가 동시에 활성일 때 `active_state`는 아래 우선순위로 결정됩니다.

```mermaid
flowchart TB
    E["error"] --> W["waiting_input"]
    W --> X["executing_tool"]
    X --> G["generating"]
    G --> T["thinking"]
    T --> D["done"]
    D --> I["idle"]

    style E fill:#d32f2f,color:#fff
    style W fill:#f9a825,color:#000
    style X fill:#ef6c00,color:#fff
    style G fill:#2e7d32,color:#fff
    style T fill:#7b1fa2,color:#fff
    style D fill:#43a047,color:#fff
    style I fill:#616161,color:#fff
```

| State | TTL / Expiration |
|---|---|
| `done` | `done_ttl` (default **15s**) → `idle` |
| `error` | `error_ttl` (default **60s**) → `idle` |
| active states | `stale_ttl` (default **300s**) no events → prune / `idle` |
| `idle` | `session_ttl` (default **3600s**) → session removed |

동률이면 가장 최근 갱신된 세션이 선택됩니다.



## 4. MQTT Topics

기본 prefix: `kitt-eye` (모두 QoS 1 + Retain).

```mermaid
flowchart TB
    PUB["Go Publisher"] --> ACT["kitt-eye/active_state<br/>대표 상태 문자열"]
    PUB --> AGT["kitt-eye/active_agent<br/>대표 에이전트명"]
    PUB --> AST["kitt-eye/agents/{agent}/state"]
    PUB --> SES["kitt-eye/agents/{agent}/sessions<br/>AgentSnapshot JSON"]
    PUB --> SYS["kitt-eye/system/status<br/>online / offline · LWT"]
    PUB --> HAD["homeassistant/sensor/kitt_eye_*/config<br/>HA Discovery"]

    ACT --> MCU["ESP32 LED Engine"]
    ACT & AGT & AST & SES & SYS --> HA["Home Assistant UI"]
    HAD --> HA
```



## 5. MCU Firmware Layout

보드 패키지는 얇고, 애플리케이션 로직은 `mcu-rust-common`에 둡니다.

```mermaid
flowchart TB
    subgraph Common["mcu-rust-common (Single Source of Truth)"]
        APP["app_main.rs"]
        STATE["state.rs"]
        THEME["theme.rs<br/>classic / colorful"]
        PAT["patterns.rs<br/>LED_COUNT = 8"]
        MQTTC["mqtt.rs"]
        CFG["config.rs · env!()"]
    end

    subgraph C3["mcu-rust-esp32c3"]
        C3BIN["thin binary wrapper<br/>riscv32imc · nightly"]
    end

    subgraph S2["mcu-rust-esp32s2"]
        S2BIN["thin binary wrapper<br/>xtensa · esp toolchain<br/>espflash --no-stub"]
    end

    C3BIN --> Common
    S2BIN --> Common
    APP --> STATE & THEME & PAT & MQTTC & CFG
    PAT --> LED["WS2812B · GPIO6 SPI2 MOSI"]
```

### Motion Themes (요약)

| State | classic | colorful |
|---|---|---|
| `idle` | Slow K.I.T.T. scanner (red) | Classic Red · Slow scanner |
| `thinking` | Center-out | Electric Violet |
| `generating` | Fill-sweep | Neo Mint Emerald |
| `executing_tool` | Fast scanner | Vivid Amber · Fast scanner |
| `waiting_input` | Blink | Warning Yellow |
| `done` | Flash → solid | Spring Lime |
| `error` | Comet | Crimson Red |



## 6. Repository Component Map

```mermaid
flowchart LR
    subgraph Repo["kitt-eye/"]
        HOOKS["hooks/"]
        PUBGO["pub-go/"]
        COMMON["mcu-rust-common/"]
        C3["mcu-rust-esp32c3/"]
        S2["mcu-rust-esp32s2/"]
        SCRIPTS["scripts/<br/>demo-mqtt · demo-serial · config-serial"]
        DOCS["docs/"]
    end

    HOOKS -->|"UDS events"| PUBGO
    PUBGO -->|"MQTT"| BROKER[(Broker)]
    BROKER --> C3 & S2
    COMMON -.->|"shared lib"| C3 & S2
    SCRIPTS -.->|"demo / config"| BROKER & C3 & S2
```



## 7. IPC Datagram Shape

Hooks → Publisher 한 줄 JSON 예시:

```json
{
  "agent": "claude",
  "state": "executing_tool",
  "detail": "Bash: npm test",
  "session_id": "abc12345",
  "event": "PreToolUse",
  "timestamp": "2026-09-12T17:00:00Z"
}
```

- 소켓: `/tmp/kitt-eye.sock`
- 전송: fire-and-forget (`nc -U`), 데몬 미기동 시 조용히 실패
- 훅 규칙: **stdout 무출력**, 부모 CLI에 영향 금지



## Related Docs

- [PLAN.md](../PLAN.md) — full architecture & protocol specification
- [AGENTS.md](../AGENTS.md) — agent/developer constraints
- [hardware_wiring.md](hardware_wiring.md) — ESP32 ↔ WS2812B wiring
- [esp32s2_usb_serial_plan.md](esp32s2_usb_serial_plan.md) — USB Serial CLI design
- [voice_system_design.md](voice_system_design.md) — optional voice playback
