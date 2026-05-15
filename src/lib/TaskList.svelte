<script lang="ts">
  import type { Task } from './types';

  interface Props {
    tasks: Task[];
    ontoggle: (task: Task) => void;
    onedit: (task: Task, text: string) => void;
    ondelete: (task: Task) => void;
  }

  let { tasks, ontoggle, onedit, ondelete }: Props = $props();

  let editingId = $state<number | null>(null);
  let editText = $state('');

  function startEdit(task: Task) {
    editingId = task.id;
    editText = task.text;
  }

  function commitEdit(task: Task) {
    if (editingId === task.id && editText.trim()) {
      onedit(task, editText.trim());
    }
    editingId = null;
  }

  function handleEditKey(e: KeyboardEvent, task: Task) {
    if (e.key === 'Enter') { commitEdit(task); }
    else if (e.key === 'Escape') { editingId = null; }
  }
</script>

<ul class="task-list" role="list">
  {#each tasks as task (task.id)}
    <li class="task-item" class:done={task.done}>
      <button
        class="checkbox"
        onclick={() => ontoggle(task)}
        aria-label={task.done ? 'Mark incomplete' : 'Mark complete'}
        aria-checked={task.done}
        role="checkbox"
      >
        {#if task.done}
          <svg viewBox="0 0 16 16" fill="currentColor" aria-hidden="true">
            <path d="M13.78 4.22a.75.75 0 0 1 0 1.06l-7.25 7.25a.75.75 0 0 1-1.06 0L2.22 9.28a.75.75 0 0 1 1.06-1.06L6 10.94l6.72-6.72a.75.75 0 0 1 1.06 0Z"/>
          </svg>
        {/if}
      </button>

      {#if editingId === task.id}
        <!-- svelte-ignore a11y_autofocus -->
        <input
          class="task-edit"
          type="text"
          bind:value={editText}
          onblur={() => commitEdit(task)}
          onkeydown={(e) => handleEditKey(e, task)}
          autofocus
        />
      {:else}
        <span
          class="task-text"
          ondblclick={() => startEdit(task)}
          role="none"
          title="Double-click to edit"
        >{task.text}</span>
      {/if}

      <button class="delete-btn" onclick={() => ondelete(task)} aria-label="Delete task" tabindex="-1">
        ×
      </button>
    </li>
  {/each}
</ul>

<style>
  .task-list {
    list-style: none;
    flex: 1;
    overflow-y: auto;
    scrollbar-width: thin;
    scrollbar-color: rgba(128,128,128,0.3) transparent;
  }

  .task-item {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 5px 10px;
    border-radius: 5px;
    transition: background 0.1s;
    min-height: 30px;
  }

  .task-item:hover {
    background: rgba(128, 128, 128, 0.12);
  }

  .task-item:hover .delete-btn {
    opacity: 1;
  }

  .checkbox {
    flex-shrink: 0;
    width: 16px;
    height: 16px;
    border-radius: 4px;
    border: 1.5px solid currentColor;
    background: transparent;
    cursor: pointer;
    display: flex;
    align-items: center;
    justify-content: center;
    padding: 0;
    color: inherit;
    opacity: 0.6;
    transition: opacity 0.1s, background 0.1s;
  }

  .checkbox:hover {
    opacity: 1;
    background: rgba(128, 128, 128, 0.15);
  }

  .task-item.done .checkbox {
    background: rgba(128, 128, 128, 0.2);
  }

  .checkbox svg {
    width: 10px;
    height: 10px;
  }

  .task-text {
    flex: 1;
    font-size: 12.5px;
    line-height: 1.4;
    cursor: default;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    opacity: 0.9;
  }

  .task-item.done .task-text {
    text-decoration: line-through;
    opacity: 0.4;
  }

  .task-edit {
    flex: 1;
    font-size: 12.5px;
    font-family: inherit;
    background: rgba(128, 128, 128, 0.1);
    border: none;
    border-radius: 3px;
    color: inherit;
    outline: none;
    padding: 1px 4px;
    user-select: text;
  }

  .delete-btn {
    flex-shrink: 0;
    width: 18px;
    height: 18px;
    border-radius: 4px;
    border: none;
    background: transparent;
    cursor: pointer;
    opacity: 0;
    font-size: 15px;
    line-height: 1;
    color: inherit;
    display: flex;
    align-items: center;
    justify-content: center;
    padding: 0;
    transition: opacity 0.1s, background 0.1s;
  }

  .delete-btn:hover {
    background: rgba(255, 80, 80, 0.25);
    opacity: 1 !important;
  }
</style>
