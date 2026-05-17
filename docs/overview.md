# ob-widget — Overview

> Scope: Single entry point for humans and AI agents working in this repo.
> Rendering context: N/A (cross-cutting)
> Project tier: 3
> Last updated: 2026-05-17

## Overview
ob-widget is a Tauri v2 desktop widget (Windows + macOS) that mirrors a single
Obsidian markdown file as an always-on-top task list. The Rust process owns
the filesystem, a native file watcher, OS notifications, global shortcuts,
the system tray, and a background reminder scheduler. The Svelte frontend
renders tasks, headings, bullets and separators inline, and provides a
calendar+time picker for `@`-triggered reminders that fire with a pulsing
visual + looping audio alert.

## Tier classification — 3
Full-stack with a clear server/client split via Tauri IPC. The Rust side
owns persistence (JSON files) and OS integrations; the Svelte side owns
rendering and user input. There is no auth, no remote database, no HTTP
server, so it is not Tier 4.

## Environment context
- Rust: edition 2021, Tauri 2, single binary crate at src-tauri/.
- Node: ≥ 18. SvelteKit 2 + Svelte 5 (runes API) + Vite 6, TypeScript 5.
- Rendering: SPA mode, SSR disabled at the SvelteKit layout level.
- Deployment target: Windows (MSI + NSIS) and macOS (DMG + .app).
- Mobile iOS WidgetKit target is mentioned in the README but no
  iOS source code lives in this repo yet — treat it as planned, not built.

## Directory map of /docs
- docs/overview.md — this file. Index + glossary + recent changes.
- docs/architecture/folder-structure.md — every folder mapped to purpose.
- docs/architecture/rendering-strategy.md — SvelteKit SPA-mode rationale.
- docs/architecture/data-flow.md — markdown file ↔ Rust ↔ Svelte lifecycle.
- docs/ui/component-library.md — every shared Svelte component.
- docs/ui/theming.md — CSS variables, light/dark, transparency, vibrancy.
- docs/api/server-actions.md — every Tauri IPC command.
- docs/api/external-services.md — OS-level integrations and plugins.
- docs/state/client-state.md — Svelte $state stores and event listeners.
- docs/infra/deployment.md — build outputs and CI.
- docs/modules/tasks.md — task parsing, editing, deletion flow.
- docs/modules/reminders.md — picker, scheduler, alert, snooze, tones.
- docs/modules/window-chrome.md — frameless window, tray, opacity, shortcuts.

## Key architectural decisions
- **Markdown is the source of truth.** Tasks live as plain `- [ ]` /
  `- [x]` lines in a user-chosen .md file. The app never owns a database.
- **Reminders live in a sibling reminders.json.** The markdown task gets
  an inline `(@YYYY-MM-DD HH:MM)` tag for visibility in Obsidian, but
  scheduling is driven from reminders.json by a background Rust thread.
- **Native filesystem watcher.** The notify crate (NTFS / FSEvents)
  drives all updates. No polling. An "ignore_change" flag suppresses
  the self-write loop after the app writes the file.
- **Tauri commands as the API layer.** Every backend operation is a
  typed Rust function exposed via invoke_handler! and called from
  Svelte using @tauri-apps/api/core invoke().
- **Web Audio for built-in tones.** No bundled audio assets — five
  presets (chime/bell/beep/digital/soft) are synthesised on demand.
- **Custom audio via Blob URL.** User-picked .mp3/.wav/etc. is read by
  Rust (capped at 5 MB), shipped to JS as Uint8Array, wrapped in a Blob.
- **Pulse waves emit outward by shrinking the widget.** When a reminder
  fires, the widget transforms to scale 0.9 and acrylic/vibrancy is
  cleared so the rings travel through real transparency, not blurred
  backdrop.

## Cross-cutting concerns
- **Theming:** CSS `light-dark()` plus a `data-theme` attribute on
  documentElement; opacity controlled via the `--bg-alpha` custom property.
- **Error handling:** best-effort. Rust commands return `Result<_, String>`;
  the frontend mostly `.catch()`s and logs. File reads are bounded
  (1 MB for notes, 5 MB for custom audio).
- **Data fetching pattern:** invoke for explicit calls; listen for
  push events (`file-changed`, `settings-changed`, `reminder-fired`).
- **Concurrency:** AppState mutexes are held briefly and never across
  IPC boundaries. The reminder scheduler runs in a std::thread loop
  with a 30-second sleep.

## Glossary
- **vault**: an Obsidian vault folder containing markdown files.
- **target file**: the single .md file the widget mirrors (e.g. Tasks.md).
- **task**: a markdown line beginning `- [ ]` or `- [x]`.
- **note**: the full parsed file content (NoteItem[]: task, heading,
  bullet, separator, text).
- **reminder**: a `{id, task_text, remind_at, fired}` record in
  reminders.json; surfaced as `(@YYYY-MM-DD HH:MM)` in the markdown.
- **tone**: a sound preset for the reminder alarm; one of chime,
  bell, beep, digital, soft, or custom (user file).
- **pulse rings**: the three blue concentric ring elements that
  emanate outward while a reminder is firing.
- **acrylic / vibrancy**: Windows / macOS frosted-glass window effect
  applied by the window-vibrancy crate.
- **firing**: the in-app reminder state (`isReminderFiring === true`)
  during which audio loops, widget pulses, and the Okay / Snooze alert
  is shown.
- **snooze**: re-schedule the firing reminder 10 minutes later and
  rewrite the (@...) tag on the matching task line.

AGENT NOTE: Update the Recent Changes section below whenever any doc
file is added, removed, or restructured. Keep it sorted newest-first
and cap at 10 entries.

## Recent Changes
- [2026-05-17] README restructured: feature list + screenshots in
  About, How to Use moved above Getting Started, dual install path
  (Releases link or build manually), architecture diagram moved
  after Build manually, Contributing section added. Project now
  carries standard OSS files at root: LICENSE (MIT),
  CONTRIBUTING.md, CODE_OF_CONDUCT.md, SECURITY.md, plus
  .github/ISSUE_TEMPLATE/{bug_report.md, feature_request.md,
  config.yml} and .github/PULL_REQUEST_TEMPLATE.md.
- [2026-05-17] Initial documentation set generated (Tier 3). 13 doc
  files under /docs plus AGENTS.md and README Documentation section.

## Update Triggers
- A new doc file is added or an existing one is split into parts.
- The tech stack changes (Tauri major version, SvelteKit major version,
  added or removed Rust crate that owns a documented concern).
- A new architectural decision is made (new persistence layer, new
  cross-cutting pattern, new platform target).
- A new domain term appears repeatedly in the code and should join
  the glossary.

AGENT UPDATE: this file, on any of the triggers above. Specifically
update the Recent Changes list whenever any other doc file in /docs
is added, removed, or restructured.

## Related Docs
All docs/ files are linked above in the Directory map.
