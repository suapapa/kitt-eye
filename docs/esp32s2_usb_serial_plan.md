# ESP32-S2 & ESP32-C3 USB Serial CLI & 제어 시스템 구현 계획

본 문서는 **ESP32-S2**(USB-OTG Full-Speed CDC-ACM) 및 **ESP32-C3**(온칩 USB-Serial-JTAG) 칩의 USB 포트를 통해 호스트(PC/Mac)에서 케이블 연결만으로 **장치 설정 변경**(Wi-Fi, MQTT, LED 테마) 및 **직접 LED 상태 제어**(IDLE, THINKING, GENERATING 등)를 수행할 수 있도록 하는 기능의 아키텍처 및 구현 계획서입니다.

---

## 1. 확정된 설계 결정 사항 (User Decisions)

1. **명시적 `save` 및 재부팅을 통한 설정 적용**:
   - `wifi`, `broker`, `set mqtt ...` 등으로 변경된 설정은 RAM에 즉시 반영되며, `save` 명령을 명시적으로 실행했을 때만 Flash 비휘발성 저장소에 기록됩니다.
   - Flash에 저장된 네트워크 및 MQTT 설정은 `reboot` 명령을 통해 MCU가 재기동될 때 완벽하게 적용됩니다.
   - 테마 변경(`theme <classic|colorful>`)은 재부팅 없이 LED 렌더러 루프에 실시간(다음 프레임) 즉각 반영됩니다.
2. **무기한 수동 상태 유지 (Indefinite Manual State Retention)**:
   - `state <NAME>`으로 지정된 수동 오버라이드 상태는 별도의 타임아웃 없이 무기한 유지됩니다.
   - 사용자가 명시적으로 `state auto` 명령을 입력했을 때만 수동 오버라이드가 해제되고 MQTT 실시간 이벤트 추적으로 복귀합니다.
3. **ESP32-S2 및 ESP32-C3 듀얼 타겟 일관 지원**:
   - `mcu-rust-common`의 CLI 파서와 명령어 핸들러는 플랫폼 독립적인 단일 진실 공급원(SSOT)으로 구성됩니다.
   - **ESP32-S2**: 내장 USB-OTG 컨트롤러(`USB0`, GPIO19=DM, GPIO20=DP) 기반 `embassy-usb` CDC-ACM 스택 연동.
   - **ESP32-C3**: 내장 USB-Serial-JTAG 컨트롤러(`USB_DEVICE`)의 비동기 드라이버(`into_async()`) 연동.
   - 두 보드 모두 동일한 CLI 명령어 세트와 대화형 쉘 인터페이스를 제공합니다.

---

## 2. 하드웨어 및 핀 구성

| 기능 | ESP32-S2 | ESP32-C3 | 설명 |
|---|---|---|---|
| **USB D- (DM)** | `GPIO19` | `GPIO18` (내장 PHY) | USB 차동 데이터 라인 (-) |
| **USB D+ (DP)** | `GPIO20` | `GPIO19` (내장 PHY) | USB 차동 데이터 라인 (+) |
| **USB 드라이버** | USB-OTG (`USB0`) | USB-Serial-JTAG (`USB_DEVICE`) | 온칩 USB 가상 시리얼 포트 |
| **WS2812 DIN** | `GPIO6` | `GPIO6` | SPI2 MOSI (LED 데이터 라인) |
| **SPI SCK** | `GPIO4` | `GPIO4` | SPI 마스터 더미 클록 (미사용) |

---

## 3. 소프트웨어 아키텍처 및 계층 구조

```text
┌─────────────────────────────────────────────────────────────┐
│                    USB Serial CLI Core                      │
│            (mcu-rust-common/src/cli.rs: no_std)             │
│    - Help / Status / Config / State / Theme / Save / Reboot │
└──────────────┬───────────────────────────────┬──────────────┘
               │                               │
       ┌───────┴───────────────────────┐       │
       ▼                               ▼       ▼
┌──────────────────────────────┐ ┌────────────────────────────┐
│   ESP32-S2 Transport         │ │   ESP32-C3 Transport       │
│   (embassy-usb CDC-ACM)      │ │   (UsbSerialJtag Async)    │
│   peripherals.USB0 (GPIO19/20)│ │   peripherals.USB_DEVICE   │
└──────────────┬───────────────┘ └─────────────┬──────────────┘
               │                               │
               └───────────────┬───────────────┘
                               ▼
┌──────────────────────────────┐ ┌────────────────────────────┐
│   Runtime Device Config      │ │   Active State Controller  │
│ - Wi-Fi SSID / Password      │ │ - Mode: Auto(MQTT) / Manual│
│ - MQTT Broker / Port / Auth  │ │ - ActiveState: Idle~Error  │
│ - MotionTheme: Classic/Color │ │ - Indefinite Manual Lock   │
└──────────────┬───────────────┘ └─────────────┬──────────────┘
               │                               │
       ┌───────┴────────┐                      │
       ▼                ▼                      ▼
┌──────────────┐ ┌─────────────┐ ┌────────────────────────────┐
│ Wi-Fi & MQTT │ │ Flash (NVS) │ │  LED Engine (WS2812 Loop)  │
│ Tasks Re-init│ │ Persistence │ │ Real-time Pattern & Color  │
└──────────────┘ └─────────────┘ └────────────────────────────┘
```

---

## 4. CLI 명령어 세부 사양

### 4.1. 상태 제어 (직접 LED 상태 지정)

| 명령어 | 설명 | 예시 및 응답 |
|---|---|---|
| `state` | 현재 활성 상태 및 모드(Auto/Manual) 확인 | `STATE: thinking (mode: MANUAL)` |
| `state <NAME>` | LED 상태를 즉시 지정하고 수동 모드로 무기한 고정 | `state thinking`<br>→ `OK: state set to 'thinking' (manual override ON)` |
| `state auto` | 수동 오버라이드를 해제하고 MQTT 브로커 이벤트 추적으로 복귀 | `state auto`<br>→ `OK: manual override OFF (listening to MQTT)` |

- **지원되는 상태 이름 (대소문자 무관)**:
  `idle`, `thinking`, `generating`, `executing_tool` (또는 `tool`), `waiting_input` (또는 `input`), `done`, `error`

### 4.2. 설정 변경 (Configuration)

| 명령어 | 설명 | 예시 및 응답 |
|---|---|---|
| `theme <classic\|colorful>` | LED 모션 테마 실시간 전환 (재부팅 불필요) | `theme colorful`<br>→ `OK: theme switched to 'colorful'` |
| `wifi <SSID> <PASS>` | Wi-Fi SSID 및 비밀번호 변경 (RAM 반영) | `wifi MyHome 12345678`<br>→ `OK: Wi-Fi credentials updated. Type 'save' then 'reboot'.` |
| `mqtt_broker <URL\|HOST> [PORT]` | MQTT 브로커 설정 (URL 포맷으로 username, password, port 일괄 지정 가능) | `mqtt_broker mqtt://mqtt:896351@172.16.0.100`<br>`mqtt_broker 192.168.1.50 1883`<br>→ `OK: MQTT broker updated.` |
| `mqtt_topic <PREFIX>` | MQTT 구독 토픽 접두어 변경 (기본: `kitt-eye`) | `mqtt_topic my-kitt`<br>→ `OK: MQTT topic updated.` |
| `mqtt-client_id <ID>` | MQTT 클라이언트 ID 변경 | `mqtt-client_id my-esp32-s2`<br>→ `OK: MQTT client ID updated.` |
| `config` 또는 `config show` | 현재 설정값 일괄 출력 (비밀번호 마스킹) | 현재 SSID, 브로커, 토픽, 클라이언트 ID, 테마 등 출력 |

### 4.3. 시스템 및 영속화 관리

| 명령어 | 설명 | 예시 및 응답 |
|---|---|---|
| `save` | 현재 RAM 설정을 Flash 메모리에 영구 저장 | `save`<br>→ `OK: configuration saved to flash` |
| `load` | Flash에 저장된 설정을 다시 불러와 RAM에 적용 | `load`<br>→ `OK: configuration loaded from flash` |
| `reset` 또는 `reset-config` | 컴파일 타임 기본값(`.env`)으로 설정 복원 | `reset`<br>→ `OK: configuration reset to defaults` |
| `status` | Wi-Fi 연결, IP 주소, MQTT 연결 상태, 테마, 상태 요약 | `status`<br>→ 종합 상태 출력 |
| `reboot` 또는 `restart` | MCU 재부팅 (새 Wi-Fi/MQTT 설정 적용) | `reboot`<br>→ `Rebooting MCU...` |
| `help` 또는 `?` | 전체 사용 가능한 명령어 도움말 출력 | 도움말 텍스트 출력 |

---

## 5. 구현 로드맵

1. **Phase 1**: `mcu-rust-common/src/cli.rs` 순수 파서 및 `RuntimeConfig` 모델 완성 (`cargo test`로 검증).
2. **Phase 2**: `mcu-rust-common/src/app_main.rs` 전역 상태 동기화 (`ACTIVE_STATE`, `MANUAL_OVERRIDE`, `ACTIVE_THEME`, `RUNTIME_CONFIG`).
3. **Phase 3**: ESP32-S2 USB-OTG CDC-ACM 비동기 드라이버 및 CLI 태스크 연동.
4. **Phase 4**: ESP32-C3 USB-Serial-JTAG 비동기 드라이버 및 CLI 태스크 연동.
5. **Phase 5**: Flash 영속화 (`save`/`load`) 모듈 연동.
6. **Phase 6**: 전체 테스트 스위트 검증 (`make test`: Go + Common + C3 + S2).
