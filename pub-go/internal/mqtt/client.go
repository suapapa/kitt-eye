// Package mqtt wraps paho.mqtt.golang with kitt-eye topic conventions:
// retained QoS-1 state topics, an LWT that marks the daemon offline, and
// reconnect-time republish so an ESP32 that boots late still sees current
// state immediately (§4.2).
package mqtt

import (
	"context"
	"encoding/json"
	"fmt"
	"log/slog"
	"net"
	"net/url"
	"strconv"
	"strings"
	"time"

	paho "github.com/eclipse/paho.mqtt.golang"

	"github.com/suapapa/kitt-eye/pub/internal/model"
	"github.com/suapapa/kitt-eye/pub/internal/state"
)

const (
	qosState    = 1
	publishWait = 5 * time.Second
)

// Options configures the client.
type Options struct {
	Host      string // bare host or scheme-qualified URL (tcp/tls/ws/wss)
	Port      int
	ClientID  string
	Username  string
	Password  string
	Prefix    string
	KeepAlive time.Duration
	Logger    *slog.Logger
	// OnConnect runs after every (re)connect; main uses it to re-notify the
	// publish worker so retained topics are refreshed.
	OnConnect func()
}

// Client is a kitt-eye-flavored MQTT publisher.
type Client struct {
	cli    paho.Client
	prefix string
	log    *slog.Logger
}

type haSensorConfig struct {
	Name                string `json:"name"`
	UniqueID            string `json:"unique_id"`
	StateTopic          string `json:"state_topic"`
	AvailabilityTopic   string `json:"availability_topic"`
	PayloadAvailable    string `json:"payload_available"`
	PayloadNotAvailable string `json:"payload_not_available"`
	Icon                string `json:"icon"`
}

type discoverySensor struct {
	id    string
	name  string
	topic string
	icon  string
}

// New builds the paho client. It does not connect; call Connect.
func New(o Options) (*Client, error) {
	if o.Logger == nil {
		o.Logger = slog.Default()
	}
	raw := o.Host
	if !strings.Contains(raw, "://") {
		raw = "tcp://" + raw
	}
	u, err := url.Parse(raw)
	if err != nil {
		return nil, fmt.Errorf("parse broker %q: %w", o.Host, err)
	}
	switch u.Scheme {
	case "tcp", "tls", "ssl", "ws", "wss":
	default:
		return nil, fmt.Errorf("unsupported broker scheme %q", u.Scheme)
	}
	if u.Port() == "" {
		port := o.Port
		if port == 0 {
			port = 1883
		}
		u.Host = net.JoinHostPort(u.Hostname(), strconv.Itoa(port))
	}

	co := paho.NewClientOptions().AddBroker(u.String())
	co.SetClientID(o.ClientID)
	if o.Username != "" {
		co.SetUsername(o.Username)
	}
	if o.Password != "" {
		co.SetPassword(o.Password)
	}
	co.SetKeepAlive(o.KeepAlive)
	co.SetCleanSession(true)
	co.SetAutoReconnect(true)
	co.SetMaxReconnectInterval(30 * time.Second)
	co.SetConnectTimeout(10 * time.Second)
	co.SetWriteTimeout(publishWait)
	co.SetOrderMatters(false)
	// Abnormal daemon death: the broker flips our status for us.
	co.SetWill(o.Prefix+"/system/status", "offline", qosState, true)
	co.SetConnectionLostHandler(func(_ paho.Client, err error) {
		o.Logger.Warn("mqtt: connection lost", "err", err)
	})
	if o.OnConnect != nil {
		co.SetOnConnectHandler(func(paho.Client) {
			o.Logger.Info("mqtt: connected", "broker", u.Host)
			o.OnConnect()
		})
	}
	return &Client{cli: paho.NewClient(co), prefix: o.Prefix, log: o.Logger}, nil
}

// Connect blocks until the initial connection succeeds, fails, or ctx ends.
func (c *Client) Connect(ctx context.Context) error {
	wait := 10 * time.Second
	if d, ok := ctx.Deadline(); ok {
		if left := time.Until(d); left < wait {
			wait = left
		}
	}
	tok := c.cli.Connect()
	if !tok.WaitTimeout(wait) {
		if ctx.Err() != nil {
			return ctx.Err()
		}
		return fmt.Errorf("connect timed out after %s", wait)
	}
	return tok.Error()
}

// PublishAgent writes the agent aggregate plus its session array.
func (c *Client) PublishAgent(snap state.AgentSnapshot) error {
	base := c.prefix + "/agents/" + snap.Agent
	if err := c.publish(base+"/state", string(snap.State), true); err != nil {
		return err
	}
	payload, err := json.Marshal(snap)
	if err != nil {
		return fmt.Errorf("encode sessions: %w", err)
	}
	return c.publish(base+"/sessions", string(payload), true)
}

// PublishActive writes the dominant state (plain string for the MCU) and,
// when known, the agent behind it.
func (c *Client) PublishActive(agent string, st model.State) error {
	if err := c.publish(c.prefix+"/active_state", string(st), true); err != nil {
		return err
	}
	if agent != "" {
		return c.publish(c.prefix+"/active_agent", agent, true)
	}
	return nil
}

// PublishStatus flips the online/offline availability topic.
func (c *Client) PublishStatus(online bool) error {
	status := "offline"
	if online {
		status = "online"
	}
	return c.publish(c.prefix+"/system/status", status, true)
}

// PublishDiscovery registers HA MQTT Discovery sensors (opt-in).
func (c *Client) PublishDiscovery(agents []string) error {
	sensors := []discoverySensor{
		{
			id:    "active_state",
			name:  "KITT Eye Active State",
			topic: c.prefix + "/active_state",
			icon:  "mdi:robot",
		},
	}
	for _, a := range agents {
		sensors = append(sensors, discoverySensor{
			id:    "agent_" + a,
			name:  "KITT Eye " + a,
			topic: c.prefix + "/agents/" + a + "/state",
			icon:  "mdi:console",
		})
	}
	for _, s := range sensors {
		cfg := haSensorConfig{
			Name:                s.name,
			UniqueID:            "kitt_eye_" + s.id,
			StateTopic:          s.topic,
			AvailabilityTopic:   c.prefix + "/system/status",
			PayloadAvailable:    "online",
			PayloadNotAvailable: "offline",
			Icon:                s.icon,
		}
		payload, err := json.Marshal(cfg)
		if err != nil {
			return err
		}
		topic := "homeassistant/sensor/kitt_eye_" + s.id + "/config"
		if err := c.publish(topic, string(payload), true); err != nil {
			return err
		}
	}
	return nil
}

// Disconnect quiesces in-flight publishes (grace is the wait budget), then
// closes the connection.
func (c *Client) Disconnect(grace time.Duration) {
	c.cli.Disconnect(uint(grace.Milliseconds()))
}

func (c *Client) publish(topic, payload string, retain bool) error {
	tok := c.cli.Publish(topic, qosState, retain, payload)
	if !tok.WaitTimeout(publishWait) {
		return fmt.Errorf("publish %q: timed out", topic)
	}
	return tok.Error()
}
