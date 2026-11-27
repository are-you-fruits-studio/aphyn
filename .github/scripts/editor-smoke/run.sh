#!/usr/bin/env bash
# Editor smoke test on Linux: launches aphyn_editor on a virtual display with the fixture project,
# drives a few scenarios with xdotool and saves screenshots. See docs/docs/editor-testing.md.
#
# Usage: .github/scripts/editor-smoke/run.sh [out_dir]
# Needs: Xvfb, xdotool, ImageMagick (import, convert, compare), python3.
# SKIP_BUILD=1 reuses target/debug/aphyn_editor. KEEP_OPEN=1 leaves the editor running for manual checks.

set -uo pipefail

SCRIPT_DIR=$(cd "$(dirname "$0")" && pwd)
ROOT=$(cd "$SCRIPT_DIR/../../.." && pwd)
OUT=$(mkdir -p "${1:-$ROOT/target/editor-smoke}" && cd "${1:-$ROOT/target/editor-smoke}" && pwd)
WORK=$(mktemp -d)
EDITOR_PID=""
XVFB_PID=""
FAILED=0

for tool in Xvfb xdotool import convert compare python3; do
    command -v "$tool" >/dev/null || { echo "missing tool: $tool" >&2; exit 2; }
done

cleanup() {
    if [ "${KEEP_OPEN:-0}" != 1 ]; then
        [ -n "$EDITOR_PID" ] && kill "$EDITOR_PID" 2>/dev/null
        [ -n "$XVFB_PID" ] && kill "$XVFB_PID" 2>/dev/null
        rm -rf "$WORK"
    fi
}
trap cleanup EXIT

pass() { echo "PASS  $1"; }
fail() { echo "FAIL  $1"; FAILED=1; }

if [ "${SKIP_BUILD:-0}" != 1 ]; then
    (cd "$ROOT" && cargo build -p aphyn_editor) || exit 1
fi
EDITOR_BIN="$ROOT/target/debug/aphyn_editor"

cp -r "$SCRIPT_DIR/project" "$WORK/project"
cp "$ROOT/crates/editor/assets/fonts/Onest.ttf" "$WORK/project/assets/fonts/Onest.ttf"

# isolated editor settings (window layout) and a null audio device
export XDG_DATA_HOME="$WORK/data" XDG_CACHE_HOME="$WORK/cache"
printf 'pcm.!default { type null }\nctl.!default { type null }\n' > "$WORK/asound.conf"
export ALSA_CONFIG_PATH="/usr/share/alsa/alsa.conf:$WORK/asound.conf"

mkdir -p "$XDG_DATA_HOME/aphyn_editor"
python3 - "$XDG_DATA_HOME/aphyn_editor/windows_layout.json" <<'PY'
import json, sys
def split(axis, ratio): return {"is_split": True, "tabs": [], "active": 0, "axis": axis, "ratio": ratio}
def leaf(tabs): return {"is_split": False, "tabs": tabs, "active": 0, "axis": 0, "ratio": 0.5}
dock = [
    split(0, 0.2), split(1, 0.5), leaf(["Hierarchy Window"]), leaf(["Project Window"]),
    split(0, 0.75), split(1, 0.62), leaf(["Scene Window", "Game Window"]),
    leaf(["Animation Window", "Profiler Window"]), leaf(["Inspector Window"]),
]
json.dump({"dock": dock, "floating": []}, open(sys.argv[1], "w"), indent=2)
PY

for n in $(seq 90 120); do
    [ -e "/tmp/.X11-unix/X$n" ] || [ -e "/tmp/.X$n-lock" ] || { DISPLAY_NUM=$n; break; }
done
Xvfb ":$DISPLAY_NUM" -screen 0 1600x900x24 >/dev/null 2>&1 &
XVFB_PID=$!
export DISPLAY=":$DISPLAY_NUM"
sleep 2

LOG="$OUT/editor.log"
"$EDITOR_BIN" edit "$WORK/project" >"$LOG" 2>&1 &
EDITOR_PID=$!

WIN=""
for _ in $(seq 1 120); do
    WIN=$(xdotool search --onlyvisible --name '^aphyn' 2>/dev/null | head -1)
    [ -n "$WIN" ] && break
    sleep 1
done
[ -n "$WIN" ] || { fail "editor window did not appear (see $LOG)"; exit 1; }
xdotool windowmove "$WIN" 0 0 windowsize "$WIN" 1600 900
# the editor rebuilds the project scripts on start and again when the window gets focus; clicks before that are lost
xdotool windowfocus --sync "$WIN" 2>/dev/null
for _ in $(seq 1 90); do
    [ "$(grep -c 'scripts: building' "$LOG")" -ge 2 ] && break
    sleep 1
done
sleep 10

shot() { import -window root "$OUT/$1.png"; }
click() { xdotool mousemove "$1" "$2"; sleep 0.6; xdotool click 1; sleep "${3:-2}"; }
double_click() { xdotool mousemove "$1" "$2"; sleep 0.6; xdotool click --repeat 2 --delay 120 1; sleep "${3:-6}"; }
park_mouse() { xdotool mousemove 1590 895; sleep 1; }
alive() { kill -0 "$EDITOR_PID" 2>/dev/null; }
# number of pixels that differ between the same region of two screenshots
region_diff() {
    compare -metric AE -fuzz 10% <(convert "$OUT/$1.png" -crop "$3" +repage png:-) <(convert "$OUT/$2.png" -crop "$3" +repage png:-) null: 2>&1 |
        cut -d' ' -f1 | cut -d. -f1
}
changed() { [ "$(region_diff "$1" "$2" "$3")" -gt "${4:-1500}" ]; }
same() { [ "$(region_diff "$1" "$2" "$3")" -lt "${4:-1500}" ]; }

HIERARCHY=310x380+5+60
PROJECT=310x380+5+500
INSPECTOR=315x780+1283+70
TOOLBAR=300x30+650+28
BOTTOM=960x300+320+580
MENU=200x200+0+26

park_mouse
shot 01_start
alive && pass "editor starts" || { fail "editor exited on start (see $LOG)"; exit 1; }

# Project window: expand "scenes" (retried only while nothing changed, so it is never toggled back)
for _ in 1 2 3; do
    click 8 606
    park_mouse
    shot 02_scenes_expanded
    changed 01_start 02_scenes_expanded "$PROJECT" 200 && break
done
changed 01_start 02_scenes_expanded "$PROJECT" 200 && pass "project folder expands" || fail "project folder did not expand"

double_click 80 650
park_mouse
shot 03_scene_opened
changed 02_scenes_expanded 03_scene_opened "$HIERARCHY" && pass "double-click opens the scene in the hierarchy" ||
    fail "hierarchy did not change after double-clicking the scene"

# Hierarchy: select "Box 1Кубик" (has an AnimationComponent and a child)
click 60 164 3
park_mouse
shot 04_selected
changed 03_scene_opened 04_selected "$INSPECTOR" && pass "inspector shows the selected entity" || fail "inspector did not change on selection"
changed 03_scene_opened 04_selected "$BOTTOM" 500 && pass "animation window picks up the selected entity's clip" ||
    fail "animation window did not change on selecting an animated entity"

# Play, then Stop
click 800 41 15
park_mouse
shot 05_play
alive || { fail "editor exited in play mode (see $LOG)"; exit 1; }
changed 04_selected 05_play "$TOOLBAR" 100 && pass "play mode starts" || fail "toolbar did not switch to play mode"
changed 04_selected 05_play "$INSPECTOR" && pass "inspector keeps the selection in play mode" || fail "inspector did not change in play mode"

click 800 41 12
park_mouse
shot 06_stopped
alive || { fail "editor exited on stop (see $LOG)"; exit 1; }
same 04_selected 06_stopped "$TOOLBAR" 100 && pass "play mode stops" || fail "toolbar did not return to edit mode"
# duplicated components or a lost selection change thousands of pixels; the entity id changes only a few
DIFF=$(region_diff 04_selected 06_stopped "$INSPECTOR")
[ "$DIFF" -lt 1500 ] && pass "inspector after stop matches the one before play ($DIFF px differ)" ||
    fail "inspector after stop differs from the one before play ($DIFF px differ)"

# Bottom panel: switching to the Profiler tab must not move the window
click 445 595 3
park_mouse
shot 07_profiler_tab
python3 - "$XDG_DATA_HOME/aphyn_editor/windows_layout.json" <<'PY_LAYOUT' && pass "tab click switches the tab, layout unchanged" || fail "tab click changed the dock layout (see 07_profiler_tab.png)"
import json, sys
layout = json.load(open(sys.argv[1]))
leaf = next(n for n in layout["dock"] if "Profiler Window" in n["tabs"])
sys.exit(not (layout["floating"] == [] and leaf["tabs"] == ["Animation Window", "Profiler Window"] and leaf["active"] == 1))
PY_LAYOUT

# Windows menu opens and closes
click 36 12
shot 08_windows_menu
changed 07_profiler_tab 08_windows_menu "$MENU" 500 && pass "windows menu opens" || fail "windows menu did not open"
click 1590 895
park_mouse
shot 09_menu_closed
same 07_profiler_tab 09_menu_closed "$MENU" 500 && pass "windows menu closes on an outside click" || fail "windows menu stayed open"

if grep -q 'panicked' "$LOG"; then fail "panic in the editor log (see $LOG)"; else pass "no panics in the editor log"; fi
alive && pass "editor still running at the end" || fail "editor exited (see $LOG)"

echo
echo "screenshots and log: $OUT"
if [ "${KEEP_OPEN:-0}" = 1 ]; then
    echo "editor left running on DISPLAY=$DISPLAY (pid $EDITOR_PID), project copy in $WORK/project"
fi
exit $FAILED
