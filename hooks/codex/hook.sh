#!/usr/bin/env bash
# hooks/codex/hook.sh — Codex CLI hook stdin JSON → kitt-eye event
# Always exit 0. Plain stdout forbidden; Stop/SubagentStop require "{}" (PLAN.md §4.6.4)
set -u

INPUT="$(cat)"
EVENT=""

# Ensure Stop/SubagentStop always emit JSON, even on early failure paths.
_finish() {
  case "${EVENT:-}" in
    Stop|SubagentStop) printf '{}\n' ;;
  esac
  exit 0
}
trap _finish EXIT

[ -n "$INPUT" ] || exit 0
command -v jq >/dev/null 2>&1 || exit 0
SEND="${HOME}/.kitt-eye/bin/kitt-eye-send"
[ -x "$SEND" ] || exit 0

IFS=$'\t' read -r EVENT STATE SID DETAIL < <(printf '%s' "$INPUT" | jq -r '
  def clean: tostring | gsub("[\r\n\t\"]"; " ") | .[0:60];
  .hook_event_name as $e
  | (if   $e == "SessionStart"      then "idle"
     elif $e == "SessionEnd"        then "idle"
     elif $e == "Interrupt"         then "idle"
     elif $e == "UserPromptSubmit"  then "thinking"
     elif $e == "PreCompact"        then "thinking"
     elif $e == "PostCompact"       then "thinking"
     elif $e == "SubagentStop"      then "thinking"
     elif $e == "PreToolUse"        then "executing_tool"
     elif $e == "SubagentStart"     then "executing_tool"
     elif $e == "PostToolUse" and ((.tool_response | tostring) | test("error")) then "error"
     elif $e == "PostToolUse"       then "executing_tool"
     elif $e == "PermissionRequest" then "waiting_input"
     elif $e == "Stop"              then "done"
     else "idle" end) as $st
  | (if   .tool_name                 then .tool_name + " " + ((.tool_input.command
             // .tool_input.file_path // .tool_input.pattern // .tool_input.query
             // .tool_input.url // "") | clean)
     elif .notification_type         then (.notification_type | clean)
     elif .last_assistant_message    then (.last_assistant_message | clean)
     elif .prompt                    then (.prompt | clean)
     elif .reason                    then (.reason | clean)
     elif $e == "Interrupt"          then "interrupted"
     elif $e == "PreCompact"         then "compacting"
     elif $e == "PostCompact"        then "compacted"
     elif $e == "SubagentStart"      then "subagent: " + ((.agent_type // "") | clean)
     elif $e == "SubagentStop"       then "subagent done"
     elif .source                    then (.source | clean)
     else "" end | sub(" +$"; "")) as $det
  | [ $e, $st, (.session_id // "default"), $det ] | @tsv' 2>/dev/null) || exit 0

[ -n "${EVENT:-}" ] || exit 0
"$SEND" codex "$STATE" "$DETAIL" "$SID" "$EVENT" || true
exit 0
