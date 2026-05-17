# Tauri Commands (Server Actions)

> Scope: Every Rust function exposed to the Svelte frontend via invoke.
> Rendering context: Server (Rust)
> Project tier: 3
> Last updated: 2026-05-17

## Overview
There are 16 Tauri commands defined in src-tauri/src/lib.rs and
registered in the `tauri::generate_handler!` macro at the bottom of
the `run()` function. The frontend calls them with `invoke(name,
args)` from `@tauri-apps/api/core`. Argument names use snake_case
on the Rust side; Tauri automatically accepts both snake_case and
camelCase from JS, but we prefer to keep JS callsites camelCase
for the parameter object.

## Settings
- **get_settings()** → `Settings`. Returns the current in-memory
  Settings clone. No side effects.
- **save_settings({ settings })** → unit. Replaces the in-memory
  Settings, persists to disk, and restarts the file watcher if
  vault_path or target_file changed.

AGENT NOTE: save_settings does NOT emit `settings-changed`. The
caller already has the new value because it constructs the
payload — emitting would cause a redundant round-trip. Only
internal pickers (pick_task_file, pick_reminder_sound) and the
tray menu emit `settings-changed`.

## Notes & tasks
- **read_tasks()** → `Result<Vec<Task>, String>`. Parses the target
  file into a flat task list. Capped at 1 MB.
- **write_tasks({ tasks })** → `Result<(), String>`. Rewrites only
  task lines (matched by line_idx) preserving every other line.
  Sets ignore_change before writing.
- **add_task({ text })** → `Result<(), String>`. Inserts a new
  `- [ ] <text>` line before the first existing task, or appends
  if none exist. Newlines in `text` are collapsed to spaces.
- **delete_task({ lineIdx })** → `Result<(), String>`. Removes the
  given line from the file (preserves trailing newline if present).
- **read_note()** → `Result<Vec<NoteItem>, String>`. Returns the
  full parsed note (tasks + headings + bullets + separators + text).
  Capped at 1 MB.

## Window & opacity
- **set_opacity({ opacity })** → `Result<(), String>`. Clamps to
  0.1–1.0, persists. The frontend writes the CSS var `--bg-alpha`
  separately; this command only persists.
- **set_window_blur({ enabled })** → `Result<(), String>`.
  Enabled=true → apply_acrylic / apply_vibrancy. Enabled=false →
  clear_acrylic / clear_vibrancy. Called by the reminder fire/dismiss
  path so pulse rings travel through real transparency.

## Shortcuts
- **update_shortcut({ shortcut })** → `Result<(), String>`.
  Unregisters the old global shortcut, registers the new accelerator
  string. Returns an error if the accelerator is invalid; the
  frontend surfaces this via the shortcut popover's error label.

## Reminders
- **add_reminder({ taskText, remindAt })** → `Result<String, String>`.
  Validates `remind_at` via parse_remind_at (rejects unparseable
  values), assigns a unique id via generate_id (nanosecond timestamp +
  AtomicU64 counter), pushes to AppState.reminders, prunes fired
  entries beyond MAX_FIRED_REMINDERS (20), persists reminders.json.
- **get_reminders()** → `Vec<Reminder>`. Returns the full list
  (including fired ones).
- **delete_reminder({ id })** → `Result<(), String>`. Removes a
  reminder by id and prunes. Not currently called from the frontend
  but exposed for future use.

AGENT NOTE: add_reminder returns the new id as the Ok value. The
frontend currently ignores it but the API is stable — keep returning
the id for callers that may want to update or delete it later.

## File pickers
- **pick_task_file()** → unit (callback-based). Opens a Markdown
  file dialog. On selection, derives vault_path from the parent dir,
  target_file from the file name, persists, restarts the watcher,
  and emits `settings-changed`.
- **pick_reminder_sound()** → unit (callback-based). Opens an audio
  file dialog (mp3/wav/ogg/m4a/flac/aac). On selection, stores the
  path in Settings.reminder_tone_path, switches Settings.reminder_tone
  to "custom", persists, and emits `settings-changed`.

AGENT AVOID: Do not await pick_task_file or pick_reminder_sound in
the frontend expecting a return value. They are fire-and-forget;
the chosen path comes back via the `settings-changed` event.

## Custom sound
- **read_reminder_sound()** → `Result<Vec<u8>, String>`. Reads the
  bytes of Settings.reminder_tone_path. Capped at 5 MB. The
  frontend wraps the result in `new Uint8Array(...)` → Blob → Blob URL
  → HTMLAudioElement.

AGENT NOTE: The 5 MB cap exists because Vec<u8> is sent over IPC as
a JSON number array (one element per byte). Raising the cap will
make first-play latency proportionally worse. If a larger cap is
needed, migrate this command to `tauri::ipc::Response::new(bytes)`
instead of bumping the limit.

## Argument naming at the IPC boundary
- Rust signature: `task_text: String, remind_at: String`.
- JS invoke payload: `{ taskText: '...', remindAt: '...' }` works.
  `{ task_text, remind_at }` also works because Tauri accepts both
  conventions. We use camelCase at the JS callsite for consistency
  with TS code style.

## Events emitted from Rust
- **`file-changed`** — payload: () unit. Emitted by the notify
  watcher callback when Modify/Create fires and ignore_change is
  not set. Triggers `read_note` on the frontend.
- **`settings-changed`** — payload: Settings. Emitted by tray menu
  handlers and the two pickers when Settings is mutated server-side.
  The frontend listener mirrors the change into `settings` and may
  invalidate the cached custom audio.
- **`reminder-fired`** — payload: String (the task text without the
  (@...) tag). Emitted by check_and_fire_reminders for each due
  reminder. Triggers handleReminderFired on the frontend.

## Update Triggers
- A new command is added or registered. (Add it under the relevant
  heading AND in the `tauri::generate_handler!` list.)
- A command's argument shape or return type changes.
- A new event is emitted from Rust.
- An existing command's side effects change (e.g. starts emitting
  `settings-changed`).
- The MAX_FIRED_REMINDERS cap or any other invariant constant changes.

AGENT UPDATE: this file, on any of the above. Also update
docs/architecture/data-flow.md if the change affects a documented
flow, and docs/modules/<feature>.md for the owning feature.

## Related Docs
- docs/architecture/data-flow.md — how these commands chain together.
- docs/api/external-services.md — the plugins behind the commands.
- docs/state/client-state.md — frontend state these commands mutate.
