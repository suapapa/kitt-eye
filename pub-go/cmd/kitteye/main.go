// Command kitteye runs the kitt-eye publisher daemon: hook events arrive over
// a Unix socket, are tracked per session with TTLs, and the aggregated state
// is published to MQTT for the ESP32-C3 device and Home Assistant.
package main

import (
	"context"
	"flag"
	"fmt"
	"log/slog"
	"net"
	"os"
	"os/signal"
	"strconv"
	"syscall"
	"time"

	"github.com/suapapa/kitt-eye/pub/internal/config"
	"github.com/suapapa/kitt-eye/pub/internal/ipc"
	"github.com/suapapa/kitt-eye/pub/internal/model"
	"github.com/suapapa/kitt-eye/pub/internal/mqtt"
	"github.com/suapapa/kitt-eye/pub/internal/publish"
	"github.com/suapapa/kitt-eye/pub/internal/state"
)

func main() {
	if err := run(); err != nil {
		fmt.Fprintln(os.Stderr, "kitteye:", err)
		os.Exit(1)
	}
}

func run() error {
	configPath := flag.String("config", defaultConfigPath(), "path to config YAML")
	flag.Parse()

	cfg, err := config.Load(*configPath)
	if err != nil {
		return err
	}
	log := slog.New(slog.NewTextHandler(os.Stderr, nil))

	ctx, stop := signal.NotifyContext(context.Background(), syscall.SIGINT, syscall.SIGTERM)
	defer stop()

	store := state.New(state.Config{
		DoneTTL:             time.Duration(cfg.State.DoneTTLSeconds) * time.Second,
		ErrorTTL:            time.Duration(cfg.State.ErrorTTLSeconds) * time.Second,
		StaleTTL:            time.Duration(cfg.State.StaleTTLSeconds) * time.Second,
		SessionTTL:          time.Duration(cfg.State.SessionTTLSeconds) * time.Second,
		MaxSessionsPerAgent: cfg.State.MaxSessionsPerAgent,
		PerAgentMax:         cfg.PerAgentMax(),
	})

	// One-slot change signal: bursts of hook events coalesce into a single
	// snapshot publish, and a full channel never blocks Apply.
	notify := make(chan struct{}, 1)
	signalChange := func() {
		select {
		case notify <- struct{}{}:
		default:
		}
	}
	store.Subscribe(signalChange)

	sinks, closer, err := buildSinks(cfg, signalChange, log)
	if err != nil {
		return err
	}
	defer closer()

	publishAll := func() {
		snaps := store.Snapshot()
		agent, active := store.ActiveState()
		for _, sink := range sinks {
			for _, snap := range snaps {
				if err := sink.PublishAgent(snap); err != nil {
					log.Warn("publish agent state failed", "agent", snap.Agent, "err", err)
				}
			}
			if err := sink.PublishActive(agent, active); err != nil {
				log.Warn("publish active state failed", "err", err)
			}
		}
	}

	// Publish worker: decouples (possibly slow) MQTT round-trips from the
	// hot IPC path.
	workerDone := make(chan struct{})
	go func() {
		defer close(workerDone)
		for {
			select {
			case <-ctx.Done():
				return
			case <-notify:
				publishAll()
			}
		}
	}()

	srv, err := ipc.New(cfg.IPC.SocketPath, makeHandler(cfg, store, log), log)
	if err != nil {
		return err
	}
	defer srv.Close()

	go srv.Serve(ctx)
	go store.RunJanitor(ctx, time.Second)

	log.Info("kitteye publisher started",
		"socket", srv.Path(),
		"broker", brokerLabel(cfg),
		"topic_prefix", cfg.MQTT.TopicPrefix,
	)

	<-ctx.Done()
	log.Info("shutting down")

	// Tell subscribers we're offline before tearing the connection down,
	// then let the worker finish its last publish.
	for _, sink := range sinks {
		if err := sink.PublishStatus(false); err != nil {
			log.Warn("publish offline status failed", "err", err)
		}
	}
	stop() // unblocks Serve and the janitor
	<-workerDone
	return nil
}

// buildSinks wires the MQTT sink (or stdout dry-run) and returns a closer.
func buildSinks(cfg *config.Config, signalChange func(), log *slog.Logger) ([]publish.Sink, func(), error) {
	if cfg.MQTT.Broker == "" {
		log.Warn("mqtt.broker is empty: running in dry-run mode, state goes to stdout as JSON lines")
		return []publish.Sink{publish.NewStdout(os.Stdout)}, func() {}, nil
	}

	cli, err := mqtt.New(mqtt.Options{
		Host:      cfg.MQTT.Broker,
		Port:      cfg.MQTT.Port,
		ClientID:  cfg.MQTT.ClientID,
		Username:  cfg.MQTT.Username,
		Password:  cfg.MQTT.Password,
		Prefix:    cfg.MQTT.TopicPrefix,
		KeepAlive: time.Duration(cfg.MQTT.KeepAliveSeconds) * time.Second,
		Logger:    log,
		OnConnect: signalChange, // refresh retained topics after (re)connect
	})
	if err != nil {
		return nil, nil, err
	}

	connectCtx, cancel := context.WithTimeout(context.Background(), 15*time.Second)
	defer cancel()
	if err := cli.Connect(connectCtx); err != nil {
		return nil, nil, fmt.Errorf("connect %q: %w", cfg.MQTT.Broker, err)
	}
	if err := cli.PublishStatus(true); err != nil {
		log.Warn("publish online status failed", "err", err)
	}
	if cfg.HomeAssistant.Discovery {
		if err := cli.PublishDiscovery(cfg.AgentNames()); err != nil {
			log.Warn("publish HA discovery failed", "err", err)
		}
	}
	closer := func() { cli.Disconnect(250 * time.Millisecond) }
	return []publish.Sink{cli}, closer, nil
}

// makeHandler validates incoming datagrams and feeds the state store.
// Rejected events are logged at debug level: hooks are third-party and must
// never see errors or backpressure from us.
func makeHandler(cfg *config.Config, store *state.Store, log *slog.Logger) ipc.Handler {
	allowed := cfg.EnabledAgents()
	return func(_ context.Context, line []byte) {
		ev, err := model.ParseEvent(line)
		if err != nil {
			log.Debug("dropped invalid event", "err", err, "line", string(line))
			return
		}
		if allowed != nil && !allowed[ev.Agent] {
			log.Debug("dropped event from disabled agent", "agent", ev.Agent)
			return
		}
		store.Apply(ev)
	}
}

func brokerLabel(cfg *config.Config) string {
	if cfg.MQTT.Broker == "" {
		return "dry-run(stdout)"
	}
	return net.JoinHostPort(cfg.MQTT.Broker, strconv.Itoa(cfg.MQTT.Port))
}

// defaultConfigPath resolves the config file: $KITTEYE_CONFIG, else ./config.yaml.
func defaultConfigPath() string {
	if v := os.Getenv("KITTEYE_CONFIG"); v != "" {
		return v
	}
	return "config.yaml"
}
