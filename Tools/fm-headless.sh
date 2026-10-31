#!/usr/bin/env bash
# Headless display for running Football Manager out of sight (Linux only).
#
# Starts a headless labwc (wlroots) compositor with XWayland on a GPU render
# node. FM, launched through Steam with the launch option below, renders there
# instead of on the desktop; screenshots (grim) and input (xdotool on the
# XWayland display) then work without touching the user's screens. Memory
# access is unaffected: the editor attaches to fm.exe as usual.
#
# Steam launch option (per FM version, set once):
#   env $(bash <repo>/Tools/fm-headless.sh launch-env 2>/dev/null) %command%
# `launch-env` prints DISPLAY plus frame-rate caps only while this compositor
# is alive, and nothing otherwise, so FM falls back to the normal desktop when
# the harness is stopped or has died. The older form still works but has no
# frame cap:
#   DISPLAY=$(bash <repo>/Tools/fm-headless.sh display 2>/dev/null || echo "$DISPLAY") %command%
#
# Usage: bash Tools/fm-headless.sh <command>
#   start [WxH]        start the compositor (default size 1920x1080)
#   stop [--force]     stop it; refuses while FM is still using it
#   status             show displays, pid and FM processes on it
#   launch-env         print env assignments for the Steam launch option
#   display            print the X display if running
#   env                print `export` lines for running clients inside it
#   launch <version>   start FM via Steam: fm24 | fm26 | fm27 | <steam appid>
#   shot [file.png]    screenshot the headless output, print the file path
#
# Environment:
#   FMH_DIR          state directory (default /tmp/fmh)
#   FMH_RENDER_NODE  render node (default: first discrete GPU, else renderD128)
#   FMH_REFRESH      headless output refresh in Hz (default 30)
#   FMH_FPS          FM frame cap via DXVK/VKD3D (default 30; 0 = uncapped).
#                    Recorded by `start` (Steam's shell does not see the
#                    caller's environment); applies to the next FM launch.
#
# The frame cap matters because FM keeps rendering its 3D menus at full speed
# when nothing throttles it; an idle game otherwise costs a large share of the
# GPU the desktop is also using.
#
# Stopping the compositor while FM runs on it takes FM's display away and will
# likely crash the game, losing unsaved progress. Save and quit FM first.
#
# Exit codes: 0 ok, 1 usage/precondition error, 2 not running (status/display).
set -euo pipefail

DIR="${FMH_DIR:-/tmp/fmh}"
PIDFILE="$DIR/labwc.pid"
DISPLAY_FILE="$DIR/display"
WAYLAND_FILE="$DIR/wayland"

die() { echo "fm-headless: $*" >&2; exit 1; }

[ "$(uname -s)" = "Linux" ] || die "Linux only"

running_pid() {
  local pid
  [ -f "$PIDFILE" ] || return 1
  pid=$(cat "$PIDFILE")
  [ -n "$pid" ] && [ "$(cat "/proc/$pid/comm" 2>/dev/null)" = "labwc" ] || return 1
  echo "$pid"
}

# fm.exe processes whose DISPLAY is the headless display.
fm_pids_on() {
  local disp="$1" pid
  for pid in $(pgrep -f '\\fm\.exe' 2>/dev/null); do
    if tr '\0' '\n' < "/proc/$pid/environ" 2>/dev/null | grep -qx "DISPLAY=$disp"; then
      echo "$pid"
    fi
  done
}

pick_render_node() {
  if [ -n "${FMH_RENDER_NODE:-}" ]; then echo "$FMH_RENDER_NODE"; return; fi
  local node dev
  for node in /dev/dri/renderD*; do
    [ -e "$node" ] || continue
    dev=$(readlink -f "/sys/class/drm/$(basename "$node")/device")
    # Prefer the node with the most VRAM (amdgpu reports it, which picks a
    # discrete card over an iGPU); ties keep /dev/dri order.
    if [ -r "$dev/mem_info_vram_total" ]; then
      echo "$(cat "$dev/mem_info_vram_total") $node"
    else
      echo "0 $node"
    fi
  done | sort -s -k1,1nr | awk 'NR==1 {print $2}'
}

cmd_start() {
  local size="${1:-1920x1080}" pid node disp way i
  [[ "$size" =~ ^[0-9]+x[0-9]+$ ]] || die "size must be WxH, got '$size'"
  [[ "${FMH_FPS:-30}" =~ ^[0-9]+$ ]] || die "FMH_FPS must be a number"
  [[ "${FMH_REFRESH:-30}" =~ ^[0-9]+$ ]] || die "FMH_REFRESH must be a number"
  if pid=$(running_pid); then
    echo "already running (pid $pid, DISPLAY=$(cat "$DISPLAY_FILE" 2>/dev/null))"
    return 0
  fi
  for tool in labwc wlr-randr; do
    command -v "$tool" >/dev/null || die "missing $tool"
  done
  node=$(pick_render_node)
  [ -n "$node" ] && [ -e "$node" ] || die "no GPU render node found (set FMH_RENDER_NODE)"

  mkdir -p "$DIR/config/labwc"
  rm -f "$DISPLAY_FILE" "$WAYLAND_FILE" "$DIR/display.pending" "$DIR/wayland.pending"
  # The display file is only published after the output is sized, so the
  # Steam launch option never sees a half-started compositor.
  cat > "$DIR/config/labwc/autostart" <<EOF
echo "\$DISPLAY" > "$DIR/display.pending"
echo "\$WAYLAND_DISPLAY" > "$DIR/wayland.pending"
EOF
  cat > "$DIR/config/labwc/rc.xml" <<'EOF'
<?xml version="1.0"?>
<labwc_config>
  <core><xwaylandPersistence>yes</xwaylandPersistence></core>
  <windowRules>
    <windowRule identifier="*" serverDecoration="no"><action name="Maximize"/></windowRule>
  </windowRules>
</labwc_config>
EOF

  # setsid: the compositor must outlive the shell (or agent session) that
  # started it, because FM dies with its display.
  env -u WAYLAND_DISPLAY -u DISPLAY \
    WLR_BACKENDS=headless WLR_HEADLESS_OUTPUTS=1 WLR_LIBINPUT_NO_DEVICES=1 \
    WLR_RENDERER=gles2 WLR_RENDER_DRM_DEVICE="$node" \
    XDG_CONFIG_HOME="$DIR/config" \
    setsid labwc > "$DIR/labwc.log" 2>&1 < /dev/null &
  echo $! > "$PIDFILE"

  for i in $(seq 1 100); do
    [ -s "$DIR/display.pending" ] && [ -s "$DIR/wayland.pending" ] && break
    running_pid >/dev/null || die "labwc exited during startup; see $DIR/labwc.log"
    sleep 0.1
  done
  [ -s "$DIR/display.pending" ] || die "timed out waiting for XWayland; see $DIR/labwc.log"
  disp=$(cat "$DIR/display.pending")
  way=$(cat "$DIR/wayland.pending")
  WAYLAND_DISPLAY="$way" wlr-randr --output HEADLESS-1 \
    --custom-mode "${size}@${FMH_REFRESH:-30}Hz" \
    || die "could not set output size $size"
  echo "${FMH_FPS:-30}" > "$DIR/fps"
  mv "$DIR/wayland.pending" "$WAYLAND_FILE"
  mv "$DIR/display.pending" "$DISPLAY_FILE"
  echo "started: DISPLAY=$disp WAYLAND_DISPLAY=$way size=$size@${FMH_REFRESH:-30}Hz fps-cap=${FMH_FPS:-30} render=$node pid=$(cat "$PIDFILE")"
}

cmd_stop() {
  local pid disp fm
  if ! pid=$(running_pid); then
    rm -f "$DISPLAY_FILE" "$WAYLAND_FILE" "$PIDFILE"
    echo "not running"
    return 0
  fi
  disp=$(cat "$DISPLAY_FILE" 2>/dev/null || true)
  fm=$([ -n "$disp" ] && fm_pids_on "$disp" || true)
  if [ -n "$fm" ] && [ "${1:-}" != "--force" ]; then
    die "FM is running on $disp (pid $(echo $fm)); save and quit it first, or use --force"
  fi
  # Unpublish first so a concurrent Steam launch falls back to the desktop.
  rm -f "$DISPLAY_FILE" "$WAYLAND_FILE"
  kill "$pid" 2>/dev/null || true
  for _ in $(seq 1 50); do running_pid >/dev/null || break; sleep 0.1; done
  running_pid >/dev/null && kill -9 "$pid" 2>/dev/null || true
  rm -f "$PIDFILE"
  echo "stopped"
}

cmd_status() {
  local pid disp fm
  if ! pid=$(running_pid); then
    echo "not running"
    [ -f "$DISPLAY_FILE" ] && echo "warning: stale $DISPLAY_FILE (run stop to clear)"
    return 2
  fi
  disp=$(cat "$DISPLAY_FILE" 2>/dev/null || echo "?")
  echo "running: pid=$pid DISPLAY=$disp WAYLAND_DISPLAY=$(cat "$WAYLAND_FILE" 2>/dev/null || echo "?")"
  fm=$(fm_pids_on "$disp" || true)
  if [ -n "$fm" ]; then echo "FM on it: pid $(echo $fm)"; else echo "FM on it: none"; fi
}

cmd_display() {
  running_pid >/dev/null && [ -s "$DISPLAY_FILE" ] || return 2
  cat "$DISPLAY_FILE"
}

# Empty output (exit 0) when not running: `env $(...) %command%` then runs the
# game unchanged on the desktop.
cmd_launch_env() {
  local fps
  running_pid >/dev/null && [ -s "$DISPLAY_FILE" ] || return 0
  fps=$(cat "$DIR/fps" 2>/dev/null || echo 30)
  [[ "$fps" =~ ^[0-9]+$ ]] || fps=30
  printf 'DISPLAY=%s' "$(cat "$DISPLAY_FILE")"
  if [ "$fps" -gt 0 ]; then
    printf ' DXVK_FRAME_RATE=%s VKD3D_FRAME_RATE=%s' "$fps" "$fps"
  fi
  printf '\n'
}

cmd_env() {
  running_pid >/dev/null || die "not running"
  echo "export DISPLAY=$(cat "$DISPLAY_FILE") WAYLAND_DISPLAY=$(cat "$WAYLAND_FILE")"
}

cmd_launch() {
  local appid
  case "${1:-}" in
    fm24) appid=2252570 ;;
    fm26) appid=3551340 ;;
    fm27) appid=4530370 ;;
    ''|*[!0-9]*) die "launch needs fm24, fm26, fm27 or a numeric Steam app id" ;;
    *) appid="$1" ;;
  esac
  running_pid >/dev/null || die "not running; start it first"
  command -v steam >/dev/null || die "missing steam"
  # The game only lands here if its Steam launch option reads `display`.
  # Run in the foreground: it hands the request to the running Steam client
  # and exits within seconds. Backgrounded and orphaned, it can die before
  # the hand-off and Steam never sees the request.
  timeout 60 steam -applaunch "$appid" > /dev/null 2>&1 \
    || die "steam -applaunch $appid failed or timed out"
  echo "requested Steam launch of $appid; check with: bash $0 status"
}

cmd_shot() {
  local out="${1:-$DIR/shot-$(date +%Y%m%d-%H%M%S).png}"
  running_pid >/dev/null || die "not running"
  command -v grim >/dev/null || die "missing grim"
  WAYLAND_DISPLAY=$(cat "$WAYLAND_FILE") grim "$out"
  echo "$out"
}

case "${1:-}" in
  start)   shift; cmd_start "$@" ;;
  stop)    shift; cmd_stop "$@" ;;
  status)  cmd_status ;;
  display) cmd_display ;;
  launch-env) cmd_launch_env ;;
  env)     cmd_env ;;
  launch)  shift; cmd_launch "$@" ;;
  shot)    shift; cmd_shot "$@" ;;
  *) awk 'NR>1 && /^set -euo/ {exit} NR>1 {print}' "$0" >&2; exit 1 ;;
esac
