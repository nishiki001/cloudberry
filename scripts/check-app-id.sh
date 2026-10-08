#!/usr/bin/env bash
# Starts the GUI for a few seconds and prints the window's app id as the compositor sees it.
# KDE Wayland: asks KWin (scripting) for desktopFileName/resourceClass; X11: xprop WM_CLASS.
# Usage: scripts/check-app-id.sh [path/to/cloudberry]   (exit 0 when the id matches)
set -u
BIN=${1:-target/debug/cloudberry}
WANT=io.github.nishiki001.Cloudberry
"$BIN" --quit-after 14 >/dev/null 2>&1 &
PID=$!
sleep 4
if [ -n "${WAYLAND_DISPLAY:-}" ] && command -v qdbus >/dev/null && command -v dbus-monitor >/dev/null; then
  # KWin script reports each window over D-Bus; dbus-monitor catches the (unanswered) call.
  JS=$(mktemp --suffix=.js); MON=$(mktemp)
  echo 'for (const w of workspace.windowList()) if (w.caption == "Cloudberry") callDBus("test.cb", "/", "test.cb", "report", "APPID " + w.desktopFileName + " " + w.resourceClass);' > "$JS"
  timeout 6 dbus-monitor "interface='test.cb'" > "$MON" 2>&1 &
  sleep 1
  N=cb-appid-$$
  qdbus org.kde.KWin /Scripting org.kde.kwin.Scripting.loadScript "$JS" "$N" >/dev/null
  qdbus org.kde.KWin /Scripting org.kde.kwin.Scripting.start >/dev/null
  sleep 2
  OUT=$(grep -o 'APPID .*[^"]' "$MON" | head -1)
  qdbus org.kde.KWin /Scripting org.kde.kwin.Scripting.unloadScript "$N" >/dev/null 2>&1
  rm -f "$JS" "$MON"
elif command -v xprop >/dev/null; then
  OUT="APPID $(xprop -root -notype >/dev/null 2>&1; xprop -name Cloudberry WM_CLASS 2>/dev/null)"
else
  OUT=""
fi
kill $PID 2>/dev/null; wait $PID 2>/dev/null
echo "${OUT:-no window found}"
case "$OUT" in *"$WANT"*) echo OK; exit 0;; *) echo "MISMATCH (want $WANT)"; exit 1;; esac
