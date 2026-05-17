<script lang="ts">
  interface Props {
    onconfirm: (date: Date) => void;
    oncancel: () => void;
  }

  let { onconfirm, oncancel }: Props = $props();

  const now = new Date();
  // Default to 10 minutes from now, rounded up to the next 5-minute mark.
  // This keeps the picker on the correct side of AM/PM, so typing "11" doesn't
  // accidentally become 11 PM when the user meant 11 AM.
  const defaultTime = new Date(now.getTime() + 10 * 60 * 1000);
  defaultTime.setMinutes(Math.ceil(defaultTime.getMinutes() / 5) * 5);
  defaultTime.setSeconds(0, 0);

  let viewYear  = $state(defaultTime.getFullYear());
  let viewMonth = $state(defaultTime.getMonth());
  let selYear   = $state(defaultTime.getFullYear());
  let selMonth  = $state(defaultTime.getMonth());
  let selDay    = $state(defaultTime.getDate());
  let selHour   = $state(defaultTime.getHours()); // stored in 24h
  let selMinute = $state(defaultTime.getMinutes());

  // ── AM/PM helpers ────────────────────────────────────────────────────────────
  let isAM        = $derived(selHour < 12);
  let displayHour = $derived(selHour === 0 ? 12 : selHour > 12 ? selHour - 12 : selHour);

  function toggleAMPM() {
    selHour = isAM ? Math.min(selHour + 12, 23) : Math.max(selHour - 12, 0);
  }

  function handleHourInput(e: Event) {
    let v = parseInt((e.target as HTMLInputElement).value);
    if (isNaN(v) || v < 1) v = 12;
    if (v > 12) v = 12;
    selHour = isAM ? (v === 12 ? 0 : v) : (v === 12 ? 12 : v + 12);
  }

  function handleMinuteInput(e: Event) {
    const v = parseInt((e.target as HTMLInputElement).value);
    selMinute = Math.max(0, Math.min(59, isNaN(v) ? 0 : v));
  }

  // ── Calendar helpers ─────────────────────────────────────────────────────────
  const DAYS   = ['Su', 'Mo', 'Tu', 'We', 'Th', 'Fr', 'Sa'];
  const MONTHS = ['January','February','March','April','May','June',
                  'July','August','September','October','November','December'];

  let cells = $derived.by(() => {
    const count = new Date(viewYear, viewMonth + 1, 0).getDate();
    const first = new Date(viewYear, viewMonth, 1).getDay();
    const arr: (number | null)[] = Array(first).fill(null);
    for (let d = 1; d <= count; d++) arr.push(d);
    return arr;
  });

  function prevMonth() {
    if (viewMonth === 0) { viewMonth = 11; viewYear--; } else viewMonth--;
  }

  function nextMonth() {
    if (viewMonth === 11) { viewMonth = 0; viewYear++; } else viewMonth++;
  }

  function isPast(day: number) {
    const d = new Date(viewYear, viewMonth, day);
    return d < new Date(now.getFullYear(), now.getMonth(), now.getDate());
  }

  function isToday(day: number) {
    return day === now.getDate() && viewMonth === now.getMonth() && viewYear === now.getFullYear();
  }

  function isSelected(day: number) {
    return day === selDay && viewMonth === selMonth && viewYear === selYear;
  }

  function selectDay(day: number) {
    if (isPast(day)) return;
    selDay = day; selMonth = viewMonth; selYear = viewYear;
  }

  // ── Quick options ─────────────────────────────────────────────────────────────
  function setQuick(type: 'in1h' | 'tonight' | 'tomorrow' | 'nextweek') {
    let d: Date;
    if      (type === 'in1h')     d = new Date(now.getTime() + 3_600_000);
    else if (type === 'tonight')  { d = new Date(now.getFullYear(), now.getMonth(), now.getDate(), 21, 0); if (d <= now) d.setDate(d.getDate() + 1); }
    else if (type === 'tomorrow') d = new Date(now.getFullYear(), now.getMonth(), now.getDate() + 1, 9, 0);
    else                          d = new Date(now.getFullYear(), now.getMonth(), now.getDate() + 7, 9, 0);
    viewYear = d.getFullYear(); viewMonth = d.getMonth();
    selYear  = d.getFullYear(); selMonth  = d.getMonth(); selDay = d.getDate();
    selHour  = d.getHours();    selMinute = d.getMinutes();
  }

  function confirm() {
    onconfirm(new Date(selYear, selMonth, selDay, selHour, selMinute, 0));
  }
</script>

<!-- stopPropagation keeps clicks inside from bubbling to the global close handler -->
<div class="picker" role="none" onclick={(e) => e.stopPropagation()}>

  <!-- Month navigation -->
  <div class="cal-header">
    <button class="nav" onclick={prevMonth} aria-label="Previous month">&#8249;</button>
    <span class="month-lbl">{MONTHS[viewMonth]} {viewYear}</span>
    <button class="nav" onclick={nextMonth} aria-label="Next month">&#8250;</button>
  </div>

  <!-- Calendar grid -->
  <div class="cal-grid">
    {#each DAYS as d}
      <div class="dow">{d}</div>
    {/each}
    {#each cells as day}
      {#if day === null}
        <div></div>
      {:else}
        <button
          class="day"
          class:today={isToday(day)}
          class:sel={isSelected(day)}
          class:past={isPast(day)}
          onclick={() => selectDay(day)}
          disabled={isPast(day)}
          aria-pressed={isSelected(day)}
        >{day}</button>
      {/if}
    {/each}
  </div>

  <!-- Time row -->
  <div class="time-row">
    <span class="time-lbl">Time</span>
    <div class="time-inputs">
      <input
        class="tnum"
        type="number"
        min={1}
        max={12}
        value={displayHour}
        oninput={handleHourInput}
        aria-label="Hour"
      />
      <span class="colon">:</span>
      <input
        class="tnum"
        type="number"
        min={0}
        max={59}
        step={5}
        value={selMinute}
        oninput={handleMinuteInput}
        aria-label="Minute"
      />
      <button class="ampm" onclick={toggleAMPM}>{isAM ? 'AM' : 'PM'}</button>
    </div>
  </div>

  <!-- Quick options -->
  <div class="quick-row">
    <button class="quick" onclick={() => setQuick('in1h')}>In 1h</button>
    <button class="quick" onclick={() => setQuick('tonight')}>Tonight 9pm</button>
    <button class="quick" onclick={() => setQuick('tomorrow')}>Tomorrow 9am</button>
    <button class="quick" onclick={() => setQuick('nextweek')}>Next week</button>
  </div>

  <!-- Actions -->
  <div class="action-row">
    <button class="btn-cancel"  onclick={oncancel}>Cancel</button>
    <button class="btn-confirm" onclick={confirm}>Set reminder</button>
  </div>
</div>

<style>
  .picker {
    background: light-dark(rgba(248,248,250,0.99), rgba(30,30,34,0.99));
    border: 1px solid rgba(128,128,128,0.2);
    border-radius: 8px;
    padding: 5px 6px;
    box-shadow: 0 6px 24px rgba(0,0,0,0.26);
    display: flex;
    flex-direction: column;
    gap: 3px;
    font-size: 10px;
    user-select: none;
    max-width: 215px;
    margin: 0 auto;
  }

  /* ── Month nav ── */
  .cal-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
  }

  .month-lbl { font-size: 10px; font-weight: 600; opacity: 0.75; }

  .nav {
    background: none; border: none; cursor: pointer; color: inherit;
    font-size: 13px; line-height: 1; padding: 0 4px;
    border-radius: 3px; opacity: 0.5; font-family: inherit;
  }
  .nav:hover { opacity: 1; background: rgba(128,128,128,0.15); }

  /* ── Calendar ── */
  .cal-grid {
    display: grid;
    grid-template-columns: repeat(7, 1fr);
    gap: 0;
  }

  .dow {
    text-align: center;
    font-size: 8px;
    font-weight: 600;
    opacity: 0.3;
    padding-bottom: 1px;
  }

  .day {
    background: none; border: none; cursor: pointer; color: inherit;
    font-family: inherit; font-size: 9.5px;
    border-radius: 50%; text-align: center; opacity: 0.7;
    transition: background 0.1s; width: 100%; aspect-ratio: 1;
    display: flex; align-items: center; justify-content: center;
    padding: 0;
  }
  .day:hover:not(:disabled) { background: rgba(128,128,128,0.18); opacity: 1; }
  .day.today  { font-weight: 700; opacity: 1; }
  .day.sel    { background: light-dark(#007aff,#0a84ff); color: #fff; opacity: 1; }
  .day.sel:hover { background: light-dark(#0066dd,#0970d8); }
  .day.past   { opacity: 0.2; cursor: default; }

  /* ── Time ── */
  .time-row {
    display: flex;
    align-items: center;
    gap: 5px;
    padding: 3px 1px 1px;
    border-top: 1px solid rgba(128,128,128,0.12);
  }

  .time-lbl { font-size: 9px; opacity: 0.4; flex-shrink: 0; }

  .time-inputs {
    display: flex;
    align-items: center;
    gap: 2px;
    margin-left: auto;
  }

  .tnum {
    width: 26px; text-align: center;
    background: rgba(128,128,128,0.1);
    border: 1px solid rgba(128,128,128,0.2);
    border-radius: 3px; color: inherit;
    font-family: monospace; font-size: 10px;
    padding: 1px; outline: none;
    appearance: textfield;
    -moz-appearance: textfield;
  }
  .tnum::-webkit-inner-spin-button,
  .tnum::-webkit-outer-spin-button { appearance: none; -webkit-appearance: none; margin: 0; }
  .tnum:focus { border-color: rgba(100,150,255,0.5); }

  .colon { opacity: 0.35; font-family: monospace; font-size: 11px; }

  .ampm {
    background: rgba(128,128,128,0.12);
    border: 1px solid rgba(128,128,128,0.2);
    border-radius: 3px; color: inherit;
    cursor: pointer; font-family: monospace;
    font-size: 9px; padding: 1px 4px; font-weight: 600;
    letter-spacing: 0.02em;
    transition: background 0.1s;
  }
  .ampm:hover { background: rgba(128,128,128,0.22); }

  /* ── Quick options ── */
  .quick-row {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 2px;
  }

  .quick {
    background: rgba(128,128,128,0.08);
    border: 1px solid rgba(128,128,128,0.16);
    border-radius: 3px; color: inherit; cursor: pointer;
    font-family: inherit; font-size: 9.5px;
    padding: 3px 2px; transition: background 0.1s; text-align: center;
  }
  .quick:hover { background: rgba(128,128,128,0.18); }

  /* ── Actions ── */
  .action-row {
    display: flex;
    gap: 4px;
    border-top: 1px solid rgba(128,128,128,0.12);
    padding-top: 4px;
  }

  .btn-cancel, .btn-confirm {
    flex: 1; border-radius: 4px;
    border: 1px solid rgba(128,128,128,0.22);
    cursor: pointer; font-family: inherit;
    font-size: 10px; padding: 4px 6px;
    transition: background 0.1s;
  }

  .btn-cancel { background: rgba(128,128,128,0.1); color: inherit; }
  .btn-cancel:hover { background: rgba(128,128,128,0.2); }

  .btn-confirm {
    background: light-dark(#007aff,#0a84ff);
    color: #fff; border-color: transparent; font-weight: 500;
  }
  .btn-confirm:hover { opacity: 0.88; }
</style>
