// Command kitteye_mock simulates AI agent lifecycle states without requiring
// real AI agent hooks (Claude, Codex, Cursor, Antigravity). It loads the MQTT
// settings directly from config.yaml and publishes mock states to MQTT topics
// (e.g. kitt-eye/active_state) for demoing ESP32 LED animations and Home Assistant.
package main

import (
	"bufio"
	"context"
	"encoding/json"
	"flag"
	"fmt"
	"io"
	"net"
	"os"
	"os/signal"
	"path/filepath"
	"strings"
	"syscall"
	"time"

	"github.com/suapapa/kitt-eye/pub/internal/config"
	"github.com/suapapa/kitt-eye/pub/internal/model"
	"github.com/suapapa/kitt-eye/pub/internal/mqtt"
	"github.com/suapapa/kitt-eye/pub/internal/publish"
	"github.com/suapapa/kitt-eye/pub/internal/state"
)

// StateMeta defines display and animation descriptions for each state.
type StateMeta struct {
	State       model.State
	ColorCode   string // ANSI color for terminal display
	ClassicDesc string // Motion in Classic theme (all red)
	ColorDesc   string // Color & Motion in Colorful theme
	DefaultMsg  string // Example detail message
}

var stateMetas = []StateMeta{
	{
		State:       model.StateIdle,
		ColorCode:   "\033[31m", // Red
		ClassicDesc: "Classic Red (Slow Scanner)",
		ColorDesc:   "Classic Red (Slow Scanner)",
		DefaultMsg:  "Standby / waiting for user prompt",
	},
	{
		State:       model.StateThinking,
		ColorCode:   "\033[35m", // Magenta / Mystic Purple
		ClassicDesc: "Classic Red (Center-Out)",
		ColorDesc:   "Mystic Purple (Center-Out)",
		DefaultMsg:  "Analyzing prompt & repository architecture",
	},
	{
		State:       model.StateGenerating,
		ColorCode:   "\033[32m", // Green / Mint Emerald
		ClassicDesc: "Classic Red (Fill-Sweep)",
		ColorDesc:   "Mint Emerald (Fill-Sweep)",
		DefaultMsg:  "Streaming response & drafting changes",
	},
	{
		State:       model.StateExecutingTool,
		ColorCode:   "\033[33m", // Yellow / Vivid Amber
		ClassicDesc: "Classic Red (Fast Scanner)",
		ColorDesc:   "Vivid Amber (Fast Scanner)",
		DefaultMsg:  "Executing tool: make test",
	},
	{
		State:       model.StateWaitingInput,
		ColorCode:   "\033[93m", // Bright Yellow
		ClassicDesc: "Classic Red (Blink)",
		ColorDesc:   "Warning Yellow (Blink)",
		DefaultMsg:  "Waiting for user confirmation / choice",
	},
	{
		State:       model.StateDone,
		ColorCode:   "\033[92m", // Bright Green / Spring Lime
		ClassicDesc: "Classic Red (Flash -> Solid)",
		ColorDesc:   "Spring Lime (Flash -> Solid)",
		DefaultMsg:  "Task completed successfully",
	},
	{
		State:       model.StateError,
		ColorCode:   "\033[91m", // Bright Red / Crimson
		ClassicDesc: "Classic Red (Comet)",
		ColorDesc:   "Crimson Red (Comet)",
		DefaultMsg:  "Error: tool execution failed with exit code 1",
	},
}

func metaFor(s model.State) StateMeta {
	for _, m := range stateMetas {
		if m.State == s {
			return m
		}
	}
	return StateMeta{
		State:       s,
		ColorCode:   "\033[0m",
		ClassicDesc: string(s),
		ColorDesc:   string(s),
		DefaultMsg:  "State: " + string(s),
	}
}

func main() {
	if err := run(); err != nil {
		fmt.Fprintf(os.Stderr, "kitteye_mock: %v\n", err)
		os.Exit(1)
	}
}

func run() error {
	var (
		configVal     string
		modeVal       string
		stateValStr   string
		brokerVal     string
		portVal       int
		retriesVal    int
		retryDelayVal time.Duration
		intervalVal   time.Duration
		agentVal      string
		detailVal     string
		dryRunVal     bool
		ipcVal        string
		onceVal       bool
	)

	flag.StringVar(&configVal, "config", "", "path to config YAML (default: auto-discover config.yaml)")
	flag.StringVar(&configVal, "c", "", "alias for -config")

	flag.StringVar(&modeVal, "mode", "", "demo mode: loop | workflow | interactive | single (default: loop)")
	flag.StringVar(&modeVal, "m", "", "alias for -mode")

	flag.StringVar(&stateValStr, "state", "", "target state for single mode (idle, thinking, generating, executing_tool, waiting_input, done, error)")
	flag.StringVar(&stateValStr, "s", "", "alias for -state")

	flag.StringVar(&brokerVal, "broker", "", "override MQTT broker host (default: from config.yaml)")
	flag.StringVar(&brokerVal, "b", "", "alias for -broker")

	flag.IntVar(&portVal, "port", 0, "override MQTT broker port (default: from config.yaml)")
	flag.IntVar(&portVal, "p", 0, "alias for -port")

	flag.IntVar(&retriesVal, "retries", 5, "number of connection attempts before failing")
	flag.DurationVar(&retryDelayVal, "retry-delay", 1500*time.Millisecond, "delay between connection retries")

	flag.DurationVar(&intervalVal, "interval", 3*time.Second, "seconds per state in loop mode")
	flag.DurationVar(&intervalVal, "i", 3*time.Second, "alias for -interval")

	flag.StringVar(&agentVal, "agent", "mock-agent", "agent name to publish")
	flag.StringVar(&agentVal, "a", "mock-agent", "alias for -agent")

	flag.StringVar(&detailVal, "detail", "", "custom detail message")
	flag.StringVar(&detailVal, "d", "", "alias for -detail")

	flag.BoolVar(&dryRunVal, "dry-run", false, "dry-run mode: print to stdout without connecting to MQTT")
	flag.StringVar(&ipcVal, "ipc", "", "also send datagrams to UDS socket path (e.g. /tmp/kitt-eye.sock)")
	flag.BoolVar(&onceVal, "once", false, "run workflow/loop once instead of indefinitely")
	flag.Parse()

	configFlag := &configVal
	modeFlag := &modeVal
	stateFlag := &stateValStr
	brokerFlag := &brokerVal
	portFlag := &portVal
	retriesFlag := &retriesVal
	retryDelayFlag := &retryDelayVal
	intervalFlag := &intervalVal
	agentFlag := &agentVal
	detailFlag := &detailVal
	dryRunFlag := &dryRunVal
	ipcFlag := &ipcVal
	onceFlag := &onceVal

	// Positional arguments can specify target state or mode as convenience shorthand:
	// e.g. "kitteye_mock thinking" or "kitteye_mock interactive"
	mode := strings.ToLower(strings.TrimSpace(*modeFlag))
	stateVal := model.State(strings.ToLower(strings.TrimSpace(*stateFlag)))

	for _, arg := range flag.Args() {
		argClean := strings.ToLower(strings.TrimSpace(arg))
		switch argClean {
		case "loop", "workflow", "interactive":
			if mode == "" {
				mode = argClean
			}
		case "tool":
			argClean = string(model.StateExecutingTool)
			fallthrough
		case "wait":
			if argClean == "wait" {
				argClean = string(model.StateWaitingInput)
			}
			fallthrough
		case string(model.StateIdle),
			string(model.StateThinking),
			string(model.StateGenerating),
			string(model.StateExecutingTool),
			string(model.StateWaitingInput),
			string(model.StateDone),
			string(model.StateError):
			stateVal = model.State(argClean)
			if mode == "" {
				mode = "single"
			}
		}
	}

	if mode == "" {
		if stateVal != "" {
			mode = "single"
		} else {
			mode = "loop"
		}
	}

	configPath := findConfigFile(*configFlag)
	cfg, err := config.Load(configPath)
	if err != nil {
		return fmt.Errorf("load config %q: %w", configPath, err)
	}

	ctx, stop := signal.NotifyContext(context.Background(), syscall.SIGINT, syscall.SIGTERM)
	defer stop()

	broker := cfg.MQTT.Broker
	if *brokerFlag != "" {
		broker = *brokerFlag
	}
	port := cfg.MQTT.Port
	if *portFlag > 0 {
		port = *portFlag
	}

	isDryRun := *dryRunFlag || broker == ""
	var sink publish.Sink
	var mqttCloser func()

	if isDryRun {
		fmt.Println("[Notice] Dry-run mode enabled (no MQTT broker connection). States printed to terminal.")
		sink = publish.NewStdout(os.Stdout)
		mqttCloser = func() {}
	} else {
		// Mosquitto kicks previous connections sharing the same client_id; use distinct mock ID.
		clientID := cfg.MQTT.ClientID + "-mock"
		var cli *mqtt.Client
		var connErr error

		fmt.Printf("Connecting to MQTT broker at %s:%d (topic prefix: %s)...\n",
			broker, port, cfg.MQTT.TopicPrefix)

		maxAttempts := *retriesFlag
		if maxAttempts < 1 {
			maxAttempts = 1
		}

		for attempt := 1; attempt <= maxAttempts; attempt++ {
			if ctx.Err() != nil {
				return ctx.Err()
			}
			cli, err = mqtt.New(mqtt.Options{
				Host:      broker,
				Port:      port,
				ClientID:  clientID,
				Username:  cfg.MQTT.Username,
				Password:  cfg.MQTT.Password,
				Prefix:    cfg.MQTT.TopicPrefix,
				KeepAlive: time.Duration(cfg.MQTT.KeepAliveSeconds) * time.Second,
			})
			if err != nil {
				return fmt.Errorf("init MQTT client: %w", err)
			}

			connCtx, cancel := context.WithTimeout(ctx, 3*time.Second)
			connErr = cli.Connect(connCtx)
			cancel()

			if connErr == nil {
				break
			}

			if attempt < maxAttempts {
				fmt.Printf("  Connection attempt %d/%d failed: %v. Retrying in %v...\n",
					attempt, maxAttempts, connErr, *retryDelayFlag)
				select {
				case <-ctx.Done():
					return ctx.Err()
				case <-time.After(*retryDelayFlag):
				}
			}
		}

		if connErr != nil {
			fmt.Fprintf(os.Stderr, "\n[MQTT Connection Error]\n")
			fmt.Fprintf(os.Stderr, "  Could not reach broker %s:%d after %d attempts.\n", broker, port, maxAttempts)
			fmt.Fprintf(os.Stderr, "  Underlying error: %v\n\n", connErr)
			fmt.Fprintf(os.Stderr, "Troubleshooting Suggestions:\n")
			fmt.Fprintf(os.Stderr, "  1. Check Wi-Fi / Local Network: Ensure this device is connected to the same LAN as %s.\n", broker)
			fmt.Fprintf(os.Stderr, "  2. Check VPN: Active VPNs (Tailscale, Wireguard, etc.) may route 172.16.x.x through a tunnel.\n")
			fmt.Fprintf(os.Stderr, "  3. Override Broker: If the broker IP changed or uses a hostname, use:\n")
			fmt.Fprintf(os.Stderr, "       ./script/mock_state.sh -broker <ip_or_host>\n")
			fmt.Fprintf(os.Stderr, "  4. Terminal Dry-run: To demo states and patterns in terminal without MQTT:\n")
			fmt.Fprintf(os.Stderr, "       ./script/mock_state.sh --dry-run\n\n")
			return connErr
		}

		mqttCloser = func() {
			_ = cli.PublishActive(*agentFlag, model.StateIdle)
			cli.Disconnect(250 * time.Millisecond)
		}
		defer mqttCloser()

		if err := cli.PublishStatus(true); err != nil {
			fmt.Fprintf(os.Stderr, "Warning: failed to publish online status: %v\n", err)
		}
		sink = cli
		fmt.Printf("Connected successfully! Ready to publish mock states.\n\n")
	}

	publisher := &MockPublisher{
		sink:        sink,
		topicPrefix: cfg.MQTT.TopicPrefix,
		ipcSocket:   *ipcFlag,
		agent:       *agentFlag,
		customMsg:   *detailFlag,
	}

	switch mode {
	case "single":
		if !stateVal.Valid() {
			return fmt.Errorf("invalid or missing state %q (valid: idle, thinking, generating, executing_tool, waiting_input, done, error)", stateVal)
		}
		meta := metaFor(stateVal)
		msg := *detailFlag
		if msg == "" {
			msg = meta.DefaultMsg
		}
		if err := publisher.Publish(stateVal, msg); err != nil {
			return err
		}
		// Brief pause so retained MQTT packet finishes before process termination
		time.Sleep(200 * time.Millisecond)
		return nil

	case "interactive":
		return runInteractive(ctx, publisher)

	case "workflow":
		return runWorkflow(ctx, publisher, *intervalFlag, *onceFlag)

	case "loop":
		fallthrough
	default:
		return runLoop(ctx, publisher, *intervalFlag, *onceFlag)
	}
}

// MockPublisher wraps publishing to sink, optional IPC, and formatted terminal output.
type MockPublisher struct {
	sink        publish.Sink
	topicPrefix string
	ipcSocket   string
	agent       string
	customMsg   string
}

func (p *MockPublisher) Publish(st model.State, detail string) error {
	if detail == "" {
		detail = p.customMsg
	}
	meta := metaFor(st)
	if detail == "" {
		detail = meta.DefaultMsg
	}

	// 1. Publish to Sink (MQTT or Stdout)
	if err := p.sink.PublishActive(p.agent, st); err != nil {
		return fmt.Errorf("publish active state %q: %w", st, err)
	}

	snap := state.AgentSnapshot{
		Agent: p.agent,
		State: st,
		Sessions: []state.Session{
			{
				SessionID: "mock-session-1",
				State:     st,
				Detail:    detail,
				UpdatedAt: time.Now(),
			},
		},
		UpdatedAt: time.Now(),
	}
	if err := p.sink.PublishAgent(snap); err != nil {
		fmt.Fprintf(os.Stderr, "Warning: publish agent snapshot failed: %v\n", err)
	}

	// 2. Optional IPC datagram
	if p.ipcSocket != "" {
		p.sendIPC(st, detail)
	}

	// 3. Pretty Terminal Output
	ts := time.Now().Format("15:04:05")
	tag := fmt.Sprintf("%-14s", "["+strings.ToUpper(string(st))+"]")
	fmt.Printf("%s  %s%s\033[0m  %-30s  | %s\n",
		ts,
		meta.ColorCode,
		tag,
		meta.ColorDesc,
		detail,
	)

	return nil
}

func (p *MockPublisher) sendIPC(st model.State, detail string) {
	conn, err := net.Dial("unix", p.ipcSocket)
	if err != nil {
		return
	}
	defer conn.Close()

	ev := model.Event{
		Agent:     p.agent,
		State:     st,
		Detail:    detail,
		SessionID: "mock-session-1",
		Timestamp: time.Now().UTC().Format(time.RFC3339),
	}
	data, err := json.Marshal(ev)
	if err == nil {
		_, _ = conn.Write(append(data, '\n'))
	}
}

func runLoop(ctx context.Context, pub *MockPublisher, interval time.Duration, once bool) error {
	printBanner("Mock Demo: All States Loop", interval)

	states := []model.State{
		model.StateIdle,
		model.StateThinking,
		model.StateGenerating,
		model.StateExecutingTool,
		model.StateWaitingInput,
		model.StateDone,
		model.StateError,
	}

	idx := 0
	ticker := time.NewTicker(interval)
	defer ticker.Stop()

	// Initial state immediately
	if err := pub.Publish(states[idx], ""); err != nil {
		return err
	}

	for {
		select {
		case <-ctx.Done():
			fmt.Println("\nStopping demo, resetting to IDLE state...")
			_ = pub.Publish(model.StateIdle, "Demo stopped")
			return nil

		case <-ticker.C:
			idx = (idx + 1) % len(states)
			if once && idx == 0 {
				fmt.Println("\nFinished full state loop.")
				_ = pub.Publish(model.StateIdle, "Demo finished")
				return nil
			}
			if err := pub.Publish(states[idx], ""); err != nil {
				return err
			}
		}
	}
}

type workflowStep struct {
	state  model.State
	detail string
	dur    time.Duration
}

func runWorkflow(ctx context.Context, pub *MockPublisher, baseInterval time.Duration, once bool) error {
	printBanner("Mock Demo: Realistic AI Agent Workflow", baseInterval)

	steps := []workflowStep{
		{state: model.StateIdle, detail: "Standby: waiting for developer prompt", dur: 2 * time.Second},
		{state: model.StateThinking, detail: "Claude: Reading repository files & planning diff", dur: 3 * time.Second},
		{state: model.StateGenerating, detail: "Claude: Generating refactored code", dur: 4 * time.Second},
		{state: model.StateExecutingTool, detail: "Claude: Running `make test` via bash tool", dur: 4 * time.Second},
		{state: model.StateWaitingInput, detail: "Claude: Asking developer confirmation for git commit", dur: 4 * time.Second},
		{state: model.StateThinking, detail: "Claude: Developer approved, generating commit message", dur: 2 * time.Second},
		{state: model.StateExecutingTool, detail: "Claude: Executing `git commit -m 'feat: ...'`", dur: 3 * time.Second},
		{state: model.StateDone, detail: "Claude: Task completed successfully!", dur: 4 * time.Second},
		{state: model.StateIdle, detail: "Standby: ready for next task", dur: 2 * time.Second},
	}

	for {
		for i, s := range steps {
			dur := s.dur
			if baseInterval != 3*time.Second {
				// Scale duration if user specified custom -interval
				dur = time.Duration(float64(s.dur) * (float64(baseInterval) / float64(3*time.Second)))
			}

			if err := pub.Publish(s.state, s.detail); err != nil {
				return err
			}

			select {
			case <-ctx.Done():
				fmt.Println("\nStopping workflow, resetting to IDLE state...")
				_ = pub.Publish(model.StateIdle, "Workflow stopped")
				return nil
			case <-time.After(dur):
			}

			if once && i == len(steps)-1 {
				fmt.Println("\nFinished workflow scenario.")
				return nil
			}
		}
	}
}

func runInteractive(ctx context.Context, pub *MockPublisher) error {
	printBanner("Mock Demo: Interactive Mode", 0)
	printInteractiveHelp()

	reader := bufio.NewReader(os.Stdin)
	inputCh := make(chan string)

	go func() {
		for {
			line, err := reader.ReadString('\n')
			if err != nil {
				if err == io.EOF {
					close(inputCh)
					return
				}
				continue
			}
			inputCh <- strings.TrimSpace(line)
		}
	}()

	// Start in idle
	_ = pub.Publish(model.StateIdle, "Interactive mode started")

	for {
		select {
		case <-ctx.Done():
			fmt.Println("\nExiting interactive mode...")
			_ = pub.Publish(model.StateIdle, "Interactive mode exited")
			return nil

		case input, ok := <-inputCh:
			if !ok {
				return nil
			}
			inputLower := strings.ToLower(input)
			switch inputLower {
			case "q", "quit", "exit":
				fmt.Println("Exiting interactive mode, resetting to IDLE...")
				_ = pub.Publish(model.StateIdle, "Interactive mode exited")
				return nil
			case "h", "help", "?":
				printInteractiveHelp()
			case "1", "idle":
				_ = pub.Publish(model.StateIdle, "User selected Idle")
			case "2", "thinking", "think":
				_ = pub.Publish(model.StateThinking, "User selected Thinking")
			case "3", "generating", "gen":
				_ = pub.Publish(model.StateGenerating, "User selected Generating")
			case "4", "executing_tool", "tool", "exec":
				_ = pub.Publish(model.StateExecutingTool, "User selected Executing Tool")
			case "5", "waiting_input", "wait", "input":
				_ = pub.Publish(model.StateWaitingInput, "User selected Waiting Input")
			case "6", "done":
				_ = pub.Publish(model.StateDone, "User selected Done")
			case "7", "error", "err":
				_ = pub.Publish(model.StateError, "User selected Error")
			case "w", "workflow":
				fmt.Println("--> Running workflow once...")
				_ = runWorkflow(ctx, pub, 3*time.Second, true)
				printInteractiveHelp()
			case "":
				// Ignore empty Enter
			default:
				fmt.Printf("Unknown choice %q. Type 'h' for menu options.\n", input)
			}
			fmt.Print("Choice [1-7, w, q] > ")
		}
	}
}

func printBanner(title string, interval time.Duration) {
	fmt.Println("================================================================================")
	fmt.Printf("  K.I.T.T. Eye — %s\n", title)
	if interval > 0 {
		fmt.Printf("  Transition Interval: %v (Ctrl+C to quit and reset to IDLE)\n", interval)
	}
	fmt.Println("================================================================================")
}

func printInteractiveHelp() {
	fmt.Print(`
Available Commands:
  [1] idle           — Classic Red (Slow K.I.T.T. Scanner)
  [2] thinking       — Mystic Purple (Center-Out)
  [3] generating     — Mint Emerald (Fill-Sweep)
  [4] executing_tool — Vivid Amber (Fast K.I.T.T. Scanner)
  [5] waiting_input  — Warning Yellow (Blink)
  [6] done           — Spring Lime (Flash -> Solid)
  [7] error          — Crimson Red (Comet)
  [w] workflow       — Run realistic agent workflow once
  [h] help           — Show this menu
  [q] quit           — Exit and reset to IDLE

Choice [1-7, w, q] > `)
}

// findConfigFile looks for config.yaml across common relative paths.
func findConfigFile(specified string) string {
	if specified != "" {
		return specified
	}
	if v := os.Getenv("KITTEYE_CONFIG"); v != "" {
		return v
	}

	candidates := []string{
		"config.yaml",
		"../config.yaml",
		"../../config.yaml",
	}

	// Also check relative to executable directory
	if exe, err := os.Executable(); err == nil {
		dir := filepath.Dir(exe)
		candidates = append(candidates,
			filepath.Join(dir, "config.yaml"),
			filepath.Join(dir, "..", "config.yaml"),
			filepath.Join(dir, "..", "..", "config.yaml"),
		)
	}

	for _, c := range candidates {
		if fi, err := os.Stat(c); err == nil && !fi.IsDir() {
			return c
		}
	}
	return "config.yaml"
}
