//! Motion themes: map `AgentState` → LED pattern and color.

use smart_leds::RGB8;

use crate::patterns::{hsv_to_rgb, PatternId};
use crate::state::AgentState;

/// 8-bit HSV color representation (Hue, Saturation, Value).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Hsv {
    pub h: u8,
    pub s: u8,
    pub v: u8,
}

impl Hsv {
    pub const fn new(h: u8, s: u8, v: u8) -> Self {
        Self { h, s, v }
    }

    /// Convert HSV to RGB directly without brightness normalization.
    pub const fn to_rgb(self) -> RGB8 {
        hsv_to_rgb(self.h, self.s, self.v)
    }

    /// Convert HSV to RGB, normalizing brightness (V) so that perceived luminance matches `target_lum`.
    pub const fn to_rgb_normalized(self, target_lum: u8) -> RGB8 {
        hsv_to_rgb_normalized(self.h, self.s, target_lum)
    }
}

/// Relative perceived luminance (ITU-R BT.601 weights: Y = 0.299*R + 0.587*G + 0.114*B).
/// Formula: (77 * R + 150 * G + 29 * B) / 256.
pub const fn perceived_luminance(c: RGB8) -> u8 {
    ((77u16 * c.r as u16 + 150u16 * c.g as u16 + 29u16 * c.b as u16) / 256) as u8
}

/// Target luminance calibrated to Classic Knight Rider Red (`RGB8(255, 0, 0)`), which is 76.
pub const TARGET_LUMINANCE: u8 = perceived_luminance(RGB8::new(255, 0, 0));

/// Convert HSV to RGB, normalizing brightness (V) so that perceived luminance matches `target_lum`.
/// This prevents highly luminous colors (Yellow, Mint, Lime, Amber) from overpowering Red.
pub const fn hsv_to_rgb_normalized(h: u8, s: u8, target_lum: u8) -> RGB8 {
    let max_rgb = hsv_to_rgb(h, s, 255);
    let base_lum = perceived_luminance(max_rgb);
    if base_lum <= target_lum || base_lum == 0 {
        max_rgb
    } else {
        let v = ((255u32 * target_lum as u32) / base_lum as u32) as u8;
        hsv_to_rgb(h, s, v)
    }
}

/// Base HSV definition for each agent state in the colorful theme.
pub const fn colorful_hsv(state: AgentState) -> Hsv {
    match state {
        AgentState::Idle => Hsv::new(0, 255, 255),           // Classic Knight Rider Red
        AgentState::Thinking => Hsv::new(199, 255, 255),     // Electric Violet
        AgentState::Generating => Hsv::new(106, 255, 255),   // Neo Mint Emerald
        AgentState::ExecutingTool => Hsv::new(23, 255, 255), // Vivid Amber
        AgentState::WaitingInput => Hsv::new(35, 255, 255),  // Radiant Warning Yellow
        AgentState::Done => Hsv::new(93, 255, 255),          // Pure Spring Lime Green
        AgentState::Error => Hsv::new(0, 235, 255),         // Fiery Crimson Red
    }
}

/// Selectable motion theme (from `MOTION_THEME` env).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MotionTheme {
    /// Classic Knight Rider theme: all patterns use red; states differ by motion only.
    Classic,
    /// Colorful theme: each state has its own distinct vivid color and expressive motion.
    Colorful,
}

impl MotionTheme {
    pub fn from_name(name: &str) -> Self {
        let n = name.trim();
        if eq_ignore_ascii_case(n, "colorful")
            || eq_ignore_ascii_case(n, "colourful")
            || eq_ignore_ascii_case(n, "color")
            || eq_ignore_ascii_case(n, "rgb")
        {
            Self::Colorful
        } else {
            Self::Classic
        }
    }

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Classic => "classic",
            Self::Colorful => "colorful",
        }
    }


    pub const fn pattern_for(self, state: AgentState) -> PatternId {
        match self {
            Self::Classic => classic_pattern(state),
            Self::Colorful => colorful_pattern(state),
        }
    }

    pub const fn color_for(self, state: AgentState) -> RGB8 {
        match self {
            Self::Classic => classic_color(state),
            Self::Colorful => colorful_color(state),
        }
    }
}

fn eq_ignore_ascii_case(a: &str, b: &str) -> bool {
    a.len() == b.len()
        && a.bytes()
            .zip(b.bytes())
            .all(|(x, y)| x.to_ascii_lowercase() == y.to_ascii_lowercase())
}

const fn classic_pattern(state: AgentState) -> PatternId {
    match state {
        AgentState::Idle => PatternId::SlowScanner,
        AgentState::Thinking => PatternId::CenterOut,
        AgentState::Generating => PatternId::FillSweep,
        AgentState::ExecutingTool => PatternId::KittScanner,
        AgentState::WaitingInput => PatternId::Blink,
        AgentState::Done => PatternId::Flash,
        AgentState::Error => PatternId::Comet,
    }
}

const fn classic_color(_state: AgentState) -> RGB8 {
    RGB8::new(255, 0, 0)
}

const fn colorful_pattern(state: AgentState) -> PatternId {
    match state {
        AgentState::Idle => PatternId::SlowScanner,
        AgentState::Thinking => PatternId::CenterOut,
        AgentState::Generating => PatternId::FillSweep,
        AgentState::ExecutingTool => PatternId::KittScanner,
        AgentState::WaitingInput => PatternId::Blink,
        AgentState::Done => PatternId::Flash,
        AgentState::Error => PatternId::Comet,
    }
}

const fn colorful_color(state: AgentState) -> RGB8 {
    colorful_hsv(state).to_rgb_normalized(TARGET_LUMINANCE)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classic_maps_all_states() {
        let t = MotionTheme::Classic;
        assert_eq!(t.pattern_for(AgentState::Idle), PatternId::SlowScanner);
        assert_eq!(t.pattern_for(AgentState::Thinking), PatternId::CenterOut);
        assert_eq!(t.pattern_for(AgentState::Generating), PatternId::FillSweep);
        assert_eq!(
            t.pattern_for(AgentState::ExecutingTool),
            PatternId::KittScanner
        );
        assert_eq!(t.pattern_for(AgentState::WaitingInput), PatternId::Blink);
        assert_eq!(t.pattern_for(AgentState::Done), PatternId::Flash);
        assert_eq!(t.pattern_for(AgentState::Error), PatternId::Comet);

        for s in [
            AgentState::Idle,
            AgentState::Thinking,
            AgentState::Generating,
            AgentState::ExecutingTool,
            AgentState::WaitingInput,
            AgentState::Done,
            AgentState::Error,
        ] {
            assert_eq!(t.color_for(s), RGB8::new(255, 0, 0));
        }
    }

    #[test]
    fn colorful_maps_all_states() {
        let t = MotionTheme::Colorful;
        assert_eq!(t.pattern_for(AgentState::Idle), PatternId::SlowScanner);
        assert_eq!(t.pattern_for(AgentState::Thinking), PatternId::CenterOut);
        assert_eq!(t.pattern_for(AgentState::Generating), PatternId::FillSweep);
        assert_eq!(
            t.pattern_for(AgentState::ExecutingTool),
            PatternId::KittScanner
        );
        assert_eq!(t.pattern_for(AgentState::WaitingInput), PatternId::Blink);
        assert_eq!(t.pattern_for(AgentState::Done), PatternId::Flash);
        assert_eq!(t.pattern_for(AgentState::Error), PatternId::Comet);

        // Verify luminance normalization across all colorful states:
        // All states must have perceived luminance matched to Classic Red (75..=76).
        for s in [
            AgentState::Idle,
            AgentState::Thinking,
            AgentState::Generating,
            AgentState::ExecutingTool,
            AgentState::WaitingInput,
            AgentState::Done,
            AgentState::Error,
        ] {
            let lum = perceived_luminance(t.color_for(s));
            assert!(
                (75..=76).contains(&lum),
                "State {:?} lum {} outside expected range 75..=76",
                s,
                lum
            );
        }

        assert_eq!(t.color_for(AgentState::Idle), RGB8::new(255, 0, 0));
        assert_eq!(t.color_for(AgentState::Thinking), RGB8::new(159, 0, 251));
        assert_eq!(t.color_for(AgentState::Generating), RGB8::new(0, 118, 55));
        assert_eq!(t.color_for(AgentState::ExecutingTool), RGB8::new(123, 66, 0));
        assert_eq!(t.color_for(AgentState::WaitingInput), RGB8::new(97, 79, 0));
        assert_eq!(t.color_for(AgentState::Done), RGB8::new(0, 125, 20));
        assert_eq!(t.color_for(AgentState::Error), RGB8::new(215, 16, 16));
    }

    #[test]
    fn parse_theme_name() {
        assert_eq!(MotionTheme::from_name("classic"), MotionTheme::Classic);
        assert_eq!(MotionTheme::from_name("kitt"), MotionTheme::Classic);
        assert_eq!(MotionTheme::from_name("red"), MotionTheme::Classic);
        assert_eq!(MotionTheme::from_name("COLORFUL"), MotionTheme::Colorful);
        assert_eq!(MotionTheme::from_name("colourful"), MotionTheme::Colorful);
        assert_eq!(MotionTheme::from_name("rgb"), MotionTheme::Colorful);
        assert_eq!(MotionTheme::from_name("color"), MotionTheme::Colorful);
        assert_eq!(MotionTheme::from_name("unknown"), MotionTheme::Classic);
    }
}
