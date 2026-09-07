#!/usr/bin/env bash
# send_event.sh — fire-and-forget IPC client for kitt-eye (protocol v1.1)
# Usage: send_event.sh <agent> <state> [detail] [session_id] [event]
#
# MUST NOT write to stdout (hooks inherit stdout; Claude/Codex inject it into
# the model or treat it as decision JSON). Failures are silent; always exit 0.
set -u

AGENT="${1:-}"
STATE="${2:-}"
DETAIL="${3:-}"
SESSION_ID="${4:-default}"
EVENT="${5:-}"
SOCKET_PATH="${KITTEYE_SOCKET:-/tmp/kitt-eye.sock}"

[ -n "$AGENT" ] && [ -n "$STATE" ] || exit 0
[ -S "$SOCKET_PATH" ] || exit 0
command -v jq >/dev/null 2>&1 || exit 0
command -v nc >/dev/null 2>&1 || exit 0

TIMESTAMP="$(date -u +"%Y-%m-%dT%H:%M:%SZ" 2>/dev/null || true)"
[ -n "$TIMESTAMP" ] || exit 0

PAYLOAD="$(jq -nc \
  --arg agent "$AGENT" \
  --arg state "$STATE" \
  --arg detail "$DETAIL" \
  --arg sid "$SESSION_ID" \
  --arg event "$EVENT" \
  --arg ts "$TIMESTAMP" \
  '{agent:$agent, state:$state, timestamp:$ts, session_id:(if $sid == "" then "default" else $sid end)}
   + (if $detail != "" then {detail:$detail} else {} end)
   + (if $event != "" then {event:$event} else {} end)' 2>/dev/null)" || exit 0

[ -n "$PAYLOAD" ] || exit 0

# Swallow all child I/O so hooks never see nc chatter on stdout/stderr.
printf '%s\n' "$PAYLOAD" | nc -U -w 1 "$SOCKET_PATH" >/dev/null 2>&1 || true
exit 0
