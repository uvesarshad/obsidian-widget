# Data Flow

> Scope: How data moves from disk → Rust → Svelte → user and back.
> Rendering context: Isomorphic (covers both sides of the IPC boundary)
> Project tier: 3
> Last updated: 2026-05-17

## Overview
There are three persistent data sources on disk and three transient
in-memory caches. Everything that the user sees is produced by
reading the user-chosen markdown file. Everything the user changes
flows back to that file through Rust commands. Reminders and settings
are stored as parallel JSON files but cross-reference task text.

## Persistent data sources
1. The target markdown file (e.g. <vault>/Tasks.md). Source of truth
   for all task text. Bounded at 1 MB on read.
2. settings.json in the OS app-data directory. Owns window geometry,
   theme, opacity, shortcut accelerator, reminder tone, custom-tone
   path, vault/target choice.
3. reminders.json in the OS app-data directory. Owns the list of
   {id, task_text, remind_at, fired} records. Pruned to keep at most
   20 `fired: true` entries plus all pending.

## In-memory state
- AppState (Rust): Mutex<Settings>, Mutex<Vec<Reminder>>,
  Mutex<Option<RecommendedWatcher>>, Arc<Mutex<bool>> (ignore_change),
  Arc<Mutex<bool>> (hotkey_aot_override).
- noteItems (Svelte $state in src/routes/+page.svelte): parsed
  NoteItem[] mirroring the current markdown content.
- settings (Svelte $state): mirror of the Rust Settings struct.
- audioCtx + customAudio + customAudioBlobUrl: cached Web Audio
  context and decoded custom tone for the current
  reminder_tone_path.

## Read flow — startup
1. main.rs calls ob_widget_lib::run.
2. run() invokes load_settings(app) → reads settings.json, normalises
   reminder_tone, returns Settings.
3. run() invokes load_reminders(app) → reads reminders.json.
4. AppState is built and `app.manage(state)` registers it.
5. start_watcher() is started if vault is configured.
6. start_reminder_scheduler() spawns a std::thread that calls
   check_and_fire_reminders immediately, then every 30 s.
7. Tray is built, global shortcut is registered, window effects
   (acrylic on Windows, vibrancy on macOS) are applied.
8. Frontend mounts +page.svelte → calls get_settings, then read_note,
   then registers listeners for `file-changed`, `settings-changed`,
   and `reminder-fired`.

## Read flow — user opens widget after edits in Obsidian
1. Obsidian writes to <vault>/Tasks.md.
2. Native notify watcher fires Modify/Create.
3. Watcher callback checks `ignore_change`; if set (we just wrote it
   ourselves), clears the flag and returns. Otherwise emits the
   Tauri event `file-changed`.
4. Frontend listener calls invoke('read_note').
5. Rust parses with parse_note_lines() and returns NoteItem[].
6. Svelte assigns to `noteItems`, NoteView.svelte re-renders.

AGENT NOTE: The `ignore_change` flag is the only thing preventing
the write-then-watch feedback loop. Every Rust command that writes
the markdown file MUST set `*state.ignore_change.lock().unwrap() = true`
immediately before std::fs::write.

## Write flow — user toggles a checkbox
1. User clicks checkbox → handleToggle in +page.svelte.
2. Frontend updates noteItems optimistically.
3. invoke('write_tasks', { tasks }) → Rust reconstruct_file() patches
   only task lines by line_idx, preserving headings/bullets/blank
   lines verbatim.
4. Rust sets `ignore_change = true`, writes the file.
5. The watcher fires, sees ignore_change set, swallows the event.
6. No round-trip to refresh — frontend already shows the new state.

## Write flow — user adds a task with a reminder
1. User types text + `@` in the new-task input.
2. handleTaskInput detects `@` (InputEvent.data === '@'), sets
   showReminderPicker = true.
3. ReminderPicker emits onconfirm(date).
4. onReminderConfirm strips the trailing `@` from newTaskText, sets
   pendingReminderDate.
5. User presses Enter → handleAddTask:
   - Builds noteText = `${text} (@${formatReminderDate(date)})`.
   - invoke('add_task', { text: noteText }) → Rust prepends the line
     before the first task or appends if none exists.
   - invoke('add_reminder', { taskText: text, remindAt: ... }) → Rust
     validates remind_at via parse_remind_at, pushes to reminders,
     prunes, saves.
6. The watcher round-trip refreshes noteItems via `file-changed`.

## Fire flow — a reminder becomes due
1. The scheduler thread (every 30 s) calls check_and_fire_reminders.
2. Locks reminders, filters where `!fired && now >= remind_at`,
   collects (id, task_text).
3. Brings window forward (unminimize, show, set_always_on_top(true),
   set_focus).
4. For each due reminder: posts the OS notification (best-effort —
   may silently fail in dev without an AUMID) and emits the Tauri
   event `reminder-fired` with the task text.
5. Marks the fired ids, prunes old fired entries to MAX_FIRED_REMINDERS
   (20), saves reminders.json.
6. Frontend listener handleReminderFired:
   - invoke('set_window_blur', { enabled: false }) → clears acrylic.
   - show + setAlwaysOnTop(true) + setFocus on the window.
   - Sets isReminderFiring + firingTaskText.
   - startSoundLoop(toneId) — Web Audio for built-ins, looping
     HTMLAudioElement for custom.
   - Schedules a 5-minute auto-dismiss timer.
7. User clicks Okay → dismissReminder restores acrylic, stops sound,
   clears state, restores user's always_on_top preference.
8. User clicks Snooze → snoozeReminder rewrites the matching task's
   `(@...)` tag via write_tasks, then add_reminder for +10 min, then
   dismissReminder.

AGENT AVOID: Do not introduce a polling loop in the frontend for
reminders. The scheduler thread + reminder-fired event are the
single source of truth for "fire now" decisions.

## Serialization boundaries
- Rust → JSON (settings.json, reminders.json): serde with `#[serde(default = ...)]`
  on every newer field so old files load. AGENT NOTE: always add a
  default fn when introducing a new Settings or Reminder field.
- Rust → Tauri IPC → JS: NoteItem, Task, Settings, Reminder are
  serialized as JSON with snake_case keys preserved. src/lib/types.ts
  mirrors those names exactly.
- Rust → JS for custom audio: `Vec<u8>` becomes a JS number array.
  The frontend converts via `new Uint8Array(bytes)`. Capped at 5 MB.

## Error propagation
- Rust commands return `Result<_, String>`. Errors surface to JS as
  the rejection of the invoke Promise.
- Frontend usually .catch()s and logs; the user-visible error path
  is the `error` $state in +page.svelte (only used for read_note).
- File-watcher errors are swallowed (notify::Result is destructured
  with `if let Ok(event) = res`).

## Update Triggers
- A new persistent file is introduced (e.g. tags.json, history.log).
- A new Tauri event is introduced (then add it to the relevant flow).
- The ignore_change mechanism is changed.
- The scheduler interval or thread model changes.
- A new serialization boundary (e.g. binary IPC via tauri::ipc::Response).

AGENT UPDATE: this file, on any of the above. Also update
docs/api/server-actions.md when adding a new command and
docs/modules/<feature>.md when adding a feature-specific flow.

## Related Docs
- docs/api/server-actions.md — every command involved in these flows.
- docs/api/external-services.md — the watcher, notification, dialog plugins.
- docs/modules/reminders.md — fire flow specifics.
- docs/modules/tasks.md — read/write task flow specifics.
