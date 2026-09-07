// Package ipc serves the Unix-domain-socket ingress that hook scripts write
// one-line JSON datagrams into (§4.1.2). Hooks are fire-and-forget, so the
// server never blocks on slow or half-open clients and never writes replies.
package ipc

import (
	"bufio"
	"bytes"
	"context"
	"errors"
	"fmt"
	"io"
	"log/slog"
	"net"
	"os"
	"path/filepath"
	"time"
)

// MaxLineBytes bounds a single datagram.
const MaxLineBytes = 8 * 1024

// readTimeout bounds how long a client may take to send its line.
const readTimeout = 5 * time.Second

// Handler receives one raw datagram. It must be quick; the server runs it
// inline per connection but never waits for anything downstream.
type Handler func(ctx context.Context, line []byte)

// Server listens on a UDS path.
type Server struct {
	path string
	ln   net.Listener
	h    Handler
	log  *slog.Logger
}

// New probes the path, clears a stale socket file, and binds. A path that
// answers connections means another daemon is live, so New fails instead of
// stealing its socket.
func New(path string, h Handler, log *slog.Logger) (*Server, error) {
	if conn, err := net.Dial("unix", path); err == nil {
		_ = conn.Close()
		return nil, fmt.Errorf("%s is already served by a running kitt-eye publisher", path)
	}
	if err := os.MkdirAll(filepath.Dir(path), 0o755); err != nil {
		return nil, fmt.Errorf("create socket dir: %w", err)
	}
	if err := os.RemoveAll(path); err != nil {
		return nil, fmt.Errorf("clear stale socket: %w", err)
	}
	ln, err := net.Listen("unix", path)
	if err != nil {
		return nil, fmt.Errorf("listen %s: %w", path, err)
	}
	return &Server{path: path, ln: ln, h: h, log: log}, nil
}

// Path returns the bound socket path.
func (s *Server) Path() string { return s.path }

// Serve accepts connections until ctx is done or the listener fails.
func (s *Server) Serve(ctx context.Context) error {
	go func() {
		<-ctx.Done()
		_ = s.ln.Close()
	}()
	for {
		conn, err := s.ln.Accept()
		if err != nil {
			if ctx.Err() != nil {
				return nil
			}
			return fmt.Errorf("accept: %w", err)
		}
		go s.handle(ctx, conn)
	}
}

func (s *Server) handle(ctx context.Context, conn net.Conn) {
	defer conn.Close()
	_ = conn.SetReadDeadline(time.Now().Add(readTimeout))
	reader := bufio.NewReaderSize(conn, MaxLineBytes)
	line, err := reader.ReadBytes('\n')
	// ReadBytes returns any data read even on timeout/EOF-without-newline;
	// `nc -U` hooks commonly omit the trailing newline, so data wins over err.
	if len(bytes.TrimSpace(line)) == 0 {
		if err != nil && !errors.Is(err, io.EOF) && !os.IsTimeout(err) {
			s.log.Debug("ipc: read failed", "err", err)
		}
		return
	}
	if len(line) > MaxLineBytes {
		s.log.Warn("ipc: oversized datagram dropped", "bytes", len(line))
		return
	}
	s.h(ctx, bytes.TrimRight(line, "\r\n"))
}

// Close stops the listener and removes the socket file.
func (s *Server) Close() error {
	err := s.ln.Close()
	if rmErr := os.Remove(s.path); rmErr != nil && !errors.Is(rmErr, os.ErrNotExist) {
		return rmErr
	}
	return err
}
