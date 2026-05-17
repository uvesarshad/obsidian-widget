# Theming

> Scope: Colours, transparency, light/dark, window vibrancy.
> Rendering context: Client
> Project tier: 3
> Last updated: 2026-05-17

## Overview
The widget has no design-token system or external theme library. All
visual concerns are handled with plain CSS: the `light-dark()` CSS
function gates light/dark colour pairs, a `data-theme` attribute on
the documentElement forces a specific mode, and the `--bg-alpha`
custom property controls the widget's background opacity. The
underlying frosted-glass look on Windows and macOS comes from the
window-vibrancy Rust crate, not CSS.

## Files involved
- src/app.html — global reset, transparent body/html, system font
  stack (-apple-system, BlinkMacSystemFont, "Segoe UI", system-ui).
- src/routes/+page.svelte — `.widget` background, theme attribute
  switching, `--bg-alpha` writes, all firing-state animations.
- src/lib/NoteView.svelte — local component styles (tasks, headings,
  bullets, separators, reminder chip).
- src/lib/ReminderPicker.svelte — picker, calendar, time inputs.
- src-tauri/src/lib.rs — apply_window_effects / clear_window_effects /
  set_window_blur command (the platform vibrancy layer).

## Theme attribute
- `:global(:root) { color-scheme: light dark; }` — system default.
- `:global(:root[data-theme="light"]) { color-scheme: light; }`
- `:global(:root[data-theme="dark"]) { color-scheme: dark; }`

The Svelte function `applyTheme(theme)` in +page.svelte sets or
removes the `data-theme` attribute on the documentElement. The
sun/moon/half-disc icon in the header cycles `system → light → dark`
via `cycleTheme()`. The chosen theme persists in settings.json as
`Settings.theme`.

AGENT NOTE: Use `light-dark(lightColor, darkColor)` for every new
colour pair. Do not branch on the documentElement attribute from
JavaScript — let CSS handle the theming reactively.

## Opacity
- The `.widget` background uses
  `rgb(255 255 255 / var(--bg-alpha, 0.85))` in light and
  `rgb(28 28 30 / var(--bg-alpha, 0.75))` in dark.
- The opacity slider popover (header) writes `--bg-alpha` to
  `document.documentElement.style` via `handleOpacityChange`, then
  calls `invoke('set_opacity', { opacity })`.
- Persistence: Settings.opacity is f64 in 0.1–1.0 (clamped Rust-side).

## Window vibrancy (frosted glass)
- Windows: `apply_acrylic(window, Some((0,0,0,0)))` from
  window-vibrancy. Cleared by `clear_acrylic(window)`.
- macOS: `apply_vibrancy(window, NSVisualEffectMaterial::HudWindow,
  None, None)`. Cleared by `clear_vibrancy(window)`.
- Toggled at runtime by the `set_window_blur(enabled)` Tauri command.
  The reminder fire flow turns it OFF so the pulse rings travel
  through real transparency; dismiss/snooze turns it back ON.

AGENT NOTE: window-vibrancy is OS-level; it composites BELOW the
webview content. CSS `backdrop-filter` does not help — toggling
vibrancy on the window is the only way to get a truly transparent
margin around the widget.

## Colour anchors
- Light mode widget body: `#1a1a1a` text on translucent white.
- Dark mode widget body: `#e8e8e8` text on translucent `#1c1c1e`.
- Accent (reminder alert, picker confirm, reminder chip): `#007aff`
  light / `#0a84ff` dark — same hue as the macOS / iOS system blue.
- Alarm pulse rings + inner widget glow: blue
  `rgba(10, 132, 255, 0.85)` for the ring, with an inset glow on
  the widget at `rgba(10, 132, 255, 0.28 → 0.55)` pulse range.

The reminder badge below the new-task input uses
`rgba(0, 122, 255, 0.1)` background and `rgba(0, 122, 255, 0.3)` border.

## Type scale
- Header title: 11.5 px, uppercase, 0.04 em letter-spacing, opacity 0.65.
- Task text: 12.5 px, line-height 1.4.
- Bullets: 12 px, opacity 0.75.
- Headings: h1 14 px → h6 12 px with decreasing opacity 0.9 → 0.6.
- Plain text lines: 12 px, opacity 0.5.
- Reminder chip: 9 px, italic-leaning, accent colour.
- Picker month label: 10 px bold.

## Spacing scale
- The widget body uses a tight scale: gaps of 2–7 px between rows,
  4–10 px padding inside popovers, 6–10 px header padding.
- Radii: 10 px on the widget shell, 8 px on popovers, 4–5 px on
  buttons, 50% on the selected calendar day.

## Update Triggers
- A new colour pair is introduced (then anchor it in Colour anchors).
- The opacity slider range changes.
- A new theme mode is added beyond system / light / dark.
- The vibrancy material changes on either platform.
- The default font stack in src/app.html changes.

AGENT UPDATE: this file, on any of the above. Also update
docs/modules/window-chrome.md if vibrancy or the theme toggle's
location moves.

## Related Docs
- docs/modules/window-chrome.md — theme button, opacity popover, tray.
- docs/api/server-actions.md — set_opacity, set_window_blur.
- docs/ui/component-library.md — per-component style choices.
