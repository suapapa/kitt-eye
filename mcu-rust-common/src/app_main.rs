#![no_std]
#![no_main]
#![deny(
    clippy::mem_forget,
    reason = "mem::forget is generally not safe to do with esp_hal types, especially those \
    holding buffers for the duration of a data transfer."
)]
#![deny(clippy::large_stack_frames)]

use core::cell::RefCell;
use core::sync::atomic::{AtomicBool, AtomicU8, Ordering};

use critical_section::Mutex;
use embassy_executor::Spawner;
use embassy_net::{dns::DnsQueryType, tcp::TcpSocket, DhcpConfig, Runner, Stack, StackResources};
use embassy_time::{Duration, Timer};
use esp_hal::{
    clock::CpuClock,
    interrupt::software::SoftwareInterruptControl,
    rng::Rng,
    spi::master::{Config as SpiConfig, Spi},
    spi::Mode as SpiMode,
    time::Rate,
    timer::timg::TimerGroup,
};
use esp_println::println;
use esp_radio::wifi::{Config as WifiConfig, WifiController, sta::StationConfig};
use heapless::String;
use kitt_eye_common::{
    config::RuntimeConfig,
    mqtt::{self, MqttClient},
    patterns::{LedEngine, ANIM_SPEED_MS, LED_COUNT},
    AgentState, MotionTheme,
};
use smart_leds::{brightness, SmartLedsWrite, RGB8};
use static_cell::StaticCell;
use ws2812_spi::prerendered::Ws2812;

#[panic_handler]
fn panic(info: &core::panic::PanicInfo) -> ! {
    println!("PANIC: {info}");
    loop {}
}

extern crate alloc;

esp_bootloader_esp_idf::esp_app_desc!();

/// Global brightness 0..=255. Dimmed a bit for eye comfort.
const BRIGHTNESS: u8 = 64;

/// SPI encode buffer: 12 bytes/LED + 140 (mosi_idle_high) + 140 (reset_single_transaction).
const SPI_BUF_LEN: usize = LED_COUNT * 12 + 280;

/// Shared agent state written by MQTT task or CLI, read by LED loop.
static ACTIVE_STATE: AtomicU8 = AtomicU8::new(AgentState::Idle as u8);

/// Manual override active flag: true = ignore MQTT updates and show manually set state indefinitely.
static MANUAL_OVERRIDE: AtomicBool = AtomicBool::new(false);

/// Active motion theme: 0 = Classic, 1 = Colorful.
static ACTIVE_THEME: AtomicU8 = AtomicU8::new(0);

/// Set by the Wi-Fi connection task; LED shows rainbow until this is true (unless manual override).
static WIFI_CONNECTED: AtomicBool = AtomicBool::new(false);

/// Dynamic runtime configuration that can be updated via CLI or Flash.
static RUNTIME_CONFIG: Mutex<RefCell<Option<RuntimeConfig>>> = Mutex::new(RefCell::new(None));

fn get_runtime_config() -> RuntimeConfig {
    critical_section::with(|cs| {
        RUNTIME_CONFIG
            .borrow_ref(cs)
            .clone()
            .unwrap_or_else(RuntimeConfig::from_env_defaults)
    })
}

fn set_runtime_config(cfg: RuntimeConfig) {
    critical_section::with(|cs| {
        *RUNTIME_CONFIG.borrow_ref_mut(cs) = Some(cfg);
    });
}

fn update_runtime_config<F: FnOnce(&mut RuntimeConfig)>(f: F) {
    critical_section::with(|cs| {
        let mut cell = RUNTIME_CONFIG.borrow_ref_mut(cs);
        if let Some(cfg) = cell.as_mut() {
            f(cfg);
        }
    });
}

const FLASH_CONFIG_ADDR: u32 = 0x3F0000;
const FLASH_CONFIG_SECTOR: u32 = FLASH_CONFIG_ADDR / 4096;

#[allow(clippy::large_stack_frames)]
fn load_config_from_flash() -> Option<RuntimeConfig> {
    let mut buf = [0u8; 512];
    unsafe {
        if esp_storage::ll::spiflash_read(FLASH_CONFIG_ADDR, buf.as_mut_ptr() as *mut u32, 512).is_err() {
            return None;
        }
    }
    RuntimeConfig::deserialize(&buf)
}

#[allow(clippy::large_stack_frames)]
fn save_config_to_flash(cfg: &RuntimeConfig) -> Result<(), &'static str> {
    let mut buf = [0u8; 512];
    cfg.serialize(&mut buf);
    unsafe {
        let _ = esp_storage::ll::spiflash_unlock();
        esp_storage::ll::spiflash_erase_sector(FLASH_CONFIG_SECTOR)
            .map_err(|_| "flash erase failed")?;
        esp_storage::ll::spiflash_write(FLASH_CONFIG_ADDR, buf.as_ptr() as *const u32, 512)
            .map_err(|_| "flash write failed")?;
    }
    Ok(())
}

macro_rules! mk_static {
    ($t:ty, $val:expr) => {{
        static STATIC_CELL: StaticCell<$t> = StaticCell::new();
        #[deny(unused_attributes)]
        let x = STATIC_CELL.uninit().write(($val));
        x
    }};
}

#[allow(clippy::large_stack_frames)]
async fn process_cli_command(line: &str, out: &mut String<512>) -> bool {
    use core::fmt::Write;
    use kitt_eye_common::cli::{parse_line, write_help, CliCommand};

    match parse_line(line) {
        Ok(CliCommand::Help) => {
            write_help(out);
            false
        }
        Ok(CliCommand::Status) => {
            let wifi_up = WIFI_CONNECTED.load(Ordering::Relaxed);
            let state = AgentState::from_u8(ACTIVE_STATE.load(Ordering::Relaxed));
            let override_active = MANUAL_OVERRIDE.load(Ordering::Relaxed);
            let theme_val = ACTIVE_THEME.load(Ordering::Relaxed);
            let theme = if theme_val == 1 { "colorful" } else { "classic" };

            let _ = write!(
                out,
                "STATUS:\r\n  Wi-Fi: {}\r\n  State: {} (mode: {})\r\n  Theme: {}\r\n",
                if wifi_up { "connected" } else { "disconnected" },
                state.as_str(),
                if override_active { "MANUAL" } else { "AUTO(MQTT)" },
                theme,
            );
            false
        }
        Ok(CliCommand::GetState) => {
            let state = AgentState::from_u8(ACTIVE_STATE.load(Ordering::Relaxed));
            let override_active = MANUAL_OVERRIDE.load(Ordering::Relaxed);
            let _ = write!(
                out,
                "STATE: {} (mode: {})\r\n",
                state.as_str(),
                if override_active { "MANUAL" } else { "AUTO(MQTT)" }
            );
            false
        }
        Ok(CliCommand::SetState(new_state)) => {
            MANUAL_OVERRIDE.store(true, Ordering::Relaxed);
            ACTIVE_STATE.store(new_state as u8, Ordering::Relaxed);
            let _ = write!(
                out,
                "OK: state set to '{}' (manual override ON)\r\n",
                new_state.as_str()
            );
            false
        }
        Ok(CliCommand::StateAuto) => {
            MANUAL_OVERRIDE.store(false, Ordering::Relaxed);
            let _ = out.push_str("OK: manual override OFF (listening to MQTT)\r\n");
            false
        }
        Ok(CliCommand::SetTheme(theme)) => {
            let val = if theme == MotionTheme::Colorful { 1 } else { 0 };
            ACTIVE_THEME.store(val, Ordering::Relaxed);
            update_runtime_config(|c| {
                c.motion_theme = theme;
            });
            let _ = write!(out, "OK: theme switched to '{}'\r\n", theme.as_str());
            false
        }
        Ok(CliCommand::SetWifi { ssid, pass }) => {
            update_runtime_config(|c| {
                c.ssid.clear();
                let _ = c.ssid.push_str(ssid);
                c.pass.clear();
                let _ = c.pass.push_str(pass);
            });
            let _ = out.push_str("OK: Wi-Fi credentials updated. Type 'save' then 'reboot'.\r\n");
            false
        }
        Ok(CliCommand::SetMqttBroker(b)) => {
            update_runtime_config(|c| {
                c.mqtt_broker.clear();
                let _ = c.mqtt_broker.push_str(b.host);
                if let Some(p) = b.port {
                    c.mqtt_port = p;
                }
                if let Some(u) = b.user {
                    c.mqtt_user.clear();
                    let _ = c.mqtt_user.push_str(u);
                }
                if let Some(p) = b.pass {
                    c.mqtt_pass.clear();
                    let _ = c.mqtt_pass.push_str(p);
                }
            });
            let _ = out.push_str("OK: MQTT broker updated. Type 'save' then 'reboot'.\r\n");
            false
        }
        Ok(CliCommand::SetMqttTopic(prefix)) => {
            update_runtime_config(|c| {
                c.mqtt_topic_prefix.clear();
                let _ = c.mqtt_topic_prefix.push_str(prefix);
            });
            let _ = out.push_str("OK: MQTT topic updated. Type 'save' then 'reboot'.\r\n");
            false
        }
        Ok(CliCommand::SetMqttClientId(client_id)) => {
            update_runtime_config(|c| {
                c.mqtt_client_id.clear();
                let _ = c.mqtt_client_id.push_str(client_id);
            });
            let _ = out.push_str("OK: MQTT client ID updated. Type 'save' then 'reboot'.\r\n");
            false
        }
        Ok(CliCommand::GetConfig) => {
            let cfg = get_runtime_config();
            let theme_str = if ACTIVE_THEME.load(Ordering::Relaxed) == 1 { "colorful" } else { "classic" };
            let _ = write!(
                out,
                "CONFIG:\r\n  SSID: {}\r\n  PASS: {}\r\n  Broker: {}:{}\r\n  User: {}\r\n  Topic: {}\r\n  Client ID: {}\r\n  Theme: {}\r\n",
                cfg.ssid.as_str(),
                if cfg.pass.is_empty() { "(none)" } else { "********" },
                cfg.mqtt_broker.as_str(),
                cfg.mqtt_port,
                if cfg.mqtt_user.is_empty() { "(anonymous)" } else { cfg.mqtt_user.as_str() },
                cfg.mqtt_topic_prefix.as_str(),
                cfg.mqtt_client_id.as_str(),
                theme_str,
            );
            false
        }
        Ok(CliCommand::Save) => {
            let cfg = get_runtime_config();
            match save_config_to_flash(&cfg) {
                Ok(()) => {
                    let _ = out.push_str("OK: configuration saved to flash. Type 'reboot' to apply.\r\n");
                }
                Err(err) => {
                    let _ = write!(out, "ERR: {}\r\n", err);
                }
            }
            false
        }
        Ok(CliCommand::Load) => {
            match load_config_from_flash() {
                Some(loaded) => {
                    let theme_val = if loaded.motion_theme == MotionTheme::Colorful { 1 } else { 0 };
                    ACTIVE_THEME.store(theme_val, Ordering::Relaxed);
                    set_runtime_config(loaded);
                    let _ = out.push_str("OK: configuration loaded from flash.\r\n");
                }
                None => {
                    let _ = out.push_str("ERR: no valid configuration in flash\r\n");
                }
            }
            false
        }
        Ok(CliCommand::ResetConfig) => {
            let def = RuntimeConfig::from_env_defaults();
            let theme_val = if def.motion_theme == MotionTheme::Colorful { 1 } else { 0 };
            ACTIVE_THEME.store(theme_val, Ordering::Relaxed);
            set_runtime_config(def);
            let _ = out.push_str("OK: configuration reset to defaults. Type 'save' to persist.\r\n");
            false
        }
        Ok(CliCommand::Reboot) => {
            let _ = out.push_str("Rebooting MCU...\r\n");
            true
        }
        Err(err) => {
            let _ = write!(out, "ERR: {}\r\n", err);
            false
        }
    }
}

#[cfg(esp32s2)]
#[embassy_executor::task]
async fn usb_runner_task(
    mut usb: embassy_usb::UsbDevice<'static, esp_hal::otg_fs::asynch::Driver<'static>>,
) -> ! {
    usb.run().await
}

#[cfg(esp32s2)]
#[allow(clippy::large_stack_frames)]
#[embassy_executor::task]
async fn usb_cli_s2_task(
    mut class: embassy_usb::class::cdc_acm::CdcAcmClass<
        'static,
        esp_hal::otg_fs::asynch::Driver<'static>,
    >,
) {
    loop {
        class.wait_connection().await;
        println!("USB Serial connected");
        send_s2(
            &mut class,
            "\r\n=== K.I.T.T.-EYE USB Serial Console ===\r\nType 'help' for commands.\r\nkitt-eye> ",
        )
        .await;

        let mut line_buf: String<128> = String::new();
        let mut rx_buf = [0u8; 64];

        loop {
            let n = match class.read_packet(&mut rx_buf).await {
                Ok(n) => n,
                Err(_) => break,
            };

            for &b in &rx_buf[..n] {
                if b == 0x08 || b == 0x7f {
                    if !line_buf.is_empty() {
                        let _ = line_buf.pop();
                        let _ = class.write_packet(b"\x08 \x08").await;
                    }
                    continue;
                }

                let _ = class.write_packet(&[b]).await;

                if b == b'\r' || b == b'\n' {
                    let _ = class.write_packet(b"\r\n").await;
                    let trimmed = line_buf.trim();
                    if !trimmed.is_empty() {
                        let mut response: String<512> = String::new();
                        let reboot = process_cli_command(trimmed, &mut response).await;
                        send_s2(&mut class, response.as_str()).await;
                        if reboot {
                            Timer::after(Duration::from_millis(200)).await;
                            esp_hal::system::software_reset();
                        }
                    }
                    line_buf.clear();
                    send_s2(&mut class, "kitt-eye> ").await;
                } else if line_buf.len() < 120 {
                    let _ = line_buf.push(b as char);
                }
            }
        }
        println!("USB Serial disconnected");
    }
}

#[cfg(esp32s2)]
async fn send_s2(
    class: &mut embassy_usb::class::cdc_acm::CdcAcmClass<
        'static,
        esp_hal::otg_fs::asynch::Driver<'static>,
    >,
    s: &str,
) {
    for chunk in s.as_bytes().chunks(64) {
        let _ = class.write_packet(chunk).await;
    }
}

#[cfg(esp32c3)]
#[allow(clippy::large_stack_frames)]
#[embassy_executor::task]
async fn usb_cli_c3_task(
    mut usb_serial: esp_hal::usb_serial_jtag::UsbSerialJtag<'static, esp_hal::Async>,
) {
    use embedded_io_async::{Read, Write};

    println!("USB-Serial-JTAG task started");
    send_c3(
        &mut usb_serial,
        "\r\n=== K.I.T.T.-EYE USB Serial Console ===\r\nType 'help' for commands.\r\nkitt-eye> ",
    )
    .await;

    let mut line_buf: String<128> = String::new();
    let mut rx_buf = [0u8; 64];

    loop {
        let n = match usb_serial.read(&mut rx_buf).await {
            Ok(n) if n > 0 => n,
            _ => {
                Timer::after(Duration::from_millis(50)).await;
                continue;
            }
        };

        for &b in &rx_buf[..n] {
            if b == 0x08 || b == 0x7f {
                if !line_buf.is_empty() {
                    let _ = line_buf.pop();
                    let _ = usb_serial.write_all(b"\x08 \x08").await;
                }
                continue;
            }

            let _ = usb_serial.write_all(&[b]).await;

            if b == b'\r' || b == b'\n' {
                let _ = usb_serial.write_all(b"\r\n").await;
                let trimmed = line_buf.trim();
                if !trimmed.is_empty() {
                    let mut response: String<512> = String::new();
                    let reboot = process_cli_command(trimmed, &mut response).await;
                    send_c3(&mut usb_serial, response.as_str()).await;
                    if reboot {
                        Timer::after(Duration::from_millis(200)).await;
                        esp_hal::system::software_reset();
                    }
                }
                line_buf.clear();
                send_c3(&mut usb_serial, "kitt-eye> ").await;
            } else if line_buf.len() < 120 {
                let _ = line_buf.push(b as char);
            }
        }
    }
}

#[cfg(esp32c3)]
async fn send_c3(
    usb_serial: &mut esp_hal::usb_serial_jtag::UsbSerialJtag<'static, esp_hal::Async>,
    s: &str,
) {
    use embedded_io_async::Write;
    let _ = usb_serial.write_all(s.as_bytes()).await;
}

#[allow(
    clippy::large_stack_frames,
    reason = "prerendered WS2812 SPI encode buffer lives on the main stack"
)]
#[esp_rtos::main]
async fn main(spawner: Spawner) -> ! {
    let config = esp_hal::Config::default().with_cpu_clock(CpuClock::max());
    let peripherals = esp_hal::init(config);

    esp_alloc::heap_allocator!(#[esp_hal::ram(reclaimed)] size: 64 * 1024);

    let timg0 = TimerGroup::new(peripherals.TIMG0);
    let sw_interrupt = SoftwareInterruptControl::new(peripherals.SW_INTERRUPT);
    esp_rtos::start(timg0.timer0, sw_interrupt.software_interrupt0);

    let initial_config = if let Some(flash_cfg) = load_config_from_flash() {
        println!("loaded config from flash");
        flash_cfg
    } else {
        println!("using default .env config");
        RuntimeConfig::from_env_defaults()
    };

    let theme = initial_config.motion_theme;
    ACTIVE_THEME.store(if theme == MotionTheme::Colorful { 1 } else { 0 }, Ordering::Relaxed);
    let mut engine = LedEngine::new(theme);
    set_runtime_config(initial_config);

    esp_println::logger::init_logger_from_env();
    println!("kitt-eye boot (theme: {})", theme.as_str());

    // Same wiring as before: GPIO4=SCK (unused), GPIO6=MOSI→DIN.
    let spi = Spi::new(
        peripherals.SPI2,
        SpiConfig::default()
            .with_frequency(Rate::from_khz(3200))
            .with_mode(SpiMode::_0),
    )
    .expect("SPI init")
    .with_sck(peripherals.GPIO4)
    .with_mosi(peripherals.GPIO6);

    let mut spi_buf = [0u8; SPI_BUF_LEN];
    let mut strip = Ws2812::new(spi, &mut spi_buf);

    critical_section::with(|_| {
        let _ = strip.write([RGB8::new(0, 0, 0); LED_COUNT]);
    });

    // --- USB Serial Tasks ---
    #[cfg(esp32s2)]
    {
        use esp_hal::otg_fs::{asynch::Driver as OtgDriver, Usb};

        let usb_peri = Usb::new(peripherals.USB0, peripherals.GPIO20, peripherals.GPIO19);
        let ep_out_buffer = mk_static!([u8; 512], [0u8; 512]);
        let otg_config = esp_hal::otg_fs::asynch::Config::default();
        let driver = OtgDriver::new(usb_peri, ep_out_buffer, otg_config);

        let mut usb_config = embassy_usb::Config::new(0x303a, 0x1001);
        usb_config.manufacturer = Some("Kitt-Eye");
        usb_config.product = Some("Kitt-Eye ESP32-S2");
        usb_config.serial_number = Some("12345678");
        usb_config.max_power = 100;
        usb_config.max_packet_size_0 = 64;

        let config_descriptor = mk_static!([u8; 256], [0u8; 256]);
        let bos_descriptor = mk_static!([u8; 256], [0u8; 256]);
        let control_buf = mk_static!([u8; 64], [0u8; 64]);

        let mut builder = embassy_usb::Builder::new(
            driver,
            usb_config,
            config_descriptor,
            bos_descriptor,
            &mut [],
            control_buf,
        );

        let state = mk_static!(
            embassy_usb::class::cdc_acm::State,
            embassy_usb::class::cdc_acm::State::new()
        );
        let class = embassy_usb::class::cdc_acm::CdcAcmClass::new(&mut builder, state, 64);
        let usb = builder.build();

        spawner.spawn(usb_runner_task(usb).unwrap());
        spawner.spawn(usb_cli_s2_task(class).unwrap());
    }

    #[cfg(esp32c3)]
    {
        use esp_hal::usb_serial_jtag::UsbSerialJtag;

        let usb_serial = UsbSerialJtag::new(peripherals.USB_DEVICE).into_async();
        spawner.spawn(usb_cli_c3_task(usb_serial).unwrap());
    }

    // --- Wi-Fi ---
    let rng = Rng::new();
    let (wifi_controller, interfaces) =
        esp_radio::wifi::new(peripherals.WIFI, Default::default()).expect("Wi-Fi init");
    let wifi_interface = interfaces.station;
    let net_seed = u64::from(rng.random()) | (u64::from(rng.random()) << 32);

    let (stack, runner) = embassy_net::new(
        wifi_interface,
        embassy_net::Config::dhcpv4(DhcpConfig::default()),
        mk_static!(StackResources<4>, StackResources::<4>::new()),
        net_seed,
    );

    spawner.spawn(connection(wifi_controller).unwrap());
    spawner.spawn(net_task(runner).unwrap());
    spawner.spawn(mqtt_task(stack).unwrap());

    // LED animation loop (blocking SPI inside critical section).
    // Rainbow bounce until Wi-Fi associates; then agent-state patterns.
    let mut wifi_was_up = false;
    loop {
        let wifi_up = WIFI_CONNECTED.load(Ordering::Relaxed);
        let manual_override = MANUAL_OVERRIDE.load(Ordering::Relaxed);
        let active_theme_val = ACTIVE_THEME.load(Ordering::Relaxed);
        let current_theme = if active_theme_val == 1 {
            MotionTheme::Colorful
        } else {
            MotionTheme::Classic
        };
        engine.set_theme(current_theme);

        let show_state = wifi_up || manual_override;

        if show_state && !wifi_was_up {
            let state = AgentState::from_u8(ACTIVE_STATE.load(Ordering::Relaxed));
            println!(
                "showing state: {} (theme: {})",
                state.as_str(),
                current_theme.as_str()
            );
            engine.sync_state(state);
        } else if !show_state && wifi_was_up {
            engine.begin_wifi_wait();
        }
        wifi_was_up = show_state;

        let frame = if show_state {
            let state = AgentState::from_u8(ACTIVE_STATE.load(Ordering::Relaxed));
            engine.set_state(state);
            *engine.step()
        } else {
            *engine.step_wifi_wait()
        };
        critical_section::with(|_| {
            let _ = strip.write(brightness(frame.into_iter(), BRIGHTNESS));
        });
        let delay_ms = if show_state {
            engine.anim_delay_ms()
        } else {
            ANIM_SPEED_MS
        };
        Timer::after(Duration::from_millis(delay_ms as u64)).await;
    }
}

async fn wait_for_connection(stack: Stack<'_>) {
    println!("waiting for link");
    loop {
        if stack.is_link_up() {
            break;
        }
        Timer::after(Duration::from_millis(500)).await;
    }
    println!("waiting for DHCP");
    loop {
        if let Some(config) = stack.config_v4() {
            println!("IP {}", config.address);
            break;
        }
        Timer::after(Duration::from_millis(500)).await;
    }
}

#[allow(clippy::large_stack_frames)]
#[embassy_executor::task]
async fn mqtt_task(stack: Stack<'static>) {
    wait_for_connection(stack).await;

    let cfg = get_runtime_config();
    let broker = cfg.mqtt_broker.clone();
    let port = cfg.mqtt_port;
    let topic_prefix = cfg.mqtt_topic_prefix.clone();
    let client_id = cfg.mqtt_client_id.clone();
    let user = cfg.mqtt_user.clone();
    let pass = cfg.mqtt_pass.clone();

    let mut topic: String<64> = String::new();
    if mqtt::active_state_topic(&topic_prefix, &mut topic).is_err() {
        println!("mqtt topic too long");
        return;
    }
    println!("mqtt subscribe {topic}");

    let mut rx_buffer = [0u8; 2048];
    let mut tx_buffer = [0u8; 2048];
    let mut payload = [0u8; 64];

    loop {
        let mut socket = TcpSocket::new(stack, &mut rx_buffer, &mut tx_buffer);
        socket.set_timeout(Some(Duration::from_secs(30)));

        let addr = match resolve_broker(stack, &broker).await {
            Some(a) => a,
            None => {
                Timer::after(Duration::from_secs(3)).await;
                continue;
            }
        };

        println!("mqtt connect {broker}:{port}");
        if socket.connect((addr, port)).await.is_err() {
            println!("mqtt tcp connect failed");
            Timer::after(Duration::from_secs(3)).await;
            continue;
        }

        let mut client = MqttClient::new(socket);
        if let Err(e) = client
            .connect(&client_id, &user, &pass, 0)
            .await
        {
            println!("mqtt CONNECT failed: {e:?}");
            Timer::after(Duration::from_secs(3)).await;
            continue;
        }
        if let Err(e) = client.subscribe(topic.as_str(), 1).await {
            println!("mqtt SUBSCRIBE failed: {e:?}");
            Timer::after(Duration::from_secs(3)).await;
            continue;
        }
        println!("mqtt subscribed");

        loop {
            match client.next_publish_payload(&mut payload).await {
                Ok(n) => {
                    let bytes = &payload[..n];
                    let end = bytes
                        .iter()
                        .rposition(|b| !b.is_ascii_whitespace() && *b != 0)
                        .map(|i| i + 1)
                        .unwrap_or(0);
                    if let Some(state) = AgentState::from_mqtt(&bytes[..end]) {
                        if !MANUAL_OVERRIDE.load(Ordering::Relaxed) {
                            ACTIVE_STATE.store(state as u8, Ordering::Relaxed);
                            let theme_val = ACTIVE_THEME.load(Ordering::Relaxed);
                            let theme = if theme_val == 1 {
                                MotionTheme::Colorful
                            } else {
                                MotionTheme::Classic
                            };
                            println!(
                                "active_state={} (theme: {})",
                                state.as_str(),
                                theme.as_str()
                            );
                        } else {
                            println!("active_state ignored (manual override active)");
                        }
                    } else {
                        println!("unknown state payload");
                    }
                }
                Err(e) => {
                    println!("mqtt read failed: {e:?}");
                    break;
                }
            }
        }

        Timer::after(Duration::from_secs(2)).await;
    }
}

async fn resolve_broker(stack: Stack<'_>, broker: &str) -> Option<embassy_net::IpAddress> {
    if let Ok(ip) = broker.parse::<embassy_net::Ipv4Address>() {
        return Some(embassy_net::IpAddress::Ipv4(ip));
    }
    match stack.dns_query(broker, DnsQueryType::A).await {
        Ok(addrs) if !addrs.is_empty() => Some(addrs[0]),
        Ok(_) => {
            println!("mqtt DNS empty for {broker}");
            None
        }
        Err(e) => {
            println!("mqtt DNS error: {e:?}");
            None
        }
    }
}

#[allow(clippy::large_stack_frames)]
#[embassy_executor::task]
async fn connection(mut controller: WifiController<'static>) {
    println!("wifi connection task");
    // Lower TX power for better WPA auth on small ESP32 boards (ESP32-C3 Super Mini / S2 Mini).
    const TX_POWER: i8 = 34;
    loop {
        if controller.is_connected() {
            WIFI_CONNECTED.store(true, Ordering::Relaxed);
            let _ = controller.wait_for_disconnect_async().await;
            WIFI_CONNECTED.store(false, Ordering::Relaxed);
            println!("wifi disconnected");
            let _ = controller.set_power_saving(esp_radio::wifi::PowerSaveMode::None);
            Timer::after(Duration::from_secs(3)).await;
        }

        WIFI_CONNECTED.store(false, Ordering::Relaxed);

        let cfg = get_runtime_config();

        let station_config = WifiConfig::Station(
            StationConfig::default()
                .with_ssid(cfg.ssid.as_str())
                .with_password(cfg.pass.as_str().into()),
        );
        if let Err(e) = controller.set_config(&station_config) {
            println!("wifi set_config: {e:?}");
            Timer::after(Duration::from_secs(3)).await;
            continue;
        }

        let _ = controller.set_power_saving(esp_radio::wifi::PowerSaveMode::None);
        if let Err(e) = controller.set_max_tx_power(TX_POWER) {
            println!("wifi set_max_tx_power: {e:?}");
        }

        println!("wifi connecting…");
        match controller.connect_async().await {
            Ok(_) => {
                WIFI_CONNECTED.store(true, Ordering::Relaxed);
                println!("wifi connected");
                let _ = controller.set_power_saving(esp_radio::wifi::PowerSaveMode::None);
            }
            Err(e) => {
                WIFI_CONNECTED.store(false, Ordering::Relaxed);
                println!("wifi connect failed: {e:?}");
                Timer::after(Duration::from_secs(5)).await;
            }
        }
    }
}

#[embassy_executor::task]
async fn net_task(mut runner: Runner<'static, esp_radio::wifi::Interface<'static>>) -> ! {
    runner.run().await
}
