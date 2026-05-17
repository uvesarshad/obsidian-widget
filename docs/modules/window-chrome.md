# Module: Window Chrome

> Scope: Frameless window, tray, opacity, theme, always-on-top, global shortcut.
> Rendering context: Isomorphic
> Project tier: 3
> Last updated: 2026-05-17

## Overview
This module owns everything that is "around" the task list rather
than "in" it: the floating frameless window, the system tray, the
header buttons (opacity, sound, theme, always-on-top, bring-to-top),
the resize handles, the click-through option, and the platform
vibrancy that makes the widget look frosted.

## Entry points
- Window config: src-tauri/tauri.conf.json — declares the main
  window (320×480, decorations: false, transparent: true,
  alwaysOnTop: true, skipTaskbar: true, resizable: true).
- Window effects: apply_window_effects / clear_window_effects /
  set_window_blur in src-tauri/src/lib.rs.
- Tray: TrayIconBuilder block inside `run()` in src-tauri/src/lib.rs.
- Frontend chrome: header + popovers + resize handles inside
  src/routes/+page.svelte.

## Frameless window mechanics
- `decorations: false` removes the OS title bar.
- The `.header` row carries `data-tauri-drag-region` so the user can
  drag the widget by its top edge. Interactive children inside the
  header explicitly opt out with `data-tauri-drag-region="false"`.
- `.rh-*` divs at all eight edges and corners call
  `(appWindow as any).startResizeDragging(direction)` on mousedown
  to drive native edge-drag resize.

AGENT NOTE: Adding any new clickable element inside the header MUST
set `data-tauri-drag-region="false"` on the element, or the click
will be swallowed by the drag handler.

## Header buttons (left → right)
1. Title (read from Settings.target_file with .md stripped).
2. Task counter (`pendingTasks.length / taskItems.length`).
3. **Opacity** — opens the slider popover. Writes `--bg-alpha` and
   calls `set_opacity` on input.
4. **Sound (bell)** — opens the tone picker popover. Lists the five
   built-in tones + Custom row + Browse…/Change…. See
   docs/modules/reminders.md.
5. **Theme** — cycles `system → light → dark` via cycleTheme.
   Persisted as Settings.theme.
6. **Always-on-top (pin)** — toggles Settings.always_on_top and calls
   `appWindow.setAlwaysOnTop(newVal)`.
7. **Bring-to-top (arrow)** — single click brings the window forward
   once. Right-click opens the shortcut recorder popover. Default
   accelerator is `CmdOrControl+Shift+O`.

All popovers dismiss on outside click via `handleGlobalClick`, which
runs from `<svelte:window onclick>`.

## Window vibrancy lifecycle
- On startup: `apply_window_effects(&window)` is called once in the
  setup closure of `run()`.
- During a reminder fire: the frontend calls
  `invoke('set_window_blur', { enabled: false })` to clear it.
- On dismiss/snooze: re-applied via `set_window_blur({ enabled: true })`.

AGENT AVOID: Do not move `apply_window_effects` outside the setup
closure. It must run after the window exists but before the first
frame paint, otherwise the user sees a flash of opaque background.

## System tray
Built in the `TrayIconBuilder::with_id("main")` block. Menu items:
- **Choose Note File…** — opens the markdown file dialog
  (duplicates the body of pick_task_file; both paths exist for
  convenience).
- **Always on Top** ✓ — toggles Settings.always_on_top.
- **Click-through** — toggles Settings.click_through_on_blur.
  Persists via persist_settings but the OS click-through behaviour
  is not yet wired (see Known edge cases).
- **Launch at Login** ✓ — calls `app.autolaunch().enable()` /
  `.disable()` via tauri-plugin-autostart.
- **Quit** — saves window position/size into Settings.window, then
  `app.exit(0)`.

Left-click on the tray icon toggles window visibility:
`is_visible` → `hide`; otherwise `show` + `set_focus`.

## Global shortcut
- Default `CmdOrControl+Shift+O`, persisted as Settings.shortcut.
- The plugin handler shows + focuses the window. If the user's
  saved always_on_top is false, it sets a temporary AOT override
  and flips `hotkey_aot_override` so the next blur event restores
  the original AOT setting.
- The shortcut popover (right-click on bring-to-top button) records
  a new accelerator via keyboard input and calls `update_shortcut`.

AGENT NOTE: Tooltip on the tray icon currently hard-codes
"CmdOrCtrl+Shift+O". If you change the default shortcut, update
both `default_shortcut()` in src-tauri/src/lib.rs and that tooltip
string.

## Window persistence
- On `CloseRequested`: the X button hides instead of closing.
  Before hiding, the current outer position and inner size are
  written into Settings.window and persisted.
- On `Quit` (tray): same window-state save, then `app.exit(0)`.
- On startup: settings.window is read and applied via `set_size` and
  `set_position` before `apply_window_effects` runs.

## Theme attribute
- `applyTheme(theme)` sets / clears `data-theme` on
  documentElement. CSS uses `light-dark()` to react automatically.
- The chosen theme is mirrored into Settings.theme on every cycle.

## Known edge cases
- Click-through (Settings.click_through_on_blur) is persisted but
  the actual OS-level click-through is not yet wired on either
  platform. The tray menu's check state reflects the saved value
  but the widget continues to receive mouse events. TODO if this
  becomes a priority.
- The tray's "Choose Note File…" handler duplicates the body of
  pick_task_file. Both work; if either is changed, mirror to the
  other or extract a shared helper.

AGENT UPDATE: docs/modules/window-chrome.md (this file) when click-
through is finally wired; also update the tray menu item
description in this file.

## Update Triggers
- A new header button or popover is added.
- A new tray menu item is added.
- The window default size, decorations, transparency, or skipTaskbar
  setting changes in tauri.conf.json.
- Click-through is finally wired into the OS layer.
- The hotkey override behaviour (hotkey_aot_override) changes.
- The shortcut popover's accelerator parsing rules change.

AGENT UPDATE: this file, on any of the above. Also update
docs/api/server-actions.md (set_opacity, update_shortcut,
set_window_blur), docs/api/external-services.md (tauri-plugin-
global-shortcut, tauri-plugin-autostart), and docs/ui/theming.md
if the colour/spacing changes.

## Related Docs
- docs/api/server-actions.md — set_opacity, update_shortcut, set_window_blur.
- docs/api/external-services.md — global-shortcut, autostart plugins.
- docs/ui/theming.md — light-dark, vibrancy, opacity.
- docs/infra/deployment.md — autostart, install/quit lifecycle.
