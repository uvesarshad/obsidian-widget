# Deployment

> Scope: Build pipeline, artifacts, install locations, autostart wiring.
> Rendering context: N/A (build-time)
> Project tier: 3
> Last updated: 2026-05-17

## Overview
ob-widget is shipped as a per-platform installer produced by
`tauri build`. There is no remote server or CDN — the binary runs
fully offline on the user's machine and writes only to the OS
app-data directory. CI exists under .github/workflows/release.yml
to produce Windows and macOS installers from one push.

## Build pipeline

### Local dev
- `npm install` once.
- `npm run tauri dev` starts Vite on port 1420 and compiles the
  Rust crate in debug mode. First compile is ~2 min; subsequent
  incremental builds are seconds.
- Hot reload applies only to Svelte. Rust changes require restarting
  the dev process.

AGENT NOTE: If you change anything inside src-tauri/, you must Ctrl+C
and re-run `npm run tauri dev`. Svelte HMR will not reload Rust.

### Release build
- `npm run tauri build` produces platform-native installers.
- Windows artifacts:
  - src-tauri/target/release/bundle/msi/ob-widget_<version>_x64_en-US.msi
  - src-tauri/target/release/bundle/nsis/ob-widget_<version>_x64-setup.exe
- macOS artifacts (Mac builder required):
  - src-tauri/target/release/bundle/dmg/ob-widget_<version>_x64.dmg
  - src-tauri/target/release/bundle/macos/ob-widget.app
- Linux is not a build target.

### CI
- .github/workflows/release.yml runs the macOS and Windows builds on
  GitHub Actions runners. macOS code-signing is conditional on
  Apple secrets being present (see commit 016d61d); the workflow
  no-ops the signing step when secrets are missing rather than
  failing the build.

## Build-time vs runtime config
- Build-time: tauri.conf.json (window size, decorations off,
  transparent: true, alwaysOnTop: true, skipTaskbar: true,
  bundle identifier `com.uves.ob-widget`, CSP).
- Runtime: settings.json under the OS app-data directory. Loaded by
  `load_settings(app)` at startup, persisted on every change.

AGENT AVOID: Do not move window defaults from tauri.conf.json into
load_settings's `Settings::default()`. The defaults live in two
places intentionally — tauri.conf.json controls the very first
frame paint (before AppState exists); Settings::default() is the
fallback when settings.json is missing or corrupt.

## Settings and data directories
- Windows: %APPDATA%\ob-widget\settings.json and reminders.json
- macOS: ~/Library/Application Support/ob-widget/settings.json
  and reminders.json
- Path is resolved via `app.path().app_data_dir()` with a fallback
  to `std::env::temp_dir().join("ob-widget")` if that fails.

The directory is created on demand by `settings_path()` and
`reminders_path()` via `std::fs::create_dir_all(..).ok()`.

## Launch-at-login
- Wired through tauri-plugin-autostart with the LaunchAgent strategy
  on macOS.
- Windows: writes a Run entry to
  `HKEY_CURRENT_USER\SOFTWARE\Microsoft\Windows\CurrentVersion\Run`.
- macOS: installs a LaunchAgent plist in `~/Library/LaunchAgents/`.
- Toggled by the tray menu item `"launch_login"` which calls
  `app.autolaunch().enable()` / `.disable()`.

AGENT NOTE: The autostart entry is NOT removed when the user
uninstalls the app on macOS — the LaunchAgent plist must be deleted
manually. This is a known limitation of tauri-plugin-autostart.

## Code signing & notarization
- Windows: unsigned by default. Users see a SmartScreen warning on
  first launch; signing requires a Windows code-signing certificate
  (~$200/yr). Not currently configured.
- macOS: unsigned by default. Users must right-click → Open the
  first time. Signing requires an Apple Developer account ($99/yr)
  and is wired into release.yml conditionally on secrets being set
  (`APPLE_CERTIFICATE`, `APPLE_CERTIFICATE_PASSWORD`,
  `APPLE_SIGNING_IDENTITY`, `APPLE_ID`, `APPLE_ID_PASSWORD`,
  `APPLE_TEAM_ID`).

AGENT AVOID: Do not commit any *.p12 or notarization secret to
this repository. Apple credentials must come from CI secrets only.

## Infrastructure dependencies
- None at runtime. The app is fully offline.
- Build-time: GitHub Actions runners with Rust toolchain, Node ≥ 18,
  cargo-tauri CLI.

## Promotion path
- No environments. There is only "release" — a tag pushed to GitHub
  triggers release.yml, which uploads artifacts to the GitHub
  Release for that tag. Users install manually.

## Update Triggers
- A new artifact target is added (e.g. Linux AppImage, iOS .ipa).
- The bundle identifier or version scheme changes.
- Apple/Windows signing is enabled and secrets are wired up.
- A new build-time config knob is introduced in tauri.conf.json.
- The settings.json or reminders.json location changes.
- The autostart strategy changes for either platform.

AGENT UPDATE: this file, on any of the above. Also update
docs/overview.md if the deployment target list changes.

## Related Docs
- docs/api/external-services.md — autostart, dialog, notification plugins.
- docs/architecture/folder-structure.md — tauri.conf.json location.
- docs/modules/window-chrome.md — tray menu wiring for launch_login.
