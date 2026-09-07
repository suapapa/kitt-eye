#!/usr/bin/env bash
# hooks/claude/hook.sh — Claude Code hook stdin JSON → kitt-eye event
# Observer only: no stdout, always exit 0 (PLAN.md §4.4.4)
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
  | (if   $e == "SessionStart"        then "idle"
     elif $e == "SessionEnd"          then "idle"
     elif $e == "UserPromptSubmit"    then "thinking"
     elif $e == "MessageDisplay"      then "generating"
     elif $e == "PreCompact"          then "thinking"
     elif $e == "PostToolBatch"       then "thinking"
     elif $e == "SubagentStop"        then "thinking"
     elif $e == "PreToolUse"          then "executing_tool"
     elif $e == "PostToolUse"         then "executing_tool"
     elif $e == "SubagentStart"       then "executing_tool"
     elif $e == "PermissionRequest"   then "waiting_input"
     elif $e == "Notification"        then "waiting_input"
     elif $e == "Stop"                then "done"
     elif $e == "StopFailure"         then "error"
     elif $e == "PostToolUseFailure"  then "error"
     else "idle" end) as $st
  | (if .tool_name and .error
       then .tool_name + " failed: " + (.error | clean)
     elif .tool_name
       then .tool_name + " " + ((.tool_input.description // .tool_input.command
             // .tool_input.file_path // .tool_input.pattern // .tool_input.query
             // .tool_input.url // .tool_input.prompt // "") | clean)
     elif .notification_type      then (.notification_type | clean)
     elif .error                  then (.error | clean)
     elif .delta                  then (.delta | clean)
     elif .last_assistant_message then (.last_assistant_message | clean)
     elif .prompt                 then (.prompt | clean)
     elif .source                 then (.source | clean)
     elif .reason                 then (.reason | clean)
     else "" end | sub(" +$"; "")) as $det
  | [ $e, $st, (.session_id // "default"), $det ] | @tsv' 2>/dev/null) || exit 0

[ -n "${EVENT:-}" ] || exit 0
"$SEND" claude "$STATE" "$DETAIL" "$SID" "$EVENT" || true
exit 0
