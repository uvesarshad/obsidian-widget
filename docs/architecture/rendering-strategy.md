# Rendering Strategy

> Scope: How SvelteKit is configured and why it runs as a pure SPA inside Tauri.
> Rendering context: Client
> Project tier: 3
> Last updated: 2026-05-17

## Overview
SvelteKit runs in static SPA mode. There is no Node server, no
prerendering at build time, and no per-route rendering decisions —
the entire frontend is a single client-rendered page that mounts
inside a Tauri webview. All "server" responsibilities are handled
by the Rust process via Tauri IPC, not by SvelteKit.

## Why SPA, not SSR / SSG / ISR
- Tauri's webview is a thin browser inside a desktop window. It
  loads `index.html` directly from the bundled `build/` directory.
  There is no Node runtime to serve pages.
- The frontend's only data source is the Rust backend, reached via
  `invoke()` and `listen()`. There is no HTTP origin to fetch from.
- A single window, single route, single page — no benefit from
  per-route rendering modes.

## How SPA mode is configured
- `src/routes/+layout.ts` exports `ssr = false` and `prerender = false`
  to disable server-side rendering and build-time prerendering.
- `svelte.config.js` uses `@sveltejs/adapter-static` with a SPA
  fallback so that the SvelteKit router never asks the file server
  for a non-existent route file.
- `vite.config.js` binds Vite's dev server to port 1420 so it
  matches the `devUrl` value in src-tauri/tauri.conf.json.

AGENT NOTE: `+layout.ts` must keep `ssr = false`. Turning SSR on
will break the Tauri build because there is no Node server to
render against, and code that calls `getCurrentWebviewWindow()` or
`invoke()` at module scope would crash during SSR.

## Routes
- `/` — the only route. Implemented in src/routes/+page.svelte.
  Owns the widget shell, header, content area, add-task row,
  reminder picker, pulse layer, alert panel, and all global event
  wiring.

There are no dynamic routes, no nested routes, no parallel routes,
no error routes. Adding one is unusual — the entire UX is a single
floating window.

AGENT AVOID: Do not add a second route without first confirming
the use case really needs SvelteKit routing. A modal/popover layer
inside +page.svelte is almost always the correct extension point.

## Caching strategy
- None at the SvelteKit level (no SSR, no fetch, no cache directives).
- The frontend caches one decoded audio Blob URL for the active
  custom tone (see docs/modules/reminders.md and the cachedTonePath
  variable in src/routes/+page.svelte).
- The Rust side does not cache file content — every `read_note`
  / `read_tasks` call re-reads from disk. The native file watcher
  pushes change events via the `file-changed` Tauri event.

## Edge runtime / streaming
- Not applicable. Pure SPA in a desktop webview.

## Bundle output
- `npm run build` writes the production frontend to ./build/ which
  is referenced by src-tauri/tauri.conf.json's `frontendDist`
  property as `../build`.

## Update Triggers
- A second route is introduced (then list it here with its purpose).
- The SSR / prerender flags in src/routes/+layout.ts change.
- The adapter or fallback strategy in svelte.config.js changes.
- A caching layer is introduced on either side of the IPC boundary.

AGENT UPDATE: this file, on any of the above. Also update
docs/overview.md if the rendering model shifts at all.

## Related Docs
- docs/architecture/folder-structure.md — where +layout.ts and +page.svelte live.
- docs/architecture/data-flow.md — how IPC drives all updates.
- docs/api/server-actions.md — the IPC commands themselves.
