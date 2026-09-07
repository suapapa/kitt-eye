// Package state tracks live agent sessions with TTL expiry and computes the
// aggregate state the publisher exposes (§4.2): per-agent state is the
// highest-priority state among its sessions, and a global active_state is the
// highest-priority state among agents.
package state

import (
	"context"
	"sort"
	"sync"
	"time"

	"github.com/suapapa/kitt-eye/pub/internal/model"
)

// Config bounds session tracking. Zero values fall back to the defaults from
// config.example.yaml.
type Config struct {
	DoneTTL    time.Duration // done -> idle after this
	ErrorTTL   time.Duration // error -> idle after this
	StaleTTL   time.Duration // active state with no events -> idle
	SessionTTL time.Duration // idle session -> removed entirely
	// MaxSessionsPerAgent caps tracked sessions; a new session evicts the
	// lowest-priority, oldest one.
	MaxSessionsPerAgent int
	// PerAgentMax overrides the cap for specific agents (config
	// agents[].max_sessions).
	PerAgentMax map[string]int
}

func (c Config) withDefaults() Config {
	if c.DoneTTL <= 0 {
		c.DoneTTL = 15 * time.Second
	}
	if c.ErrorTTL <= 0 {
		c.ErrorTTL = 60 * time.Second
	}
	if c.StaleTTL <= 0 {
		c.StaleTTL = 300 * time.Second
	}
	if c.SessionTTL <= 0 {
		c.SessionTTL = 3600 * time.Second
	}
	if c.MaxSessionsPerAgent <= 0 {
		c.MaxSessionsPerAgent = 8
	}
	return c
}

// Session is one tracked agent session, published verbatim on the sessions
// topic.
type Session struct {
	SessionID string      `json:"session_id"`
	State     model.State `json:"state"`
	Detail    string      `json:"detail,omitempty"`
	Event     string      `json:"event,omitempty"`
	UpdatedAt time.Time   `json:"updated_at"`
}

// AgentSnapshot is the retained aggregate view of one agent.
type AgentSnapshot struct {
	Agent     string      `json:"agent"`
	State     model.State `json:"state"`
	Sessions  []Session   `json:"sessions"`
	UpdatedAt time.Time   `json:"updated_at"`
}

// Store is the source of truth for agent state. Apply and Expire mutate;
// subscribers are notified after every mutation so sinks can republish the
// current snapshot (coalesced by the caller).
type Store struct {
	cfg Config
	now func() time.Time

	mu     sync.Mutex
	agents map[string]map[string]Session // agent -> sessionID -> session

	// subscribers must be registered before concurrent use (wiring time).
	subscribers []func()
}

// New returns an empty Store.
func New(cfg Config) *Store {
	return &Store{cfg: cfg.withDefaults(), now: time.Now, agents: make(map[string]map[string]Session)}
}

// SetClock replaces the time source (test seam for TTL behavior).
// Call before any concurrent use.
func (s *Store) SetClock(f func() time.Time) { s.now = f }

// Subscribe registers a change callback invoked without locks held.
func (s *Store) Subscribe(fn func()) { s.subscribers = append(s.subscribers, fn) }

// Apply records an incoming hook event. Every event refreshes UpdatedAt, so a
// busy session never expires via TTL.
func (s *Store) Apply(ev model.Event) {
	s.mu.Lock()
	sessions, ok := s.agents[ev.Agent]
	if !ok {
		sessions = make(map[string]Session)
		s.agents[ev.Agent] = sessions
	}
	if _, known := sessions[ev.SessionID]; !known && len(sessions) >= s.maxFor(ev.Agent) {
		s.evictOldestWorth(sessions)
	}
	sessions[ev.SessionID] = Session{
		SessionID: ev.SessionID,
		State:     ev.State,
		Detail:    ev.Detail,
		Event:     ev.EventName,
		UpdatedAt: s.now(),
	}
	s.mu.Unlock()
	s.notify()
}

func (s *Store) maxFor(agent string) int {
	if n, ok := s.cfg.PerAgentMax[agent]; ok && n > 0 {
		return n
	}
	return s.cfg.MaxSessionsPerAgent
}

// evictOldestWorth removes the least significant session: lowest priority
// first, then oldest update.
func (s *Store) evictOldestWorth(sessions map[string]Session) {
	var victimID string
	var victim Session
	for id, sess := range sessions {
		worse := victimID == "" ||
			sess.State.Priority() < victim.State.Priority() ||
			(sess.State.Priority() == victim.State.Priority() && sess.UpdatedAt.Before(victim.UpdatedAt))
		if worse {
			victimID, victim = id, sess
		}
	}
	if victimID != "" {
		delete(sessions, victimID)
	}
}

// Expire applies the TTL table (§5 of pub-go/PLAN.md) once. Run periodically
// via RunJanitor; exported so tests drive it directly with a fake clock.
func (s *Store) Expire() {
	s.mu.Lock()
	now := s.now()
	changed := false
	for agent, sessions := range s.agents {
		for id, sess := range sessions {
			age := now.Sub(sess.UpdatedAt)
			switch sess.State {
			case model.StateIdle:
				if s.cfg.SessionTTL > 0 && age >= s.cfg.SessionTTL {
					delete(sessions, id)
					changed = true
					continue
				}
			case model.StateDone:
				if s.cfg.DoneTTL > 0 && age >= s.cfg.DoneTTL {
					sess.State, sess.Detail, sess.Event, sess.UpdatedAt = model.StateIdle, "", "", now
					sessions[id] = sess
					changed = true
				}
			case model.StateError:
				if s.cfg.ErrorTTL > 0 && age >= s.cfg.ErrorTTL {
					sess.State, sess.Detail, sess.Event, sess.UpdatedAt = model.StateIdle, "", "", now
					sessions[id] = sess
					changed = true
				}
			default: // thinking / generating / executing_tool / waiting_input
				if s.cfg.StaleTTL > 0 && age >= s.cfg.StaleTTL {
					// Safety net for lost Stop/SessionEnd hooks (§4.4.6-1).
					sess.State, sess.Detail, sess.Event, sess.UpdatedAt = model.StateIdle, "stale", "", now
					sessions[id] = sess
					changed = true
				}
			}
		}
		if len(sessions) == 0 {
			delete(s.agents, agent)
		}
	}
	s.mu.Unlock()
	if changed {
		s.notify()
	}
}

// RunJanitor calls Expire on an interval until ctx is done.
func (s *Store) RunJanitor(ctx context.Context, interval time.Duration) {
	t := time.NewTicker(interval)
	defer t.Stop()
	for {
		select {
		case <-ctx.Done():
			return
		case <-t.C:
			s.Expire()
		}
	}
}

// Snapshot returns every agent aggregate sorted by agent name.
func (s *Store) Snapshot() []AgentSnapshot {
	s.mu.Lock()
	defer s.mu.Unlock()
	out := make([]AgentSnapshot, 0, len(s.agents))
	for agent, sessions := range s.agents {
		out = append(out, aggregate(agent, sessions))
	}
	sort.Slice(out, func(i, j int) bool { return out[i].Agent < out[j].Agent })
	return out
}

// ActiveState returns the globally dominant agent and state, or ("", idle)
// when nothing is tracked.
func (s *Store) ActiveState() (string, model.State) {
	var agent string
	var st model.State = model.StateIdle
	var updated time.Time
	for _, snap := range s.Snapshot() {
		better := snap.State.Priority() > st.Priority() ||
			(snap.State.Priority() == st.Priority() && snap.UpdatedAt.After(updated))
		if better && snap.State != model.StateIdle {
			agent, st, updated = snap.Agent, snap.State, snap.UpdatedAt
		}
	}
	if agent == "" {
		return "", model.StateIdle
	}
	return agent, st
}

func aggregate(agent string, sessions map[string]Session) AgentSnapshot {
	snap := AgentSnapshot{Agent: agent, State: model.StateIdle, Sessions: make([]Session, 0, len(sessions))}
	for _, sess := range sessions {
		snap.Sessions = append(snap.Sessions, sess)
		better := sess.State.Priority() > snap.State.Priority() ||
			(sess.State.Priority() == snap.State.Priority() && sess.UpdatedAt.After(snap.UpdatedAt))
		if better {
			snap.State = sess.State
		}
		if sess.UpdatedAt.After(snap.UpdatedAt) {
			snap.UpdatedAt = sess.UpdatedAt
		}
	}
	sort.Slice(snap.Sessions, func(i, j int) bool {
		a, b := snap.Sessions[i], snap.Sessions[j]
		if !a.UpdatedAt.Equal(b.UpdatedAt) {
			return a.UpdatedAt.Before(b.UpdatedAt)
		}
		return a.SessionID < b.SessionID
	})
	return snap
}

func (s *Store) notify() {
	for _, fn := range s.subscribers {
		fn()
	}
}
