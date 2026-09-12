#![no_std]
#![no_main]
#![deny(
    clippy::mem_forget,
    reason = "mem::forget is generally not safe to do with esp_hal types, especially those \
    holding buffers for the duration of a data transfer."
)]
#![deny(clippy::large_stack_frames)]

use esp_hal::clock::CpuClock;
use esp_hal::delay::Delay;
use esp_hal::spi::master::{Config as SpiConfig, Spi};
use esp_hal::spi::Mode as SpiMode;
use esp_hal::time::Rate;
use kitt_eye::{KittScanner, ANIM_SPEED_MS, BASE_COLOR, LED_COUNT};
use smart_leds::{brightness, SmartLedsWrite, RGB8};
use ws2812_spi::prerendered::Ws2812;

#[panic_handler]
fn panic(_: &core::panic::PanicInfo) -> ! {
    loop {}
}

// This creates a default app-descriptor required by the esp-idf bootloader.
// For more information see: <https://docs.espressif.com/projects/esp-idf/en/stable/esp32/api-reference/system/app_image_format.html#application-description>
esp_bootloader_esp_idf::esp_app_desc!();

/// Global brightness 0..=255 (KnightRider default: 255). Dimmed a bit for eye comfort.
const BRIGHTNESS: u8 = 64;

/// SPI encode buffer: 12 bytes/LED + 140 (mosi_idle_high) + 140 (reset_single_transaction).
const SPI_BUF_LEN: usize = LED_COUNT * 12 + 280;

#[allow(
    clippy::large_stack_frames,
    reason = "prerendered WS2812 SPI encode buffer lives on the main stack"
)]
#[esp_hal::main]
fn main() -> ! {
    let config = esp_hal::Config::default().with_cpu_clock(CpuClock::max());
    let peripherals = esp_hal::init(config);

    // Same wiring as rusty-hangulclock: GPIO4=SCK (unused), GPIO6=MOSI→DIN.
    // Frequency must stay in 2.0–3.8 MHz (ws2812-spi docs).
    let spi = Spi::new(
        peripherals.SPI2,
        SpiConfig::default()
            .with_frequency(Rate::from_khz(3200))
            .with_mode(SpiMode::_0),
    )
    .expect("SPI init")
    .with_sck(peripherals.GPIO4)
    .with_mosi(peripherals.GPIO6);

    // Prerendered: one continuous SPI write per frame (avoids mid-frame gaps).
    let mut spi_buf = [0u8; SPI_BUF_LEN];
    let mut strip = Ws2812::new(spi, &mut spi_buf);
    let mut scanner = KittScanner::new(BASE_COLOR);
    let delay = Delay::new();

    // Hold interrupts off during transfer so no gaps appear mid-frame
    // (ws2812-spi README: gaps → random / stuck LEDs).
    critical_section::with(|_| {
        let _ = strip.write([RGB8::new(0, 0, 0); LED_COUNT]);
    });

    loop {
        let frame = *scanner.step();
        critical_section::with(|_| {
            let _ = strip.write(brightness(frame.into_iter(), BRIGHTNESS));
        });
        delay.delay_millis(ANIM_SPEED_MS);
    }
}
