<script lang="ts">
  import { onMount } from 'svelte';
  import { invoke } from '@tauri-apps/api/core';
  import { listen } from '@tauri-apps/api/event';
  import { getCurrentWebviewWindow } from '@tauri-apps/api/webviewWindow';
  import NoteView from '$lib/NoteView.svelte';
  import type { Task, NoteItem, Settings } from '$lib/types';

  const appWindow = getCurrentWebviewWindow();

  let noteItems   = $state<NoteItem[]>([]);
  let settings    = $state<Settings | null>(null);
  let newTaskText = $state('');
  let loading     = $state(true);
  let error       = $state<string | null>(null);

  // ── Transparency popover ────────────────────────────────────────────────────
  let showOpacitySlider = $state(false);
  let opacity = $state(1.0);

  async function handleOpacityChange(val: number) {
    opacity = val;
    // Control the background alpha via CSS custom property
    document.documentElement.style.setProperty('--bg-alpha', String(opacity));
    await invoke('set_opacity', { opacity });
  }

  function toggleOpacitySlider() {
    showOpacitySlider = !showOpacitySlider;
  }

  // Close popover when clicking outside
  function handleGlobalClick(e: MouseEvent) {
    const target = e.target as HTMLElement;
    if (showOpacitySlider && !target.closest('.opacity-wrap')) showOpacitySlider = false;
    if (showShortcutPopover && !target.closest('.shortcut-wrap')) {
      showShortcutPopover = false;
      recordingShortcut = false;
    }
  }

  // ── Theme ──────────────────────────────────────────────────────────────────
  type Theme = 'system' | 'light' | 'dark';
  const themeOrder: Theme[] = ['system', 'light', 'dark'];

  function applyTheme(theme: Theme) {
    if (theme === 'system') {
      document.documentElement.removeAttribute('data-theme');
    } else {
      document.documentElement.setAttribute('data-theme', theme);
    }
  }

  async function cycleTheme() {
    if (!settings) return;
    const cur = (settings.theme ?? 'system') as Theme;
    const next = themeOrder[(themeOrder.indexOf(cur) + 1) % themeOrder.length];
    const updated = { ...settings, theme: next };
    settings = updated;
    applyTheme(next);
    await invoke('save_settings', { settings: updated });
  }

  // ── Always-on-top ──────────────────────────────────────────────────────────
  async function toggleAOT() {
    if (!settings) return;
    const newVal = !settings.always_on_top;
    const updated = { ...settings, always_on_top: newVal };
    settings = updated;
    await appWindow.setAlwaysOnTop(newVal);
    await invoke('save_settings', { settings: updated });
  }

  // ── Bring to top once (Ctrl+Shift+Space) ───────────────────────────────────
  async function bringToTopOnce() {
    await appWindow.setAlwaysOnTop(true);
    await appWindow.setFocus();
    // Restore the real AOT setting when the window next loses focus
    // The Rust side handles that via hotkey_aot_override.
    // From JS we mirror the same: if AOT was off, schedule restore on blur.
    if (settings && !settings.always_on_top) {
      const restore = async () => {
        await appWindow.setAlwaysOnTop(false);
        window.removeEventListener('blur', restore);
      };
      window.addEventListener('blur', restore, { once: true });
    }
  }

  // ── Resize dragging ────────────────────────────────────────────────────────
  type ResizeDir = 'North' | 'South' | 'East' | 'West' |
                   'NorthEast' | 'NorthWest' | 'SouthEast' | 'SouthWest';

  function onResizeMousedown(e: MouseEvent, dir: ResizeDir) {
    if (e.buttons === 1) (appWindow as any).startResizeDragging(dir);
  }

  // ── Note / Tasks ───────────────────────────────────────────────────────────
  async function loadNote() {
    try {
      noteItems = await invoke<NoteItem[]>('read_note');
      error = null;
    } catch (e) {
      error = String(e);
    }
  }

  async function loadSettings() {
    settings = await invoke<Settings>('get_settings');
    opacity = settings?.opacity ?? 1.0;
    document.documentElement.style.setProperty('--bg-alpha', String(opacity));
  }

  onMount(() => {
    let unlistenFile: (() => void) | undefined;
    let unlistenSettings: (() => void) | undefined;

    (async () => {
      await loadSettings();
      if (settings?.vault_path) await loadNote();
      applyTheme((settings?.theme ?? 'system') as Theme);
      loading = false;

      unlistenFile = await listen('file-changed', () => loadNote());
      unlistenSettings = await listen<Settings>('settings-changed', (e) => {
        settings = e.payload;
        applyTheme((settings?.theme ?? 'system') as Theme);
        if (settings?.vault_path) loadNote();
      });
    })();

    return () => { unlistenFile?.(); unlistenSettings?.(); };
  });

  // ── Task operations ────────────────────────────────────────────────────────
  // Build Task list from NoteItems for write_tasks
  function buildTasksFromItems(items: NoteItem[]): Task[] {
    return items
      .filter(i => i.kind === 'task')
      .map(i => ({ id: i.task_id, done: i.done, text: i.text, line_idx: i.line_idx }));
  }

  async function handleToggle(taskId: number, lineIdx: number) {
    noteItems = noteItems.map(i =>
      i.kind === 'task' && i.task_id === taskId
        ? { ...i, done: !i.done }
        : i
    );
    await invoke('write_tasks', { tasks: buildTasksFromItems(noteItems) });
  }

  async function handleEdit(taskId: number, lineIdx: number, text: string) {
    noteItems = noteItems.map(i =>
      i.kind === 'task' && i.task_id === taskId
        ? { ...i, text }
        : i
    );
    await invoke('write_tasks', { tasks: buildTasksFromItems(noteItems) });
  }

  async function handleDelete(lineIdx: number) {
    noteItems = noteItems.filter(i => i.line_idx !== lineIdx);
    await invoke('delete_task', { lineIdx });
  }

  async function handleAddTask() {
    const text = newTaskText.trim();
    if (!text) return;
    newTaskText = '';
    await invoke('add_task', { text });
    await loadNote();
  }

  async function pickVault() {
    await invoke('pick_task_file');
  }

  let taskItems    = $derived(noteItems.filter(i => i.kind === 'task'));
  let pendingTasks = $derived(taskItems.filter(i => !i.done));
  let currentTheme = $derived((settings?.theme ?? 'system') as Theme);
  let hasContent   = $derived(noteItems.length > 0);

  // ── Shortcut popover ────────────────────────────────────────────────────────
  let showShortcutPopover = $state(false);
  let recordingShortcut   = $state(false);
  let shortcutError       = $state('');

  function currentShortcutDisplay() {
    return settings?.shortcut ?? 'CmdOrControl+Shift+O';
  }

  function keyToAccelerator(e: KeyboardEvent): string | null {
    const mods: string[] = [];
    if (e.ctrlKey)  mods.push('Ctrl');
    if (e.shiftKey) mods.push('Shift');
    if (e.altKey)   mods.push('Alt');
    const key = e.key;
    if (['Control','Shift','Alt','Meta'].includes(key)) return null;
    const map: Record<string, string> = {
      ' ': 'Space', 'ArrowUp': 'Up', 'ArrowDown': 'Down',
      'ArrowLeft': 'Left', 'ArrowRight': 'Right',
      'Enter': 'Return', 'Backspace': 'Backspace',
      'Delete': 'Delete', 'Escape': 'Escape', 'Tab': 'Tab',
    };
    const k = map[key] ?? (key.length === 1 ? key.toUpperCase() : key);
    mods.push(k);
    return mods.join('+');
  }

  async function handleShortcutKeydown(e: KeyboardEvent) {
    if (!recordingShortcut) return;
    e.preventDefault(); e.stopPropagation();
    const acc = keyToAccelerator(e);
    if (!acc) return;
    recordingShortcut = false;
    shortcutError = '';
    try {
      await invoke('update_shortcut', { shortcut: acc });
      if (settings) settings = { ...settings, shortcut: acc };
    } catch (err) {
      shortcutError = String(err);
    }
  }
</script>

<svelte:window onclick={handleGlobalClick} onkeydown={handleShortcutKeydown} />

<!-- Invisible resize handles on all 8 edges/corners -->
<div class="rh rh-n"  onmousedown={(e) => onResizeMousedown(e, 'North')}     role="none"></div>
<div class="rh rh-s"  onmousedown={(e) => onResizeMousedown(e, 'South')}     role="none"></div>
<div class="rh rh-e"  onmousedown={(e) => onResizeMousedown(e, 'East')}      role="none"></div>
<div class="rh rh-w"  onmousedown={(e) => onResizeMousedown(e, 'West')}      role="none"></div>
<div class="rh rh-ne" onmousedown={(e) => onResizeMousedown(e, 'NorthEast')} role="none"></div>
<div class="rh rh-nw" onmousedown={(e) => onResizeMousedown(e, 'NorthWest')} role="none"></div>
<div class="rh rh-se" onmousedown={(e) => onResizeMousedown(e, 'SouthEast')} role="none"></div>
<div class="rh rh-sw" onmousedown={(e) => onResizeMousedown(e, 'SouthWest')} role="none"></div>

<div class="widget">
  <!-- Header — data-tauri-drag-region makes the whole row draggable;
       buttons inside are automatically excluded from drag detection -->
  <div class="header" data-tauri-drag-region role="none">
    <span class="header-title" data-tauri-drag-region>
      {settings?.target_file?.replace(/\.md$/i, '') ?? 'ob-widget'}
    </span>

    <div class="header-right" data-tauri-drag-region>
      {#if taskItems.length > 0}
        <span class="header-count" data-tauri-drag-region>
          {pendingTasks.length}/{taskItems.length}
        </span>
      {/if}

      <!-- Opacity control -->
      <div class="opacity-wrap" data-tauri-drag-region>
        <button
          class="icon-btn"
          id="opacity-btn"
          onclick={(e) => { e.stopPropagation(); toggleOpacitySlider(); }}
          title="Adjust transparency"
          aria-label="Adjust transparency"
          data-tauri-drag-region="false"
        >
          <!-- Layers / opacity icon -->
          <svg viewBox="0 0 16 16" fill="currentColor">
            <circle cx="8" cy="8" r="5.5" fill="none" stroke="currentColor" stroke-width="1.4" opacity="0.9"/>
            <path d="M8 2.5A5.5 5.5 0 0 1 8 13.5Z" fill="currentColor"/>
          </svg>
        </button>

        {#if showOpacitySlider}
          <div class="opacity-popover" role="none" onclick={(e) => e.stopPropagation()} onkeydown={(e) => e.stopPropagation()}>
            <span class="opacity-label">{Math.round(opacity * 100)}%</span>
            <input
              id="opacity-slider"
              class="opacity-slider"
              type="range"
              min="0.15"
              max="1"
              step="0.05"
              value={opacity}
              oninput={(e) => handleOpacityChange(parseFloat((e.target as HTMLInputElement).value))}
            />
          </div>
        {/if}
      </div>

      <!-- Theme toggle: cycles system → light → dark -->
      <button
        class="icon-btn"
        onclick={cycleTheme}
        title="Theme: {currentTheme}"
        aria-label="Cycle theme ({currentTheme})"
      >
        {#if currentTheme === 'light'}
          <!-- Sun -->
          <svg viewBox="0 0 16 16" fill="currentColor">
            <circle cx="8" cy="8" r="3"/>
            <path d="M8 1v2M8 13v2M1 8h2M13 8h2M3.22 3.22l1.42 1.42M11.36 11.36l1.42 1.42M3.22 12.78l1.42-1.42M11.36 4.64l1.42-1.42" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" fill="none"/>
          </svg>
        {:else if currentTheme === 'dark'}
          <!-- Moon -->
          <svg viewBox="0 0 16 16" fill="currentColor">
            <path d="M6 2a6 6 0 1 0 8 8 4.5 4.5 0 0 1-8-8Z"/>
          </svg>
        {:else}
          <!-- System (half circle) -->
          <svg viewBox="0 0 16 16" fill="currentColor">
            <path d="M8 2a6 6 0 1 0 0 12A6 6 0 0 0 8 2Zm0 1v10a5 5 0 0 1 0-10Z"/>
          </svg>
        {/if}
      </button>

      <!-- Always-on-top toggle -->
      <button
        class="icon-btn"
        class:active={settings?.always_on_top}
        onclick={toggleAOT}
        title={settings?.always_on_top ? 'Always on top: on' : 'Always on top: off'}
        aria-label="Toggle always on top"
      >
        <!-- Pin icon — filled when active -->
        {#if settings?.always_on_top}
          <svg viewBox="0 0 16 16" fill="currentColor">
            <path d="M9.5 1h-3l-.5 4H4l1 2h2v4l1 3 1-3V7h2l1-2H10L9.5 1Z"/>
          </svg>
        {:else}
          <svg viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.4" stroke-linecap="round">
            <path d="M9.5 1h-3l-.5 4H4l1 2h2v4l1 3 1-3V7h2l1-2H10L9.5 1Z"/>
          </svg>
        {/if}
      </button>

      <!-- Bring-to-top-once + shortcut config -->
      <div class="shortcut-wrap" data-tauri-drag-region>
        <button
          class="icon-btn"
          onclick={(e) => { e.stopPropagation(); bringToTopOnce(); }}
          oncontextmenu={(e) => { e.preventDefault(); e.stopPropagation(); showShortcutPopover = !showShortcutPopover; recordingShortcut = false; }}
          title="Bring to top once ({currentShortcutDisplay()}) — right-click to change shortcut"
          aria-label="Bring to top once"
        >
          <svg viewBox="0 0 16 16" fill="currentColor">
            <path d="M8 2 L14 8 H10 V14 H6 V8 H2 Z"/>
          </svg>
        </button>

        {#if showShortcutPopover}
          <div class="shortcut-popover" role="none"
            onclick={(e) => e.stopPropagation()}
            onkeydown={(e) => { e.stopPropagation(); handleShortcutKeydown(e); }}
          >
            <p class="shortcut-title">Bring-to-top shortcut</p>
            <button
              class="shortcut-recorder"
              class:recording={recordingShortcut}
              onclick={(e) => { e.stopPropagation(); recordingShortcut = true; shortcutError = ''; }}
            >
              {recordingShortcut ? 'Press keys…' : currentShortcutDisplay()}
            </button>
            {#if shortcutError}
              <p class="shortcut-error">{shortcutError}</p>
            {/if}
            <p class="shortcut-hint">Click above, then press your key combo</p>
          </div>
        {/if}
      </div>
    </div>
  </div>

  <!-- Body -->
  {#if loading}
    <div class="state-msg">Loading…</div>

  {:else if !settings?.vault_path}
    <div class="setup">
      <p class="setup-title">No note file selected</p>
      <p class="setup-hint">Choose an Obsidian note (.md file) to use as your task list.</p>
      <button class="setup-btn" onclick={pickVault}>Choose Note File</button>
    </div>

  {:else if error}
    <div class="state-msg error">
      <p>Could not read file:</p>
      <code>{error}</code>
      <button class="setup-btn" onclick={pickVault} style="margin-top:12px">Choose Note File</button>
    </div>

  {:else if !hasContent}
    <div class="state-msg">
      No content in <em>{settings.target_file}</em>
    </div>

  {:else}
    <NoteView
      items={noteItems}
      ontoggle={handleToggle}
      onedit={handleEdit}
      ondelete={handleDelete}
    />
  {/if}

  {#if settings?.vault_path && !error}
    <div class="add-row">
      <input
        class="add-input"
        type="text"
        placeholder="New task…"
        bind:value={newTaskText}
        onkeydown={(e) => e.key === 'Enter' && handleAddTask()}
      />
    </div>
  {/if}
</div>

<style>
  /* ── Theme via color-scheme ──────────────────────────────────────────────── */
  :global(:root)                     { color-scheme: light dark; }
  :global(:root[data-theme="light"]) { color-scheme: light; }
  :global(:root[data-theme="dark"])  { color-scheme: dark; }

  /* ── Resize handles ──────────────────────────────────────────────────────── */
  .rh { position: fixed; z-index: 9999; }
  .rh-n  { top: 0;    left: 6px;  right: 6px;  height: 5px; cursor: n-resize; }
  .rh-s  { bottom: 0; left: 6px;  right: 6px;  height: 5px; cursor: s-resize; }
  .rh-e  { right: 0;  top: 6px;   bottom: 6px; width: 5px;  cursor: e-resize; }
  .rh-w  { left: 0;   top: 6px;   bottom: 6px; width: 5px;  cursor: w-resize; }
  .rh-ne { top: 0;    right: 0;   width: 8px;  height: 8px; cursor: ne-resize; }
  .rh-nw { top: 0;    left: 0;    width: 8px;  height: 8px; cursor: nw-resize; }
  .rh-se { bottom: 0; right: 0;   width: 8px;  height: 8px; cursor: se-resize; }
  .rh-sw { bottom: 0; left: 0;    width: 8px;  height: 8px; cursor: sw-resize; }

  /* ── Widget shell ────────────────────────────────────────────────────────── */
  .widget {
    display: flex;
    flex-direction: column;
    width: 100vw;
    height: 100vh;
    color: light-dark(#1a1a1a, #e8e8e8);
    background: light-dark(
      rgb(255 255 255 / var(--bg-alpha, 0.85)),
      rgb(28 28 30 / var(--bg-alpha, 0.75))
    );
    border-radius: 10px;
    overflow: hidden;
  }

  /* ── Header ──────────────────────────────────────────────────────────────── */
  .header {
    display: flex;
    align-items: center;
    gap: 6px;
    padding: 7px 8px 6px 12px;
    cursor: move;
    border-bottom: 1px solid rgba(128, 128, 128, 0.15);
    flex-shrink: 0;
  }

  .header-title {
    font-size: 11.5px;
    font-weight: 600;
    opacity: 0.65;
    letter-spacing: 0.04em;
    text-transform: uppercase;
    flex: 1;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    cursor: move;
  }

  .header-right {
    display: flex;
    align-items: center;
    gap: 2px;
    flex-shrink: 0;
    cursor: move;
    position: relative;
  }

  .header-count {
    font-size: 11px;
    opacity: 0.4;
    font-variant-numeric: tabular-nums;
    padding-right: 4px;
    cursor: move;
  }

  /* ── Icon buttons ─────────────────────────────────────────────────────────── */
  .icon-btn {
    width: 22px;
    height: 22px;
    border-radius: 5px;
    border: none;
    background: transparent;
    color: inherit;
    cursor: pointer;
    display: flex;
    align-items: center;
    justify-content: center;
    padding: 0;
    opacity: 0.45;
    transition: opacity 0.12s, background 0.12s;
    flex-shrink: 0;
  }

  .icon-btn svg {
    width: 13px;
    height: 13px;
  }

  .icon-btn:hover {
    opacity: 0.9;
    background: rgba(128, 128, 128, 0.15);
  }

  .icon-btn.active {
    opacity: 0.85;
  }

  /* ── Opacity popover ──────────────────────────────────────────────────────── */
  .opacity-wrap {
    position: relative;
  }

  .opacity-popover {
    position: absolute;
    top: calc(100% + 6px);
    right: 0;
    background: light-dark(rgba(255, 255, 255, 0.97), rgba(38, 38, 42, 0.97));
    border: 1px solid rgba(128, 128, 128, 0.2);
    border-radius: 8px;
    padding: 8px 10px;
    display: flex;
    align-items: center;
    gap: 8px;
    z-index: 1000;
    box-shadow: 0 4px 16px rgba(0,0,0,0.18);
    white-space: nowrap;
    min-width: 160px;
  }

  .opacity-label {
    font-size: 11px;
    opacity: 0.65;
    font-variant-numeric: tabular-nums;
    min-width: 30px;
    text-align: right;
  }

  .opacity-slider {
    flex: 1;
    -webkit-appearance: none;
    appearance: none;
    height: 4px;
    border-radius: 2px;
    background: rgba(128,128,128,0.3);
    outline: none;
    cursor: pointer;
  }

  .opacity-slider::-webkit-slider-thumb {
    -webkit-appearance: none;
    width: 13px;
    height: 13px;
    border-radius: 50%;
    background: light-dark(#555, #ccc);
    cursor: pointer;
    transition: background 0.1s;
  }

  .opacity-slider::-webkit-slider-thumb:hover {
    background: light-dark(#222, #fff);
  }

  /* ── States ──────────────────────────────────────────────────────────────── */
  .state-msg {
    flex: 1;
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: 6px;
    opacity: 0.55;
    font-size: 12px;
    text-align: center;
    padding: 20px;
  }

  .state-msg.error { opacity: 0.8; gap: 8px; }
  .state-msg.error code { font-size: 11px; opacity: 0.6; word-break: break-all; }

  .setup {
    flex: 1;
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: 8px;
    padding: 24px 20px;
    text-align: center;
  }

  .setup-title { font-weight: 600; font-size: 13px; }

  .setup-hint { font-size: 11.5px; opacity: 0.55; line-height: 1.5; }

  .setup-btn {
    margin-top: 4px;
    padding: 6px 14px;
    border-radius: 6px;
    border: 1px solid rgba(128, 128, 128, 0.35);
    background: rgba(128, 128, 128, 0.12);
    color: inherit;
    font-family: inherit;
    font-size: 12px;
    cursor: pointer;
    transition: background 0.15s;
  }

  .setup-btn:hover { background: rgba(128, 128, 128, 0.22); }

  /* ── Add-task row ────────────────────────────────────────────────────────── */
  .add-row {
    flex-shrink: 0;
    padding: 6px 10px 8px;
    border-top: 1px solid rgba(128, 128, 128, 0.15);
  }

  .add-input {
    width: 100%;
    background: rgba(128, 128, 128, 0.1);
    border: 1px solid rgba(128, 128, 128, 0.2);
    border-radius: 5px;
    padding: 5px 8px;
    font-family: inherit;
    font-size: 12px;
    color: inherit;
    outline: none;
    user-select: text;
    transition: border-color 0.15s;
  }

  .add-input::placeholder { opacity: 0.4; }

  .add-input:focus {
    border-color: rgba(128, 128, 128, 0.45);
    background: rgba(128, 128, 128, 0.14);
  }

  /* ── Shortcut popover ─────────────────────────────────────────────────────── */
  .shortcut-wrap { position: relative; }

  .shortcut-popover {
    position: absolute;
    top: calc(100% + 6px);
    right: 0;
    background: light-dark(rgba(255,255,255,0.97), rgba(38,38,42,0.97));
    border: 1px solid rgba(128,128,128,0.2);
    border-radius: 8px;
    padding: 10px 12px;
    z-index: 1000;
    box-shadow: 0 4px 16px rgba(0,0,0,0.18);
    min-width: 180px;
    display: flex;
    flex-direction: column;
    gap: 6px;
  }

  .shortcut-title {
    font-size: 10.5px;
    font-weight: 600;
    opacity: 0.55;
    letter-spacing: 0.05em;
    text-transform: uppercase;
    margin: 0;
  }

  .shortcut-recorder {
    font-family: monospace;
    font-size: 12px;
    padding: 5px 10px;
    border-radius: 5px;
    border: 1px solid rgba(128,128,128,0.3);
    background: rgba(128,128,128,0.1);
    color: inherit;
    cursor: pointer;
    text-align: center;
    transition: border-color 0.12s, background 0.12s;
  }
  .shortcut-recorder.recording {
    border-color: rgba(100,150,255,0.6);
    background: rgba(100,150,255,0.08);
    animation: pulse 0.8s infinite alternate;
  }
  @keyframes pulse {
    from { opacity: 0.7; } to { opacity: 1; }
  }

  .shortcut-hint {
    font-size: 10px;
    opacity: 0.4;
    margin: 0;
    line-height: 1.4;
  }

  .shortcut-error {
    font-size: 10.5px;
    color: #e05050;
    margin: 0;
  }
</style>
