# Folder Structure

> Scope: Top-level layout of the repo and what each folder owns.
> Rendering context: N/A
> Project tier: 3
> Last updated: 2026-05-17

## Overview
ob-widget is split into a Rust crate (src-tauri/) and a SvelteKit
frontend (src/). The Rust side owns persistence and OS access. The
SvelteKit side owns rendering and user input. Build configuration
lives at the repo root.

## Top-level folders and files
- src/ — SvelteKit frontend (client-side, SPA mode). Contains
  routes/, lib/, app.html, static/.
- src-tauri/ — Rust backend crate (server-side from the webview's
  perspective). Owns all Tauri commands, the filesystem watcher,
  the reminder scheduler, the system tray, and OS effects.
- docs/ — Documentation tree consumed by AI agents (this set).
- temp/ — Local scratch (PRD.md, TODO.md). Not part of the build.
- .github/workflows/ — CI release workflow (release.yml).
- .claude/, .vscode/ — IDE/agent configuration. Not part of the build.

## src/ layout
- src/app.html — Root HTML shell. Hosts the global reset, transparent
  background, hidden overflow, and the system font stack. Framework
  special file (SvelteKit template).
- src/routes/+layout.ts — Disables SSR (`export const ssr = false`)
  and prerender. Framework special file.
- src/routes/+page.svelte — The single root page. Owns the widget
  shell, header, reminder picker, pulse layer, alert panel, sound
  popover, and global event wiring. Framework special file.
- src/lib/NoteView.svelte — Renders parsed NoteItem[] (tasks,
  headings, bullets, separators, text).
- src/lib/ReminderPicker.svelte — Calendar + time picker shown when
  the user types `@` in the new-task input.
- src/lib/TaskList.svelte — Legacy task list component, no longer
  the primary renderer. Kept for reference, not in the runtime path.
- src/lib/types.ts — Shared TypeScript types mirroring the Rust
  Settings / Task / NoteItem / Reminder shapes.

AGENT AVOID: Do not add new code to src/lib/TaskList.svelte. NoteView
is the live renderer. If you find yourself editing TaskList, you are
probably in the wrong file.

## src-tauri/ layout
- src-tauri/src/main.rs — Trivial entry; defers to ob_widget_lib::run.
- src-tauri/src/lib.rs — Everything else: types, helpers, all Tauri
  commands, scheduler, watcher, tray, window event handlers.
- src-tauri/Cargo.toml — Rust dependencies (tauri, notify, chrono,
  window-vibrancy, the four tauri-plugin-* crates).
- src-tauri/tauri.conf.json — Window definition (frameless,
  transparent, alwaysOnTop default, 320×480), bundle identifier
  (`com.uves.ob-widget`), CSP, build commands.
- src-tauri/capabilities/default.json — ACL permissions for the
  main window: dialog, window control, global shortcut, notification.
- src-tauri/icons/ — App icons in every required format.
- src-tauri/gen/schemas/ — Auto-generated ACL schemas. Do not edit.
- src-tauri/build.rs — Build hook used by tauri-build.

AGENT NOTE: Both src-tauri/src/main.rs and the existing #[cfg_attr(mobile, ...)]
attribute on `pub fn run()` exist to support a future mobile target.
Do not remove them when refactoring the entry point.

## Repo-root build and config files
- package.json — Node scripts: `dev`, `build`, `preview`, `check`, `tauri`.
- package-lock.json — Frozen Node deps.
- svelte.config.js — Uses adapter-static; SPA fallback configured.
- vite.config.js — Dev server on port 1420 (matches devUrl in tauri.conf.json).
- tsconfig.json — TS strict mode.
- AGENTS.md — Agent bootstrap. Always at repo root.
- README.md — Human-facing intro plus the Documentation pointer block.

## Naming conventions
- Svelte files: PascalCase (NoteView.svelte, ReminderPicker.svelte).
- Framework special files in src/routes: lowercase with leading `+`
  (`+page.svelte`, `+layout.ts`).
- Rust functions: snake_case. Tauri commands keep snake_case at the
  Rust boundary and Tauri auto-maps to JS callers using snake_case
  too (e.g. invoke('add_reminder', { taskText, remindAt })).
- JSON keys in settings.json and reminders.json: snake_case to match
  the Rust struct field names — required for serde to round-trip
  cleanly.
- TypeScript field names mirror the Rust struct names exactly
  (snake_case) — see src/lib/types.ts.

AGENT NOTE: Do not "fix" snake_case to camelCase in the shared
Settings / Task / NoteItem / Reminder shapes. The serde round-trip
between settings.json and the Settings struct relies on identical
key names.

## Co-located vs. shared
- src/routes/+page.svelte is co-located with its CSS and scripts in
  the same file (Svelte single-file component convention).
- All reusable components live in src/lib/. Anything in src/lib/ is
  importable via the `$lib` alias.
- Rust is monolithic in src-tauri/src/lib.rs (≈ 1000 lines). When it
  reaches a point where splitting helps, the conventional Tauri
  layout is `src/commands/`, `src/state/`, `src/watcher/`. Do not
  split prematurely.

## Update Triggers
- A folder is added, renamed, or removed.
- A new file is added directly under src/ or src-tauri/src/.
- Naming conventions change (e.g. someone introduces a camelCase
  Settings field).
- src/lib/TaskList.svelte is finally deleted (then remove its
  reference here).

AGENT UPDATE: this file, on any of the above. Also update
docs/overview.md if a top-level folder moves.

## Related Docs
- docs/overview.md — top-level mental model.
- docs/architecture/rendering-strategy.md — why SSR is off.
- docs/api/server-actions.md — what the Rust side exposes.
