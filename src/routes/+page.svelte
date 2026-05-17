<script lang="ts">
  import { onMount } from 'svelte';
  import { invoke } from '@tauri-apps/api/core';
  import { listen } from '@tauri-apps/api/event';
  import { getCurrentWebviewWindow } from '@tauri-apps/api/webviewWindow';
  import NoteView from '$lib/NoteView.svelte';
  import ReminderPicker from '$lib/ReminderPicker.svelte';
  import type { Task, NoteItem, Settings } from '$lib/types';

  const appWindow = getCurrentWebviewWindow();

  let noteItems   = $state<NoteItem[]>([]);
  let settings    = $state<Settings | null>(null);
  let newTaskText = $state('');
  let loading     = $state(true);
  let error       = $state<string | null>(null);

  // ── Reminder state ──────────────────────────────────────────────────────────
  let showReminderPicker  = $state(false);
  let pendingReminderDate = $state<Date | null>(null);
  let isReminderFiring    = $state(false);
  let firingTaskText      = $state<string | null>(null);

  /** "2026-05-17 11:15" — sent to the backend & embedded in the note tag */
  function formatReminderDate(d: Date): string {
    const p = (n: number) => String(n).padStart(2, '0');
    return `${d.getFullYear()}-${p(d.getMonth() + 1)}-${p(d.getDate())} ${p(d.getHours())}:${p(d.getMinutes())}`;
  }

  /** "(@2026-05-17 11:15)" — the form that lives inside the markdown task */
  function formatReminderTag(d: Date): string {
    return `(@${formatReminderDate(d)})`;
  }

  function formatReminderLabel(d: Date): string {
    const days   = ['Sun', 'Mon', 'Tue', 'Wed', 'Thu', 'Fri', 'Sat'];
    const months = ['Jan', 'Feb', 'Mar', 'Apr', 'May', 'Jun', 'Jul', 'Aug', 'Sep', 'Oct', 'Nov', 'Dec'];
    const h = d.getHours(), m = d.getMinutes();
    const ampm = h < 12 ? 'AM' : 'PM';
    const h12  = h === 0 ? 12 : h > 12 ? h - 12 : h;
    return `${days[d.getDay()]} ${months[d.getMonth()]} ${d.getDate()}, ${h12}:${String(m).padStart(2,'0')} ${ampm}`;
  }

  // ── Reminder tones via Web Audio (no asset files needed) ───────────────────
  let audioCtx: AudioContext | null = null;

  function getAudio(): AudioContext | null {
    if (!audioCtx) {
      try {
        const Ctor = (window as any).AudioContext ?? (window as any).webkitAudioContext;
        if (!Ctor) return null;
        audioCtx = new Ctor();
      } catch {
        return null;
      }
    }
    if (audioCtx && audioCtx.state === 'suspended') audioCtx.resume().catch(() => {});
    return audioCtx;
  }

  type ToneId = 'chime' | 'bell' | 'beep' | 'digital' | 'soft' | 'custom';
  interface ToneNote { freq: number; start: number; duration: number; peak?: number; type?: OscillatorType; }
  interface Tone { label: string; notes: ToneNote[]; cycle: number; }

  // `cycle` is how long one full loop iteration takes (sound + gap), in seconds.
  // Only built-in tones live here; 'custom' is handled via HTMLAudioElement.
  const TONES: Record<Exclude<ToneId, 'custom'>, Tone> = {
    chime: {
      label: 'Chime',
      notes: [
        { freq: 523.25, start: 0.00, duration: 0.55 },
        { freq: 659.25, start: 0.14, duration: 0.55 },
        { freq: 783.99, start: 0.28, duration: 0.80 },
        { freq: 1046.5, start: 0.45, duration: 1.00, peak: 0.12 },
      ],
      cycle: 2.2,
    },
    bell: {
      label: 'Bell',
      notes: [
        { freq: 880,  start: 0,    duration: 1.4, peak: 0.2 },
        { freq: 1760, start: 0,    duration: 0.7, peak: 0.08 },
        { freq: 2640, start: 0,    duration: 0.4, peak: 0.04 },
      ],
      cycle: 1.8,
    },
    beep: {
      label: 'Beep',
      notes: [
        { freq: 880, start: 0.00, duration: 0.14, peak: 0.18, type: 'square' },
        { freq: 880, start: 0.25, duration: 0.14, peak: 0.18, type: 'square' },
        { freq: 880, start: 0.50, duration: 0.14, peak: 0.18, type: 'square' },
      ],
      cycle: 1.2,
    },
    digital: {
      label: 'Digital',
      notes: [
        { freq: 1200, start: 0.00, duration: 0.20, peak: 0.15, type: 'triangle' },
        { freq: 900,  start: 0.22, duration: 0.20, peak: 0.15, type: 'triangle' },
        { freq: 1200, start: 0.44, duration: 0.20, peak: 0.15, type: 'triangle' },
        { freq: 900,  start: 0.66, duration: 0.20, peak: 0.15, type: 'triangle' },
      ],
      cycle: 1.6,
    },
    soft: {
      label: 'Soft',
      notes: [
        { freq: 440, start: 0.0, duration: 0.9, peak: 0.13 },
        { freq: 554, start: 0.4, duration: 0.9, peak: 0.11 },
      ],
      cycle: 2.0,
    },
  };

  function playTone(toneId: Exclude<ToneId, 'custom'>) {
    const ctx  = getAudio();
    if (!ctx) return;
    const tone = TONES[toneId] ?? TONES.chime;
    for (const n of tone.notes) {
      const osc  = ctx.createOscillator();
      const gain = ctx.createGain();
      osc.type = n.type ?? 'sine';
      osc.frequency.value = n.freq;
      osc.connect(gain);
      gain.connect(ctx.destination);
      const t0   = ctx.currentTime + n.start;
      const peak = n.peak ?? 0.18;
      gain.gain.setValueAtTime(0.0001, t0);
      gain.gain.exponentialRampToValueAtTime(peak, t0 + 0.02);
      gain.gain.exponentialRampToValueAtTime(0.0001, t0 + n.duration);
      osc.start(t0);
      osc.stop(t0 + n.duration);
    }
  }

  let soundLoopTimer: ReturnType<typeof setInterval> | null = null;

  // ── Custom audio (user-selected file) ──────────────────────────────────────
  let customAudio:       HTMLAudioElement | null = null;
  let customAudioBlobUrl: string | null = null;
  let cachedTonePath:    string = '';

  async function getCustomAudio(): Promise<HTMLAudioElement | null> {
    const path = settings?.reminder_tone_path ?? '';
    if (!path) return null;
    if (path === cachedTonePath && customAudio) return customAudio;

    // Invalidate any previously cached audio
    if (customAudio) { customAudio.pause(); customAudio = null; }
    if (customAudioBlobUrl) { URL.revokeObjectURL(customAudioBlobUrl); customAudioBlobUrl = null; }

    try {
      const bytes = await invoke<number[]>('read_reminder_sound');
      const blob  = new Blob([new Uint8Array(bytes)]);
      customAudioBlobUrl = URL.createObjectURL(blob);
      customAudio        = new Audio(customAudioBlobUrl);
      cachedTonePath     = path;
      return customAudio;
    } catch (e) {
      console.error('[ob-widget] failed to load custom sound:', e);
      return null;
    }
  }

  async function playCustomSound(loop: boolean) {
    const audio = await getCustomAudio();
    if (!audio) return;
    audio.loop = loop;
    audio.currentTime = 0;
    audio.play().catch(err => console.error('[ob-widget] custom-sound play failed:', err));
  }

  function stopCustomSound() {
    if (customAudio) {
      customAudio.pause();
      customAudio.currentTime = 0;
    }
  }

  function startSoundLoop(toneId: ToneId) {
    stopSoundLoop();
    if (toneId === 'custom') {
      playCustomSound(true);
      return;
    }
    const tone = TONES[toneId as Exclude<ToneId, 'custom'>] ?? TONES.chime;
    playTone(toneId as Exclude<ToneId, 'custom'>);
    soundLoopTimer = setInterval(() => playTone(toneId as Exclude<ToneId, 'custom'>), tone.cycle * 1000);
  }

  function stopSoundLoop() {
    if (soundLoopTimer) {
      clearInterval(soundLoopTimer);
      soundLoopTimer = null;
    }
    stopCustomSound();
  }

  let reminderTimer: ReturnType<typeof setTimeout> | null = null;

  async function handleReminderFired(taskText: string) {
    // Disable the window's acrylic/vibrancy blur so the pulse rings travel through
    // genuinely transparent air rather than through the frosted backdrop.
    invoke('set_window_blur', { enabled: false }).catch(() => {});

    // Bring widget to top (frontend mirror of the Rust side, in case of timing)
    try {
      await appWindow.show();
      await appWindow.unminimize?.();
      await appWindow.setAlwaysOnTop(true);
      await appWindow.setFocus();
    } catch {}

    // Visual + audio feedback
    isReminderFiring = true;
    firingTaskText   = taskText;
    const toneId = (settings?.reminder_tone as ToneId) ?? 'chime';
    startSoundLoop(toneId);

    // Auto-dismiss after 5 minutes if user never interacts
    if (reminderTimer) clearTimeout(reminderTimer);
    reminderTimer = setTimeout(() => dismissReminder(), 5 * 60 * 1000);
  }

  function dismissReminder() {
    isReminderFiring = false;
    firingTaskText   = null;
    if (reminderTimer) { clearTimeout(reminderTimer); reminderTimer = null; }
    stopSoundLoop();
    // Restore the platform blur effect
    invoke('set_window_blur', { enabled: true }).catch(() => {});
    // Restore the user's preferred always-on-top setting
    if (settings && !settings.always_on_top) {
      appWindow.setAlwaysOnTop(false).catch(() => {});
    }
  }

  // Strip any "(@YYYY-MM-DD HH:MM)" / legacy "@YYYY-MM-DDTHH:MM" tag from a task line
  const REMINDER_TAG_RE = /\s*\(@\d{4}-\d{2}-\d{2}[ T]\d{2}:\d{2}\)\s*|\s*@\d{4}-\d{2}-\d{2}T\d{2}:\d{2}\s*/g;
  function stripReminderTag(s: string): string {
    return s.replace(REMINDER_TAG_RE, ' ').replace(/\s+/g, ' ').trim();
  }

  async function snoozeReminder() {
    const text = firingTaskText;
    if (!text) { dismissReminder(); return; }

    const snoozeAt  = new Date(Date.now() + 10 * 60 * 1000);
    const snoozeIso = formatReminderDate(snoozeAt);
    const newTag    = `(@${snoozeIso})`;

    // Rewrite the matching task in the markdown file so the (@time) reflects the snooze
    let modified = false;
    const updatedItems = noteItems.map(item => {
      if (item.kind !== 'task') return item;
      if (stripReminderTag(item.text) !== text) return item;
      modified = true;
      return { ...item, text: `${text} ${newTag}` };
    });

    if (modified) {
      noteItems = updatedItems;
      try {
        await invoke('write_tasks', { tasks: buildTasksFromItems(updatedItems) });
      } catch (e) {
        console.error('snooze write_tasks failed', e);
      }
    }

    // Schedule a fresh reminder 10 minutes out
    try {
      await invoke('add_reminder', { taskText: text, remindAt: snoozeIso });
    } catch (e) {
      console.error('snooze add_reminder failed', e);
    }

    dismissReminder();
  }

  function handleTaskInput(e: Event) {
    const ie = e as InputEvent;
    if (ie.data === '@') {
      showReminderPicker = true;
    }
  }

  function onReminderConfirm(date: Date) {
    pendingReminderDate = date;
    showReminderPicker  = false;
    // Remove the trailing @ from the input
    const idx = newTaskText.lastIndexOf('@');
    if (idx !== -1) newTaskText = newTaskText.slice(0, idx) + newTaskText.slice(idx + 1);
  }

  function onReminderCancel() {
    showReminderPicker  = false;
    pendingReminderDate = null;
    const idx = newTaskText.lastIndexOf('@');
    if (idx !== -1) newTaskText = newTaskText.slice(0, idx) + newTaskText.slice(idx + 1);
  }

  // ── Sound (tone) popover ───────────────────────────────────────────────────
  let showSoundPopover = $state(false);
  const BUILT_IN_TONE_IDS: Exclude<ToneId, 'custom'>[] = ['chime', 'bell', 'beep', 'digital', 'soft'];

  function toggleSoundPopover() { showSoundPopover = !showSoundPopover; }

  async function selectTone(id: ToneId) {
    if (!settings) return;
    const updated = { ...settings, reminder_tone: id };
    settings = updated;
    await invoke('save_settings', { settings: updated });
  }

  async function previewTone(id: ToneId) {
    // Stop any in-progress preview/loop so the new tone is heard cleanly
    stopSoundLoop();
    if (id === 'custom') {
      await playCustomSound(false);
      return;
    }
    playTone(id);
  }

  async function pickCustomSound() {
    // Result comes back via the `settings-changed` event (the picker is callback-based).
    await invoke('pick_reminder_sound');
  }

  function fileBaseName(p: string): string {
    const m = p.match(/[\\\/]([^\\\/]+)$/);
    return m ? m[1] : p;
  }

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

  // Close popovers when clicking outside
  function handleGlobalClick(e: MouseEvent) {
    const target = e.target as HTMLElement;
    if (showOpacitySlider && !target.closest('.opacity-wrap')) showOpacitySlider = false;
    if (showSoundPopover  && !target.closest('.sound-wrap'))   showSoundPopover  = false;
    if (showShortcutPopover && !target.closest('.shortcut-wrap')) {
      showShortcutPopover = false;
      recordingShortcut = false;
    }
    if (showReminderPicker && !target.closest('.reminder-overlay')) onReminderCancel();
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
    let unlistenFile:     (() => void) | undefined;
    let unlistenSettings: (() => void) | undefined;
    let unlistenReminder: (() => void) | undefined;

    (async () => {
      await loadSettings();
      if (settings?.vault_path) await loadNote();
      applyTheme((settings?.theme ?? 'system') as Theme);
      loading = false;

      unlistenFile = await listen('file-changed', () => loadNote());
      unlistenSettings = await listen<Settings>('settings-changed', (e) => {
        const oldTonePath = settings?.reminder_tone_path ?? '';
        settings = e.payload;
        applyTheme((settings?.theme ?? 'system') as Theme);
        if (settings?.vault_path) loadNote();
        // If the user picked a new custom sound, drop the cached audio so the next
        // play (preview or fire) re-reads the file.
        if ((settings?.reminder_tone_path ?? '') !== oldTonePath) {
          stopCustomSound();
          if (customAudio) customAudio = null;
          if (customAudioBlobUrl) { URL.revokeObjectURL(customAudioBlobUrl); customAudioBlobUrl = null; }
          cachedTonePath = '';
        }
      });
      unlistenReminder = await listen<string>('reminder-fired', (e) => {
        console.log('[ob-widget] reminder-fired event received:', e.payload);
        handleReminderFired(e.payload);
      });
    })();

    return () => {
      // Release all listeners, timers, and audio resources. The root component
      // doesn't usually unmount during normal use, but this keeps things tidy
      // for HMR during dev and avoids dangling resources if it ever does.
      unlistenFile?.();
      unlistenSettings?.();
      unlistenReminder?.();
      stopSoundLoop();
      if (reminderTimer) { clearTimeout(reminderTimer); reminderTimer = null; }
      if (customAudioBlobUrl) { URL.revokeObjectURL(customAudioBlobUrl); customAudioBlobUrl = null; }
      customAudio = null;
      if (audioCtx) { audioCtx.close().catch(() => {}); audioCtx = null; }
    };
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
    const raw  = newTaskText.trim();
    const text = raw.replace(/@\s*$/, '').trim(); // strip lone trailing @ if picker dismissed
    if (!text) return;
    // Embed the (@YYYY-MM-DD HH:MM) tag in the note so it's visible in Obsidian
    const noteText = pendingReminderDate
      ? `${text} ${formatReminderTag(pendingReminderDate)}`
      : text;
    newTaskText = '';
    await invoke('add_task', { text: noteText });
    if (pendingReminderDate) {
      await invoke('add_reminder', {
        taskText: text,                                // clean text for notification body
        remindAt: formatReminderDate(pendingReminderDate),
      });
      pendingReminderDate = null;
    }
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

<svelte:window
  onclick={handleGlobalClick}
  onkeydown={(e) => {
    if (e.key === 'Escape' && showReminderPicker) { onReminderCancel(); return; }
    handleShortcutKeydown(e);
  }}
/>

<!-- Invisible resize handles on all 8 edges/corners -->
<div class="rh rh-n"  onmousedown={(e) => onResizeMousedown(e, 'North')}     role="none"></div>
<div class="rh rh-s"  onmousedown={(e) => onResizeMousedown(e, 'South')}     role="none"></div>
<div class="rh rh-e"  onmousedown={(e) => onResizeMousedown(e, 'East')}      role="none"></div>
<div class="rh rh-w"  onmousedown={(e) => onResizeMousedown(e, 'West')}      role="none"></div>
<div class="rh rh-ne" onmousedown={(e) => onResizeMousedown(e, 'NorthEast')} role="none"></div>
<div class="rh rh-nw" onmousedown={(e) => onResizeMousedown(e, 'NorthWest')} role="none"></div>
<div class="rh rh-se" onmousedown={(e) => onResizeMousedown(e, 'SouthEast')} role="none"></div>
<div class="rh rh-sw" onmousedown={(e) => onResizeMousedown(e, 'SouthWest')} role="none"></div>

<!-- Outward pulse rings — siblings of .widget so they aren't clipped by widget overflow:hidden -->
{#if isReminderFiring}
  <div class="pulse-rings" aria-hidden="true">
    <span class="ring"></span>
    <span class="ring" style="animation-delay: 0.55s"></span>
    <span class="ring" style="animation-delay: 1.1s"></span>
  </div>
{/if}

<div class="widget" class:firing={isReminderFiring}>
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

      <!-- Reminder sound (bell) -->
      <div class="sound-wrap" data-tauri-drag-region>
        <button
          class="icon-btn"
          onclick={(e) => { e.stopPropagation(); toggleSoundPopover(); }}
          title="Reminder sound"
          aria-label="Reminder sound"
          data-tauri-drag-region="false"
        >
          <!-- Bell icon -->
          <svg viewBox="0 0 16 16" fill="currentColor">
            <path d="M8 1.5a.75.75 0 0 1 .75.75v.32A4.001 4.001 0 0 1 12 6.5v2.382l.894 1.789A.75.75 0 0 1 12.224 11.75H3.776a.75.75 0 0 1-.67-1.079L4 8.882V6.5a4.001 4.001 0 0 1 3.25-3.93V2.25A.75.75 0 0 1 8 1.5Zm-1.5 11.25h3a1.5 1.5 0 0 1-3 0Z"/>
          </svg>
        </button>

        {#if showSoundPopover}
          <div class="sound-popover" role="none" onclick={(e) => e.stopPropagation()}>
            <p class="sound-title">Reminder sound</p>
            {#each BUILT_IN_TONE_IDS as id}
              <div class="sound-row">
                <button
                  class="sound-pick"
                  class:active={settings?.reminder_tone === id}
                  onclick={() => selectTone(id)}
                >
                  <span class="sound-radio">{settings?.reminder_tone === id ? '●' : '○'}</span>
                  <span class="sound-label">{TONES[id].label}</span>
                </button>
                <button
                  class="sound-preview"
                  onclick={(e) => { e.stopPropagation(); previewTone(id); }}
                  title="Preview {TONES[id].label}"
                  aria-label="Preview {TONES[id].label}"
                >&#9654;</button>
              </div>
            {/each}

            <!-- Custom tone -->
            <div class="sound-row">
              <button
                class="sound-pick"
                class:active={settings?.reminder_tone === 'custom'}
                onclick={() => selectTone('custom')}
                disabled={!settings?.reminder_tone_path}
                title={!settings?.reminder_tone_path ? 'Choose a file first' : ''}
              >
                <span class="sound-radio">{settings?.reminder_tone === 'custom' ? '●' : '○'}</span>
                <span class="sound-label">Custom</span>
              </button>
              {#if settings?.reminder_tone_path}
                <button
                  class="sound-preview"
                  onclick={(e) => { e.stopPropagation(); previewTone('custom'); }}
                  title="Preview custom sound"
                  aria-label="Preview custom sound"
                >&#9654;</button>
              {/if}
            </div>

            <!-- File picker row -->
            <div class="sound-file-row">
              <span class="sound-file-name" title={settings?.reminder_tone_path ?? ''}>
                {settings?.reminder_tone_path ? fileBaseName(settings.reminder_tone_path) : 'No file selected'}
              </span>
              <button class="sound-browse" onclick={pickCustomSound}>
                {settings?.reminder_tone_path ? 'Change…' : 'Browse…'}
              </button>
            </div>
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
        placeholder="New task… (@ for reminder)"
        bind:value={newTaskText}
        oninput={handleTaskInput}
        onkeydown={(e) => { if (e.key === 'Enter' && !showReminderPicker) handleAddTask(); }}
      />
      {#if pendingReminderDate}
        <div class="reminder-badge">
          <span class="reminder-badge-icon">&#9200;</span>
          <span class="reminder-badge-label">{formatReminderLabel(pendingReminderDate)}</span>
          <button
            class="reminder-badge-clear"
            onclick={() => { pendingReminderDate = null; }}
            aria-label="Clear reminder"
          >&#215;</button>
        </div>
      {/if}
    </div>
  {/if}

  {#if showReminderPicker}
    <div class="reminder-overlay">
      <ReminderPicker onconfirm={onReminderConfirm} oncancel={onReminderCancel} />
    </div>
  {/if}
</div>

<!-- Reminder alert — sibling of .widget so it stays full-size while the widget is scaled -->
{#if isReminderFiring && firingTaskText}
  <div class="reminder-alert" role="alert">
    <div class="alert-icon">&#9200;</div>
    <div class="alert-title">Reminder</div>
    <div class="alert-text">{firingTaskText}</div>
    <div class="alert-actions">
      <button class="alert-btn alert-snooze" onclick={snoozeReminder}>Snooze 10 min</button>
      <button class="alert-btn alert-ok"     onclick={dismissReminder}>Okay</button>
    </div>
  </div>
{/if}

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
    position: relative;
    isolation: isolate;
    transform-origin: center center;
    transition: transform 0.3s ease, box-shadow 0.3s;
    z-index: 1;
  }

  /* When firing, shrink the widget so the rings have transparent room to expand into */
  .widget.firing {
    transform: scale(0.9);
    /* No outward box-shadow — keep the area outside the widget completely transparent */
  }

  /* Inner blue glow at the widget's inside edge (clipped inside widget by overflow:hidden) */
  .widget.firing::after {
    content: '';
    position: absolute;
    inset: 0;
    border-radius: inherit;
    pointer-events: none;
    box-shadow: inset 0 0 22px rgba(10, 132, 255, 0.35);
    animation: inner-glow 1.4s ease-in-out infinite;
  }

  @keyframes inner-glow {
    0%, 100% { box-shadow: inset 0 0 16px rgba(10, 132, 255, 0.28); }
    50%      { box-shadow: inset 0 0 30px rgba(10, 132, 255, 0.55); }
  }

  /* ── Outward pulse rings — thin wave lines in transparent air ────────────── */
  .pulse-rings {
    position: fixed;
    inset: 0;
    pointer-events: none;
    z-index: 0;
  }

  .ring {
    position: absolute;
    inset: 0;
    border-radius: 10px;
    background: transparent;
    /* Thin wave line — no box-shadow, no filter:blur. The acrylic blur is also
       disabled while firing (via set_window_blur), so the margin is fully transparent. */
    border: 1.5px solid rgba(10, 132, 255, 0.85);
    box-sizing: border-box;
    transform: scale(0.9);
    transform-origin: center center;
    animation: ring-emit 1.7s cubic-bezier(0.22, 0.61, 0.36, 1) infinite;
    will-change: transform, opacity, border-width;
  }

  @keyframes ring-emit {
    0%   { transform: scale(0.9);  opacity: 0.95; border-width: 2px;   }
    60%  {                          opacity: 0.5;  border-width: 1px;   }
    100% { transform: scale(1.06); opacity: 0;    border-width: 0;    }
  }

  /* ── Reminder alert (Okay / Snooze) ─────────────────────────────────────── */
  .reminder-alert {
    position: fixed;
    top: 50%;
    left: 50%;
    transform: translate(-50%, -50%);
    z-index: 400;
    width: calc(100% - 36px);
    max-width: 240px;
    background: light-dark(rgba(248, 250, 255, 0.98), rgba(24, 28, 38, 0.98));
    border: 2px solid rgba(10, 132, 255, 0.65);
    border-radius: 10px;
    padding: 12px 14px 10px;
    text-align: center;
    box-shadow: 0 10px 36px rgba(10, 132, 255, 0.3), 0 4px 14px rgba(0, 0, 0, 0.3);
    color: light-dark(#1a1a1a, #eaf1fb);
  }

  .alert-icon { font-size: 22px; line-height: 1; }

  .alert-title {
    font-size: 9.5px;
    font-weight: 700;
    letter-spacing: 0.1em;
    text-transform: uppercase;
    opacity: 0.55;
    margin-top: 2px;
  }

  .alert-text {
    font-size: 12.5px;
    font-weight: 500;
    margin: 5px 0 10px;
    line-height: 1.3;
    word-wrap: break-word;
    max-height: 64px;
    overflow: hidden;
  }

  .alert-actions {
    display: flex;
    gap: 5px;
  }

  .alert-btn {
    flex: 1;
    border-radius: 5px;
    border: 1px solid rgba(128, 128, 128, 0.25);
    cursor: pointer;
    font-family: inherit;
    font-size: 10.5px;
    padding: 5px 6px;
    transition: background 0.12s, opacity 0.12s;
  }

  .alert-snooze {
    background: rgba(128, 128, 128, 0.12);
    color: inherit;
  }
  .alert-snooze:hover { background: rgba(128, 128, 128, 0.22); }

  .alert-ok {
    background: light-dark(#007aff, #0a84ff);
    color: #fff;
    border-color: transparent;
    font-weight: 600;
  }
  .alert-ok:hover { opacity: 0.88; }

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

  /* ── Sound (tone) popover ────────────────────────────────────────────────── */
  .sound-wrap { position: relative; }

  .sound-popover {
    position: absolute;
    top: calc(100% + 6px);
    right: 0;
    background: light-dark(rgba(255,255,255,0.97), rgba(38,38,42,0.97));
    border: 1px solid rgba(128,128,128,0.2);
    border-radius: 8px;
    padding: 8px 8px 6px;
    z-index: 1000;
    box-shadow: 0 4px 16px rgba(0,0,0,0.18);
    min-width: 160px;
    display: flex;
    flex-direction: column;
    gap: 2px;
  }

  .sound-title {
    font-size: 10px;
    font-weight: 600;
    opacity: 0.55;
    letter-spacing: 0.05em;
    text-transform: uppercase;
    margin: 0 0 4px;
    padding: 0 4px;
  }

  .sound-row {
    display: flex;
    align-items: center;
    gap: 4px;
  }

  .sound-pick {
    flex: 1;
    display: flex;
    align-items: center;
    gap: 6px;
    background: transparent;
    border: none;
    border-radius: 4px;
    color: inherit;
    cursor: pointer;
    font-family: inherit;
    font-size: 11.5px;
    padding: 4px 6px;
    text-align: left;
    transition: background 0.1s;
  }
  .sound-pick:hover { background: rgba(128,128,128,0.12); }
  .sound-pick.active { background: rgba(10,132,255,0.12); color: light-dark(#005fcc, #4aabff); }

  .sound-radio {
    width: 12px;
    font-size: 11px;
    line-height: 1;
    opacity: 0.7;
  }

  .sound-label { flex: 1; }

  .sound-preview {
    width: 22px;
    height: 22px;
    border-radius: 4px;
    border: 1px solid rgba(128,128,128,0.2);
    background: rgba(128,128,128,0.08);
    color: inherit;
    cursor: pointer;
    font-size: 9px;
    padding: 0;
    display: flex;
    align-items: center;
    justify-content: center;
    transition: background 0.1s;
  }
  .sound-preview:hover { background: rgba(128,128,128,0.2); }

  .sound-pick:disabled { opacity: 0.4; cursor: not-allowed; }
  .sound-pick:disabled:hover { background: transparent; }

  .sound-file-row {
    display: flex;
    align-items: center;
    gap: 6px;
    padding: 5px 6px 2px;
    margin-top: 4px;
    border-top: 1px solid rgba(128,128,128,0.15);
  }

  .sound-file-name {
    flex: 1;
    font-size: 10px;
    opacity: 0.55;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    font-style: italic;
  }

  .sound-browse {
    background: rgba(128,128,128,0.12);
    border: 1px solid rgba(128,128,128,0.22);
    border-radius: 4px;
    color: inherit;
    cursor: pointer;
    font-family: inherit;
    font-size: 10px;
    padding: 3px 8px;
    transition: background 0.1s;
    flex-shrink: 0;
  }
  .sound-browse:hover { background: rgba(128,128,128,0.22); }

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

  /* ── Reminder overlay ───────────────────────────────────────────────────── */
  .reminder-overlay {
    position: absolute;
    bottom: 48px;
    left: 8px;
    right: 8px;
    z-index: 200;
  }

  /* ── Reminder badge ──────────────────────────────────────────────────────── */
  .reminder-badge {
    display: flex;
    align-items: center;
    gap: 4px;
    margin-top: 4px;
    padding: 3px 6px;
    background: rgba(0, 122, 255, 0.1);
    border: 1px solid rgba(0, 122, 255, 0.3);
    border-radius: 4px;
    font-size: 10.5px;
    color: light-dark(#0060cc, #3fa0ff);
  }

  .reminder-badge-icon { font-size: 11px; }

  .reminder-badge-label { flex: 1; }

  .reminder-badge-clear {
    background: none;
    border: none;
    cursor: pointer;
    color: inherit;
    font-size: 13px;
    line-height: 1;
    padding: 0 1px;
    opacity: 0.6;
  }
  .reminder-badge-clear:hover { opacity: 1; }

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
