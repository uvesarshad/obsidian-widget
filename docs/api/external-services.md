# External Services & OS Integrations

> Scope: Every third-party crate or OS API the app talks to.
> Rendering context: Server (Rust)
> Project tier: 3
> Last updated: 2026-05-17

## Overview
ob-widget is fully offline and never makes a network request. What
plays the role of "external services" here are the OS-level
integrations and Rust crates that bridge into native platform
behaviour: filesystem, notifications, dialogs, global shortcuts,
window vibrancy, autostart. Each is owned by a specific Tauri
plugin or Rust crate.

## Tauri plugins
Configured in src-tauri/Cargo.toml and initialised inside the
`tauri::Builder::default()` chain at the top of `run()`.

### tauri-plugin-notification
- **Purpose:** Native OS toast / banner notifications when a
  reminder fires.
- **Owner module:** check_and_fire_reminders in src-tauri/src/lib.rs.
- **Capability:** `notification:default` in
  src-tauri/capabilities/default.json.
- **Fallback:** Best-effort. Returns silently on failure. The
  in-app pulse + sound + alert provide the guaranteed feedback.
- **Known issue:** In `cargo tauri dev` mode on Windows the app has
  no registered AUMID, so toasts may silently fail. Installed
  release builds work correctly.

### tauri-plugin-dialog
- **Purpose:** Native file picker for the markdown target file and
  for the custom reminder sound.
- **Owner modules:** pick_task_file, pick_reminder_sound, and the
  tray menu's `change_file` handler.
- **Capability:** `dialog:default`.
- **Filters:** Markdown (.md, .MD) for the target file; Audio
  (.mp3, .wav, .ogg, .m4a, .flac, .aac and uppercase variants) for
  the reminder sound.
- **Fallback:** Picker is callback-based; if the user cancels, the
  callback is invoked with None and nothing happens.

### tauri-plugin-global-shortcut
- **Purpose:** Register a system-wide keyboard accelerator that
  brings the widget to the front from any application.
- **Owner:** `update_shortcut` command + the `with_handler` closure
  in the plugin builder.
- **Capability:** all five `global-shortcut:*` permissions.
- **Default:** `CmdOrControl+Shift+O`. Persisted as Settings.shortcut.
- **Behaviour on press:** Shows the window, sets focus, and if the
  user's saved always_on_top is false, temporarily forces it ON and
  flips `hotkey_aot_override` so the next blur restores the original
  AOT setting.

### tauri-plugin-autostart
- **Purpose:** Toggle launch-at-login.
- **Owner:** tray menu's `launch_login` handler.
- **macOS strategy:** `MacosLauncher::LaunchAgent` (writes a plist).
- **Windows strategy:** Registry Run key (default plugin behaviour).
- **Fallback:** Errors are swallowed via `.ok()`; the tray check
  state may drift if enable/disable fails silently.

AGENT NOTE: Plugin initialisation order in src-tauri/src/lib.rs `run()`
matters for some platforms. Keep notification first (it needs to
register the AUMID before other plugins on Windows).

## Rust crates (non-plugin)

### notify (file watcher)
- **Purpose:** Cross-platform filesystem watcher. NTFS on Windows,
  FSEvents on macOS.
- **Owner:** start_watcher in src-tauri/src/lib.rs.
- **Mode:** RecursiveMode::NonRecursive — only the target file is
  watched, not the whole vault.
- **Events handled:** Modify, Create. Others ignored.
- **Loop guard:** Arc<Mutex<bool>> `ignore_change` is set true by
  every Rust command that writes the file; the watcher consumes
  and resets the flag on its next event.
- **Fallback:** If `recommended_watcher` fails to construct, the
  watcher is silently dropped and the frontend never gets
  `file-changed` events — the file still works for read/write, just
  no auto-refresh.

AGENT AVOID: Do not switch notify to RecursiveMode::Recursive. The
performance is fine but it would emit events for every other file
in the vault and the ignore_change flag would be insufficient.

### chrono
- **Purpose:** Parse and compare reminder times.
- **Owner:** parse_remind_at, check_and_fire_reminders.
- **Formats accepted:** `%Y-%m-%d %H:%M` (current), `%Y-%m-%dT%H:%M`
  and `%Y-%m-%dT%H:%M:%S` (legacy compatibility).
- **Timezone:** Local. Times are stored without offset and resolved
  via `Local.from_local_datetime(&naive).single()`.

### window-vibrancy
- **Purpose:** Apply / clear the Windows acrylic effect or the macOS
  vibrancy material on the main window.
- **Owner:** apply_window_effects, clear_window_effects,
  set_window_blur in src-tauri/src/lib.rs.
- **Linux:** No-op (the `#[cfg(not(any(...)))]` variants are empty
  functions).

## Web-platform APIs (frontend)

### Web Audio API
- **Purpose:** Synthesise the five built-in reminder tones (chime,
  bell, beep, digital, soft) on demand. No bundled audio assets.
- **Owner:** `getAudio()`, `playTone()`, `startSoundLoop()` in
  src/routes/+page.svelte.
- **AudioContext lifecycle:** Created lazily on first `getAudio()`
  call. Resumed if suspended (browser autoplay policy). Closed in
  the `onMount` cleanup.

### HTMLAudioElement
- **Purpose:** Play user-selected custom audio files via Blob URL.
- **Owner:** `getCustomAudio()`, `playCustomSound()` in
  src/routes/+page.svelte.
- **Lifecycle:** One cached Audio element per
  Settings.reminder_tone_path; invalidated when the path changes
  via the `settings-changed` listener.

AGENT NOTE: Both audio paths share `stopSoundLoop()`. Adding a new
audio source requires extending that function (and `previewTone`
and `startSoundLoop`) to handle the new source's stop path.

## Update Triggers
- A Tauri plugin is added, removed, or upgraded across a major version.
- A capability is added or removed in src-tauri/capabilities/default.json.
- A Rust crate that owns an OS-level concern is replaced.
- A new Web platform API is integrated on the frontend.
- The notify watcher's RecursiveMode or filter set changes.
- The default global shortcut changes (mirror in tauri.conf.json
  tooltip too).

AGENT UPDATE: this file, on any of the above. Also update
src-tauri/capabilities/default.json when adding plugin permissions,
and docs/api/server-actions.md when commands change as a result.

## Related Docs
- docs/api/server-actions.md — the commands that wrap these integrations.
- docs/infra/deployment.md — autostart, code signing, bundle output.
- docs/modules/reminders.md — Web Audio + custom sound integration.
- docs/modules/window-chrome.md — vibrancy, tray, global shortcut.
