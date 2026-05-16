<script lang="ts">
  import type { NoteItem } from './types';

  interface Props {
    items: NoteItem[];
    ontoggle: (taskId: number, lineIdx: number) => void;
    onedit:   (taskId: number, lineIdx: number, text: string) => void;
    ondelete: (lineIdx: number) => void;
  }

  let { items, ontoggle, onedit, ondelete }: Props = $props();

  let editingLineIdx = $state<number | null>(null);
  let editText       = $state('');

  // ── Inline markdown → HTML (bold, italic, code, strikethrough) ──────────────
  function md(text: string): string {
    return text
      .replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;')
      .replace(/\*\*(.+?)\*\*/g, '<strong>$1</strong>')
      .replace(/__(.+?)__/g, '<strong>$1</strong>')
      .replace(/\*(.+?)\*/g, '<em>$1</em>')
      .replace(/_(.+?)_/g, '<em>$1</em>')
      .replace(/~~(.+?)~~/g, '<s>$1</s>')
      .replace(/`(.+?)`/g, '<code>$1</code>');
  }

  // ── Reorder: within each contiguous task block, undone first then done ──────
  let displayItems = $derived.by(() => {
    const result: NoteItem[] = [];
    let i = 0;
    while (i < items.length) {
      if (items[i].kind === 'task') {
        let j = i;
        while (j < items.length && items[j].kind === 'task') j++;
        const block = items.slice(i, j);
        const undone = block.filter(t => !t.done);
        const done   = block.filter(t =>  t.done);
        result.push(...undone, ...done);
        i = j;
      } else {
        result.push(items[i]);
        i++;
      }
    }
    return result;
  });

  function startEdit(item: NoteItem) {
    editingLineIdx = item.line_idx;
    editText       = item.text;
  }

  function commitEdit(item: NoteItem) {
    if (editingLineIdx === item.line_idx && editText.trim()) {
      onedit(item.task_id, item.line_idx, editText.trim());
    }
    editingLineIdx = null;
  }

  function handleEditKey(e: KeyboardEvent, item: NoteItem) {
    if (e.key === 'Enter')   commitEdit(item);
    else if (e.key === 'Escape') editingLineIdx = null;
  }
</script>

<div class="note-view" role="list">
  {#each displayItems as item (item.line_idx)}

    {#if item.kind === 'separator'}
      <hr class="note-sep" />

    {:else if item.kind === 'heading'}
      <div class="note-heading note-h{item.level}" role="heading" aria-level={item.level}>
        {@html md(item.text)}
      </div>

    {:else if item.kind === 'bullet'}
      <div class="note-bullet" style="padding-left: {8 + item.indent * 14}px" role="listitem">
        <span class="bullet-dot">•</span>
        <span class="bullet-text">{@html md(item.text)}</span>
      </div>

    {:else if item.kind === 'task'}
      <div
        class="task-item"
        class:done={item.done}
        style="padding-left: {8 + item.indent * 14}px"
        role="listitem"
      >
        <button
          class="checkbox"
          onclick={() => ontoggle(item.task_id, item.line_idx)}
          aria-label={item.done ? 'Mark incomplete' : 'Mark complete'}
          aria-checked={item.done}
          role="checkbox"
        >
          {#if item.done}
            <svg viewBox="0 0 16 16" fill="currentColor" aria-hidden="true">
              <path d="M13.78 4.22a.75.75 0 0 1 0 1.06l-7.25 7.25a.75.75 0 0 1-1.06 0L2.22 9.28a.75.75 0 0 1 1.06-1.06L6 10.94l6.72-6.72a.75.75 0 0 1 1.06 0Z"/>
            </svg>
          {/if}
        </button>

        {#if editingLineIdx === item.line_idx}
          <!-- svelte-ignore a11y_autofocus -->
          <input
            class="task-edit"
            type="text"
            bind:value={editText}
            onblur={() => commitEdit(item)}
            onkeydown={(e) => handleEditKey(e, item)}
            autofocus
          />
        {:else}
          <span
            class="task-text"
            ondblclick={() => startEdit(item)}
            role="none"
            title="Double-click to edit"
          >{@html md(item.text)}</span>
        {/if}

        <button class="delete-btn" onclick={() => ondelete(item.line_idx)} aria-label="Delete task" tabindex="-1">×</button>
      </div>

    {:else}
      <!-- Plain text line -->
      {#if item.text}
        <div class="note-text" role="none">{@html md(item.text)}</div>
      {:else}
        <div class="note-blank" role="none"></div>
      {/if}
    {/if}

  {/each}
</div>

<style>
  .note-view {
    flex: 1;
    overflow-y: auto;
    scrollbar-width: thin;
    scrollbar-color: rgba(128,128,128,0.3) transparent;
    padding: 2px 0;
  }

  /* ── Headings ── */
  .note-heading { font-weight: 700; padding: 6px 10px 2px; line-height: 1.3; }
  .note-h1 { font-size: 14px; opacity: 0.9; }
  .note-h2 { font-size: 13px; opacity: 0.8; }
  .note-h3 { font-size: 12.5px; opacity: 0.7; }
  .note-h4, .note-h5, .note-h6 { font-size: 12px; opacity: 0.6; }

  /* ── Separator ── */
  .note-sep {
    border: none;
    border-top: 1px solid rgba(128,128,128,0.2);
    margin: 4px 10px;
  }

  /* ── Bullet ── */
  .note-bullet {
    display: flex;
    align-items: baseline;
    gap: 5px;
    padding-top: 2px;
    padding-bottom: 2px;
    padding-right: 10px;
  }
  .bullet-dot { font-size: 10px; opacity: 0.5; flex-shrink: 0; line-height: 1.6; }
  .bullet-text { font-size: 12px; opacity: 0.75; line-height: 1.5; }

  /* ── Plain text ── */
  .note-text {
    font-size: 12px;
    padding: 1px 10px;
    opacity: 0.5;
    line-height: 1.5;
  }
  .note-blank { height: 4px; }

  /* ── Inline code from md() ── */
  :global(.note-text code), :global(.bullet-text code), :global(.task-text code) {
    font-family: monospace;
    font-size: 11px;
    background: rgba(128,128,128,0.15);
    border-radius: 3px;
    padding: 0 3px;
  }

  /* ── Tasks ── */
  .task-item {
    display: flex;
    align-items: center;
    gap: 8px;
    padding-top: 4px;
    padding-bottom: 4px;
    padding-right: 10px;
    border-radius: 5px;
    transition: background 0.1s;
    min-height: 27px;
  }
  .task-item:hover { background: rgba(128,128,128,0.12); }
  .task-item:hover .delete-btn { opacity: 1; }

  .checkbox {
    flex-shrink: 0;
    width: 15px; height: 15px;
    border-radius: 3px;
    border: 1.5px solid currentColor;
    background: transparent;
    cursor: pointer;
    display: flex; align-items: center; justify-content: center;
    padding: 0; color: inherit; opacity: 0.6;
    transition: opacity 0.1s, background 0.1s;
  }
  .checkbox:hover { opacity: 1; background: rgba(128,128,128,0.15); }
  .task-item.done .checkbox { background: rgba(128,128,128,0.2); }
  .checkbox svg { width: 9px; height: 9px; }

  .task-text {
    flex: 1; font-size: 12.5px; line-height: 1.4;
    cursor: default; overflow: hidden;
    text-overflow: ellipsis; white-space: nowrap; opacity: 0.9;
  }
  .task-item.done .task-text { text-decoration: line-through; opacity: 0.4; }

  .task-edit {
    flex: 1; font-size: 12.5px; font-family: inherit;
    background: rgba(128,128,128,0.1); border: none; border-radius: 3px;
    color: inherit; outline: none; padding: 1px 4px; user-select: text;
  }

  .delete-btn {
    flex-shrink: 0; width: 17px; height: 17px;
    border-radius: 4px; border: none; background: transparent;
    cursor: pointer; opacity: 0; font-size: 14px; line-height: 1;
    color: inherit; display: flex; align-items: center;
    justify-content: center; padding: 0;
    transition: opacity 0.1s, background 0.1s;
  }
  .delete-btn:hover { background: rgba(255,80,80,0.25); opacity: 1 !important; }
</style>
