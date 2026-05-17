# Module: Reminders

> Scope: Creating, persisting, scheduling, and firing reminders.
> Rendering context: Isomorphic
> Project tier: 3
> Last updated: 2026-05-17

## Overview
A reminder is a scheduled audio + visual alert tied to a task. The
reminder time also appears inline in the markdown task as
`(@YYYY-MM-DD HH:MM)` so it is visible in Obsidian. The Rust side
owns the scheduler thread and persistence to reminders.json. The
Svelte side owns the calendar picker, the firing alarm UX, and
custom-audio playback.

## Entry points
- Trigger: typing `@` inside the bottom add-task input in
  src/routes/+page.svelte (handleTaskInput → showReminderPicker = true).
- Picker UI: src/lib/ReminderPicker.svelte.
- Alarm UI: in src/routes/+page.svelte — the `.pulse-rings` ring
  layer + the `.reminder-alert` panel (Okay / Snooze 10 min).

## Key components
- **src/lib/ReminderPicker.svelte** — calendar + 12-hour time picker.
  Defaults to "now + 10 min, rounded to the next 5 min" to avoid
  AM/PM traps.
- **src/routes/+page.svelte** — handleReminderFired, dismissReminder,
  snoozeReminder, plus the Web Audio + custom-audio plumbing.

## Key Tauri commands
- `add_reminder({ taskText, remindAt })` → validates remindAt via
  parse_remind_at, generates a unique id, prunes fired backlog,
  persists. Returns the new id.
- `get_reminders()` → returns the full Vec<Reminder>.
- `delete_reminder({ id })` → exposed but currently unused.
- `set_window_blur({ enabled })` → clears acrylic/vibrancy while
  the alarm is firing, restores on dismiss.
- `pick_reminder_sound()` / `read_reminder_sound()` → see docs/modules
  audio section below.

## Storage shape
File: <app_data>/reminders.json
Each entry: `{ id: string, task_text: string, remind_at: string,
fired: boolean }`. The `remind_at` value is local time, no timezone
offset, format `YYYY-MM-DD HH:MM` (legacy ISO `T`-separated forms
are still accepted by parse_remind_at on read).

AGENT NOTE: `task_text` here is the CLEAN text without the
`(@...)` tag. The markdown file holds the canonical scheduled time;
reminders.json holds the trigger. If they drift, the scheduler wins
because it is what the Rust thread polls.

## Scheduler thread
- `start_reminder_scheduler(app)` spawns one std::thread.
- The thread calls `check_and_fire_reminders` immediately on
  startup (catches up any reminders that became due while the app
  was closed) and then every 30 seconds in a loop.
- `check_and_fire_reminders` filters `!fired && now >= remind_at`,
  brings the window forward, emits `reminder-fired` per due item,
  marks them fired, prunes the backlog, persists.

AGENT NOTE: 30 seconds is the maximum delay between "due" and
"fires". For testing, the picker's default of "now + 10 min" means
the alarm should fire within 30 s of the chosen minute mark — give
the scheduler one full tick.

## Fire UX (frontend)
On `reminder-fired`:
1. `set_window_blur({ enabled: false })` clears OS frosted glass.
2. The window is shown, focused, and forced always-on-top.
3. `isReminderFiring = true` + `firingTaskText = payload`.
4. `startSoundLoop(toneId)` begins the loop:
   - Built-in tones: `playTone` once, then `setInterval(..., cycle * 1000)`.
   - Custom: `playCustomSound(true)` with `audio.loop = true`.
5. The widget receives the `.firing` class, scaling to 0.9 and
   pulsing its inset blue glow.
6. Three `.ring` siblings of the widget animate scale 0.9 → 1.06
   into the real-transparent margin.
7. The `.reminder-alert` panel renders with Okay + Snooze 10 min
   buttons.
8. A 5-minute auto-dismiss timer is scheduled.

## Dismiss (Okay)
- Stops the sound loop (Web Audio interval cleared, HTMLAudioElement
  paused).
- Clears `isReminderFiring` and `firingTaskText`.
- Restores acrylic/vibrancy via `set_window_blur({ enabled: true })`.
- Restores the user's saved always_on_top.

## Snooze 10 min
1. Compute `snoozeAt = now + 10 * 60 * 1000`.
2. Find the task in `noteItems` whose `stripReminderTag(text)` equals
   `firingTaskText`. Rewrite its text to `${text} (@${snoozeIso})`.
3. `invoke('write_tasks', { tasks })` patches that line in the file.
4. `invoke('add_reminder', { taskText, remindAt: snoozeIso })` schedules
   a fresh reminder at the new time.
5. `dismissReminder()` cleans up the alarm state.

AGENT NOTE: The original (now `fired: true`) reminder is NOT deleted
on snooze. It is kept until pruned via MAX_FIRED_REMINDERS. This
preserves the historical signal that "this reminder fired at X".

## Tones (built-in)
Defined inline in src/routes/+page.svelte as `TONES: Record<...>`:
- `chime` — C-E-G-C major triad climb (default).
- `bell` — layered harmonics at 880/1760/2640 Hz.
- `beep` — three short square-wave pulses.
- `digital` — alternating triangle waves at 1200/900 Hz.
- `soft` — two gentle sine waves at 440/554 Hz.

Each has a `cycle` value (seconds per loop iteration). `playTone` is
synthesised on demand via OscillatorNode → GainNode → destination.
No bundled audio files.

## Custom tone
- User selects an audio file via `pick_reminder_sound` (file dialog).
- Selection auto-sets Settings.reminder_tone to `"custom"` and stores
  the path in Settings.reminder_tone_path.
- On first play, `getCustomAudio()` calls `read_reminder_sound` (5 MB
  cap), wraps the bytes in a Blob, creates a Blob URL, caches it.
- Subsequent plays reuse the cached HTMLAudioElement. The cache
  invalidates when `settings-changed` reports a new path.
- `cachedTonePath` tracks which path the current cache belongs to.

AGENT NOTE: The 5 MB cap is enforced server-side AND is necessary
for IPC performance. If a user picks a larger file the read rejects
with "file too large (X KB); max 5 MB". Show the error in
`console.error` and fall back to the default chime gracefully.

## Tone selection UX
- Bell icon in the header opens the sound popover.
- Each of the five built-in tones has a radio + ▶ preview button.
- The Custom row is disabled until a file is picked.
- A `Browse…` / `Change…` button at the bottom invokes
  `pick_reminder_sound`.

## Validation invariants
- `add_reminder` rejects unparseable `remind_at` strings with a
  clear error — prevents silent "scheduled but never fires" bugs.
- `load_settings` falls back to `chime` if Settings.reminder_tone is
  not in KNOWN_TONES.
- `generate_id` returns nanoseconds + AtomicU64 counter — uniqueness
  is guaranteed even on the same-nanosecond case.
- `prune_fired_reminders` keeps at most MAX_FIRED_REMINDERS (20) fired
  records.

## Known edge cases
- Multiple tasks with identical clean text: snooze updates ALL of
  their `(@...)` tags. Rare but documented.
- User deletes the task line manually after scheduling: the reminder
  still fires (the trigger lives in reminders.json). The alert just
  has no `(@...)` to update on snooze.
- Window hidden when alarm fires: the Rust side calls show / focus
  / set_always_on_top so the user sees the alert immediately.

## Update Triggers
- The canonical reminder tag format changes.
- A new built-in tone is added.
- The scheduler interval, MAX_FIRED_REMINDERS, or custom-audio cap
  changes.
- A new firing-state UI element is added (alert action, snooze
  duration option, etc.).
- The audio cache invalidation logic in the settings-changed listener
  changes.

AGENT UPDATE: this file, on any of the above. Also update
docs/api/server-actions.md when the relevant commands change and
docs/modules/tasks.md when the tag format changes.

## Related Docs
- docs/api/server-actions.md — add_reminder, set_window_blur, pick/read sound.
- docs/api/external-services.md — Web Audio + HTMLAudioElement.
- docs/architecture/data-flow.md — fire flow end-to-end.
- docs/modules/tasks.md — how the tag lives inside the markdown task.
