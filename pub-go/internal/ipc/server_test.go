package ipc

import (
	"context"
	"errors"
	"fmt"
	"io"
	"log/slog"
	"net"
	"os"
	"sync/atomic"
	"syscall"
	"testing"
	"time"
)

func testLogger() *slog.Logger { return slog.New(slog.NewTextHandler(io.Discard, nil)) }

var socketSeq atomic.Uint32

// macOS limits UDS paths to ~104 bytes, so t.TempDir() is unusable; bind
// short names directly under TMPDIR instead (os.TempDir honors TMPDIR, and
// the sandbox gives us a writable one).
func socketPath(t *testing.T) string {
	t.Helper()
	p := fmt.Sprintf("%s/ke-%d-%d.sock", os.TempDir(), os.Getpid(), socketSeq.Add(1))
	t.Cleanup(func() { os.Remove(p) })
	return p
}

// newOrSkip binds like New but skips tests where the OS sandbox forbids
// binding unix sockets (e.g. restricted CI); normal dev machines run fully.
func newOrSkip(t *testing.T, path string, h Handler) *Server {
	t.Helper()
	srv, err := New(path, h, testLogger())
	if err != nil {
		if errors.Is(err, syscall.EPERM) || errors.Is(err, syscall.EACCES) {
			t.Skipf("unix socket bind not permitted here: %v", err)
		}
		t.Fatal(err)
	}
	return srv
}

func startServer(t *testing.T) (*Server, chan string) {
	t.Helper()
	path := socketPath(t)
	events := make(chan string, 8)
	ctx, cancel := context.WithCancel(context.Background())
	srv := newOrSkip(t, path, func(_ context.Context, line []byte) { events <- string(line) })
	go srv.Serve(ctx)
	t.Cleanup(func() { cancel(); srv.Close() })
	return srv, events
}

func send(t *testing.T, path, payload string) {
	t.Helper()
	conn, err := net.Dial("unix", path)
	if err != nil {
		t.Fatal(err)
	}
	_ = conn.SetDeadline(time.Now().Add(2 * time.Second))
	if _, err := io.WriteString(conn, payload); err != nil {
		t.Fatal(err)
	}
	conn.Close()
}

func recv(t *testing.T, events chan string) string {
	t.Helper()
	select {
	case got := <-events:
		return got
	case <-time.After(3 * time.Second):
		t.Fatal("timed out waiting for event")
		return ""
	}
}

func TestDeliversDatagrams(t *testing.T) {
	srv, events := startServer(t)
	send(t, srv.Path(), `{"agent":"claude","state":"idle"}`+"\n")
	send(t, srv.Path(), `{"agent":"codex","state":"thinking"}`) // no trailing newline (nc -U style)

	if got := recv(t, events); got != `{"agent":"claude","state":"idle"}` {
		t.Errorf("event 1 = %q", got)
	}
	if got := recv(t, events); got != `{"agent":"codex","state":"thinking"}` {
		t.Errorf("event 2 = %q", got)
	}
}

func TestOversizedDropped(t *testing.T) {
	srv, events := startServer(t)
	junk := `{"agent":"claude","state":"idle","detail":"` + string(make([]byte, MaxLineBytes)) + `"}`
	send(t, srv.Path(), junk)
	select {
	case got := <-events:
		t.Errorf("oversized datagram must be dropped, got %d bytes", len(got))
	case <-time.After(300 * time.Millisecond):
	}
}

func TestSilentClientIgnored(t *testing.T) {
	srv, events := startServer(t)
	conn, err := net.Dial("unix", srv.Path())
	if err != nil {
		t.Fatal(err)
	}
	defer conn.Close()
	_ = conn.SetDeadline(time.Now().Add(300 * time.Millisecond))
	io.WriteString(conn, "") // connect and idle without sending
	select {
	case got := <-events:
		t.Errorf("no event expected, got %q", got)
	case <-time.After(300 * time.Millisecond):
	}
}

func TestSecondBindRejectedWhileRunning(t *testing.T) {
	srv, _ := startServer(t)
	if _, err := New(srv.Path(), func(context.Context, []byte) {}, testLogger()); err == nil {
		t.Fatal("expected 'already served' error for a live socket")
	}
}

func TestCloseRemovesSocketFile(t *testing.T) {
	path := socketPath(t)
	srv := newOrSkip(t, path, func(context.Context, []byte) {})
	if err := srv.Close(); err != nil {
		t.Fatal(err)
	}
	if _, err := net.Dial("unix", path); err == nil {
		t.Error("socket still reachable after Close")
	}
}
