<script>
  import { onMount, onDestroy } from 'svelte';

  let appWindow = null;
  let tauriReady = false;
  let emit = null;

  // Same idea as the toolbar window: this element's real box is what the
  // OS window is kept sized to, so the keyboard window never extends past
  // its own visible edges.
  let kbEl;
  let resizeObserver;

  function syncWindowSize() {
    if (!tauriReady || !kbEl) return;
    const rect = kbEl.getBoundingClientRect();
    if (rect.width > 0 && rect.height > 0) {
      appWindow.setSize(new (window.__tauriLogicalSize)(Math.ceil(rect.width), Math.ceil(rect.height)));
    }
  }

  onMount(async () => {
    if (typeof window !== 'undefined' && window.__TAURI_INTERNALS__) {
      const { getCurrentWindow, LogicalSize } = await import('@tauri-apps/api/window');
      const eventApi = await import('@tauri-apps/api/event');

      appWindow = getCurrentWindow();
      emit = eventApi.emit;
      tauriReady = true;
      window.__tauriLogicalSize = LogicalSize;

      resizeObserver = new ResizeObserver(() => syncWindowSize());
      resizeObserver.observe(kbEl);
      syncWindowSize();

      // If the user closes this window some other way (Alt+F4, dock, etc.)
      // still let the toolbar know so its toggle button un-highlights.
      const unlisten = await appWindow.onCloseRequested(async () => {
        await emit('keyboard-window-closed');
      });
      onDestroy(unlisten);
    }
  });

  async function dragWindow(e) {
    if (e.button !== 0) return;
    if (tauriReady) await appWindow.startDragging();
  }

  async function closeKeyboard() {
    if (!tauriReady) return;
    await emit('keyboard-window-closed');
    await appWindow.hide();
  }

  onDestroy(() => {
    if (resizeObserver) resizeObserver.disconnect();
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
    // If key presses need to actually type into the focused external app,
    // call your Rust command here, e.g.:
    // invoke('send_key', { key: k });
  }
</script>

<div class="kb-panel" bind:this={kbEl}>
  <div class="kb-header" on:pointerdown={dragWindow}>
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
    <button class="kb-close" on:click={closeKeyboard} title="Close">
      <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round">
        <path d="M6 6l12 12M18 6L6 18" />
      </svg>
    </button>
  </div>

  <div class="kb-body">
    {#each rows as row}
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

<style>
  :global(html), :global(body) {
    margin: 0;
    background: transparent;
    overflow: hidden;
    color-scheme: dark;
  }

  /* No position:absolute / draggable-x-y — the OS window itself is what
     moves and resizes now, so this panel just fills it in normal flow. */
  .kb-panel {
    width: 360px;
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
