#!/usr/bin/env bash
set -euo pipefail

# VOICEVOX Reset Script - Kill daemon processes and remove socket file
# Used during development to reset daemon state after rebuilds

echo "Resetting VOICEVOX daemon state..."

# Resolve socket path (mirrors src/paths.rs get_socket_path)
if [ -n "${VOICEVOX_SOCKET_PATH:-}" ]; then
  SOCKET_PATH="$VOICEVOX_SOCKET_PATH"
elif [ -n "${XDG_RUNTIME_DIR:-}" ]; then
  SOCKET_PATH="$XDG_RUNTIME_DIR/voicevox-daemon.sock"
elif [ -n "${XDG_STATE_HOME:-}" ]; then
  SOCKET_PATH="$XDG_STATE_HOME/voicevox-daemon.sock"
elif [ -n "${HOME:-}" ]; then
  SOCKET_PATH="$HOME/.local/state/voicevox-daemon.sock"
else
  SOCKET_PATH="/tmp/voicevox-daemon.sock"
fi

# Kill daemon processes
PIDS=$(pgrep -u "$(id -u)" -f voicevox-daemon 2>/dev/null || true)
if [ -n "$PIDS" ]; then
  echo "Stopping daemon processes: $PIDS"
  kill -TERM $PIDS 2>/dev/null || true
  sleep 1
  # Force kill any remaining
  REMAINING=$(pgrep -u "$(id -u)" -f voicevox-daemon 2>/dev/null || true)
  if [ -n "$REMAINING" ]; then
    echo "Force killing: $REMAINING"
    kill -9 $REMAINING 2>/dev/null || true
  fi
else
  echo "No daemon processes found"
fi

# Remove socket file
if [ -e "$SOCKET_PATH" ]; then
  rm -f "$SOCKET_PATH"
  echo "Removed socket: $SOCKET_PATH"
else
  echo "No socket file: $SOCKET_PATH"
fi

echo "Reset complete"
