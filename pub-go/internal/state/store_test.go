package state

import (
	"testing"
	"time"

	"github.com/suapapa/kitt-eye/pub/internal/model"
)

type fakeClock struct{ t time.Time }

func (c *fakeClock) Now() time.Time          { return c.t }
func (c *fakeClock) Advance(d time.Duration) { c.t = c.t.Add(d) }

func newTestStore(t *testing.T, cfg Config) (*Store, *fakeClock, *int) {
	t.Helper()
	clock := &fakeClock{t: time.Date(2026, 9, 7, 12, 0, 0, 0, time.UTC)}
	s := New(cfg)
	s.SetClock(clock.Now)
	changes := 0
	s.Subscribe(func() { changes++ })
	return s, clock, &changes
}

func event(agent, session string, st model.State) model.Event {
	return model.Event{Agent: agent, SessionID: session, State: st}
}

func TestApplyAggregatesByPriority(t *testing.T) {
	s, _, _ := newTestStore(t, Config{})
	s.Apply(event("claude", "s1", model.StateThinking))
	s.Apply(event("claude", "s2", model.StateExecutingTool))
	s.Apply(event("claude", "s3", model.StateIdle))

	snaps := s.Snapshot()
	if len(snaps) != 1 || snaps[0].Agent != "claude" {
		t.Fatalf("snapshot = %+v", snaps)
	}
	if snaps[0].State != model.StateExecutingTool {
		t.Errorf("agent state = %q, want executing_tool", snaps[0].State)
	}
	if len(snaps[0].Sessions) != 3 {
		t.Errorf("sessions = %d, want 3", len(snaps[0].Sessions))
	}
}

func TestActiveStateAcrossAgents(t *testing.T) {
	s, _, _ := newTestStore(t, Config{})
	s.Apply(event("codex", "default", model.StateThinking))
	s.Apply(event("claude", "a", model.StateExecutingTool))

	agent, st := s.ActiveState()
	if agent != "claude" || st != model.StateExecutingTool {
		t.Errorf("ActiveState = (%q,%q), want (claude,executing_tool)", agent, st)
	}
	// error outranks everything
	s.Apply(event("cursor-cli", "x", model.StateError))
	if agent, st := s.ActiveState(); agent != "cursor-cli" || st != model.StateError {
		t.Errorf("after error: ActiveState = (%q,%q)", agent, st)
	}
}

func TestActiveStateIdleWhenNothingTracked(t *testing.T) {
	s, _, _ := newTestStore(t, Config{})
	if agent, st := s.ActiveState(); agent != "" || st != model.StateIdle {
		t.Errorf("empty store ActiveState = (%q,%q), want (\"\",idle)", agent, st)
	}
}

func TestExpireDoneAndError(t *testing.T) {
	cfg := Config{DoneTTL: 15 * time.Second, ErrorTTL: 60 * time.Second}
	s, clock, changes := newTestStore(t, cfg)
	s.Apply(event("claude", "d", model.StateDone))
	s.Apply(event("codex", "e", model.StateError))
	before := *changes

	clock.Advance(14 * time.Second)
	s.Expire()
	if snap := s.Snapshot(); snap[0].State != model.StateDone {
		t.Errorf("done expired early: %q", snap[0].State)
	}

	clock.Advance(time.Second) // done TTL elapsed, error has not
	s.Expire()
	snap := s.Snapshot()
	if findAgent(t, snap, "claude").State != model.StateIdle {
		t.Error("done should have expired to idle")
	}
	if findAgent(t, snap, "codex").State != model.StateError {
		t.Error("error should still be within TTL")
	}
	if *changes <= before {
		t.Error("expiry must notify subscribers")
	}
}

func TestExpireStaleActiveState(t *testing.T) {
	cfg := Config{StaleTTL: 300 * time.Second}
	s, clock, _ := newTestStore(t, cfg)
	s.Apply(event("claude", "crashed", model.StateExecutingTool))

	clock.Advance(299 * time.Second)
	s.Expire()
	if findAgent(t, s.Snapshot(), "claude").State != model.StateExecutingTool {
		t.Fatal("stale must not fire before TTL")
	}
	clock.Advance(2 * time.Second)
	s.Expire()
	sess := findAgent(t, s.Snapshot(), "claude").Sessions[0]
	if sess.State != model.StateIdle || sess.Detail != "stale" {
		t.Errorf("stale session = %+v, want idle/stale", sess)
	}
}

func TestExpireRemovesIdleSessionsThenAgents(t *testing.T) {
	cfg := Config{SessionTTL: 3600 * time.Second}
	s, clock, _ := newTestStore(t, cfg)
	s.Apply(event("claude", "s1", model.StateIdle))

	clock.Advance(3601 * time.Second)
	s.Expire()
	if snaps := s.Snapshot(); len(snaps) != 0 {
		t.Errorf("agent should be dropped entirely, got %+v", snaps)
	}
}

func TestEventsKeepSessionAlive(t *testing.T) {
	cfg := Config{StaleTTL: 10 * time.Second}
	s, clock, _ := newTestStore(t, cfg)
	s.Apply(event("claude", "busy", model.StateThinking))
	for i := 0; i < 5; i++ {
		clock.Advance(9 * time.Second)
		s.Expire()
		s.Apply(event("claude", "busy", model.StateThinking)) // heartbeat
	}
	if findAgent(t, s.Snapshot(), "claude").State != model.StateThinking {
		t.Error("active session with events must not go stale")
	}
}

func TestEvictionPrefersLowestPriorityOldest(t *testing.T) {
	cfg := Config{PerAgentMax: map[string]int{"claude": 2}}
	s, clock, _ := newTestStore(t, cfg)
	s.Apply(event("claude", "idle-old", model.StateIdle))
	clock.Advance(time.Second)
	s.Apply(event("claude", "busy-a", model.StateExecutingTool))
	clock.Advance(time.Second)
	s.Apply(event("claude", "busy-b", model.StateThinking))

	// Third distinct session at cap: idle-old must be the victim.
	ids := map[string]bool{}
	for _, sess := range findAgent(t, s.Snapshot(), "claude").Sessions {
		ids[sess.SessionID] = true
	}
	if ids["idle-old"] {
		t.Errorf("idle-old should have been evicted, sessions=%v", ids)
	}
	if !ids["busy-a"] || !ids["busy-b"] {
		t.Errorf("active sessions must survive eviction, got %v", ids)
	}
}

func TestNotifyFiresPerApply(t *testing.T) {
	s, _, changes := newTestStore(t, Config{})
	s.Apply(event("claude", "s", model.StateThinking))
	s.Apply(event("claude", "s", model.StateDone))
	if *changes != 2 {
		t.Errorf("subscriber calls = %d, want 2", *changes)
	}
}

func TestSnapshotSortedAndStable(t *testing.T) {
	s, _, _ := newTestStore(t, Config{})
	s.Apply(event("codex", "s", model.StateThinking))
	s.Apply(event("claude", "s", model.StateIdle))
	s.Apply(event("cursor-cli", "s", model.StateIdle))
	snaps := s.Snapshot()
	want := []string{"claude", "codex", "cursor-cli"}
	for i, name := range want {
		if snaps[i].Agent != name {
			t.Fatalf("snapshot order = %v, want %v", []string{snaps[0].Agent, snaps[1].Agent, snaps[2].Agent}, want)
		}
	}
}

func findAgent(t *testing.T, snaps []AgentSnapshot, agent string) AgentSnapshot {
	t.Helper()
	for _, s := range snaps {
		if s.Agent == agent {
			return s
		}
	}
	t.Fatalf("agent %q not in snapshot %+v", agent, snaps)
	return AgentSnapshot{}
}
