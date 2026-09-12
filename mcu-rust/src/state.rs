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
