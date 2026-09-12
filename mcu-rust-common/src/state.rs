//! Agent active states published on `kitt-eye/active_state`.

/// Aggregated agent state (matches Go publisher wire values).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum AgentState {
    Idle = 0,
    Thinking = 1,
    Generating = 2,
    ExecutingTool = 3,
    WaitingInput = 4,
    Done = 5,
    Error = 6,
}

impl AgentState {
    /// Parse MQTT payload bytes (`executing_tool`, etc.). Unknown → `None`.
    pub fn from_mqtt(payload: &[u8]) -> Option<Self> {
        match payload {
            b"idle" => Some(Self::Idle),
            b"thinking" => Some(Self::Thinking),
            b"generating" => Some(Self::Generating),
            b"executing_tool" => Some(Self::ExecutingTool),
            b"waiting_input" => Some(Self::WaitingInput),
            b"done" => Some(Self::Done),
            b"error" => Some(Self::Error),
            _ => None,
        }
    }

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Idle => "idle",
            Self::Thinking => "thinking",
            Self::Generating => "generating",
            Self::ExecutingTool => "executing_tool",
            Self::WaitingInput => "waiting_input",
            Self::Done => "done",
            Self::Error => "error",
        }
    }

    pub const fn from_u8(v: u8) -> Self {
        match v {
            1 => Self::Thinking,
            2 => Self::Generating,
            3 => Self::ExecutingTool,
            4 => Self::WaitingInput,
            5 => Self::Done,
            6 => Self::Error,
            _ => Self::Idle,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_mqtt_states() {
        assert_eq!(AgentState::from_mqtt(b"idle"), Some(AgentState::Idle));
        assert_eq!(AgentState::from_mqtt(b"thinking"), Some(AgentState::Thinking));
        assert_eq!(AgentState::from_mqtt(b"generating"), Some(AgentState::Generating));
        assert_eq!(AgentState::from_mqtt(b"executing_tool"), Some(AgentState::ExecutingTool));
        assert_eq!(AgentState::from_mqtt(b"waiting_input"), Some(AgentState::WaitingInput));
        assert_eq!(AgentState::from_mqtt(b"done"), Some(AgentState::Done));
        assert_eq!(AgentState::from_mqtt(b"error"), Some(AgentState::Error));
        assert_eq!(AgentState::from_mqtt(b"invalid"), None);
        assert_eq!(AgentState::from_mqtt(b""), None);
    }

    #[test]
    fn roundtrip_u8() {
        for v in 0..=6 {
            let state = AgentState::from_u8(v);
            assert_eq!(state as u8, v);
        }
        assert_eq!(AgentState::from_u8(99), AgentState::Idle);
    }

    #[test]
    fn str_representation() {
        assert_eq!(AgentState::Idle.as_str(), "idle");
        assert_eq!(AgentState::ExecutingTool.as_str(), "executing_tool");
    }
}

