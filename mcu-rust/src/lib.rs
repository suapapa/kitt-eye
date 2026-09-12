#![no_std]

pub mod config;
pub mod mqtt;
pub mod patterns;
pub mod state;
pub mod theme;

pub use patterns::{
    LedEngine, PatternId, ANIM_SPEED_MS, BASE_COLOR, FAST_SCANNER_SPEED_MS, LED_COUNT,
    SLOW_SCANNER_SPEED_MS, SWEEP_SPEED_MS,
};
pub use state::AgentState;
pub use theme::MotionTheme;
