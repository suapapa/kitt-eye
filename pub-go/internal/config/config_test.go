package config

import (
	"os"
	"path/filepath"
	"strings"
	"testing"
)

func write(t *testing.T, body string) string {
	t.Helper()
	p := filepath.Join(t.TempDir(), "config.yaml")
	if err := os.WriteFile(p, []byte(body), 0o600); err != nil {
		t.Fatal(err)
	}
	return p
}

func TestLoadDefaults(t *testing.T) {
	path := write(t, "mqtt:\n  broker: \"ha.local\"\n")
	cfg, err := Load(path)
	if err != nil {
		t.Fatal(err)
	}
	mqttDefaultsOK := cfg.MQTT.Port == 1883 &&
		cfg.MQTT.ClientID == "kitt-eye-pub" &&
		cfg.MQTT.TopicPrefix == "kitt-eye" &&
		cfg.MQTT.KeepAliveSeconds == 60
	if !mqttDefaultsOK {
		t.Errorf("mqtt defaults wrong: %+v", cfg.MQTT)
	}
	if cfg.IPC.SocketPath != "/tmp/kitt-eye.sock" {
		t.Errorf("socket default = %q", cfg.IPC.SocketPath)
	}
	stateDefaultsOK := cfg.State.DoneTTLSeconds == 15 &&
		cfg.State.ErrorTTLSeconds == 60 &&
		cfg.State.StaleTTLSeconds == 300 &&
		cfg.State.SessionTTLSeconds == 3600 &&
		cfg.State.MaxSessionsPerAgent == 8
	if !stateDefaultsOK {
		t.Errorf("state defaults wrong: %+v", cfg.State)
	}
}

func TestLoadFullExample(t *testing.T) {
	path := write(t, `
mqtt:
  broker: "homeassistant.local"
  port: 1884
  client_id: "pub1"
  username: "u"
  password: "p"
  topic_prefix: "kitt-eye/v2"
  keep_alive_seconds: 30
ipc:
  socket_path: "/run/user/501/kitt-eye.sock"
state:
  done_ttl_seconds: 5
  error_ttl_seconds: 10
  stale_ttl_seconds: 120
  session_ttl_seconds: 600
  max_sessions_per_agent: 4
homeassistant:
  discovery: true
agents:
  - name: "claude"
    enabled: true
    detail: "minimal"
    max_sessions: 2
  - name: "codex"
    enabled: false
`)
	cfg, err := Load(path)
	if err != nil {
		t.Fatal(err)
	}
	if cfg.MQTT.TopicPrefix != "kitt-eye/v2" || !cfg.HomeAssistant.Discovery {
		t.Errorf("explicit values lost: %+v", cfg)
	}
	if got := cfg.EnabledAgents(); !got["claude"] || got["codex"] {
		t.Errorf("EnabledAgents = %v", got)
	}
	if got := cfg.PerAgentMax(); got["claude"] != 2 {
		t.Errorf("PerAgentMax = %v", got)
	}
}

func TestLoadRejectsUnknownKeys(t *testing.T) {
	path := write(t, "mqtt:\n  broker: \"x\"\n  cleint_id: \"typo\"\n")
	if _, err := Load(path); err == nil || !strings.Contains(err.Error(), "cleint_id") {
		t.Fatalf("expected strict decode error naming the typo, got %v", err)
	}
}

func TestValidateErrors(t *testing.T) {
	cases := map[string]string{
		"relative socket":  "ipc:\n  socket_path: \"rel.sock\"\n",
		"bad topic prefix": "mqtt:\n  topic_prefix: \"a/#/b\"\n",
		"dup agents":       "agents:\n  - name: claude\n    enabled: true\n  - name: claude\n    enabled: true\n",
		"bad detail":       "agents:\n  - name: claude\n    enabled: true\n    detail: verbose\n",
		"tiny keepalive":   "mqtt:\n  keep_alive_seconds: 1\n",
	}
	for name, body := range cases {
		if _, err := Load(write(t, body)); err == nil {
			t.Errorf("%s: expected validation error", name)
		}
	}
}

func TestEnvFallbacks(t *testing.T) {
	t.Setenv("KITTEYE_MQTT_BROKER", "env-broker")
	t.Setenv("KITTEYE_MQTT_PASSWORD", "env-pw")
	cfg, err := Load(write(t, "ipc:\n  socket_path: \"/tmp/k.sock\"\n"))
	if err != nil {
		t.Fatal(err)
	}
	if cfg.MQTT.Broker != "env-broker" || cfg.MQTT.Password != "env-pw" {
		t.Errorf("env fallback not applied: %+v", cfg.MQTT)
	}
}

func TestEmptyAgentsMeansNoWhitelist(t *testing.T) {
	cfg, err := Load(write(t, "mqtt:\n  broker: \"x\"\n"))
	if err != nil {
		t.Fatal(err)
	}
	if cfg.EnabledAgents() != nil {
		t.Error("empty agents list must mean accept-all")
	}
}
