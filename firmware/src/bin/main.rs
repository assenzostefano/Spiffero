#![no_std]
#![no_main]
#![deny(
    clippy::mem_forget,
    reason = "mem::forget is generally not safe to do with esp_hal types, especially those \
    holding buffers for the duration of a data transfer."
)]
#![deny(clippy::large_stack_frames)]

use embassy_executor::Spawner;
use embassy_time::{Duration, Timer};
use esp_backtrace as _;
use esp_hal::clock::CpuClock;
use esp_hal::gpio::{Input, InputConfig, Level, Output, OutputConfig, Pull};
use esp_hal::timer::timg::TimerGroup;
use log::info;

const REED_PIN_GPIO: u8 = 4;
const REED_CLOSED_IS_LOW: bool = true;

#[derive(Clone, Copy, PartialEq, Eq)]
enum WindowState {
    Open,
    Closed,
}

impl WindowState {
    fn from_reed_level(level: Level) -> Self {
        if REED_CLOSED_IS_LOW {
            if level == Level::Low {
                Self::Closed
            } else {
                Self::Open
            }
        } else if level == Level::High {
            Self::Closed
        } else {
            Self::Open
        }
    }

    fn as_str(self) -> &'static str {
        match self {
            Self::Open => "finestra APERTA",
            Self::Closed => "finestra CHIUSA",
        }
    }
}

// This creates a default app-descriptor required by the esp-idf bootloader.
// For more information see: <https://docs.espressif.com/projects/esp-idf/en/stable/esp32/api-reference/system/app_image_format.html#application-description>
esp_bootloader_esp_idf::esp_app_desc!();

#[allow(
    clippy::large_stack_frames,
    reason = "it's not unusual to allocate larger buffers etc. in main"
)]
#[esp_rtos::main]
async fn main(_spawner: Spawner) -> ! {
    // generator version: 1.2.0

    esp_println::logger::init_logger_from_env();

    let config = esp_hal::Config::default().with_cpu_clock(CpuClock::max());
    let peripherals = esp_hal::init(config);

    let timg0 = TimerGroup::new(peripherals.TIMG0);
    esp_rtos::start(timg0.timer0);

    // Most ESP32 dev boards expose the onboard LED on GPIO2.
    let mut led = Output::new(peripherals.GPIO2, Level::Low, OutputConfig::default());
    let reed_cfg = InputConfig::default().with_pull(Pull::Up);
    let reed = Input::new(peripherals.GPIO4, reed_cfg);

    info!("Reed sensor configurato su GPIO{} (pull-up interno)", REED_PIN_GPIO);
    info!(
        "Mappatura: {} = CHIUSA",
        if REED_CLOSED_IS_LOW { "LOW" } else { "HIGH" }
    );

    // Keep LED off during early boot noise, then start the diagnostic pattern.
    led.set_low();
    Timer::after(Duration::from_secs(2)).await;

    let mut last_state = WindowState::from_reed_level(reed.level());
    info!("Stato iniziale: {}", last_state.as_str());

    loop {
        let current_state = WindowState::from_reed_level(reed.level());

        if current_state != last_state {
            // Simple debounce: re-check after 30ms before confirming a transition.
            Timer::after(Duration::from_millis(30)).await;
            let confirmed_state = WindowState::from_reed_level(reed.level());
            if confirmed_state != last_state {
                last_state = confirmed_state;
                info!("Cambio stato: {}", last_state.as_str());
            }
        }

        match last_state {
            WindowState::Closed => led.set_high(),
            WindowState::Open => led.set_low(),
        }

        Timer::after(Duration::from_millis(100)).await;
    }

    // for inspiration have a look at the examples at https://github.com/esp-rs/esp-hal/tree/esp-hal-v1.0.0/examples
}