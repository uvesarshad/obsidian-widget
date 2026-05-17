use chrono::{DateTime, Local, NaiveDateTime, TimeZone};
use notify::{Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

/// Max number of `fired: true` reminders kept on disk. Anything older is pruned
/// on the next save to prevent reminders.json from growing without bound.
const MAX_FIRED_REMINDERS: usize = 20;
/// Known tone identifiers — validated when settings load so a corrupted or stale
/// value can't break playback.
const KNOWN_TONES: &[&str] = &["chime", "bell", "beep", "digital", "soft", "custom"];
use tauri::{
    menu::{CheckMenuItem, Menu, MenuItem, PredefinedMenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    AppHandle, Emitter, Manager, State,
};

// ── Types ──────────────────────────────────────────────────────────────────────

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct WindowConfig {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
}

impl Default for WindowConfig {
    fn default() -> Self {
        WindowConfig { x: 100, y: 100, width: 320, height: 480 }
    }
}

fn default_theme() -> String { "system".to_string() }
fn default_opacity() -> f64 { 1.0 }
fn default_shortcut() -> String { "CmdOrControl+Shift+O".to_string() }
fn default_reminder_tone() -> String { "chime".to_string() }
fn default_reminder_tone_path() -> String { String::new() }

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Settings {
    pub vault_path: String,
    pub target_file: String,
    pub window: WindowConfig,
    pub always_on_top: bool,
    pub click_through_on_blur: bool,
    #[serde(default = "default_theme")]
    pub theme: String,
    #[serde(default = "default_opacity")]
    pub opacity: f64,
    #[serde(default = "default_shortcut")]
    pub shortcut: String,
    #[serde(default = "default_reminder_tone")]
    pub reminder_tone: String,
    #[serde(default = "default_reminder_tone_path")]
    pub reminder_tone_path: String,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            vault_path: String::new(),
            target_file: "Tasks.md".to_string(),
            window: WindowConfig::default(),
            always_on_top: true,
            click_through_on_blur: false,
            theme: "system".to_string(),
            opacity: 1.0,
            shortcut: "CmdOrControl+Shift+O".to_string(),
            reminder_tone: "chime".to_string(),
            reminder_tone_path: String::new(),
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct NoteItem {
    pub line_idx: usize,
    pub kind: String,
    pub text: String,
    pub done: bool,
    pub task_id: usize,
    pub level: u8,
    pub indent: usize,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Task {
    pub id: usize,
    pub done: bool,
    pub text: String,
    pub line_idx: usize,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Reminder {
    pub id: String,
    pub task_text: String,
    /// ISO 8601 local time: "2026-05-17T14:30"
    pub remind_at: String,
    pub fired: bool,
}

pub struct AppState {
    pub settings: Mutex<Settings>,
    pub watcher: Mutex<Option<RecommendedWatcher>>,
    pub ignore_change: Arc<Mutex<bool>>,
    pub hotkey_aot_override: Arc<Mutex<bool>>,
    pub reminders: Mutex<Vec<Reminder>>,
}

// ── Settings helpers ───────────────────────────────────────────────────────────

fn settings_path(app: &AppHandle) -> PathBuf {
    let dir = app
        .path()
        .app_data_dir()
        .unwrap_or_else(|_| std::env::temp_dir().join("ob-widget"));
    std::fs::create_dir_all(&dir).ok();
    dir.join("settings.json")
}

fn load_settings(app: &AppHandle) -> Settings {
    let path = settings_path(app);
    let mut settings: Settings = std::fs::read_to_string(&path)
        .ok()
        .and_then(|c| serde_json::from_str(&c).ok())
        .unwrap_or_default();
    // Normalise reminder_tone — fall back to the default if the persisted value
    // is unknown (corrupt file, manual edit, or a future-version remnant).
    if !KNOWN_TONES.contains(&settings.reminder_tone.as_str()) {
        settings.reminder_tone = default_reminder_tone();
    }
    settings
}

fn persist_settings(app: &AppHandle, settings: &Settings) {
    let path = settings_path(app);
    if let Ok(json) = serde_json::to_string_pretty(settings) {
        std::fs::write(path, json).ok();
    }
}

fn target_file_path(settings: &Settings) -> Option<PathBuf> {
    if settings.vault_path.is_empty() {
        return None;
    }
    let target = std::path::Path::new(&settings.target_file);
    let has_traversal = target
        .components()
        .any(|c| matches!(c, std::path::Component::ParentDir));
    if has_traversal {
        return None;
    }
    Some(PathBuf::from(&settings.vault_path).join(target))
}

// ── Reminder helpers ───────────────────────────────────────────────────────────

fn reminders_path(app: &AppHandle) -> PathBuf {
    let dir = app
        .path()
        .app_data_dir()
        .unwrap_or_else(|_| std::env::temp_dir().join("ob-widget"));
    std::fs::create_dir_all(&dir).ok();
    dir.join("reminders.json")
}

fn load_reminders(app: &AppHandle) -> Vec<Reminder> {
    let path = reminders_path(app);
    std::fs::read_to_string(&path)
        .ok()
        .and_then(|c| serde_json::from_str(&c).ok())
        .unwrap_or_default()
}

fn save_reminders(app: &AppHandle, reminders: &[Reminder]) {
    let path = reminders_path(app);
    if let Ok(json) = serde_json::to_string_pretty(reminders) {
        std::fs::write(path, json).ok();
    }
}

/// Process-local monotonic counter. Combined with a nanosecond timestamp this
/// guarantees uniqueness across reminders created in the same nanosecond.
static ID_COUNTER: AtomicU64 = AtomicU64::new(0);

fn generate_id() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let counter = ID_COUNTER.fetch_add(1, Ordering::Relaxed);
    format!("{:x}-{:x}", nanos, counter)
}

/// Drop the oldest `fired: true` reminders so the list stays bounded. Keeps all
/// pending (`fired: false`) reminders untouched.
fn prune_fired_reminders(reminders: &mut Vec<Reminder>) {
    let fired_count = reminders.iter().filter(|r| r.fired).count();
    if fired_count <= MAX_FIRED_REMINDERS {
        return;
    }
    let drop_n = fired_count - MAX_FIRED_REMINDERS;
    let mut dropped = 0usize;
    reminders.retain(|r| {
        if !r.fired { return true; }
        if dropped < drop_n { dropped += 1; return false; }
        true
    });
}

fn parse_remind_at(s: &str) -> Option<DateTime<Local>> {
    // Try the new "YYYY-MM-DD HH:MM" format first, then fall back to ISO variants
    NaiveDateTime::parse_from_str(s, "%Y-%m-%d %H:%M")
        .or_else(|_| NaiveDateTime::parse_from_str(s, "%Y-%m-%dT%H:%M"))
        .or_else(|_| NaiveDateTime::parse_from_str(s, "%Y-%m-%dT%H:%M:%S"))
        .ok()
        .and_then(|naive| Local.from_local_datetime(&naive).single())
}

fn check_and_fire_reminders(app: &AppHandle) {
    use tauri_plugin_notification::NotificationExt;

    let state = app.state::<AppState>();
    let now = Local::now();

    let due: Vec<(String, String)> = {
        let reminders = state.reminders.lock().unwrap();
        reminders
            .iter()
            .filter(|r| !r.fired)
            .filter_map(|r| {
                parse_remind_at(&r.remind_at)
                    .filter(|t| now >= *t)
                    .map(|_| (r.id.clone(), r.task_text.clone()))
            })
            .collect()
    };

    if due.is_empty() {
        return;
    }

    // Bring widget to front (always-on-top, show, focus)
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.unminimize();
        let _ = window.show();
        let _ = window.set_always_on_top(true);
        let _ = window.set_focus();
    }

    for (id, text) in &due {
        eprintln!("[reminder] firing {} — {}", id, text);

        // OS toast (best-effort: may silently fail in dev without AUMID)
        app.notification()
            .builder()
            .title("Reminder")
            .body(text)
            .show()
            .ok();

        // Notify the frontend so it can pulse + play sound
        app.emit("reminder-fired", text.clone()).ok();
    }

    let fired_ids: std::collections::HashSet<String> =
        due.into_iter().map(|(id, _)| id).collect();
    let mut reminders = state.reminders.lock().unwrap();
    for r in reminders.iter_mut() {
        if fired_ids.contains(&r.id) {
            r.fired = true;
        }
    }
    prune_fired_reminders(&mut reminders);
    save_reminders(app, &reminders);
}

fn start_reminder_scheduler(app: AppHandle) {
    std::thread::spawn(move || {
        // Fire any reminders that were due while the app was closed
        check_and_fire_reminders(&app);
        loop {
            std::thread::sleep(std::time::Duration::from_secs(30));
            check_and_fire_reminders(&app);
        }
    });
}

// ── Markdown helpers ──────────────────────────────────────────────────────────

fn parse_tasks(content: &str) -> Vec<Task> {
    let mut id = 0;
    content
        .lines()
        .enumerate()
        .filter_map(|(line_idx, line)| {
            let t = line.trim_start();
            if t.starts_with("- [ ]") || t.starts_with("- [x]") || t.starts_with("- [X]") {
                let done = t.starts_with("- [x]") || t.starts_with("- [X]");
                let text = t[5..].trim().to_string();
                let task = Task { id, done, text, line_idx };
                id += 1;
                Some(task)
            } else {
                None
            }
        })
        .collect()
}

fn parse_note_lines(content: &str) -> Vec<NoteItem> {
    let mut task_id = 0usize;
    content
        .lines()
        .enumerate()
        .map(|(line_idx, line)| {
            let indent_chars = line.chars().take_while(|c| c.is_whitespace()).count();
            let indent = indent_chars / 2;
            let t = line.trim_start();

            if t.starts_with("- [ ]") || t.starts_with("- [x]") || t.starts_with("- [X]") {
                let done = t.starts_with("- [x]") || t.starts_with("- [X]");
                let text = t[5..].trim().to_string();
                let id = task_id;
                task_id += 1;
                return NoteItem { line_idx, kind: "task".to_string(), text, done, task_id: id, level: 0, indent };
            }

            if t.starts_with('#') {
                let level = t.chars().take_while(|c| *c == '#').count() as u8;
                let text = t[level as usize..].trim().to_string();
                return NoteItem { line_idx, kind: "heading".to_string(), text, done: false, task_id: 0, level, indent: 0 };
            }

            let stripped = t.replace('-', "").replace('*', "").replace('_', "").replace(' ', "");
            if stripped.is_empty() && (t.contains("---") || t.contains("***") || t.contains("___")) {
                return NoteItem { line_idx, kind: "separator".to_string(), text: String::new(), done: false, task_id: 0, level: 0, indent: 0 };
            }

            if t.starts_with("- ") || t.starts_with("* ") {
                let text = t[2..].trim().to_string();
                return NoteItem { line_idx, kind: "bullet".to_string(), text, done: false, task_id: 0, level: 0, indent };
            }

            NoteItem { line_idx, kind: "text".to_string(), text: t.to_string(), done: false, task_id: 0, level: 0, indent: 0 }
        })
        .collect()
}

fn first_task_line(content: &str) -> Option<usize> {
    content.lines().enumerate().find_map(|(i, line)| {
        let t = line.trim_start();
        if t.starts_with("- [ ]") || t.starts_with("- [x]") || t.starts_with("- [X]") {
            Some(i)
        } else {
            None
        }
    })
}

fn reconstruct_file(original: &str, tasks: &[Task]) -> String {
    let mut lines: Vec<String> = original.lines().map(|l| l.to_string()).collect();
    for task in tasks {
        if task.line_idx < lines.len() {
            let original_line = &lines[task.line_idx];
            let indent: String = original_line.chars().take_while(|c| c.is_whitespace()).collect();
            let mark = if task.done { "x" } else { " " };
            lines[task.line_idx] = format!("{}- [{}] {}", indent, mark, task.text);
        }
    }
    let mut result = lines.join("\n");
    if original.ends_with('\n') {
        result.push('\n');
    }
    result
}

// ── Platform visual effects ────────────────────────────────────────────────────

#[cfg(target_os = "windows")]
fn apply_window_effects(window: &tauri::WebviewWindow) {
    use window_vibrancy::apply_acrylic;
    apply_acrylic(window, Some((0, 0, 0, 0))).ok();
}

#[cfg(target_os = "macos")]
fn apply_window_effects(window: &tauri::WebviewWindow) {
    use window_vibrancy::{apply_vibrancy, NSVisualEffectMaterial};
    apply_vibrancy(window, NSVisualEffectMaterial::HudWindow, None, None).ok();
}

#[cfg(not(any(target_os = "windows", target_os = "macos")))]
fn apply_window_effects(_window: &tauri::WebviewWindow) {}

#[cfg(target_os = "windows")]
fn clear_window_effects(window: &tauri::WebviewWindow) {
    use window_vibrancy::clear_acrylic;
    clear_acrylic(window).ok();
}

#[cfg(target_os = "macos")]
fn clear_window_effects(window: &tauri::WebviewWindow) {
    use window_vibrancy::clear_vibrancy;
    clear_vibrancy(window).ok();
}

#[cfg(not(any(target_os = "windows", target_os = "macos")))]
fn clear_window_effects(_window: &tauri::WebviewWindow) {}

/// Toggle the platform blur effect (acrylic on Windows, vibrancy on macOS).
/// When the reminder is firing we turn it OFF so the rings emit through real transparent
/// air rather than through the window's frosted backdrop.
#[tauri::command]
fn set_window_blur(app: AppHandle, enabled: bool) -> Result<(), String> {
    if let Some(window) = app.get_webview_window("main") {
        if enabled {
            apply_window_effects(&window);
        } else {
            clear_window_effects(&window);
        }
    }
    Ok(())
}

// ── File watcher ───────────────────────────────────────────────────────────────

fn start_watcher(
    app: &AppHandle,
    file_path: PathBuf,
    ignore: Arc<Mutex<bool>>,
    watcher_guard: &mut Option<RecommendedWatcher>,
) {
    *watcher_guard = None;

    let app_clone = app.clone();
    let ignore_clone = Arc::clone(&ignore);

    let result = notify::recommended_watcher(move |res: notify::Result<Event>| {
        if let Ok(event) = res {
            match event.kind {
                EventKind::Modify(_) | EventKind::Create(_) => {
                    let mut ig = ignore_clone.lock().unwrap();
                    if *ig {
                        *ig = false;
                        return;
                    }
                    drop(ig);
                    app_clone.emit("file-changed", ()).ok();
                }
                _ => {}
            }
        }
    });

    if let Ok(mut w) = result {
        if w.watch(&file_path, RecursiveMode::NonRecursive).is_ok() {
            *watcher_guard = Some(w);
        }
    }
}

// ── Tauri commands ─────────────────────────────────────────────────────────────

#[tauri::command]
fn get_settings(state: State<'_, AppState>) -> Settings {
    state.settings.lock().unwrap().clone()
}

#[tauri::command]
fn save_settings(app: AppHandle, state: State<'_, AppState>, settings: Settings) {
    let (old_vault, old_file) = {
        let s = state.settings.lock().unwrap();
        (s.vault_path.clone(), s.target_file.clone())
    };
    {
        let mut s = state.settings.lock().unwrap();
        *s = settings.clone();
    }
    persist_settings(&app, &settings);

    let path_changed = settings.vault_path != old_vault || settings.target_file != old_file;
    if path_changed && !settings.vault_path.is_empty() {
        if let Some(file_path) = target_file_path(&settings) {
            let mut wg = state.watcher.lock().unwrap();
            start_watcher(&app, file_path, Arc::clone(&state.ignore_change), &mut wg);
        }
    }
}

#[tauri::command]
fn read_tasks(state: State<'_, AppState>) -> Result<Vec<Task>, String> {
    let path = {
        let s = state.settings.lock().unwrap();
        target_file_path(&s).ok_or_else(|| "vault not configured".to_string())?
    };
    let size = std::fs::metadata(&path).map(|m| m.len()).unwrap_or(0);
    if size > 1_000_000 {
        return Err(format!("file too large ({} KB); max 1 MB", size / 1024));
    }
    let content = std::fs::read_to_string(&path).map_err(|e| e.to_string())?;
    Ok(parse_tasks(&content))
}

#[tauri::command]
fn write_tasks(state: State<'_, AppState>, tasks: Vec<Task>) -> Result<(), String> {
    let path = {
        let s = state.settings.lock().unwrap();
        target_file_path(&s).ok_or_else(|| "vault not configured".to_string())?
    };
    let content = std::fs::read_to_string(&path).map_err(|e| e.to_string())?;
    let new_content = reconstruct_file(&content, &tasks);
    *state.ignore_change.lock().unwrap() = true;
    std::fs::write(&path, new_content).map_err(|e| e.to_string())
}

#[tauri::command]
fn add_task(state: State<'_, AppState>, text: String) -> Result<(), String> {
    let text = text.replace(['\n', '\r'], " ").trim().to_string();
    if text.is_empty() {
        return Ok(());
    }
    let path = {
        let s = state.settings.lock().unwrap();
        target_file_path(&s).ok_or_else(|| "vault not configured".to_string())?
    };
    let content = std::fs::read_to_string(&path).unwrap_or_default();
    let new_line = format!("- [ ] {}\n", text);
    let new_content = if let Some(first_idx) = first_task_line(&content) {
        let mut lines: Vec<&str> = content.lines().collect();
        lines.insert(first_idx, new_line.trim_end_matches('\n'));
        let mut result = lines.join("\n");
        if content.ends_with('\n') { result.push('\n'); }
        result
    } else {
        let mut c = content.clone();
        if !c.ends_with('\n') && !c.is_empty() { c.push('\n'); }
        c.push_str(&new_line);
        c
    };
    *state.ignore_change.lock().unwrap() = true;
    std::fs::write(&path, new_content).map_err(|e| e.to_string())
}

#[tauri::command]
fn read_note(state: State<'_, AppState>) -> Result<Vec<NoteItem>, String> {
    let path = {
        let s = state.settings.lock().unwrap();
        target_file_path(&s).ok_or_else(|| "vault not configured".to_string())?
    };
    let size = std::fs::metadata(&path).map(|m| m.len()).unwrap_or(0);
    if size > 1_000_000 {
        return Err(format!("file too large ({} KB); max 1 MB", size / 1024));
    }
    let content = std::fs::read_to_string(&path).map_err(|e| e.to_string())?;
    Ok(parse_note_lines(&content))
}

#[tauri::command]
fn set_opacity(app: AppHandle, state: State<'_, AppState>, opacity: f64) -> Result<(), String> {
    let opacity = opacity.clamp(0.1, 1.0);
    {
        let mut s = state.settings.lock().unwrap();
        s.opacity = opacity;
    }
    let settings = state.settings.lock().unwrap().clone();
    persist_settings(&app, &settings);
    Ok(())
}

#[tauri::command]
fn update_shortcut(app: AppHandle, state: State<'_, AppState>, shortcut: String) -> Result<(), String> {
    use tauri_plugin_global_shortcut::GlobalShortcutExt;
    let old_shortcut = state.settings.lock().unwrap().shortcut.clone();
    app.global_shortcut().unregister(old_shortcut.as_str()).ok();
    app.global_shortcut().register(shortcut.as_str())
        .map_err(|e| format!("Invalid shortcut '{shortcut}': {e}"))?;
    {
        let mut s = state.settings.lock().unwrap();
        s.shortcut = shortcut;
    }
    let settings = state.settings.lock().unwrap().clone();
    persist_settings(&app, &settings);
    Ok(())
}

#[tauri::command]
fn delete_task(state: State<'_, AppState>, line_idx: usize) -> Result<(), String> {
    let path = {
        let s = state.settings.lock().unwrap();
        target_file_path(&s).ok_or_else(|| "vault not configured".to_string())?
    };
    let content = std::fs::read_to_string(&path).map_err(|e| e.to_string())?;
    let new_lines: Vec<&str> = content
        .lines()
        .enumerate()
        .filter(|(i, _)| *i != line_idx)
        .map(|(_, l)| l)
        .collect();
    let mut result = new_lines.join("\n");
    if content.ends_with('\n') {
        result.push('\n');
    }
    *state.ignore_change.lock().unwrap() = true;
    std::fs::write(&path, result).map_err(|e| e.to_string())
}

#[tauri::command]
fn pick_task_file(app: AppHandle) {
    use tauri_plugin_dialog::DialogExt;
    let app_cb = app.clone();
    app.dialog()
        .file()
        .add_filter("Markdown", &["md", "MD"])
        .pick_file(move |file_path| {
            let Some(fp) = file_path else { return };
            let path_str = fp.to_string();
            let path = std::path::Path::new(&path_str);
            let vault_path = path.parent()
                .map(|p| p.to_string_lossy().to_string())
                .unwrap_or_default();
            let target_file = path.file_name()
                .map(|f| f.to_string_lossy().to_string())
                .unwrap_or_else(|| "Tasks.md".to_string());
            let state = app_cb.state::<AppState>();
            let task_file = {
                let mut s = state.settings.lock().unwrap();
                s.vault_path = vault_path;
                s.target_file = target_file;
                target_file_path(&s)
            };
            let settings = state.settings.lock().unwrap().clone();
            persist_settings(&app_cb, &settings);
            if let Some(task_path) = task_file {
                let mut wg = state.watcher.lock().unwrap();
                start_watcher(&app_cb, task_path, Arc::clone(&state.ignore_change), &mut wg);
            }
            app_cb.emit("settings-changed", settings).ok();
        });
}

/// Open a file picker for an audio file. On selection, stores the path in settings
/// and switches the active tone to "custom". Result is delivered via `settings-changed`.
#[tauri::command]
fn pick_reminder_sound(app: AppHandle) {
    use tauri_plugin_dialog::DialogExt;
    let app_cb = app.clone();
    app.dialog()
        .file()
        .add_filter("Audio", &["mp3", "wav", "ogg", "m4a", "flac", "aac", "MP3", "WAV", "OGG", "M4A", "FLAC", "AAC"])
        .pick_file(move |file_path| {
            let Some(fp) = file_path else { return };
            let path_str = fp.to_string();
            let state = app_cb.state::<AppState>();
            {
                let mut s = state.settings.lock().unwrap();
                s.reminder_tone_path = path_str.clone();
                s.reminder_tone = "custom".to_string();
            }
            let settings = state.settings.lock().unwrap().clone();
            persist_settings(&app_cb, &settings);
            app_cb.emit("settings-changed", settings).ok();
        });
}

/// Read the raw bytes of the configured custom reminder sound file.
/// Frontend wraps these in a Blob URL for HTMLAudioElement playback.
#[tauri::command]
fn read_reminder_sound(state: State<'_, AppState>) -> Result<Vec<u8>, String> {
    let path = {
        let s = state.settings.lock().unwrap();
        s.reminder_tone_path.clone()
    };
    if path.is_empty() {
        return Err("no custom sound configured".to_string());
    }
    let size = std::fs::metadata(&path).map(|m| m.len()).unwrap_or(0);
    // 5 MB cap. The bytes are serialised as a JSON number array over IPC, which
    // is bandwidth-heavy; 5 MB is plenty for any alarm tone and keeps the
    // first-play latency reasonable.
    if size > 5_000_000 {
        return Err(format!("file too large ({} KB); max 5 MB", size / 1024));
    }
    std::fs::read(&path).map_err(|e| e.to_string())
}

#[tauri::command]
fn add_reminder(
    app: AppHandle,
    state: State<'_, AppState>,
    task_text: String,
    remind_at: String,
) -> Result<String, String> {
    // Validate the time string parses to a real datetime — otherwise the
    // reminder would sit in storage forever and never fire (silent failure).
    if parse_remind_at(&remind_at).is_none() {
        return Err(format!("invalid remind_at format: '{}'", remind_at));
    }
    let id = generate_id();
    let reminder = Reminder { id: id.clone(), task_text, remind_at, fired: false };
    let mut reminders = state.reminders.lock().unwrap();
    reminders.push(reminder);
    prune_fired_reminders(&mut reminders);
    save_reminders(&app, &reminders);
    Ok(id)
}

#[tauri::command]
fn get_reminders(state: State<'_, AppState>) -> Vec<Reminder> {
    state.reminders.lock().unwrap().clone()
}

#[tauri::command]
fn delete_reminder(
    app: AppHandle,
    state: State<'_, AppState>,
    id: String,
) -> Result<(), String> {
    let mut reminders = state.reminders.lock().unwrap();
    reminders.retain(|r| r.id != id);
    prune_fired_reminders(&mut reminders);
    save_reminders(&app, &reminders);
    Ok(())
}

// ── Entry point ────────────────────────────────────────────────────────────────

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            None,
        ))
        .plugin(
            tauri_plugin_global_shortcut::Builder::new()
                .with_handler(|app, _shortcut, event| {
                    use tauri_plugin_global_shortcut::ShortcutState;
                    if event.state() != ShortcutState::Pressed {
                        return;
                    }
                    let state = app.state::<AppState>();
                    if let Some(window) = app.get_webview_window("main") {
                        let saved_aot = state.settings.lock().unwrap().always_on_top;
                        if !saved_aot {
                            *state.hotkey_aot_override.lock().unwrap() = true;
                            window.set_always_on_top(true).ok();
                        }
                        window.show().ok();
                        window.set_focus().ok();
                    }
                })
                .build(),
        )
        .setup(|app| {
            let settings = load_settings(app.handle());

            if let Some(window) = app.get_webview_window("main") {
                window.set_always_on_top(settings.always_on_top).ok();
                let _ = window.set_size(tauri::Size::Physical(tauri::PhysicalSize {
                    width: settings.window.width,
                    height: settings.window.height,
                }));
                let _ = window.set_position(tauri::Position::Physical(tauri::PhysicalPosition {
                    x: settings.window.x,
                    y: settings.window.y,
                }));
                apply_window_effects(&window);
            }

            let initial_shortcut = settings.shortcut.clone();
            let ignore_change = Arc::new(Mutex::new(false));
            let hotkey_aot_override = Arc::new(Mutex::new(false));

            let reminders = load_reminders(app.handle());

            let state = AppState {
                watcher: Mutex::new(None),
                ignore_change: Arc::clone(&ignore_change),
                hotkey_aot_override: Arc::clone(&hotkey_aot_override),
                settings: Mutex::new(settings),
                reminders: Mutex::new(reminders),
            };
            app.manage(state);

            {
                let state = app.state::<AppState>();
                let file_path = {
                    let s = state.settings.lock().unwrap();
                    target_file_path(&s)
                };
                if let Some(fp) = file_path {
                    let mut wg = state.watcher.lock().unwrap();
                    start_watcher(app.handle(), fp, Arc::clone(&ignore_change), &mut wg);
                }
            }

            use tauri_plugin_global_shortcut::GlobalShortcutExt;
            if let Err(e) = app.global_shortcut().register(initial_shortcut.as_str()) {
                eprintln!("Could not register global shortcut '{initial_shortcut}': {e}");
            }

            // Start background reminder scheduler
            start_reminder_scheduler(app.handle().clone());

            let (aot, click_thru) = {
                let state = app.state::<AppState>();
                let s = state.settings.lock().unwrap();
                (s.always_on_top, s.click_through_on_blur)
            };
            use tauri_plugin_autostart::ManagerExt;
            let launch_at_login = app.autolaunch().is_enabled().unwrap_or(false);

            let change_file = MenuItem::with_id(app, "change_file", "Choose Note File…", true, None::<&str>)?;
            let always_top = CheckMenuItem::with_id(app, "always_top", "Always on Top", true, aot, None::<&str>)?;
            let click_through = CheckMenuItem::with_id(app, "click_through", "Click-through", true, click_thru, None::<&str>)?;
            let launch_login = CheckMenuItem::with_id(app, "launch_login", "Launch at Login", true, launch_at_login, None::<&str>)?;
            let sep = PredefinedMenuItem::separator(app)?;
            let quit = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;
            let menu = Menu::with_items(app, &[&change_file, &always_top, &click_through, &launch_login, &sep, &quit])?;

            TrayIconBuilder::with_id("main")
                .icon(app.default_window_icon().unwrap().clone())
                .menu(&menu)
                .tooltip("ob-widget  •  CmdOrCtrl+Shift+O")
                .on_menu_event(|app, event| {
                    let state = app.state::<AppState>();
                    match event.id().as_ref() {
                        "change_file" => {
                            use tauri_plugin_dialog::DialogExt;
                            let app_cb = app.clone();
                            app.clone().dialog()
                                .file()
                                .add_filter("Markdown", &["md", "MD"])
                                .pick_file(move |file_path| {
                                    let Some(fp) = file_path else { return };
                                    let path_str = fp.to_string();
                                    let path = std::path::Path::new(&path_str);
                                    let vault_path = path.parent()
                                        .map(|p| p.to_string_lossy().to_string())
                                        .unwrap_or_default();
                                    let target_file = path.file_name()
                                        .map(|f| f.to_string_lossy().to_string())
                                        .unwrap_or_else(|| "Tasks.md".to_string());
                                    let state = app_cb.state::<AppState>();
                                    let task_file = {
                                        let mut s = state.settings.lock().unwrap();
                                        s.vault_path = vault_path;
                                        s.target_file = target_file;
                                        target_file_path(&s)
                                    };
                                    let settings = state.settings.lock().unwrap().clone();
                                    persist_settings(&app_cb, &settings);
                                    if let Some(task_path) = task_file {
                                        let mut wg = state.watcher.lock().unwrap();
                                        start_watcher(&app_cb, task_path, Arc::clone(&state.ignore_change), &mut wg);
                                    }
                                    app_cb.emit("settings-changed", settings).ok();
                                });
                        }
                        "always_top" => {
                            let new_val = {
                                let mut s = state.settings.lock().unwrap();
                                s.always_on_top = !s.always_on_top;
                                s.always_on_top
                            };
                            if let Some(window) = app.get_webview_window("main") {
                                window.set_always_on_top(new_val).ok();
                            }
                            let settings = state.settings.lock().unwrap().clone();
                            persist_settings(app, &settings);
                        }
                        "click_through" => {
                            {
                                let mut s = state.settings.lock().unwrap();
                                s.click_through_on_blur = !s.click_through_on_blur;
                            }
                            let settings = state.settings.lock().unwrap().clone();
                            persist_settings(app, &settings);
                            app.emit("settings-changed", settings).ok();
                        }
                        "launch_login" => {
                            use tauri_plugin_autostart::ManagerExt;
                            let enabled = app.autolaunch().is_enabled().unwrap_or(false);
                            if enabled {
                                app.autolaunch().disable().ok();
                            } else {
                                app.autolaunch().enable().ok();
                            }
                        }
                        "quit" => {
                            if let Some(window) = app.get_webview_window("main") {
                                if let (Ok(pos), Ok(size)) = (window.outer_position(), window.inner_size()) {
                                    let mut s = state.settings.lock().unwrap();
                                    s.window.x = pos.x;
                                    s.window.y = pos.y;
                                    s.window.width = size.width;
                                    s.window.height = size.height;
                                    drop(s);
                                    let settings = state.settings.lock().unwrap().clone();
                                    persist_settings(app, &settings);
                                }
                            }
                            app.exit(0);
                        }
                        _ => {}
                    }
                })
                .on_tray_icon_event(|tray, event| {
                    if let TrayIconEvent::Click {
                        button: MouseButton::Left,
                        button_state: MouseButtonState::Up,
                        ..
                    } = event
                    {
                        let app = tray.app_handle();
                        if let Some(window) = app.get_webview_window("main") {
                            if window.is_visible().unwrap_or(false) {
                                window.hide().ok();
                            } else {
                                window.show().ok();
                                window.set_focus().ok();
                            }
                        }
                    }
                })
                .build(app)?;

            Ok(())
        })
        .on_window_event(|window, event| {
            match event {
                tauri::WindowEvent::CloseRequested { api, .. } => {
                    let app = window.app_handle();
                    if let (Ok(pos), Ok(size)) = (window.outer_position(), window.inner_size()) {
                        let state = app.state::<AppState>();
                        let mut s = state.settings.lock().unwrap();
                        s.window.x = pos.x;
                        s.window.y = pos.y;
                        s.window.width = size.width;
                        s.window.height = size.height;
                        drop(s);
                        let settings = state.settings.lock().unwrap().clone();
                        persist_settings(app, &settings);
                    }
                    window.hide().ok();
                    api.prevent_close();
                }
                tauri::WindowEvent::Focused(false) => {
                    let app = window.app_handle();
                    let state = app.state::<AppState>();
                    let mut override_flag = state.hotkey_aot_override.lock().unwrap();
                    if *override_flag {
                        *override_flag = false;
                        let saved_aot = state.settings.lock().unwrap().always_on_top;
                        window.set_always_on_top(saved_aot).ok();
                    }
                }
                _ => {}
            }
        })
        .invoke_handler(tauri::generate_handler![
            get_settings,
            save_settings,
            read_tasks,
            write_tasks,
            add_task,
            delete_task,
            pick_task_file,
            read_note,
            set_opacity,
            update_shortcut,
            add_reminder,
            get_reminders,
            delete_reminder,
            set_window_blur,
            pick_reminder_sound,
            read_reminder_sound,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
