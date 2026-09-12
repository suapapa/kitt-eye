//! Motion themes: map `AgentState` → LED pattern (color stays red for `classic`).

use crate::patterns::PatternId;
use crate::state::AgentState;

/// Selectable motion theme (from `MOTION_THEME` env).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MotionTheme {
    /// All patterns use red; states differ by motion only.
    Classic,
}

impl MotionTheme {
    pub fn from_name(name: &str) -> Self {
        // Only `classic` is implemented; unknown names fall back to it.
        let n = name.trim();
        if n.is_empty()
            || eq_ignore_ascii_case(n, "classic")
            || eq_ignore_ascii_case(n, "kitt")
            || eq_ignore_ascii_case(n, "red")
        {
            Self::Classic
        } else {
            Self::Classic
        }
    }

    pub const fn pattern_for(self, state: AgentState) -> PatternId {
        match self {
            Self::Classic => classic_pattern(state),
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
    }
}
