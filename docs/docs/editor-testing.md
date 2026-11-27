---
title: Editor Testing
description: Automated tests, the editor smoke test and the manual checklist for aphyn_editor changes.
---

# Editor Testing

Changes to the editor or to the crates it draws with (`aphyn_ui`, `aphyn_render*`, `aphyn_app`) are checked in three layers:

1. **Automated tests** with `cargo test`. Fast, run them always.
2. **Smoke test**: a Linux script that launches the real editor on a virtual display, clicks through the main scenarios and compares screenshots.
3. **Manual checklist** for what the script does not cover: text input, drag and drop, animation editing, visual quality.

## Automated tests

```bash
cargo test --workspace --no-fail-fast
```

Most tests are doctests in the crate-root `//!` docs. Unit tests live next to the code they cover, for example the font atlas tests in `crates/ui/src/assets.rs` and the dock tree tests in `crates/editor/src/features/docking.rs`. To run one doctest:

```bash
cargo test -p aphyn_ecs --doc <name_filter>
```

Logic that does not need a window, such as layout math, text measuring or tree operations, belongs in a unit test.

## Smoke test

`.github/scripts/editor-smoke/run.sh` builds `aphyn_editor`, starts it with the fixture project from `.github/scripts/editor-smoke/project/` and checks:

| Step | Check |
| --- | --- |
| Start | the editor window appears and the process stays alive |
| Project window | a folder expands, double-clicking `scenes/main.asset` fills the hierarchy |
| Hierarchy | selecting an animated entity updates the inspector and the Animation window |
| Play | the toolbar switches to play mode, the inspector keeps the selection |
| Stop | the toolbar returns to edit mode, the inspector matches the one before Play (no duplicated components, same selection) |
| Tabs | a quick click on the Profiler tab switches the tab without changing the dock layout |
| Menu | the Windows menu opens and closes on an outside click |
| Log | no panics in the editor output |

Requirements: Linux, `Xvfb`, `xdotool`, ImageMagick (`import`, `convert`, `compare`) and `python3`. A GPU is not needed; Mesa's software renderer is enough.

```bash
.github/scripts/editor-smoke/run.sh
```

Each step prints `PASS` or `FAIL`, and the script exits with a non-zero code if any step failed. Screenshots and `editor.log` go to `target/editor-smoke/`, or to the directory passed as the first argument. Options:

- `SKIP_BUILD=1` reuses `target/debug/aphyn_editor`.
- `KEEP_OPEN=1` leaves the editor running on the virtual display for further manual checks.

The script runs on its own X display and sets `XDG_DATA_HOME` / `XDG_CACHE_HOME` to a temporary directory, so it does not touch the saved window layout of your editor. Audio goes to an ALSA null device.

Clicks target fixed coordinates in a 1600×900 window with a fixed dock layout. When a change moves panels, rows or buttons, update the coordinates in `run.sh` and check the screenshots once by eye. Checks compare screenshot regions before and after each action, so a click that misses its target fails instead of passing silently.

## Manual checklist

Run these with the fixture project (copy `.github/scripts/editor-smoke/project/` somewhere and add `assets/fonts/Onest.ttf` from `crates/editor/assets/fonts/`) or any project with a scene, a camera and an animated entity. Click both slowly and quickly: a press and release within one frame is a separate case.

### Windows and docking

- [ ] Clicking a tab switches to it; the window does not move or undock.
- [ ] Dragging a tab out makes a floating window; dropping it on a dock edge or another window's center docks it back, with a highlight before the drop.
- [ ] Hovering a tab shows the close button; closing and reopening a window through the Windows menu works.
- [ ] Shrinking and enlarging the main window keeps floating windows inside the dock area.
- [ ] The window layout is restored after restarting the editor.

### Project and hierarchy

- [ ] Folders expand and collapse with the arrow; double-clicking a scene opens it and marks it `open`.
- [ ] The hierarchy shows the opened asset in its source row and entity ids on the right; foldout arrows expand children.
- [ ] Context menus open on right click and close on an outside click.

### Inspector

- [ ] The header shows the entity name and id; components are listed once, without the `aphyn::` prefix.
- [ ] Editing a number, a vector component and an enum dropdown changes the entity (visible in the Scene view).
- [ ] `+ Add component…` filters suggestions as you type and adds the chosen component.
- [ ] Text fields accept Cyrillic; clicking places the caret between the right characters; dragging selects text.

### Play mode

- [ ] Play switches to the Game view, tints the top bar and shows the inspector banner about unsaved edits.
- [ ] Stop returns to edit mode with the same entity selected and no duplicated components. Repeat after opening another scene.

### Animation

- [ ] Selecting an entity with an `AnimationComponent` shows its clip tracks.
- [ ] Preview and Play move the scrubber; Record shows `recording edits`, and edits in the inspector or with the gizmo add keys.
- [ ] Dope sheet and Curves views switch; selecting keys on another track does not shift the curve view.

### Profiler

- [ ] Record, then Play: the frame count grows; the histogram is clipped at 50 ms and colored by frame budget.
- [ ] Selecting a frame shows the ruler and threads; clicking a segment shows its details.

### Scene view

- [ ] Right mouse button looks around, WASD flies; double-clicking an entity in the hierarchy focuses it.
- [ ] Move, Rotate and Scale gizmos edit the selected entity; the Light panel switches apply.

### Fonts and text

- [ ] Text is crisp at 100% scale; Cyrillic, punctuation and arrows render without missing-glyph boxes.
- [ ] A font asset whose `raw_font` file is missing logs an error instead of panicking.

## When a bug is found

Fix it with a test where possible: a unit test for logic, or a new step in the smoke test for an interaction. Make sure the test fails without the fix and passes with it.
