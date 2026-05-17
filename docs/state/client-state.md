# Client State

> Scope: All in-memory state on the Svelte side.
> Rendering context: Client
> Project tier: 3
> Last updated: 2026-05-17

## Overview
There is no external state manager (no svelte/store, no Redux,
no Zustand). All state lives in Svelte 5 `$state` and `$derived`
declarations inside src/routes/+page.svelte and the two reusable
components. Cross-component communication is via callback props.
Cross-process state (anything that lives on the Rust side) is
mirrored into local `$state` via `invoke()` reads and `listen()`
event subscriptions.

## Top-level state (src/routes/+page.svelte)

### Core data
- `noteItems: NoteItem[]` — current parsed view of the markdown file.
  Mirror of what `read_note` returns. Re-fetched on `file-changed`.
- `settings: Settings | null` — mirror of the Rust Settings struct.
  Initially null; populated via `get_settings()` on mount. Re-mirrored
  on every `settings-changed` event.
- `newTaskText: string` — bound to the bottom add-task input.
- `loading: boolean` — initial setup spinner gate.
- `error: string | null` — surfaced under the content area when
  `read_note` rejects.

### Reminder state
- `showReminderPicker: boolean` — controls the floating
  ReminderPicker overlay.
- `pendingReminderDate: Date | null` — date chosen in the picker
  but not yet attached to a task (waiting for the user to press
  Enter on the input).
- `isReminderFiring: boolean` — true while the alarm is active.
  Drives the pulse-rings render, the widget `firing` class, and
  the alert panel.
- `firingTaskText: string | null` — the task text being alarmed.
- `reminderTimer: ReturnType<typeof setTimeout> | null` — the 5-min
  auto-dismiss timer.
- `soundLoopTimer: ReturnType<typeof setInterval> | null` — Web
  Audio loop ticker for built-in tones.
- `audioCtx`, `customAudio`, `customAudioBlobUrl`, `cachedTonePath`
  — Web Audio / HTMLAudioElement lifecycle state.

### Header popovers
- `showOpacitySlider: boolean`, `opacity: number` (1.0 default).
- `showSoundPopover: boolean`.
- `showShortcutPopover: boolean`, `recordingShortcut: boolean`,
  `shortcutError: string`.

### Derived values
- `taskItems = $derived(noteItems.filter(i => i.kind === 'task'))`
- `pendingTasks = $derived(taskItems.filter(i => !i.done))`
- `currentTheme = $derived((settings?.theme ?? 'system') as Theme)`
- `hasContent = $derived(noteItems.length > 0)`

AGENT NOTE: Do not reach for a separate store for any of the above.
Svelte 5 `$state` IS the store. Components subscribe by reading the
prop or value, and reactivity is automatic. If state ever genuinely
needs to be shared across multiple pages, only then introduce a
$lib/stores.svelte.ts file.

## Component-local state

### src/lib/NoteView.svelte
- `editingLineIdx: number | null` — line being inline-edited.
- `editText: string` — working buffer during inline edit.
- `displayItems` (derived) — task block re-ordering.

### src/lib/ReminderPicker.svelte
- `viewYear`, `viewMonth` — currently displayed month.
- `selYear`, `selMonth`, `selDay` — selected date.
- `selHour` (0–23), `selMinute` — chosen time.
- `cells` (derived) — calendar grid array with leading null padding.
- `isAM`, `displayHour` (derived) — 12-hour projection of selHour.

## Event subscriptions
Registered in the `onMount` of +page.svelte, all unsubscribed in
the returned cleanup function.
- `file-changed` → calls `loadNote()` which re-`invoke('read_note')`.
- `settings-changed` (payload: Settings) → mirrors into `settings`,
  re-applies theme, reloads note if vault changed, invalidates
  cached custom audio if `reminder_tone_path` changed.
- `reminder-fired` (payload: string) → calls `handleReminderFired`.

AGENT AVOID: Do not add an event subscription outside `onMount`.
The cleanup must call the unsubscribe function returned by `listen`,
and that is wired up via the closure inside `onMount`.

## Cleanup contract (onMount return)
The unmount cleanup releases everything that could leak:
- All three `unlisten*` callbacks.
- `stopSoundLoop()` (clears the Web Audio interval and pauses
  HTMLAudioElement).
- `clearTimeout(reminderTimer)`.
- `URL.revokeObjectURL(customAudioBlobUrl)`.
- `audioCtx.close()`.

AGENT NOTE: The root `+page.svelte` rarely unmounts in production
but does during dev HMR. The cleanup function exists primarily for
HMR hygiene and future-proofing.

## Persistence boundary
- Every state write that should survive a restart goes through one
  of: `save_settings`, `set_opacity`, `update_shortcut`,
  `add_reminder`, `delete_reminder`, or a write that touches the
  markdown file (`add_task`, `write_tasks`, `delete_task`).
- Transient state (showReminderPicker, isReminderFiring,
  editingLineIdx, etc.) is never persisted.

## Update Triggers
- A new top-level `$state` declaration is added to +page.svelte.
- A new Tauri event is listened to (then add it under Event
  subscriptions AND in docs/api/server-actions.md).
- A reusable store is finally introduced under src/lib/.
- The onMount cleanup adds or removes a disposal step.

AGENT UPDATE: this file, on any of the above. Also update
docs/architecture/data-flow.md if a new state ↔ persistence loop
is introduced.

## Related Docs
- docs/api/server-actions.md — the commands that hydrate this state.
- docs/architecture/data-flow.md — the flows that write to this state.
- docs/ui/component-library.md — component-local state lives there too.
