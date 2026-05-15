<script lang="ts">
  import { onMount } from 'svelte';
  import { invoke } from '@tauri-apps/api/core';
  import { listen } from '@tauri-apps/api/event';
  import { getCurrentWebviewWindow } from '@tauri-apps/api/webviewWindow';
  import TaskList from '$lib/TaskList.svelte';
  import type { Task, Settings } from '$lib/types';

  const appWindow = getCurrentWebviewWindow();

  let tasks       = $state<Task[]>([]);
  let settings    = $state<Settings | null>(null);
  let newTaskText = $state('');
  let loading     = $state(true);
  let error       = $state<string | null>(null);

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

  // ── Resize dragging ────────────────────────────────────────────────────────
  type ResizeDir = 'North' | 'South' | 'East' | 'West' |
                   'NorthEast' | 'NorthWest' | 'SouthEast' | 'SouthWest';

  function onResizeMousedown(e: MouseEvent, dir: ResizeDir) {
    if (e.buttons === 1) (appWindow as any).startResizeDragging(dir);
  }

  // ── Tasks ──────────────────────────────────────────────────────────────────
  async function loadTasks() {
    try {
      tasks = await invoke<Task[]>('read_tasks');
      error = null;
    } catch (e) {
      error = String(e);
    }
  }

  async function loadSettings() {
    settings = await invoke<Settings>('get_settings');
  }

  onMount(() => {
    let unlistenFile: (() => void) | undefined;
    let unlistenSettings: (() => void) | undefined;

    (async () => {
      await loadSettings();
      if (settings?.vault_path) await loadTasks();
      applyTheme((settings?.theme ?? 'system') as Theme);
      loading = false;

      unlistenFile = await listen('file-changed', () => loadTasks());
      unlistenSettings = await listen<Settings>('settings-changed', (e) => {
        settings = e.payload;
        applyTheme((settings?.theme ?? 'system') as Theme);
        if (settings?.vault_path) loadTasks();
      });
    })();

    return () => { unlistenFile?.(); unlistenSettings?.(); };
  });

  async function handleToggle(task: Task) {
    const updated = tasks.map(t => t.id === task.id ? { ...t, done: !t.done } : t);
    tasks = updated;
    await invoke('write_tasks', { tasks: updated });
  }

  async function handleEdit(task: Task, text: string) {
    const updated = tasks.map(t => t.id === task.id ? { ...t, text } : t);
    tasks = updated;
    await invoke('write_tasks', { tasks: updated });
  }

  async function handleDelete(task: Task) {
    tasks = tasks.filter(t => t.id !== task.id);
    await invoke('delete_task', { lineIdx: task.line_idx });
  }

  async function handleAddTask() {
    const text = newTaskText.trim();
    if (!text) return;
    newTaskText = '';
    await invoke('add_task', { text });
    await loadTasks();
  }

  async function pickVault() {
    const path = await invoke<string | null>('pick_vault_folder');
    if (path && settings) {
      const updated = { ...settings, vault_path: path };
      await invoke('save_settings', { settings: updated });
      settings = updated;
      await loadTasks();
    }
  }

  let pendingTasks = $derived(tasks.filter(t => !t.done));
  let currentTheme = $derived((settings?.theme ?? 'system') as Theme);
</script>

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
      {#if tasks.length > 0}
        <span class="header-count" data-tauri-drag-region>
          {pendingTasks.length}/{tasks.length}
        </span>
      {/if}

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
    </div>
  </div>

  <!-- Body -->
  {#if loading}
    <div class="state-msg">Loading…</div>

  {:else if !settings?.vault_path}
    <div class="setup">
      <p class="setup-title">No vault configured</p>
      <p class="setup-hint">Point the widget to your Obsidian vault folder.</p>
      <button class="setup-btn" onclick={pickVault}>Choose Vault Folder</button>
    </div>

  {:else if error}
    <div class="state-msg error">
      <p>Could not read file:</p>
      <code>{error}</code>
      <button class="setup-btn" onclick={pickVault} style="margin-top:12px">Change Vault</button>
    </div>

  {:else if tasks.length === 0}
    <div class="state-msg">
      No tasks in <em>{settings.target_file}</em>
    </div>

  {:else}
    <TaskList tasks={tasks} ontoggle={handleToggle} onedit={handleEdit} ondelete={handleDelete} />
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
    background: light-dark(rgba(242, 242, 247, 0.75), rgba(28, 28, 30, 0.75));
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
  }

  .header-count {
    font-size: 11px;
    opacity: 0.4;
    font-variant-numeric: tabular-nums;
    padding-right: 4px;
    cursor: move;
  }

  /* ── Icon buttons (theme + AOT) ──────────────────────────────────────────── */
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
</style>
