use notify::{Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
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

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Settings {
    pub vault_path: String,
    pub target_file: String,
    pub window: WindowConfig,
    pub always_on_top: bool,
    pub click_through_on_blur: bool,
    #[serde(default = "default_theme")]
    pub theme: String,
    /// Window opacity 0.0–1.0 (default 1.0)
    #[serde(default = "default_opacity")]
    pub opacity: f64,
    /// Global shortcut to bring widget to front
    #[serde(default = "default_shortcut")]
    pub shortcut: String,
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
        }
    }
}

/// A single rendered item from the note file.
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct NoteItem {
    /// 0-indexed line number in the source file
    pub line_idx: usize,
    /// "task", "heading", "separator", "text"
    pub kind: String,
    /// Raw display text (for tasks: just the task content, not the `- [ ]` prefix)
    pub text: String,
    /// Only meaningful when kind == "task"
    pub done: bool,
    /// Only meaningful when kind == "task"
    pub task_id: usize,
    /// Heading level (1–6), only set when kind == "heading"
    pub level: u8,
    /// Indentation depth (number of leading spaces / 2), for sub-tasks
    pub indent: usize,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Task {
    pub id: usize,
    pub done: bool,
    pub text: String,
    pub line_idx: usize,
}

pub struct AppState {
    pub settings: Mutex<Settings>,
    pub watcher: Mutex<Option<RecommendedWatcher>>,
    pub ignore_change: Arc<Mutex<bool>>,
    /// Set to true when the hotkey temporarily overrides always-on-top.
    /// Cleared and restored when the window next loses focus.
    pub hotkey_aot_override: Arc<Mutex<bool>>,
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
    std::fs::read_to_string(&path)
        .ok()
        .and_then(|c| serde_json::from_str(&c).ok())
        .unwrap_or_default()
}

fn persist_settings(app: &AppHandle, settings: &Settings) {
    let path = settings_path(app);
    if let Ok(json) = serde_json::to_string_pretty(settings) {
        std::fs::write(path, json).ok();
    }
}

/// Resolves the target file path, guarding against path traversal.
/// `target_file` may contain subdirectory components (e.g. `Daily/Tasks.md`)
/// but must not escape the vault via `..`.
fn target_file_path(settings: &Settings) -> Option<PathBuf> {
    if settings.vault_path.is_empty() {
        return None;
    }
    let target = std::path::Path::new(&settings.target_file);
    // Reject any path that contains a parent-directory (`..`) component.
    let has_traversal = target
        .components()
        .any(|c| matches!(c, std::path::Component::ParentDir));
    if has_traversal {
        return None;
    }
    Some(PathBuf::from(&settings.vault_path).join(target))
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

/// Parse every line of the note into a typed NoteItem for rich display.
fn parse_note_lines(content: &str) -> Vec<NoteItem> {
    let mut task_id = 0usize;
    content
        .lines()
        .enumerate()
        .map(|(line_idx, line)| {
            let indent_chars = line.chars().take_while(|c| c.is_whitespace()).count();
            let indent = indent_chars / 2;
            let t = line.trim_start();

            // Task line
            if t.starts_with("- [ ]") || t.starts_with("- [x]") || t.starts_with("- [X]") {
                let done = t.starts_with("- [x]") || t.starts_with("- [X]");
                let text = t[5..].trim().to_string();
                let id = task_id;
                task_id += 1;
                return NoteItem { line_idx, kind: "task".to_string(), text, done, task_id: id, level: 0, indent };
            }

            // Heading
            if t.starts_with('#') {
                let level = t.chars().take_while(|c| *c == '#').count() as u8;
                let text = t[level as usize..].trim().to_string();
                return NoteItem { line_idx, kind: "heading".to_string(), text, done: false, task_id: 0, level, indent: 0 };
            }

            // Horizontal rule
            let stripped = t.replace('-', "").replace('*', "").replace('_', "").replace(' ', "");
            if stripped.is_empty() && (t.contains("---") || t.contains("***") || t.contains("___")) {
                return NoteItem { line_idx, kind: "separator".to_string(), text: String::new(), done: false, task_id: 0, level: 0, indent: 0 };
            }

            // Bullet point (non-task list item: "- text" or "* text")
            if t.starts_with("- ") || t.starts_with("* ") {
                let text = t[2..].trim().to_string();
                return NoteItem { line_idx, kind: "bullet".to_string(), text, done: false, task_id: 0, level: 0, indent };
            }

            // Plain text / empty
            NoteItem { line_idx, kind: "text".to_string(), text: t.to_string(), done: false, task_id: 0, level: 0, indent: 0 }
        })
        .collect()
}

/// Returns the 0-indexed line number of the FIRST task in the file, or None.
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
    // Apply acrylic blur — makes the window truly see-through on Windows.
    // The RGBA tint (0,0,0,0) means fully transparent tint.
    apply_acrylic(window, Some((0, 0, 0, 0))).ok();
}

#[cfg(target_os = "macos")]
fn apply_window_effects(window: &tauri::WebviewWindow) {
    use window_vibrancy::{apply_vibrancy, NSVisualEffectMaterial};
    apply_vibrancy(window, NSVisualEffectMaterial::HudWindow, None, None).ok();
}

#[cfg(not(any(target_os = "windows", target_os = "macos")))]
fn apply_window_effects(_window: &tauri::WebviewWindow) {}

// ── File watcher ───────────────────────────────────────────────────────────────


fn start_watcher(
    app: &AppHandle,
    file_path: PathBuf,
    ignore: Arc<Mutex<bool>>,
    watcher_guard: &mut Option<RecommendedWatcher>,
) {
    *watcher_guard = None; // drop old watcher

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
    // Guard against reading unexpectedly large files (> 1 MB).
    let size = std::fs::metadata(&path)
        .map(|m| m.len())
        .unwrap_or(0);
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
    // Strip newlines to prevent markdown injection.
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
        // Insert the new task BEFORE the first existing task line.
        let mut lines: Vec<&str> = content.lines().collect();
        lines.insert(first_idx, new_line.trim_end_matches('\n'));
        let mut result = lines.join("\n");
        if content.ends_with('\n') { result.push('\n'); }
        result
    } else {
        // No tasks yet — append at end.
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
    // Unregister old
    app.global_shortcut().unregister(old_shortcut.as_str()).ok();
    // Register new
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

// ── Entry point ────────────────────────────────────────────────────────────────

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
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
                        // If not already always-on-top, set a temporary override so
                        // the widget pops above everything just this once.
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

            // Apply window config from saved settings
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

            // Build app state — clone shortcut before `settings` is moved in
            let initial_shortcut = settings.shortcut.clone();
            let ignore_change = Arc::new(Mutex::new(false));
            let hotkey_aot_override = Arc::new(Mutex::new(false));
            let state = AppState {
                watcher: Mutex::new(None),
                ignore_change: Arc::clone(&ignore_change),
                hotkey_aot_override: Arc::clone(&hotkey_aot_override),
                settings: Mutex::new(settings),
            };
            app.manage(state);

            // Start file watcher if vault is configured
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

            // Register global hotkey from saved settings
            // Use the `settings` local (loaded above) to avoid borrow-after-move issues.
            use tauri_plugin_global_shortcut::GlobalShortcutExt;
            let saved_shortcut = initial_shortcut;
            if let Err(e) = app.global_shortcut().register(saved_shortcut.as_str()) {
                eprintln!("Could not register global shortcut '{saved_shortcut}': {e}");
            }

            // Build tray menu
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
                // When the window loses focus, undo any hotkey-triggered AOT override.
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
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
