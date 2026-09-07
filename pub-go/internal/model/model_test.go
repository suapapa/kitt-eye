package model

import (
	"strings"
	"testing"
)

func TestStatePriorityOrder(t *testing.T) {
	ordered := []State{
		StateIdle, StateDone, StateThinking, StateGenerating,
		StateExecutingTool, StateWaitingInput, StateError,
	}
	for i := 1; i < len(ordered); i++ {
		if ordered[i].Priority() <= ordered[i-1].Priority() {
			t.Fatalf("priority %q (%d) must exceed %q (%d)",
				ordered[i], ordered[i].Priority(), ordered[i-1], ordered[i-1].Priority())
		}
	}
	if !StateIdle.Valid() || State("bogus").Valid() {
		t.Fatal("Valid disagrees with Priority")
	}
}

func TestParseEventV1DefaultsSession(t *testing.T) {
	ev, err := ParseEvent([]byte(
		`{"agent":"cursor-cli","state":"executing_tool","detail":"Running git diff","timestamp":"2026-09-07T11:26:00Z"}`,
	))
	if err != nil {
		t.Fatal(err)
	}
	if ev.SessionID != DefaultSession {
		t.Errorf("SessionID = %q, want %q", ev.SessionID, DefaultSession)
	}
	if ev.Agent != "cursor-cli" || ev.State != StateExecutingTool {
		t.Errorf("unexpected event %+v", ev)
	}
}

func TestParseEventV1SessionAndEvent(t *testing.T) {
	ev, err := ParseEvent([]byte(`{"agent":"claude","state":"done","detail":"ok","session_id":"abc123","event":"Stop"}`))
	if err != nil {
		t.Fatal(err)
	}
	if ev.SessionID != "abc123" || ev.EventName != "Stop" {
		t.Errorf("v1.1 fields lost: %+v", ev)
	}
}

func TestParseEventRejects(t *testing.T) {
	for _, tc := range []struct {
		name string
		line string
	}{
		{name: "not json", line: `{"agent":`},
		{name: "topic injection in agent", line: `{"agent":"a/b","state":"idle"}`},
		{name: "empty agent", line: `{"agent":"","state":"idle"}`},
		{name: "unknown state", line: `{"agent":"claude","state":"sleeping"}`},
		{name: "bad session chars", line: `{"agent":"claude","state":"idle","session_id":"weird/../id"}`},
	} {
		if _, err := ParseEvent([]byte(tc.line)); err == nil {
			t.Errorf("%s: expected error, got nil", tc.name)
		}
	}
}

func TestParseEventDropsHostileEventNameOnly(t *testing.T) {
	ev, err := ParseEvent([]byte(`{"agent":"claude","state":"idle","event":"Bad Event!"}`))
	if err != nil {
		t.Fatalf("event must survive a bad event name: %v", err)
	}
	if ev.EventName != "" {
		t.Errorf("EventName = %q, want dropped to empty", ev.EventName)
	}
}

func TestSanitizeDetail(t *testing.T) {
	ev, err := ParseEvent([]byte(`{"agent":"claude","state":"thinking","detail":"a\tb\"c\nd"}`))
	if err != nil {
		t.Fatal(err)
	}
	if strings.ContainsAny(ev.Detail, "\"\t\n") {
		t.Errorf("detail not sanitized: %q", ev.Detail)
	}

	long := `{"agent":"claude","state":"thinking","detail":"` + strings.Repeat("x", 200) + `"}`
	ev, err = ParseEvent([]byte(long))
	if err != nil {
		t.Fatal(err)
	}
	if len(ev.Detail) > MaxDetailLen {
		t.Errorf("detail len %d exceeds cap %d", len(ev.Detail), MaxDetailLen)
	}

	// Truncation must not cut a multi-byte rune in half.
	uni := `{"agent":"claude","state":"thinking","detail":"` + strings.Repeat("한", 100) + `"}`
	ev, err = ParseEvent([]byte(uni))
	if err != nil {
		t.Fatal(err)
	}
	if !utf8Valid(ev.Detail) {
		t.Errorf("truncation split a rune: %q", ev.Detail)
	}
}

func utf8Valid(s string) bool {
	for _, r := range s {
		if r == '\uFFFD' {
			return false
		}
	}
	return true
}
