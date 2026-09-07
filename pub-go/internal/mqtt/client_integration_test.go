package mqtt

import (
	"context"
	"os"
	"testing"
	"time"

	"github.com/suapapa/kitt-eye/pub/internal/model"
	"github.com/suapapa/kitt-eye/pub/internal/state"
)

// TestLiveBrokerSmoke exercises real connect/publish against a broker set via
// KITTEYE_TEST_MQTT (e.g. "homeassistant.local:1883", with optional
// KITTEYE_TEST_MQTT_USER / _PASSWORD). Skipped by default so `go test ./...`
// stays hermetic.
func TestLiveBrokerSmoke(t *testing.T) {
	addr := os.Getenv("KITTEYE_TEST_MQTT")
	if addr == "" {
		t.Skip("set KITTEYE_TEST_MQTT=host:port to run against a real broker")
	}
	client, err := New(Options{
		Host:      addr,
		ClientID:  "kitteye-test",
		Username:  os.Getenv("KITTEYE_TEST_MQTT_USER"),
		Password:  os.Getenv("KITTEYE_TEST_MQTT_PASSWORD"),
		Prefix:    "kitt-eye-test",
		KeepAlive: 15 * time.Second,
	})
	if err != nil {
		t.Fatal(err)
	}
	ctx, cancel := context.WithTimeout(context.Background(), 10*time.Second)
	defer cancel()
	if err := client.Connect(ctx); err != nil {
		t.Fatalf("connect %s: %v", addr, err)
	}
	defer client.Disconnect(250 * time.Millisecond)

	snap := state.AgentSnapshot{
		Agent:     "smoke",
		State:     model.StateThinking,
		Sessions:  []state.Session{{SessionID: "s", State: model.StateThinking, UpdatedAt: time.Now()}},
		UpdatedAt: time.Now(),
	}
	for _, step := range []struct {
		name string
		fn   func() error
	}{
		{"status online", func() error { return client.PublishStatus(true) }},
		{"agent", func() error { return client.PublishAgent(snap) }},
		{"active", func() error { return client.PublishActive("smoke", model.StateThinking) }},
		{"discovery", func() error { return client.PublishDiscovery([]string{"smoke"}) }},
		{"status offline", func() error { return client.PublishStatus(false) }},
	} {
		if err := step.fn(); err != nil {
			t.Errorf("%s: %v", step.name, err)
		}
	}
}
