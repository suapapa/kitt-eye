#![no_std]

pub mod cli;
pub mod config;
pub mod mqtt;
pub mod patterns;
pub mod state;
pub mod theme;

pub use patterns::{
    calculate_idle_brightness, LedEngine, PatternId, ANIM_SPEED_MS, BASE_COLOR,
    FAST_SCANNER_SPEED_MS, IDLE_FADE_OUT_DURATION_MS, IDLE_OFF_TIMEOUT_MS, LED_COUNT,
    SLOW_SCANNER_SPEED_MS, SWEEP_SPEED_MS,
};
pub use state::AgentState;
pub use theme::MotionTheme;
