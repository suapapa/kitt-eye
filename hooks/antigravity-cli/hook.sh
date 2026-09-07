#!/usr/bin/env bash
# hooks/antigravity-cli/hook.sh — Antigravity (agy) hook → kitt-eye event
# Event name is argv[1] (payload has no event field). stdout contract:
#   PostToolUse → "{}"; Stop → {"decision":"allow"}; else silent (PLAN.md §4.5.4)
set -u
EVENT="${1:-unknown}"
INPUT="$(cat)"

# stdout contract must hold regardless of jq/SEND availability.
case "$EVENT" in
  PostToolUse) printf '{}\n' ;;
  Stop)        printf '{"decision":"allow"}\n' ;;
esac

[ -n "$INPUT" ] || exit 0
command -v jq >/dev/null 2>&1 || exit 0
SEND="${HOME}/.kitt-eye/bin/kitt-eye-send"
[ -x "$SEND" ] || exit 0

IFS=$'\t' read -r STATE SID DETAIL < <(printf '%s' "$INPUT" | jq -r --arg e "$EVENT" '
  def clean: tostring | gsub("[\r\n\t\"]"; " ") | .[0:60];
  (if   $e == "PreInvocation"  then "thinking"
   elif $e == "PostInvocation" then "generating"
   elif $e == "PostToolUse" and ((.error // "") != "") then "error"
   elif $e == "PostToolUse"    then "executing_tool"
   elif $e == "Stop" and (((.terminationReason // "") == "error") or ((.error // "") != "")) then "error"
   elif $e == "Stop"           then "done"
   else "idle" end) as $st
  | (if   .toolCall and .toolCall.name
       then .toolCall.name + " " + ((.toolCall.args // {} | [.[]] | map(tostring) | join(" ")) | clean)
     elif $e == "Stop" then ((.terminationReason // "stop") | clean)
     elif $e == "PreInvocation" or $e == "PostInvocation"
       then "invocation " + ((.invocationNum // 0) | tostring)
     else "" end | sub(" +$"; "")) as $det
  | [ $st, (.conversationId // "default"), $det ] | @tsv' 2>/dev/null) || exit 0

[ -n "${STATE:-}" ] || exit 0
"$SEND" antigravity-cli "$STATE" "$DETAIL" "$SID" "$EVENT" || true
exit 0
