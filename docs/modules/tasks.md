# Module: Tasks

> Scope: Reading, displaying, and mutating markdown tasks.
> Rendering context: Isomorphic
> Project tier: 3
> Last updated: 2026-05-17

## Overview
This module owns the full lifecycle of a task: parsing it out of
the markdown file, rendering it interactively, and committing edits
back to disk. There is no database — the markdown file is the
source of truth and reconstruction is line-by-line.

## Entry points
- Pages: src/routes/+page.svelte (the only route).
- The widget content area renders src/lib/NoteView.svelte once
  Settings.vault_path is set and read_note succeeds.

## Key components
- **src/lib/NoteView.svelte** — renders the parsed note list and
  handles checkbox / inline-edit / delete interactions.
- **src/routes/+page.svelte** — owns the add-task input and the
  three Tauri write paths (handleToggle, handleEdit, handleDelete,
  handleAddTask).
- **src/lib/types.ts** — Task and NoteItem mirror the Rust shapes.

## Key Tauri commands (server-side)
- `read_note` → returns Vec<NoteItem> via parse_note_lines.
- `read_tasks` → returns Vec<Task> via parse_tasks. Used less often
  than read_note in the runtime path.
- `write_tasks` → reconstruct_file patches only task lines by
  line_idx, preserves everything else.
- `add_task` → first_task_line + insert-before, or append at end.
- `delete_task` → remove a single line by line_idx.

## Parse rules
- A task line is any line whose `trim_start()` begins with
  `- [ ]`, `- [x]`, or `- [X]`.
- A heading is any line starting with one or more `#` characters.
- A horizontal rule is any line that — after stripping `-`, `*`, `_`,
  and spaces — is empty AND contains one of `---`, `***`, `___`.
- A bullet is any line starting with `- ` or `* ` that does not
  match the task pattern.
- Anything else is `kind: "text"`.

Indentation is stored as `indent = leading_whitespace_count / 2`.
Two spaces of indent counts as one level. Tabs are not normalised.

AGENT NOTE: parse_note_lines is the only place tasks get their
`task_id`. Tasks are numbered in source order (counter resets to 0
on each read). Do not reorder tasks before assigning `task_id`,
or write_tasks's line-index matching will scramble.

## Reconstruction rules
`reconstruct_file(original, tasks)` works by:
1. Splitting `original` into lines (keeping their original whitespace).
2. For each Task whose line_idx is in range, recomputing only that
   line as `{indent}- [{x|space}] {text}`, where `{indent}` is the
   ORIGINAL whitespace from that line.
3. Joining with `\n`, restoring the trailing newline if the
   original had one.

Non-task lines are passed through untouched. Empty lines, headings,
bullets, and arbitrary text are preserved exactly.

AGENT AVOID: Do not rebuild non-task lines from NoteItem.text inside
write_tasks. The frontend only sends back tasks; the file's other
lines come from the original disk read inside reconstruct_file.
Touching that breaks Obsidian-side formatting.

## Display reordering
NoteView.svelte's `displayItems` re-orders each contiguous BLOCK of
tasks so undone tasks render above done tasks, but only within a
block. Headings, bullets, separators, and text lines break a block,
so an "Errands" section's tasks won't shuffle past a "Personal"
section's tasks. The original source order is preserved in the file.

## Inline editing
- Double-click a `.task-text` span → `startEdit(item)`.
- `editText` is bound to a `<input class="task-edit">` with autofocus.
- Enter or blur → `commitEdit` → `onedit(taskId, lineIdx, text)`
  → handleEdit in +page.svelte → invoke('write_tasks', { tasks }).
- Escape → discards the edit.

## Adding a task with a reminder
See docs/modules/reminders.md for full flow. The relevant part for
this module: handleAddTask embeds the reminder tag inline into the
task text before invoking add_task, so what gets persisted is
`- [ ] Buy milk (@2026-05-17 11:15)`.

## Display of reminder chip
NoteView.svelte's `extractReminder(item.text)` returns `{ clean,
date }`. The `clean` value is what renders in the task; the chip
renders separately with `fmtChip(date)` (which uses 12-hour AM/PM
format). The chip is hidden when the task is marked done.

The regex used by both extractReminder (NoteView) and
stripReminderTag (+page.svelte for snooze) accepts the new
parenthesised form AND the legacy non-parenthesised ISO form
`@YYYY-MM-DDTHH:MM` so old data still reads correctly.

AGENT NOTE: If you change the canonical reminder tag format, you
must update three regexes in lockstep: REMINDER_RE in
src/lib/NoteView.svelte, REMINDER_TAG_RE in src/routes/+page.svelte,
and the format-strings list in parse_remind_at in
src-tauri/src/lib.rs.

## Known edge cases
- Tasks larger than 1 MB are rejected by read_note / read_tasks with
  a clear error. The frontend's `error` state surfaces it.
- Newlines in newTaskText are collapsed to spaces inside add_task
  to prevent markdown-injection of additional list items.
- If the watcher races a write and emits before ignore_change is
  set, the worst case is one redundant `read_note`. No data loss.
- If two tasks have identical clean text and one is snoozed, both
  get their (@time) tag rewritten (see docs/modules/reminders.md).

## Update Triggers
- A new task variant (e.g. `* [ ]` from asterisk lists) is supported.
- The parse rules in parse_note_lines change.
- reconstruct_file's logic changes (e.g. respecting an explicit
  indent column).
- handleAddTask, handleToggle, handleEdit, or handleDelete change
  their write semantics.

AGENT UPDATE: this file, on any of the above. Also update
docs/architecture/data-flow.md if the read/write loop changes.

## Related Docs
- docs/api/server-actions.md — exact contracts of the five task commands.
- docs/architecture/data-flow.md — full write loop with ignore_change.
- docs/ui/component-library.md — NoteView Props.
- docs/modules/reminders.md — how reminder tags are embedded in tasks.
