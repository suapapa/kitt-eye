package publish

import (
	"bytes"
	"encoding/json"
	"strings"
	"testing"
	"time"

	"github.com/suapapa/kitt-eye/pub/internal/model"
	"github.com/suapapa/kitt-eye/pub/internal/state"
)

func TestStdoutSinkEmitsTaggedLines(t *testing.T) {
	var buf bytes.Buffer
	s := NewStdout(&buf)
	snap := state.AgentSnapshot{
		Agent:     "claude",
		State:     model.StateExecutingTool,
		Sessions:  []state.Session{{SessionID: "s1", State: model.StateExecutingTool, UpdatedAt: time.Now()}},
		UpdatedAt: time.Now(),
	}

	for _, err := range []error{
		s.PublishAgent(snap),
		s.PublishActive("claude", model.StateExecutingTool),
		s.PublishStatus(true),
		s.PublishStatus(false),
	} {
		if err != nil {
			t.Fatal(err)
		}
	}

	lines := strings.Split(strings.TrimSpace(buf.String()), "\n")
	if len(lines) != 4 {
		t.Fatalf("want 4 JSON lines, got %d: %q", len(lines), buf.String())
	}
	wantTypes := []string{"agent", "active", "status", "status"}
	for i, line := range lines {
		var rec struct {
			Type string `json:"type"`
		}
		if err := json.Unmarshal([]byte(line), &rec); err != nil {
			t.Fatalf("line %d not JSON: %v", i, err)
		}
		if rec.Type != wantTypes[i] {
			t.Errorf("line %d type = %q, want %q", i, rec.Type, wantTypes[i])
		}
	}
}
