//! Motion themes: map `AgentState` → LED pattern and color.

use smart_leds::RGB8;

use crate::patterns::PatternId;
use crate::state::AgentState;

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
        AgentState::Idle => PatternId::Breathing,
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
        AgentState::Idle => PatternId::Breathing,
        AgentState::Thinking => PatternId::CenterOut,
        AgentState::Generating => PatternId::FillSweep,
        AgentState::ExecutingTool => PatternId::KittScanner,
        AgentState::WaitingInput => PatternId::Blink,
        AgentState::Done => PatternId::Flash,
        AgentState::Error => PatternId::Comet,
    }
}

const fn colorful_color(state: AgentState) -> RGB8 {
    match state {
        AgentState::Idle => RGB8::new(0, 180, 255),          // Cool Ice Cyan
        AgentState::Thinking => RGB8::new(170, 0, 255),      // Electric Violet
        AgentState::Generating => RGB8::new(0, 240, 120),    // Neo Mint Emerald
        AgentState::ExecutingTool => RGB8::new(255, 140, 0), // Vivid Amber (Classic K.I.T.T. Scanner)
        AgentState::WaitingInput => RGB8::new(255, 215, 0),  // Radiant Warning Yellow
        AgentState::Done => RGB8::new(0, 255, 50),           // Pure Spring Lime Green
        AgentState::Error => RGB8::new(255, 20, 20),         // Fiery Crimson Red
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classic_maps_all_states() {
        let t = MotionTheme::Classic;
        assert_eq!(t.pattern_for(AgentState::Idle), PatternId::Breathing);
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
        assert_eq!(t.pattern_for(AgentState::Idle), PatternId::Breathing);
        assert_eq!(t.pattern_for(AgentState::Thinking), PatternId::CenterOut);
        assert_eq!(t.pattern_for(AgentState::Generating), PatternId::FillSweep);
        assert_eq!(
            t.pattern_for(AgentState::ExecutingTool),
            PatternId::KittScanner
        );
        assert_eq!(t.pattern_for(AgentState::WaitingInput), PatternId::Blink);
        assert_eq!(t.pattern_for(AgentState::Done), PatternId::Flash);
        assert_eq!(t.pattern_for(AgentState::Error), PatternId::Comet);

        assert_eq!(t.color_for(AgentState::Idle), RGB8::new(0, 180, 255));
        assert_eq!(t.color_for(AgentState::Thinking), RGB8::new(170, 0, 255));
        assert_eq!(t.color_for(AgentState::Generating), RGB8::new(0, 240, 120));
        assert_eq!(t.color_for(AgentState::ExecutingTool), RGB8::new(255, 140, 0));
        assert_eq!(t.color_for(AgentState::WaitingInput), RGB8::new(255, 215, 0));
        assert_eq!(t.color_for(AgentState::Done), RGB8::new(0, 255, 50));
        assert_eq!(t.color_for(AgentState::Error), RGB8::new(255, 20, 20));
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
