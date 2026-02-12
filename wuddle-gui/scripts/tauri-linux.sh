#!/usr/bin/env bash
set -euo pipefail

args=("$@")

echo "wuddle: trying Wayland backend first..."
set +e
env WUDDLE_LAUNCH_MODE=wayland-primary WUDDLE_WAYLAND_FALLBACK=0 GDK_BACKEND=wayland tauri "${args[@]}"
status=$?
set -e

# Respect user interruption and normal exits.
if [[ "$status" -eq 0 || "$status" -eq 130 || "$status" -eq 143 ]]; then
  exit "$status"
fi

echo "wuddle: Wayland launch failed (exit ${status}); retrying with X11 compatibility mode..."
exec env WUDDLE_LAUNCH_MODE=x11-fallback WUDDLE_WAYLAND_FALLBACK=1 GDK_BACKEND=x11 WINIT_UNIX_BACKEND=x11 WEBKIT_DISABLE_DMABUF_RENDERER=1 tauri "${args[@]}"
