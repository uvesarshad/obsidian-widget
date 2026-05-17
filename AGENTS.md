# Agent Instructions — ob-widget

## Start here
Read docs/overview.md before doing anything else.
It contains the full mental model: stack, architecture, data flow,
module map, and glossary.

## Documentation index
docs/overview.md lists every doc file and what it covers.
Navigate from there. Do not rely on memory or assumptions.

## Before every task
1. Read docs/overview.md
2. Read the relevant module doc in docs/modules/ if one exists
3. Make the change
4. Run the update decision tree in the docs (AGENT UPDATE tags)
5. Output a DOCS UPDATED summary before marking the task complete

## Hard rules
- Never invent file paths, component names, or type names.
  Always verify against the actual codebase.
- Never add a new Tauri command without updating
  docs/api/server-actions.md and registering it in the
  invoke_handler macro at the bottom of src-tauri/src/lib.rs.
- Never add an OS-level integration (filesystem, dialog, notification,
  shortcut, window effect) without updating
  docs/api/external-services.md and the relevant capability entry in
  src-tauri/capabilities/default.json.
- Never modify the Settings struct in src-tauri/src/lib.rs without
  mirroring the field in src/lib/types.ts and adding a serde default
  so older settings.json files continue to load.
- Never add an environment variable — this app uses settings.json
  and reminders.json under the OS app-data directory instead.
- If a docs/ file would exceed 200 lines after your update, split it
  and update docs/overview.md to list both parts.

## Docs update tags
Throughout the /docs files you will find:
  AGENT NOTE:  — constraint you must follow
  AGENT SEE:   — cross-reference to read
  AGENT AVOID: — anti-pattern to skip
  AGENT UPDATE: — doc files to update when this area changes

## Stack summary
Tauri 2 (Rust 2021) backend with SvelteKit 2 + Svelte 5 runes
frontend in SPA mode. Filesystem persistence (settings.json,
reminders.json, user-selected markdown file). Native notify-crate
file watcher, background reminder scheduler thread, OS notifications
via tauri-plugin-notification, window-vibrancy for acrylic/vibrancy,
Web Audio for built-in tones, HTMLAudioElement for custom sounds.

## Key paths
- src-tauri/src/lib.rs — all Rust logic, every Tauri command
- src-tauri/tauri.conf.json — window/bundle/CSP config
- src-tauri/capabilities/default.json — ACL permissions
- src/routes/+page.svelte — single-page widget shell
- src/lib/NoteView.svelte — markdown rendering
- src/lib/ReminderPicker.svelte — calendar + time picker
- src/lib/types.ts — TS mirror of Rust Settings / Task / Reminder
- docs/ — this documentation set; start at docs/overview.md
