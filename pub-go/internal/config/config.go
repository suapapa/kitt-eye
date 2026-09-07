// Package config loads the publisher YAML (see ../../config.example.yaml),
// applying defaults and validating values before anything binds to them.
package config

import (
	"bytes"
	"errors"
	"fmt"
	"os"
	"path/filepath"
	"regexp"

	"gopkg.in/yaml.v3"

	"github.com/suapapa/kitt-eye/pub/internal/model"
)

// Config is the root document.
type Config struct {
	MQTT          MQTT    `yaml:"mqtt"`
	IPC           IPC     `yaml:"ipc"`
	State         State   `yaml:"state"`
	HomeAssistant HA      `yaml:"homeassistant"`
	Agents        []Agent `yaml:"agents"`
}

type MQTT struct {
	Broker           string `yaml:"broker"`
	Port             int    `yaml:"port"`
	ClientID         string `yaml:"client_id"`
	Username         string `yaml:"username"`
	Password         string `yaml:"password"`
	TopicPrefix      string `yaml:"topic_prefix"`
	KeepAliveSeconds int    `yaml:"keep_alive_seconds"`
}

type IPC struct {
	SocketPath string `yaml:"socket_path"`
}

// State holds TTLs in seconds (integers keep the YAML friendly).
type State struct {
	DoneTTLSeconds      int `yaml:"done_ttl_seconds"`
	ErrorTTLSeconds     int `yaml:"error_ttl_seconds"`
	StaleTTLSeconds     int `yaml:"stale_ttl_seconds"`
	SessionTTLSeconds   int `yaml:"session_ttl_seconds"`
	MaxSessionsPerAgent int `yaml:"max_sessions_per_agent"`
}

type HA struct {
	Discovery bool `yaml:"discovery"`
}

type Agent struct {
	Name        string `yaml:"name"`
	Enabled     bool   `yaml:"enabled"`
	Detail      string `yaml:"detail"` // enforced hook-side; recorded for observability
	MaxSessions int    `yaml:"max_sessions"`
}

// Load reads, decodes (strict), defaults and validates a config file.
func Load(path string) (*Config, error) {
	data, err := os.ReadFile(path)
	if err != nil {
		return nil, fmt.Errorf("read config: %w", err)
	}
	var cfg Config
	dec := yaml.NewDecoder(bytes.NewReader(data))
	dec.KnownFields(true) // reject typos instead of silently ignoring them
	if err := dec.Decode(&cfg); err != nil {
		return nil, fmt.Errorf("parse config %q: %w", path, err)
	}
	if env := os.Getenv("KITTEYE_MQTT_BROKER"); env != "" && cfg.MQTT.Broker == "" {
		cfg.MQTT.Broker = env
	}
	if env := os.Getenv("KITTEYE_MQTT_PASSWORD"); env != "" && cfg.MQTT.Password == "" {
		cfg.MQTT.Password = env
	}
	cfg.applyDefaults()
	if err := cfg.validate(); err != nil {
		return nil, fmt.Errorf("invalid config %q: %w", path, err)
	}
	return &cfg, nil
}

func (c *Config) applyDefaults() {
	if c.MQTT.Port == 0 {
		c.MQTT.Port = 1883
	}
	if c.MQTT.ClientID == "" {
		c.MQTT.ClientID = "kitt-eye-pub"
	}
	if c.MQTT.TopicPrefix == "" {
		c.MQTT.TopicPrefix = "kitt-eye"
	}
	if c.MQTT.KeepAliveSeconds == 0 {
		c.MQTT.KeepAliveSeconds = 60
	}
	if c.IPC.SocketPath == "" {
		c.IPC.SocketPath = "/tmp/kitt-eye.sock"
	}
	if c.State.DoneTTLSeconds == 0 {
		c.State.DoneTTLSeconds = 15
	}
	if c.State.ErrorTTLSeconds == 0 {
		c.State.ErrorTTLSeconds = 60
	}
	if c.State.StaleTTLSeconds == 0 {
		c.State.StaleTTLSeconds = 300
	}
	if c.State.SessionTTLSeconds == 0 {
		c.State.SessionTTLSeconds = 3600
	}
	if c.State.MaxSessionsPerAgent == 0 {
		c.State.MaxSessionsPerAgent = 8
	}
}

var topicPrefixRe = regexp.MustCompile(`^[A-Za-z0-9_-]+(/[A-Za-z0-9_-]+)*$`)

func (c *Config) validate() error {
	errs := []error{}
	if !filepath.IsAbs(c.IPC.SocketPath) {
		errs = append(errs, fmt.Errorf("ipc.socket_path must be absolute, got %q", c.IPC.SocketPath))
	}
	if !topicPrefixRe.MatchString(c.MQTT.TopicPrefix) {
		errs = append(errs, fmt.Errorf("mqtt.topic_prefix %q must be '/'-joined [A-Za-z0-9_-] segments", c.MQTT.TopicPrefix))
	}
	portOutOfRange := c.MQTT.Port < 1 || c.MQTT.Port > 65535
	if c.MQTT.Broker != "" && portOutOfRange {
		errs = append(errs, fmt.Errorf("mqtt.port %d out of range", c.MQTT.Port))
	}
	if c.MQTT.KeepAliveSeconds < 5 {
		errs = append(errs, fmt.Errorf("mqtt.keep_alive_seconds must be >= 5, got %d", c.MQTT.KeepAliveSeconds))
	}
	seen := map[string]bool{}
	for _, a := range c.Agents {
		if !model.ValidAgent(a.Name) {
			errs = append(errs, fmt.Errorf("agents[].name %q is not topic-safe [A-Za-z0-9][A-Za-z0-9_-]{0,31}", a.Name))
		}
		switch a.Detail {
		case "", "minimal", "full":
		default:
			errs = append(errs, fmt.Errorf("agents[%q].detail %q must be minimal or full", a.Name, a.Detail))
		}
		if seen[a.Name] {
			errs = append(errs, fmt.Errorf("duplicate agent %q", a.Name))
			continue
		}
		seen[a.Name] = true
	}
	return errors.Join(errs...)
}

// EnabledAgents maps configured agent names to their enabled flag. A nil map
// means no whitelist is configured and every agent is accepted.
func (c *Config) EnabledAgents() map[string]bool {
	if len(c.Agents) == 0 {
		return nil
	}
	out := make(map[string]bool, len(c.Agents))
	for _, a := range c.Agents {
		out[a.Name] = a.Enabled
	}
	return out
}

// PerAgentMax collects agents[].max_sessions overrides for the state store.
func (c *Config) PerAgentMax() map[string]int {
	out := map[string]int{}
	for _, a := range c.Agents {
		if a.MaxSessions > 0 {
			out[a.Name] = a.MaxSessions
		}
	}
	return out
}

// AgentNames lists configured agent names (for HA discovery).
func (c *Config) AgentNames() []string {
	out := []string{}
	for _, a := range c.Agents {
		if a.Enabled {
			out = append(out, a.Name)
		}
	}
	return out
}
