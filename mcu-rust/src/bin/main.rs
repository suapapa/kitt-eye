#![no_std]
#![no_main]
#![deny(
    clippy::mem_forget,
    reason = "mem::forget is generally not safe to do with esp_hal types, especially those \
    holding buffers for the duration of a data transfer."
)]
#![deny(clippy::large_stack_frames)]

use core::sync::atomic::{AtomicU8, Ordering};

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
use kitt_eye::{
    config::{
        self, MQTT_BROKER, MQTT_CLIENT_ID, MQTT_PASS, MQTT_TOPIC_PREFIX, MQTT_USER, MOTION_THEME,
        PASS, SSID,
    },
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

/// Shared agent state written by MQTT task, read by LED loop.
static ACTIVE_STATE: AtomicU8 = AtomicU8::new(AgentState::Idle as u8);

macro_rules! mk_static {
    ($t:ty, $val:expr) => {{
        static STATIC_CELL: StaticCell<$t> = StaticCell::new();
        #[deny(unused_attributes)]
        let x = STATIC_CELL.uninit().write(($val));
        x
    }};
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

    esp_println::logger::init_logger_from_env();
    println!("kitt-eye boot");
    println!(
        "SSID len={} theme={} broker={}:{}",
        SSID.len(),
        MOTION_THEME,
        MQTT_BROKER,
        config::mqtt_port()
    );

    let theme = MotionTheme::from_name(MOTION_THEME);
    let mut engine = LedEngine::new(theme);

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
    loop {
        let state = AgentState::from_u8(ACTIVE_STATE.load(Ordering::Relaxed));
        engine.set_state(state);
        let frame = *engine.step();
        critical_section::with(|_| {
            let _ = strip.write(brightness(frame.into_iter(), BRIGHTNESS));
        });
        Timer::after(Duration::from_millis(ANIM_SPEED_MS as u64)).await;
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

    let mut topic: String<64> = String::new();
    if mqtt::active_state_topic(MQTT_TOPIC_PREFIX, &mut topic).is_err() {
        println!("mqtt topic too long");
        return;
    }
    println!("mqtt subscribe {topic}");

    let port = config::mqtt_port();
    let mut rx_buffer = [0u8; 2048];
    let mut tx_buffer = [0u8; 2048];
    let mut payload = [0u8; 64];

    loop {
        let mut socket = TcpSocket::new(stack, &mut rx_buffer, &mut tx_buffer);
        socket.set_timeout(Some(Duration::from_secs(30)));

        let addr = match resolve_broker(stack).await {
            Some(a) => a,
            None => {
                Timer::after(Duration::from_secs(3)).await;
                continue;
            }
        };

        println!("mqtt connect {MQTT_BROKER}:{port}");
        if socket.connect((addr, port)).await.is_err() {
            println!("mqtt tcp connect failed");
            Timer::after(Duration::from_secs(3)).await;
            continue;
        }

        let mut client = MqttClient::new(socket);
        if let Err(e) = client
            .connect(MQTT_CLIENT_ID, MQTT_USER, MQTT_PASS, 0)
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
                        ACTIVE_STATE.store(state as u8, Ordering::Relaxed);
                        println!("active_state={}", state.as_str());
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

async fn resolve_broker(stack: Stack<'_>) -> Option<embassy_net::IpAddress> {
    if let Ok(ip) = MQTT_BROKER.parse::<embassy_net::Ipv4Address>() {
        return Some(embassy_net::IpAddress::Ipv4(ip));
    }
    match stack.dns_query(MQTT_BROKER, DnsQueryType::A).await {
        Ok(addrs) if !addrs.is_empty() => Some(addrs[0]),
        Ok(_) => {
            println!("mqtt DNS empty for {MQTT_BROKER}");
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
    // ESP32-C3 Super Mini: lower TX power for better WPA auth (see metric-gauge).
    const TX_POWER: i8 = 34;
    loop {
        if controller.is_connected() {
            let _ = controller.wait_for_disconnect_async().await;
            println!("wifi disconnected");
            let _ = controller.set_power_saving(esp_radio::wifi::PowerSaveMode::None);
            Timer::after(Duration::from_secs(3)).await;
        }

        let station_config = WifiConfig::Station(
            StationConfig::default()
                .with_ssid(SSID)
                .with_password(PASS.into()),
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
                println!("wifi connected");
                let _ = controller.set_power_saving(esp_radio::wifi::PowerSaveMode::None);
            }
            Err(e) => {
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
