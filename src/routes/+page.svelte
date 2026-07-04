<script>
  import { onMount, onDestroy } from 'svelte';

  // ---------- Tauri window handle ----------
  let appWindow = null;
  let tauriReady = false;

  onMount(async () => {
    if (typeof window !== 'undefined' && window.__TAURI_INTERNALS__) {
      const { getCurrentWindow } = await import('@tauri-apps/api/window');
      appWindow = getCurrentWindow();
      tauriReady = true;
      // Whole surface starts click-through; toolbar/keyboard re-enable it on hover.
      await appWindow.setIgnoreCursorEvents(true);
    }
    keyboardPos = { x: window.innerWidth - KB_WIDTH - 40, y: 90 };
  });

  async function setInteractive(interactive) {
    if (tauriReady) await appWindow.setIgnoreCursorEvents(!interactive);
  }

  async function dragWindow(e) {
    if (e.button !== 0) return;
    if (tauriReady) await appWindow.startDragging();
  }

  // ---------- Toolbar state ----------
  let activeTool = 'move';
  let keyboardVisible = false;
  let keyboardLocked = false;
  let touchpadLocked = false;

  function selectTool(name) {
    activeTool = name;
  }

  function toggleKeyboardPanel() {
    keyboardVisible = !keyboardVisible;
  }

  function toggleKeyboardLock() {
    keyboardLocked = !keyboardLocked;
    // TODO(rust): invoke('set_keyboard_lock', { locked: keyboardLocked })
    // Rust side should register a low-level keyboard hook (Windows: WH_KEYBOARD_LL)
    // and swallow events while locked, aside from this app's own shortcuts.
  }

  function toggleTouchpadLock() {
    touchpadLocked = !touchpadLocked;
    // TODO(rust): invoke('set_touchpad_lock', { locked: touchpadLocked })
    // Rust side should disable the precision touchpad HID device or filter
    // WM_INPUT/pointer events originating from it while locked.
  }

  // ---------- Draggable on-screen keyboard ----------
  const KB_WIDTH = 360;
  let keyboardPos = { x: 400, y: 90 };
  let kbDragging = false;
  let kbOffset = { x: 0, y: 0 };

  function startKbDrag(e) {
    kbDragging = true;
    kbOffset = { x: e.clientX - keyboardPos.x, y: e.clientY - keyboardPos.y };
    window.addEventListener('pointermove', onKbDrag);
    window.addEventListener('pointerup', endKbDrag);
  }

  function onKbDrag(e) {
    if (!kbDragging) return;
    keyboardPos = {
      x: Math.max(8, e.clientX - kbOffset.x),
      y: Math.max(8, e.clientY - kbOffset.y)
    };
  }

  function endKbDrag() {
    kbDragging = false;
    window.removeEventListener('pointermove', onKbDrag);
    window.removeEventListener('pointerup', endKbDrag);
  }

  onDestroy(() => {
    window.removeEventListener('pointermove', onKbDrag);
    window.removeEventListener('pointerup', endKbDrag);
  });

  // ---------- Keyboard key layout ----------
  const rows = [
    ['1','2','3','4','5','6','7','8','9','0','⌫'],
    ['q','w','e','r','t','y','u','i','o','p'],
    ['a','s','d','f','g','h','j','k','l','↵'],
    ['⇧','z','x','c','v','b','n','m',',','.'],
    ['Ctrl','Alt','␣','←','↓','↑','→']
  ];
  let pressed = '';
  function keyPress(k) {
    pressed = k;
    setTimeout(() => { if (pressed === k) pressed = ''; }, 120);
  }
</script>

<div class="stage">
  <!-- ===================== TOOLBAR ===================== -->
  <aside
    class="toolbar"
    on:pointerenter={() => setInteractive(true)}
    on:pointerleave={() => setInteractive(false)}
  >
    <div class="grip" on:pointerdown={dragWindow} title="Drag toolbar">
      <svg viewBox="0 0 20 20" fill="none">
        <circle cx="6" cy="5" r="1.4" fill="currentColor" />
        <circle cx="14" cy="5" r="1.4" fill="currentColor" />
        <circle cx="6" cy="10" r="1.4" fill="currentColor" />
        <circle cx="14" cy="10" r="1.4" fill="currentColor" />
        <circle cx="6" cy="15" r="1.4" fill="currentColor" />
        <circle cx="14" cy="15" r="1.4" fill="currentColor" />
      </svg>
    </div>

    <div class="divider"></div>

    <button
      class="tool-btn"
      class:active={activeTool === 'move'}
      title="Move"
      on:click={() => selectTool('move')}
    >
      <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linecap="round" stroke-linejoin="round">
        <path d="M12 3v18M3 12h18" />
        <path d="M12 3l-3 3M12 3l3 3M12 21l-3-3M12 21l3-3M3 12l3-3M3 12l3 3M21 12l-3-3M21 12l-3 3" />
      </svg>
    </button>

    <button
      class="tool-btn"
      class:active={activeTool === 'pointer'}
      title="Pointer"
      on:click={() => selectTool('pointer')}
    >
      <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linecap="round" stroke-linejoin="round">
        <path d="M5 3l6 17 2.2-7.2L20 10.6 5 3z" />
      </svg>
    </button>

    <div class="divider"></div>

    <!-- On-screen keyboard toggle -->
    <button
      class="tool-btn"
      class:active={keyboardVisible}
      title="On-screen keyboard"
      on:click={toggleKeyboardPanel}
    >
      <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linecap="round" stroke-linejoin="round">
        <rect x="2.5" y="6" width="19" height="12" rx="2.4" />
        <path d="M6 10h.01M9.5 10h.01M13 10h.01M16.5 10h.01M6 14h9M16.5 14h1.5" />
      </svg>
    </button>

    <!-- Lock keyboard -->
    <button
      class="tool-btn lockable"
      class:active={keyboardLocked}
      title={keyboardLocked ? 'Unlock keyboard input' : 'Lock keyboard input'}
      on:click={toggleKeyboardLock}
    >
      <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linecap="round" stroke-linejoin="round">
        <rect x="2.5" y="6" width="19" height="12" rx="2.4" />
        <path d="M6 10h.01M9.5 10h.01M13 10h.01M16.5 10h.01M6 14h12" />
      </svg>
      <svg class="badge" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
        <rect x="8.5" y="11" width="7" height="6" rx="1.4" />
        <path d="M10 11V8.7a2 2 0 0 1 4 0V11" />
      </svg>
    </button>

    <!-- Lock touchpad -->
    <button
      class="tool-btn lockable"
      class:active={touchpadLocked}
      title={touchpadLocked ? 'Unlock touchpad' : 'Lock touchpad'}
      on:click={toggleTouchpadLock}
    >
      <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linecap="round" stroke-linejoin="round">
        <rect x="3" y="4" width="18" height="16" rx="2.6" />
        <path d="M3 15h18" />
      </svg>
      <svg class="badge" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
        <rect x="8.5" y="11" width="7" height="6" rx="1.4" />
        <path d="M10 11V8.7a2 2 0 0 1 4 0V11" />
      </svg>
    </button>

    <div class="divider"></div>

    <button class="tool-btn" title="Settings">
      <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linecap="round" stroke-linejoin="round">
        <circle cx="12" cy="12" r="3" />
        <path d="M19.4 13a7.9 7.9 0 0 0 0-2l2-1.5-2-3.4-2.3.9a8 8 0 0 0-1.7-1L15 3h-4l-.4 2.6a8 8 0 0 0-1.7 1l-2.3-.9-2 3.4L6.6 11a7.9 7.9 0 0 0 0 2l-2 1.5 2 3.4 2.3-.9c.5.4 1.1.7 1.7 1L11 21h4l.4-2.6c.6-.3 1.2-.6 1.7-1l2.3.9 2-3.4-2-1.5z" />
      </svg>
    </button>
  </aside>

  <!-- ===================== ON-SCREEN KEYBOARD ===================== -->
  {#if keyboardVisible}
    <div
      class="kb-panel"
      style="left:{keyboardPos.x}px; top:{keyboardPos.y}px; width:{KB_WIDTH}px;"
      on:pointerenter={() => setInteractive(true)}
      on:pointerleave={() => setInteractive(false)}
    >
      <div class="kb-header" on:pointerdown={startKbDrag}>
        <div class="kb-grip">
          <svg viewBox="0 0 20 8" fill="none">
            <circle cx="2" cy="2" r="1.3" fill="currentColor" />
            <circle cx="7" cy="2" r="1.3" fill="currentColor" />
            <circle cx="12" cy="2" r="1.3" fill="currentColor" />
            <circle cx="17" cy="2" r="1.3" fill="currentColor" />
            <circle cx="2" cy="6" r="1.3" fill="currentColor" />
            <circle cx="7" cy="6" r="1.3" fill="currentColor" />
            <circle cx="12" cy="6" r="1.3" fill="currentColor" />
            <circle cx="17" cy="6" r="1.3" fill="currentColor" />
          </svg>
        </div>
        <span class="kb-title">On-Screen Keyboard</span>
        <button class="kb-close" on:click={toggleKeyboardPanel} title="Close">
          <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round">
            <path d="M6 6l12 12M18 6L6 18" />
          </svg>
        </button>
      </div>

      <div class="kb-body">
        {#each rows as row, i}
          <div class="kb-row">
            {#each row as key}
              <button
                class="kb-key"
                class:pressed={pressed === key}
                class:wide={key === '⌫' || key === '↵' || key === '⇧' || key === 'Ctrl' || key === 'Alt'}
                class:space={key === '␣'}
                on:click={() => keyPress(key)}
              >
                {key}
              </button>
            {/each}
          </div>
        {/each}
      </div>
    </div>
  {/if}
</div>

<style>
  :global(html), :global(body) {
    margin: 0;
    background: transparent;
    overflow: hidden;
    color-scheme: dark;
  }

  .stage {
    position: fixed;
    inset: 0;
    pointer-events: none;
    font-family: -apple-system, 'Segoe UI', Inter, sans-serif;
  }

  /* ===================== Toolbar ===================== */
  /* Solid, self-contained vertical strip — not a floating glass blob. */
  .toolbar {
    pointer-events: auto;
    position: absolute;
    top: 60px;
    left: 18px;
    width: 50px;
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 4px;
    padding: 10px 0;
    border-radius: 16px;
    background: #1b1b1d;
    border: 1px solid rgba(255, 255, 255, 0.07);
    box-shadow:
      0 14px 34px rgba(0, 0, 0, 0.55),
      inset 0 1px 0 rgba(255, 255, 255, 0.05);
  }

  .grip {
    width: 30px;
    height: 20px;
    display: flex;
    align-items: center;
    justify-content: center;
    color: rgba(255, 255, 255, 0.28);
    cursor: grab;
    border-radius: 6px;
    margin-bottom: 2px;
  }
  .grip:active { cursor: grabbing; }
  .grip svg { width: 15px; height: 15px; }

  .divider {
    width: 28px;
    height: 1px;
    background: rgba(255, 255, 255, 0.08);
    margin: 5px 0;
  }

  /* Each tool gets a fixed, always-visible slot — a defined rectangle of its
     own, like a real toolbar cell, rather than a bare icon sitting in space. */
  .tool-btn {
    position: relative;
    width: 38px;
    height: 38px;
    display: flex;
    align-items: center;
    justify-content: center;
    border: 1px solid rgba(255, 255, 255, 0.05);
    border-radius: 10px;
    background: #232326;
    color: rgba(230, 230, 233, 0.68);
    cursor: pointer;
    transition: background 0.12s ease, color 0.12s ease,
      border-color 0.12s ease, transform 0.08s ease;
  }
  .tool-btn svg { width: 18px; height: 18px; }

  .tool-btn:hover {
    background: #2b2b2f;
    border-color: rgba(255, 255, 255, 0.12);
    color: #f2f2f4;
  }
  .tool-btn:active { transform: scale(0.94); }

  .tool-btn.active {
    background: #f2f2f4;
    border-color: #f2f2f4;
    color: #17171a;
  }

  .tool-btn.lockable .badge {
    position: absolute;
    right: 1px;
    bottom: 1px;
    width: 13px;
    height: 13px;
    background: #1b1b1d;
    border-radius: 50%;
    padding: 1px;
    color: rgba(230, 230, 233, 0.5);
  }
  .tool-btn.lockable.active .badge {
    background: #17171a;
    color: #e14a4a;
  }

  /* ===================== Keyboard panel ===================== */
  .kb-panel {
    pointer-events: auto;
    position: absolute;
    border-radius: 14px;
    background: rgba(20, 20, 22, 0.68);
    backdrop-filter: blur(24px) saturate(140%);
    -webkit-backdrop-filter: blur(24px) saturate(140%);
    border: 1px solid rgba(255, 255, 255, 0.09);
    box-shadow:
      0 24px 60px rgba(0, 0, 0, 0.5),
      inset 0 1px 0 rgba(255, 255, 255, 0.06);
    overflow: hidden;
    user-select: none;
  }

  .kb-header {
    display: flex;
    align-items: center;
    gap: 6px;
    padding: 6px 8px;
    cursor: grab;
    border-bottom: 1px solid rgba(255, 255, 255, 0.07);
  }
  .kb-header:active { cursor: grabbing; }

  .kb-grip { color: rgba(255, 255, 255, 0.3); display: flex; }
  .kb-grip svg { width: 14px; height: 6px; }

  .kb-title {
    flex: 1;
    font-size: 9px;
    letter-spacing: 0.03em;
    text-transform: uppercase;
    color: rgba(255, 255, 255, 0.45);
  }

  .kb-close {
    width: 18px;
    height: 18px;
    border: none;
    border-radius: 6px;
    background: transparent;
    color: rgba(255, 255, 255, 0.5);
    display: flex;
    align-items: center;
    justify-content: center;
    cursor: pointer;
  }
  .kb-close:hover { background: rgba(255, 255, 255, 0.1); color: #fff; }
  .kb-close svg { width: 11px; height: 11px; }

  .kb-body {
    padding: 8px;
    display: flex;
    flex-direction: column;
    gap: 4px;
  }

  .kb-row {
    display: flex;
    gap: 4px;
    justify-content: center;
  }

  .kb-key {
    flex: 1;
    min-width: 0;
    height: 22px;
    border: 1px solid rgba(255, 255, 255, 0.06);
    border-radius: 6px;
    background: rgba(255, 255, 255, 0.045);
    color: rgba(235, 235, 238, 0.85);
    font-size: 9px;
    text-transform: uppercase;
    cursor: pointer;
    transition: background 0.08s ease, transform 0.06s ease;
  }
  .kb-key:hover { background: rgba(255, 255, 255, 0.1); }
  .kb-key.pressed,
  .kb-key:active {
    background: #f2f2f4;
    color: #17171a;
    transform: scale(0.95);
  }
  .kb-key.wide { flex: 1.8; font-size: 8px; }
  .kb-key.space { flex: 5; }
</style>