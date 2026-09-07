#!/usr/bin/env bash
# hooks/cursor-cli/hook.sh — Cursor CLI / cursor-agent hook → kitt-eye event
# Always exit 0 (2=block). stdout: silent or "{}" only (PLAN.md §4.7.4)
set -u
trap 'exit 0' EXIT

INPUT="$(cat)"
[ -n "$INPUT" ] || exit 0
command -v jq >/dev/null 2>&1 || exit 0
SEND="${HOME}/.kitt-eye/bin/kitt-eye-send"
[ -x "$SEND" ] || exit 0

IFS=$'\t' read -r EVENT STATE SID DETAIL < <(printf '%s' "$INPUT" | jq -r '
  def clean: tostring | gsub("[\r\n\t\"]"; " ") | .[0:60];
  .hook_event_name as $e
  | (if   $e == "sessionStart"         then "idle"
     elif $e == "sessionEnd"           then "idle"
     elif $e == "beforeSubmitPrompt"   then "thinking"
     elif $e == "preCompact"           then "thinking"
     elif $e == "postToolUseFailure"   then "error"
     elif $e == "stop" and ((.status // "") == "aborted") then "idle"
     elif $e == "stop" and ((.status // "") == "error")   then "error"
     elif $e == "stop"                 then "done"
     else "executing_tool" end) as $st
  | (if   .command       then ("$ " + (.command | clean))
     elif .file_path     then ((.file_path | split("/") | last) | clean)
     elif .error_message then ((.failure_type // "error") | clean) + ": " + (.error_message | clean)
     elif .status        then (.status | clean)
     elif .reason        then (.reason | clean)
     elif .prompt        then (.prompt | clean)
     elif .tool_name     then .tool_name
     else "" end | sub(" +$"; "")) as $det
  | [ $e, $st, (.conversation_id // "default"), $det ] | @tsv' 2>/dev/null) || exit 0

[ -n "${EVENT:-}" ] || exit 0
"$SEND" cursor-cli "$STATE" "$DETAIL" "$SID" "$EVENT" || true
exit 0
