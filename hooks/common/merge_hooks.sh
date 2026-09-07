#!/usr/bin/env bash
# merge_hooks.sh — idempotent jq merge of kitt-eye hook snippets into CLI settings.
# Usage:
#   merge_hooks.sh install <claude|codex|cursor-cli|antigravity-cli>
#   merge_hooks.sh uninstall <claude|codex|cursor-cli|antigravity-cli|all>
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
BIN_DIR="${HOME}/.kitt-eye/bin"
ACTION="${1:-}"
AGENT="${2:-}"

die() { echo "merge_hooks: $*" >&2; exit 1; }

need_jq() {
  command -v jq >/dev/null 2>&1 || die "jq is required (brew install jq / apt install jq)"
}

backup() {
  local target="$1"
  if [ -f "$target" ]; then
    local bak="${target}.bak.$(date +%Y%m%d%H%M%S)"
    cp -f "$target" "$bak"
    echo "  backup: $bak"
  fi
}

atomic_write() {
  local target="$1"
  local tmp
  tmp="$(mktemp "${target}.XXXXXX")"
  cat >"$tmp"
  jq empty "$tmp" >/dev/null 2>&1 || { rm -f "$tmp"; die "refusing to write invalid JSON to $target"; }
  mv -f "$tmp" "$target"
}

ensure_parent() {
  mkdir -p "$(dirname "$1")"
}

install_bins() {
  mkdir -p "$BIN_DIR"
  cp -f "$ROOT/hooks/common/send_event.sh" "$BIN_DIR/kitt-eye-send"
  chmod +x "$BIN_DIR/kitt-eye-send"
}

# Strip matcher-group handlers whose nested .hooks[].command contains $needle,
# and drop empty groups. Used by claude/codex.
strip_nested_cmd() {
  local needle="$1"
  jq --arg n "$needle" '
    def strip_groups:
      map(
        if has("hooks") then
          .hooks |= map(select(((.command // "") | tostring | contains($n)) | not))
          | select((.hooks | length) > 0)
        else . end
      );
    .hooks //= {}
    | .hooks |= with_entries(.value |= (if type == "array" then strip_groups else . end))
  '
}

# Append snippet.hooks event groups after stripping prior kitt-eye entries.
merge_nested() {
  local target="$1"
  local snippet="$2"
  local needle="$3"
  local default_json="$4"

  ensure_parent "$target"
  if [ ! -f "$target" ]; then
    printf '%s\n' "$default_json" >"$target"
  fi
  backup "$target"

  local stripped merged
  stripped="$(strip_nested_cmd "$needle" <"$target")"
  merged="$(jq -s --slurpfile snip "$snippet" '
    .[0] as $root
    | ($snip[0].hooks // {}) as $add
    | $root
    | .hooks //= {}
    | reduce ($add | to_entries[]) as $e (.;
        .hooks[$e.key] = ((.hooks[$e.key] // []) + $e.value)
      )
  ' <<<"$stripped")"
  printf '%s\n' "$merged" | atomic_write "$target"
}

# Cursor: flat handler arrays with .command on each entry.
merge_cursor() {
  local target="$1"
  local snippet="$2"
  local needle="kitt-eye-cursor-hook"

  ensure_parent "$target"
  if [ ! -f "$target" ]; then
    printf '%s\n' '{"version":1,"hooks":{}}' >"$target"
  fi
  backup "$target"

  local merged
  merged="$(jq -s --slurpfile snip "$snippet" --arg n "$needle" '
    .[0] as $root
    | ($snip[0].hooks // {}) as $add
    | $root
    | .version = (.version // ($snip[0].version // 1))
    | .hooks //= {}
    | .hooks |= with_entries(
        .value |= (if type == "array"
          then map(select(((.command // "") | tostring | contains($n)) | not))
          else . end)
      )
    | reduce ($add | to_entries[]) as $e (.;
        .hooks[$e.key] = ((.hooks[$e.key] // []) + $e.value)
      )
  ' "$target")"
  printf '%s\n' "$merged" | atomic_write "$target"
}

# Antigravity: named bundle at top-level key "kitt-eye".
merge_agy() {
  local target="$1"
  local snippet="$2"

  ensure_parent "$target"
  if [ ! -f "$target" ]; then
    printf '%s\n' '{}' >"$target"
  fi
  backup "$target"

  local merged
  merged="$(jq -s --slurpfile snip "$snippet" '
    .[0] | del(.["kitt-eye"]) | . + $snip[0]
  ' "$target")"
  printf '%s\n' "$merged" | atomic_write "$target"
}

uninstall_nested() {
  local target="$1"
  local needle="$2"
  [ -f "$target" ] || return 0
  backup "$target"
  strip_nested_cmd "$needle" <"$target" | atomic_write "$target"
}

uninstall_cursor() {
  local target="${HOME}/.cursor/hooks.json"
  local needle="kitt-eye-cursor-hook"
  [ -f "$target" ] || return 0
  backup "$target"
  jq --arg n "$needle" '
    .hooks //= {}
    | .hooks |= with_entries(
        .value |= (if type == "array"
          then map(select(((.command // "") | tostring | contains($n)) | not))
          else . end)
      )
  ' "$target" | atomic_write "$target"
}

uninstall_agy() {
  local target="${HOME}/.gemini/config/hooks.json"
  [ -f "$target" ] || return 0
  backup "$target"
  jq 'del(.["kitt-eye"])' "$target" | atomic_write "$target"
}

do_install() {
  need_jq
  install_bins
  case "$AGENT" in
    claude)
      cp -f "$ROOT/hooks/claude/hook.sh" "$BIN_DIR/kitt-eye-claude-hook"
      chmod +x "$BIN_DIR/kitt-eye-claude-hook"
      merge_nested \
        "${HOME}/.claude/settings.json" \
        "$ROOT/hooks/claude/settings.snippet.json" \
        "kitt-eye-claude-hook" \
        '{}'
      echo "Installed claude hooks → ~/.claude/settings.json"
      echo "  verify: jq '.hooks | keys' ~/.claude/settings.json  (and /hooks in Claude Code)"
      ;;
    codex)
      cp -f "$ROOT/hooks/codex/hook.sh" "$BIN_DIR/kitt-eye-codex-hook"
      chmod +x "$BIN_DIR/kitt-eye-codex-hook"
      merge_nested \
        "${HOME}/.codex/hooks.json" \
        "$ROOT/hooks/codex/hooks.snippet.json" \
        "kitt-eye-codex-hook" \
        '{"hooks":{}}'
      echo "Installed codex hooks → ~/.codex/hooks.json"
      echo "  REQUIRED: open a codex session and /hooks → approve kitt-eye (hash trust)."
      ;;
    cursor-cli)
      cp -f "$ROOT/hooks/cursor-cli/hook.sh" "$BIN_DIR/kitt-eye-cursor-hook"
      chmod +x "$BIN_DIR/kitt-eye-cursor-hook"
      merge_cursor \
        "${HOME}/.cursor/hooks.json" \
        "$ROOT/hooks/cursor-cli/hooks.snippet.json"
      echo "Installed cursor-cli hooks → ~/.cursor/hooks.json (file watcher reloads)"
      echo "  note: this file is shared with Cursor IDE sessions"
      ;;
    antigravity-cli)
      cp -f "$ROOT/hooks/antigravity-cli/hook.sh" "$BIN_DIR/kitt-eye-agy-hook"
      chmod +x "$BIN_DIR/kitt-eye-agy-hook"
      merge_agy \
        "${HOME}/.gemini/config/hooks.json" \
        "$ROOT/hooks/antigravity-cli/hooks.snippet.json"
      echo "Installed antigravity-cli hooks → ~/.gemini/config/hooks.json (bundle: kitt-eye)"
      echo "  REQUIRED: workspace must be in trustedWorkspaces (agy /hooks)"
      ;;
    *)
      die "unknown agent '$AGENT' (claude|codex|cursor-cli|antigravity-cli)"
      ;;
  esac
}

do_uninstall() {
  need_jq
  case "$AGENT" in
    claude)
      uninstall_nested "${HOME}/.claude/settings.json" "kitt-eye-claude-hook"
      rm -f "$BIN_DIR/kitt-eye-claude-hook"
      echo "Removed claude kitt-eye hooks"
      ;;
    codex)
      uninstall_nested "${HOME}/.codex/hooks.json" "kitt-eye-codex-hook"
      rm -f "$BIN_DIR/kitt-eye-codex-hook"
      echo "Removed codex kitt-eye hooks"
      ;;
    cursor-cli)
      uninstall_cursor
      rm -f "$BIN_DIR/kitt-eye-cursor-hook"
      echo "Removed cursor-cli kitt-eye hooks"
      ;;
    antigravity-cli)
      uninstall_agy
      rm -f "$BIN_DIR/kitt-eye-agy-hook"
      echo "Removed antigravity-cli kitt-eye hooks"
      ;;
    *)
      die "unknown agent '$AGENT' (claude|codex|cursor-cli|antigravity-cli|all)"
      ;;
  esac
}

do_uninstall_all() {
  need_jq
  uninstall_nested "${HOME}/.claude/settings.json" "kitt-eye-claude-hook" || true
  uninstall_nested "${HOME}/.codex/hooks.json" "kitt-eye-codex-hook" || true
  uninstall_cursor || true
  uninstall_agy || true
  rm -f "$BIN_DIR"/kitt-eye-*-hook "$BIN_DIR"/kitt-eye-send
  rmdir "$BIN_DIR" 2>/dev/null || true
  rmdir "${HOME}/.kitt-eye" 2>/dev/null || true
  echo "Removed all kitt-eye hooks from CLI settings and ~/.kitt-eye/bin"
}

case "$ACTION" in
  install)
    [ -n "$AGENT" ] || die "usage: merge_hooks.sh install <agent>"
    do_install
    ;;
  uninstall)
    [ -n "$AGENT" ] || die "usage: merge_hooks.sh uninstall <agent|all>"
    if [ "$AGENT" = "all" ]; then
      do_uninstall_all
    else
      do_uninstall
    fi
    ;;
  *)
    die "usage: merge_hooks.sh {install|uninstall} <agent>"
    ;;
esac
