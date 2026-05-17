# Component Library

> Scope: Every Svelte component, its props, and whether it owns state.
> Rendering context: Client
> Project tier: 3
> Last updated: 2026-05-17

## Overview
There are two live shared components in src/lib/. The widget shell
itself lives in src/routes/+page.svelte and is not a reusable
component — it is the page. All components use Svelte 5 runes
($props, $state, $derived).

## src/lib/NoteView.svelte
- **Purpose:** Renders the parsed markdown content as an interactive
  list. Handles task toggling, inline editing, deletion, and the
  blue reminder chip beside any task whose text contains a `(@...)` tag.
- **Type:** Connected (mutates parent state via callbacks).
- **Props (interface Props):**
  - `items: NoteItem[]` — full parsed list from `read_note`.
  - `ontoggle: (taskId: number, lineIdx: number) => void` — invoked
    when the user clicks a checkbox.
  - `onedit: (taskId: number, lineIdx: number, text: string) => void`
    — invoked after the user commits an inline edit.
  - `ondelete: (lineIdx: number) => void` — invoked when the user
    clicks the × that appears on hover.
- **Internal state:** `editingLineIdx` (line being edited) and
  `editText` (working buffer).
- **Derived:** `displayItems` re-orders each contiguous task block
  so undone tasks render before done tasks. Non-task lines are kept
  in source order.
- **Inline markdown:** `md()` escapes &<> first, then transforms
  `**bold**`, `__bold__`, `*italic*`, `_italic_`, `~~strike~~`, and
  `` `code` ``. Order matters — escape first, then inject tags.
- **Reminder chip:** `extractReminder()` runs against `item.text` and
  returns `{ clean, date }`. The chip only renders for non-done tasks.
- **Accessibility:** root `role="list"`, tasks `role="listitem"`,
  checkbox is a real `<button role="checkbox" aria-checked>`,
  heading role with `aria-level` matches `item.level`.

AGENT NOTE: `md()` is the XSS guardrail. It must escape entities
**before** any markdown transformation runs. Do not reorder the chain.
The `{@html ...}` consumer relies on `md()` being safe.

AGENT AVOID: Do not pipe arbitrary HTML through NoteView via
`{@html}`. Anything new that needs HTML rendering should go through
`md()` (after extending its escape step) or use plain text + DOM.

## src/lib/ReminderPicker.svelte
- **Purpose:** Compact calendar + 12-hour time picker shown when the
  user types `@` in the new-task input. Capped at 215 px max-width.
- **Type:** Presentational with two outbound callbacks; owns its own
  selection state until confirm/cancel.
- **Props (interface Props):**
  - `onconfirm: (date: Date) => void` — fires with the chosen Date.
  - `oncancel: () => void` — fires on Cancel or outside-click dismiss.
- **Internal state:** `viewYear`, `viewMonth`, `selYear`, `selMonth`,
  `selDay`, `selHour` (stored 24h), `selMinute`.
- **Derived:** `cells` (calendar grid array with leading nulls),
  `isAM`, `displayHour` (12h projection of `selHour`).
- **Default selection:** now + 10 minutes, rounded up to the next
  5-minute mark. This deliberately keeps the picker on the correct
  side of AM/PM so typing `11` while it is 11:34 AM does not become
  11 PM.
- **Quick options:** "In 1h", "Tonight 9pm", "Tomorrow 9am",
  "Next week" each rewrite the selection to a sensible Date.
- **AM/PM toggle:** `toggleAMPM()` flips selHour by ±12 (clamped 0–23).

AGENT NOTE: Do not change the default selection back to "next round
hour". That was the original behaviour and it created an AM/PM trap
where users typed `11` thinking AM and got 23:00 (PM).

## src/lib/TaskList.svelte (legacy)
- **Purpose:** Earlier, simpler task list. Superseded by NoteView.svelte.
- **Type:** Not in the runtime path of src/routes/+page.svelte.
- **Status:** Kept for reference only.

AGENT AVOID: Do not import TaskList.svelte from new code. Use
NoteView.svelte. If TaskList is finally deleted, remove its entry
from docs/architecture/folder-structure.md and from this file.

## Composition patterns
- Callback props (Svelte 5 runes style) are used everywhere instead
  of `createEventDispatcher`. Names follow the `on<verb>` convention
  (ontoggle, onedit, ondelete, onconfirm, oncancel).
- The widget shell lives in +page.svelte and composes the two
  components plus several inline popovers (opacity, sound, shortcut).
  Those popovers are not extracted into components yet.

## Accessibility requirements
- Tasks expose `role="checkbox"` + `aria-checked` on the toggle button.
- Headings expose `role="heading"` + `aria-level`.
- The reminder picker buttons have `aria-pressed={isSelected(day)}`
  on each calendar day.
- The reminder-firing alert panel uses `role="alert"`.
- Pulse rings, decorative SVGs, and the resize handles use
  `aria-hidden` / `role="none"` because they are visual only.

## Update Triggers
- A new shared component is added under src/lib/.
- An existing component's Props interface changes.
- A component is renamed or deleted.
- The inline markdown rules in `md()` are extended (then update the
  Inline markdown bullet above).
- The default picker selection logic changes.

AGENT UPDATE: this file, on any of the above. Also update
docs/modules/tasks.md or docs/modules/reminders.md depending on the
component's owning feature.

## Related Docs
- docs/state/client-state.md — where the data passed in as props lives.
- docs/modules/tasks.md — full task lifecycle.
- docs/modules/reminders.md — full reminder lifecycle.
