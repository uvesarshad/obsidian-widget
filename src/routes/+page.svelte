<script lang="ts">
  import { onMount } from 'svelte';
  import { invoke } from '@tauri-apps/api/core';
  import { listen } from '@tauri-apps/api/event';
  import { getCurrentWebviewWindow } from '@tauri-apps/api/webviewWindow';
  import TaskList from '$lib/TaskList.svelte';
  import type { Task, Settings } from '$lib/types';

  const appWindow = getCurrentWebviewWindow();

  let tasks = $state<Task[]>([]);
  let settings = $state<Settings | null>(null);
  let newTaskText = $state('');
  let loading = $state(true);
  let error = $state<string | null>(null);

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
      loading = false;

      unlistenFile = await listen('file-changed', () => loadTasks());
      unlistenSettings = await listen<Settings>('settings-changed', (e) => {
        settings = e.payload;
        if (settings?.vault_path) loadTasks();
      });
    })();

    return () => {
      unlistenFile?.();
      unlistenSettings?.();
    };
  });

  async function handleToggle(task: Task) {
    // Optimistic update
    const updated = tasks.map(t =>
      t.id === task.id ? { ...t, done: !t.done } : t
    );
    tasks = updated;
    await invoke('write_tasks', { tasks: updated });
  }

  async function handleEdit(task: Task, text: string) {
    const updated = tasks.map(t =>
      t.id === task.id ? { ...t, text } : t
    );
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

  function handleAddKeydown(e: KeyboardEvent) {
    if (e.key === 'Enter') handleAddTask();
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

  function onHeaderMousedown(e: MouseEvent) {
    if (e.buttons === 1) appWindow.startDragging();
  }

  let pendingTasks = $derived(tasks.filter(t => !t.done));
  let doneTasks = $derived(tasks.filter(t => t.done));
</script>

<div class="widget">
  <!-- Header / drag handle -->
  <div class="header" onmousedown={onHeaderMousedown} role="none">
    <span class="header-title">
      {#if settings?.target_file}
        {settings.target_file.replace(/\.md$/i, '')}
      {:else}
        ob-widget
      {/if}
    </span>
    {#if tasks.length > 0}
      <span class="header-count">{pendingTasks.length}/{tasks.length}</span>
    {/if}
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
    <TaskList
      tasks={tasks}
      ontoggle={handleToggle}
      onedit={handleEdit}
      ondelete={handleDelete}
    />
  {/if}

  <!-- Add task input (only when configured) -->
  {#if settings?.vault_path && !error}
    <div class="add-row">
      <input
        class="add-input"
        type="text"
        placeholder="New task…"
        bind:value={newTaskText}
        onkeydown={handleAddKeydown}
      />
    </div>
  {/if}
</div>

<style>
  .widget {
    display: flex;
    flex-direction: column;
    width: 100vw;
    height: 100vh;
    color: light-dark(#1a1a1a, #e8e8e8);
    background: light-dark(rgba(242, 242, 247, 0.72), rgba(28, 28, 30, 0.72));
    border-radius: 10px;
    overflow: hidden;
  }

  /* Fallback for browsers without light-dark() — Mica handles the real bg anyway */
  @media (prefers-color-scheme: dark) {
    .widget {
      color: #e8e8e8;
      background: rgba(28, 28, 30, 0.72);
    }
  }

  @media (prefers-color-scheme: light) {
    .widget {
      color: #1a1a1a;
      background: rgba(242, 242, 247, 0.72);
    }
  }

  /* Header */
  .header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 8px 12px 6px;
    cursor: move;
    border-bottom: 1px solid rgba(128, 128, 128, 0.15);
    flex-shrink: 0;
  }

  .header-title {
    font-size: 12px;
    font-weight: 600;
    opacity: 0.7;
    letter-spacing: 0.02em;
    text-transform: uppercase;
  }

  .header-count {
    font-size: 11px;
    opacity: 0.45;
    font-variant-numeric: tabular-nums;
  }

  /* States */
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

  .state-msg.error {
    opacity: 0.8;
    gap: 8px;
  }

  .state-msg.error code {
    font-size: 11px;
    opacity: 0.6;
    word-break: break-all;
  }

  /* Setup screen */
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

  .setup-title {
    font-weight: 600;
    font-size: 13px;
  }

  .setup-hint {
    font-size: 11.5px;
    opacity: 0.55;
    line-height: 1.5;
  }

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

  .setup-btn:hover {
    background: rgba(128, 128, 128, 0.22);
  }

  /* Add-task row */
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

  .add-input::placeholder {
    opacity: 0.4;
  }

  .add-input:focus {
    border-color: rgba(128, 128, 128, 0.45);
    background: rgba(128, 128, 128, 0.14);
  }
</style>
