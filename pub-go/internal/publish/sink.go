// Package publish defines where state snapshots go. The MQTT client is the
// production sink; Stdout backs the broker-less dry-run mode.
package publish

import (
	"encoding/json"
	"io"
	"sync"

	"github.com/suapapa/kitt-eye/pub/internal/model"
	"github.com/suapapa/kitt-eye/pub/internal/state"
)

// Sink consumes publisher output. Implementations must be safe for concurrent
// use from the single publish worker; errors are logged, not fatal.
type Sink interface {
	PublishAgent(snap state.AgentSnapshot) error
	// PublishActive publishes the dominant state; agent is "" when idle.
	PublishActive(agent string, st model.State) error
	PublishStatus(online bool) error
}

// Stdout writes every sink call as a tagged JSON line. Used when
// mqtt.broker is empty so the hook→aggregate pipeline is verifiable
// without a broker.
type Stdout struct {
	mu  sync.Mutex
	enc *json.Encoder
}

var _ Sink = (*Stdout)(nil)

// NewStdout returns a Sink writing JSON lines to w.
func NewStdout(w io.Writer) *Stdout {
	return &Stdout{enc: json.NewEncoder(w)}
}

type agentRecord struct {
	Type     string              `json:"type"`
	Snapshot state.AgentSnapshot `json:"snapshot"`
}

type activeRecord struct {
	Type  string      `json:"type"`
	Agent string      `json:"agent"`
	State model.State `json:"state"`
}

type statusRecord struct {
	Type   string `json:"type"`
	Status string `json:"status"`
}

func (s *Stdout) PublishAgent(snap state.AgentSnapshot) error {
	s.mu.Lock()
	defer s.mu.Unlock()
	return s.enc.Encode(agentRecord{Type: "agent", Snapshot: snap})
}

func (s *Stdout) PublishActive(agent string, st model.State) error {
	s.mu.Lock()
	defer s.mu.Unlock()
	return s.enc.Encode(activeRecord{Type: "active", Agent: agent, State: st})
}

func (s *Stdout) PublishStatus(online bool) error {
	status := "offline"
	if online {
		status = "online"
	}
	s.mu.Lock()
	defer s.mu.Unlock()
	return s.enc.Encode(statusRecord{Type: "status", Status: status})
}
