#!/usr/bin/env bash
# hooks/hermes/hook.sh — Hermes Agent hook stdin JSON → kitt-eye event
# Observer only: always exit 0. stdout silent.
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
  | (if   $e == "on_session_start"     then "idle"
     elif $e == "on_session_reset"     then "idle"
     elif $e == "pre_llm_call"         then "thinking"
     elif $e == "on_stream_start"      then "generating"
     elif $e == "on_stream_delta"      then "generating"
     elif $e == "pre_tool_call"        then "executing_tool"
     elif $e == "post_tool_call"       then "thinking"
     elif $e == "subagent_start"       then "executing_tool"
     elif $e == "subagent_stop"        then "thinking"
     elif $e == "pre_approval_request" then "waiting_input"
     elif $e == "api_request_error"    then "error"
     elif $e == "on_session_end"       then "done"
     elif $e == "on_session_finalize"  then "done"
     else "idle" end) as $st
  | (if   .tool_name and .tool_input
       then .tool_name + " " + ((.tool_input.command // .tool_input.path // .tool_input.file_path // .tool_input.query // .tool_input.prompt // "") | clean)
     elif .tool_name
       then .tool_name
     elif .extra.error_message
       then (.extra.error_message | clean)
     elif .extra.error
       then (.extra.error | clean)
     elif .extra.command
       then ("$ " + (.extra.command | clean))
     elif .extra.child_role
       then ("subagent: " + (.extra.child_role | clean))
     elif .extra.description
       then (.extra.description | clean)
     elif .extra.user_message
       then (.extra.user_message | clean)
     elif $e == "on_session_end"
       then "session ended"
     elif $e == "on_session_start"
       then "session started"
     else "" end | sub(" +$"; "")) as $det
  | [ $e, $st, (.session_id // "default"), $det ] | @tsv' 2>/dev/null) || exit 0

[ -n "${EVENT:-}" ] || exit 0
"$SEND" hermes "$STATE" "$DETAIL" "$SID" "$EVENT" || true
exit 0
