// Command kitteye_sub_example is a small MQTT subscriber for exercising
// kitt-eye retained topics (same role as `mosquitto_sub -v -t 'kitt-eye/#'`).
// It shares config.yaml with the publisher via internal/config.
package main

import (
	"context"
	"flag"
	"fmt"
	"log/slog"
	"net"
	"net/url"
	"os"
	"os/signal"
	"strconv"
	"strings"
	"syscall"
	"time"

	paho "github.com/eclipse/paho.mqtt.golang"

	"github.com/suapapa/kitt-eye/pub/internal/config"
)

func main() {
	if err := run(); err != nil {
		fmt.Fprintln(os.Stderr, "kitteye_sub_example:", err)
		os.Exit(1)
	}
}

func run() error {
	configPath := flag.String("config", defaultConfigPath(), "path to config YAML")
	topic := flag.String("topic", "", "subscribe filter (default <mqtt.topic_prefix>/#)")
	flag.Parse()

	cfg, err := config.Load(*configPath)
	if err != nil {
		return err
	}
	if cfg.MQTT.Broker == "" {
		return fmt.Errorf("mqtt.broker is empty: set it in %q or KITTEYE_MQTT_BROKER", *configPath)
	}

	filter := *topic
	if filter == "" {
		filter = strings.TrimRight(cfg.MQTT.TopicPrefix, "/") + "/#"
	}

	// Never reuse the publisher client_id — Mosquitto kicks the older session.
	clientID := cfg.MQTT.ClientID + "-sub"

	log := slog.New(slog.NewTextHandler(os.Stderr, nil))
	cli, err := connect(cfg, clientID, filter, log)
	if err != nil {
		return err
	}
	defer cli.Disconnect(250)

	ctx, stop := signal.NotifyContext(context.Background(), syscall.SIGINT, syscall.SIGTERM)
	defer stop()

	log.Info("subscribed",
		"broker", net.JoinHostPort(cfg.MQTT.Broker, strconv.Itoa(cfg.MQTT.Port)),
		"topic", filter,
		"client_id", clientID,
		"config", *configPath,
	)
	<-ctx.Done()
	log.Info("shutting down")
	return nil
}

func connect(cfg *config.Config, clientID, filter string, log *slog.Logger) (paho.Client, error) {
	raw := cfg.MQTT.Broker
	if !strings.Contains(raw, "://") {
		raw = "tcp://" + raw
	}
	u, err := url.Parse(raw)
	if err != nil {
		return nil, fmt.Errorf("parse broker %q: %w", cfg.MQTT.Broker, err)
	}
	if u.Port() == "" {
		port := cfg.MQTT.Port
		if port == 0 {
			port = 1883
		}
		u.Host = net.JoinHostPort(u.Hostname(), strconv.Itoa(port))
	}

	subscribe := func(c paho.Client) {
		tok := c.Subscribe(filter, 1, onMessage)
		if !tok.WaitTimeout(5 * time.Second) {
			log.Error("subscribe timed out", "topic", filter)
			return
		}
		if err := tok.Error(); err != nil {
			log.Error("subscribe failed", "topic", filter, "err", err)
			return
		}
		log.Info("mqtt: subscribed", "topic", filter)
	}

	opts := paho.NewClientOptions().AddBroker(u.String())
	opts.SetClientID(clientID)
	if cfg.MQTT.Username != "" {
		opts.SetUsername(cfg.MQTT.Username)
	}
	if cfg.MQTT.Password != "" {
		opts.SetPassword(cfg.MQTT.Password)
	}
	opts.SetKeepAlive(time.Duration(cfg.MQTT.KeepAliveSeconds) * time.Second)
	opts.SetCleanSession(true)
	opts.SetAutoReconnect(true)
	opts.SetMaxReconnectInterval(30 * time.Second)
	opts.SetConnectTimeout(10 * time.Second)
	opts.SetConnectionLostHandler(func(_ paho.Client, err error) {
		log.Warn("mqtt: connection lost", "err", err)
	})
	opts.SetOnConnectHandler(func(c paho.Client) {
		log.Info("mqtt: connected", "broker", u.Host)
		subscribe(c)
	})

	cli := paho.NewClient(opts)
	tok := cli.Connect()
	if !tok.WaitTimeout(10 * time.Second) {
		return nil, fmt.Errorf("connect timed out")
	}
	if err := tok.Error(); err != nil {
		return nil, fmt.Errorf("connect %q: %w", u.Host, err)
	}
	return cli, nil
}

func onMessage(_ paho.Client, msg paho.Message) {
	ts := time.Now().Format(time.RFC3339)
	var retain string
	if msg.Retained() {
		retain = " [retained]"
	}
	fmt.Printf(
		"%s  %s%s  %s\n",
		ts,
		msg.Topic(),
		retain,
		string(msg.Payload()),
	)
}

func defaultConfigPath() string {
	if v := os.Getenv("KITTEYE_CONFIG"); v != "" {
		return v
	}
	return "config.yaml"
}
