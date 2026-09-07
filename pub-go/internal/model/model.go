// Package model defines the wire protocol between kitt-eye hooks and the
// publisher daemon (PLAN.md §4.1.2), plus the state vocabulary and its
// aggregation priority (§4.1.1).
package model

import (
	"encoding/json"
	"fmt"
	"regexp"
	"strings"
	"unicode/utf8"
)

// State is a normalized agent state.
type State string

const (
	StateIdle          State = "idle"
	StateThinking      State = "thinking"
	StateGenerating    State = "generating"
	StateExecutingTool State = "executing_tool"
	StateWaitingInput  State = "waiting_input"
	StateError         State = "error"
	StateDone          State = "done"
)

// Priority implements the aggregation order from §4.1.1:
// error > waiting_input > executing_tool > generating > thinking > done > idle.
// Unknown states sort lowest so they can never mask a real state.
func (s State) Priority() int {
	switch s {
	case StateError:
		return 6
	case StateWaitingInput:
		return 5
	case StateExecutingTool:
		return 4
	case StateGenerating:
		return 3
	case StateThinking:
		return 2
	case StateDone:
		return 1
	case StateIdle:
		return 0
	default:
		return -1
	}
}

// Valid reports whether s is part of the state spec.
func (s State) Valid() bool { return s.Priority() >= 0 }

const (
	// DefaultSession is the session key assumed for protocol v1.0 events
	// that omit session_id (§4.1.2).
	DefaultSession = "default"
	// MaxDetailLen bounds the sanitized detail field in bytes.
	MaxDetailLen = 120
)

// agentRe guards against MQTT topic injection: agent names embed into topics.
var agentRe = regexp.MustCompile(`^[A-Za-z0-9][A-Za-z0-9_-]{0,31}$`)

// ValidAgent reports whether name is safe to embed in an MQTT topic.
func ValidAgent(name string) bool { return agentRe.MatchString(name) }

// sessionRe is likewise topic/JSON-log safe.
var sessionRe = regexp.MustCompile(`^[A-Za-z0-9_.:-]{1,64}$`)

// Event is the JSON datagram hooks send over the UDS. Optional fields follow
// the v1.1 extension (session_id, event).
type Event struct {
	Agent     string `json:"agent"`
	State     State  `json:"state"`
	Detail    string `json:"detail,omitempty"`
	Timestamp string `json:"timestamp,omitempty"`
	SessionID string `json:"session_id,omitempty"`
	EventName string `json:"event,omitempty"`
}

// ParseEvent decodes and normalizes a datagram received on the IPC socket.
// It is intentionally strict: a malformed or hostile event is dropped by the
// caller, never guessed at.
func ParseEvent(data []byte) (Event, error) {
	var ev Event
	if err := json.Unmarshal(data, &ev); err != nil {
		return ev, fmt.Errorf("decode event: %w", err)
	}
	if !agentRe.MatchString(ev.Agent) {
		return Event{}, fmt.Errorf("invalid agent name %q", ev.Agent)
	}
	if !ev.State.Valid() {
		return Event{}, fmt.Errorf("unknown state %q", ev.State)
	}
	if ev.SessionID != "" && !sessionRe.MatchString(ev.SessionID) {
		return Event{}, fmt.Errorf("invalid session_id %q", ev.SessionID)
	}
	if ev.SessionID == "" {
		ev.SessionID = DefaultSession
	}
	ev.Detail = sanitizeDetail(ev.Detail)
	if ev.EventName != "" && !agentRe.MatchString(ev.EventName) {
		// EventName is observational metadata only; a hostile value is
		// dropped rather than rejecting the whole event, so one odd field
		// never loses a real state transition.
		ev.EventName = ""
	}
	return ev, nil
}

// sanitizeDetail strips control characters and quotes that could corrupt
// downstream JSON/terminals, then truncates to MaxDetailLen bytes on a
// rune boundary.
func sanitizeDetail(s string) string {
	s = strings.Map(func(r rune) rune {
		needsReplace := r < 0x20 || r == '"' || r == 0x7f
		if needsReplace {
			return ' '
		}
		return r
	}, s)
	s = strings.TrimSpace(s)
	if len(s) > MaxDetailLen {
		cut := MaxDetailLen
		for cut > 0 && !utf8.RuneStart(s[cut]) {
			cut--
		}
		s = s[:cut]
	}
	return s
}
