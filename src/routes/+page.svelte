<script>
  import { onMount, onDestroy } from 'svelte';

  // ---------- Tauri window handle ----------
  let appWindow = null;
  let tauriReady = false;
  let invoke = null;
  let keyboardWindow = null; // handle to the separate keyboard window

  // Brief, dismissable status line for lock errors (e.g. unsupported OS)
  // and for the escape-hotkey force-unlock, surfaced near the lock buttons.
  // NOTE: this is now rendered in normal document flow (not position:absolute)
  // so that when it appears, the ResizeObserver below picks up the extra
  // height and grows the OS window to fit it — it's never clipped and it
  // never needs a bigger-than-content window to have room to show up in.
  let lockNotice = '';
  let lockNoticeTimer = null;
  function flashLockNotice(text) {
    lockNotice = text;
    clearTimeout(lockNoticeTimer);
    lockNoticeTimer = setTimeout(() => (lockNotice = ''), 3500);
  }

  let unlistenForceUnlock = null;
  let unlistenKeyboardClosed = null;

  // Wraps the toolbar (+ lock notice, when shown). This element's real,
  // rendered box is what the OS window is kept sized to at all times.
  let rootEl;
  let resizeObserver;

  function syncWindowSize() {
    if (!tauriReady || !rootEl) return;
    const rect = rootEl.getBoundingClientRect();
    if (rect.width > 0 && rect.height > 0) {
      appWindow.setSize(new (window.__tauriLogicalSize)(Math.ceil(rect.width), Math.ceil(rect.height)));
    }
  }

  onMount(async () => {
    if (typeof window !== 'undefined' && window.__TAURI_INTERNALS__) {
      const { getCurrentWindow, LogicalSize, LogicalPosition } = await import('@tauri-apps/api/window');
      const { WebviewWindow } = await import('@tauri-apps/api/webviewWindow');
      const core = await import('@tauri-apps/api/core');
      const { listen } = await import('@tauri-apps/api/event');

      appWindow = getCurrentWindow();
      invoke = core.invoke;
      tauriReady = true;
      window.__tauriLogicalSize = LogicalSize;
      window.__tauriLogicalPosition = LogicalPosition;

      keyboardWindow = await WebviewWindow.getByLabel('keyboard');

      unlistenForceUnlock = await listen('input-lock://force-unlock', async () => {
        if (!keyboardLocked) return;
        keyboardLocked = false;
        try {
          await invoke('set_keyboard_lock', { locked: false });
        } catch (err) {
          console.error('Failed to clear keyboard lock after escape hotkey:', err);
        }
        flashLockNotice('Keyboard unlocked (Ctrl+Alt+Shift+U)');
      });

      unlistenKeyboardClosed = await listen('keyboard-window-closed', () => {
        keyboardVisible = false;
      });

      resizeObserver = new ResizeObserver(() => syncWindowSize());
      resizeObserver.observe(rootEl);
      syncWindowSize();
    }
  });

  async function dragWindow(e) {
    if (e.button !== 0) return;
    if (tauriReady) await appWindow.startDragging();
  }

  // ---------- Toolbar state ----------
  let keyboardVisible = false;
  let keyboardLocked = false;
  let touchpadLocked = false;
  let mouseHeld = false;

  async function toggleKeyboardPanel() {
    keyboardVisible = !keyboardVisible;
    if (!tauriReady || !keyboardWindow) return;

    if (keyboardVisible) {
      const LogicalPosition = window.__tauriLogicalPosition;
      const physPos = await appWindow.outerPosition();
      const physSize = await appWindow.outerSize();
      const scale = await appWindow.scaleFactor();
      const pos = physPos.toLogical(scale);
      const size = physSize.toLogical(scale);
      await keyboardWindow.setPosition(new LogicalPosition(pos.x + size.width + 12, pos.y));
      await keyboardWindow.show();
    } else {
      await keyboardWindow.hide();
    }
  }

  async function toggleKeyboardLock() {
    if (!tauriReady) return;
    const next = !keyboardLocked;
    keyboardLocked = next;
    try {
      await invoke('set_keyboard_lock', { locked: next });
    } catch (err) {
      keyboardLocked = !next;
      console.error('set_keyboard_lock failed:', err);
      flashLockNotice(typeof err === 'string' ? err : 'Could not lock keyboard on this OS.');
    }
  }

  async function toggleTouchpadLock() {
    if (!tauriReady) return;
    const next = !touchpadLocked;
    touchpadLocked = next;
    try {
      await invoke('set_touchpad_lock', { locked: next });
    } catch (err) {
      touchpadLocked = !next;
      console.error('set_touchpad_lock failed:', err);
      flashLockNotice(typeof err === 'string' ? err : 'Could not lock touchpad on this OS.');
    }
  }

  // ---------- Drag/selection state ----------
  let isDragging = false;
  let dragStartX = 0;
  let dragStartY = 0;
  let currentDirection = '';

  // ---------- Directional buttons with drag support ----------
  let pressed = '';
  function keyPress(k) {
    pressed = k;
    setTimeout(() => { if (pressed === k) pressed = ''; }, 120);
    if (tauriReady) {
      invoke('send_key', { key: k, ctrl: false, alt: false, shift: false }).catch((err) =>
        console.error('send_key failed:', err)
      );
    }
  }

  // Drag/selection handlers
  function handleDragStart(e, direction) {
    isDragging = true;
    dragStartX = e.clientX;
    dragStartY = e.clientY;
    currentDirection = direction;
    
    // Start shift+arrow sequence
    if (tauriReady) {
      // First, start with shift held + arrow key
      invoke('send_modified_key', { key: direction, shift: true }).catch(console.error);
      
      // Show visual feedback - set active state with animation
      document.querySelectorAll('.drag-btn').forEach(el => {
        if (el.dataset.direction === direction) {
          el.classList.add('drag-active');
        }
        el.style.opacity = '0.5';
      });
    }
  }

  function handleDragMove(e) {
    if (!isDragging) return;
    
    // Calculate movement delta
    const deltaX = e.clientX - dragStartX;
    const deltaY = e.clientY - dragStartY;
    
    // Determine direction based on dominant movement
    let newDirection = currentDirection;
    if (Math.abs(deltaX) > 30 || Math.abs(deltaY) > 30) {
      const absX = Math.abs(deltaX);
      const absY = Math.abs(deltaY);
      
      if (absX > absY) {
        newDirection = deltaX > 0 ? '→' : '←';
      } else {
        newDirection = deltaY > 0 ? '↓' : '↑';
      }
      
      // If direction changed, send new shift+arrow
      if (newDirection !== currentDirection) {
        currentDirection = newDirection;
        if (tauriReady) {
          invoke('send_modified_key', { key: newDirection, shift: true }).catch(console.error);
        }
        // Update visual feedback
        document.querySelectorAll('.drag-btn').forEach(el => {
          if (el.dataset.direction === newDirection) {
            el.classList.add('drag-active');
          } else {
            el.classList.remove('drag-active');
          }
        });
      }
    }
  }

  function handleDragEnd() {
    if (!isDragging) return;
    isDragging = false;
    
    // Stop shift+arrow
    if (tauriReady) {
      // Send release of shift key and stop selection
      invoke('send_key', { key: currentDirection, ctrl: false, alt: false, shift: false }).catch(console.error);
    }
    
    // Reset visual feedback
    document.querySelectorAll('.drag-btn').forEach(el => {
      el.classList.remove('drag-active');
      el.style.opacity = '1';
    });
  }

  onDestroy(() => {
    clearTimeout(lockNoticeTimer);
    if (unlistenForceUnlock) unlistenForceUnlock();
    if (unlistenKeyboardClosed) unlistenKeyboardClosed();
    if (resizeObserver) resizeObserver.disconnect();
    document.removeEventListener('mousemove', handleDragMove);
    document.removeEventListener('mouseup', handleDragEnd);
  });

  // ---------- Mouse hold (click and drag) ----------
  async function toggleMouseHold() {
    if (!tauriReady) return;
    mouseHeld = !mouseHeld;
    try {
      if (mouseHeld) {
        await invoke('mouse_click_down');
      } else {
        await invoke('mouse_click_up');
      }
    } catch (err) {
      mouseHeld = !mouseHeld;
      console.error('mouse_click failed:', err);
      flashLockNotice(typeof err === 'string' ? err : 'Could not control mouse on this OS.');
    }
  }

  // ---------- Copy / paste / undo / redo ----------
  async function copyPress() {
    pressed = 'copy';
    setTimeout(() => { if (pressed === 'copy') pressed = ''; }, 120);
    if (!tauriReady) return;
    try {
      await invoke('copy_shortcut');
    } catch (err) {
      console.error('copy_shortcut failed:', err);
      flashLockNotice(typeof err === 'string' ? err : 'Could not copy on this OS.');
    }
  }

  async function pastePress() {
    pressed = 'paste';
    setTimeout(() => { if (pressed === 'paste') pressed = ''; }, 120);
    if (!tauriReady) return;
    try {
      await invoke('paste_shortcut');
    } catch (err) {
      console.error('paste_shortcut failed:', err);
      flashLockNotice(typeof err === 'string' ? err : 'Could not paste on this OS.');
    }
  }

  async function undoPress() {
    pressed = 'undo';
    setTimeout(() => { if (pressed === 'undo') pressed = ''; }, 120);
    if (!tauriReady) return;
    try {
      await invoke('undo_shortcut');
    } catch (err) {
      console.error('undo_shortcut failed:', err);
      flashLockNotice(typeof err === 'string' ? err : 'Could not undo on this OS.');
    }
  }

  async function redoPress() {
    pressed = 'redo';
    setTimeout(() => { if (pressed === 'redo') pressed = ''; }, 120);
    if (!tauriReady) return;
    try {
      await invoke('redo_shortcut');
    } catch (err) {
      console.error('redo_shortcut failed:', err);
      flashLockNotice(typeof err === 'string' ? err : 'Could not redo on this OS.');
    }
  }

  // ---------- Alt+Tab ----------
  async function altTabPress() {
    pressed = 'alttab';
    setTimeout(() => { if (pressed === 'alttab') pressed = ''; }, 120);
    if (!tauriReady) return;
    try {
      await invoke('send_alt_tab');
    } catch (err) {
      console.error('send_alt_tab failed:', err);
      flashLockNotice(typeof err === 'string' ? err : 'Could not switch windows on this OS.');
    }
  }
</script>

<div class="root" bind:this={rootEl}>
  <!-- ===================== TOOLBAR ===================== -->
  <aside class="toolbar">
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

    <!-- Directional buttons with drag support -->
    <div class="drag-btn-group">
      <button
        class="tool-btn drag-btn"
        class:active={pressed === '↑'}
        class:drag-active={isDragging && currentDirection === '↑'}
        data-direction="↑"
        title="Up (click) / Drag to select"
        on:click={() => keyPress('↑')}
        on:mousedown={() => handleDragStart(event, '↑')}
        on:mouseenter={handleDragMove}
        on:mouseleave={handleDragEnd}
      >
        <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linecap="round" stroke-linejoin="round">
          <path d="M12 19V5M5 12l7-7 7 7" />
        </svg>
      </button>

      <button
        class="tool-btn drag-btn"
        class:active={pressed === '↓'}
        class:drag-active={isDragging && currentDirection === '↓'}
        data-direction="↓"
        title="Down (click) / Drag to select"
        on:click={() => keyPress('↓')}
        on:mousedown={() => handleDragStart(event, '↓')}
        on:mouseenter={handleDragMove}
        on:mouseleave={handleDragEnd}
      >
        <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linecap="round" stroke-linejoin="round">
          <path d="M12 5v14M5 12l7 7 7-7" />
        </svg>
      </button>

      <button
        class="tool-btn drag-btn"
        class:active={pressed === '←'}
        class:drag-active={isDragging && currentDirection === '←'}
        data-direction="←"
        title="Left (click) / Drag to select"
        on:click={() => keyPress('←')}
        on:mousedown={() => handleDragStart(event, '←')}
        on:mouseenter={handleDragMove}
        on:mouseleave={handleDragEnd}
      >
        <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linecap="round" stroke-linejoin="round">
          <path d="M19 12H5M12 5l-7 7 7 7" />
        </svg>
      </button>

      <button
        class="tool-btn drag-btn"
        class:active={pressed === '→'}
        class:drag-active={isDragging && currentDirection === '→'}
        data-direction="→"
        title="Right (click) / Drag to select"
        on:click={() => keyPress('→')}
        on:mousedown={() => handleDragStart(event, '→')}
        on:mouseenter={handleDragMove}
        on:mouseleave={handleDragEnd}
      >
        <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linecap="round" stroke-linejoin="round">
          <path d="M5 12h14M12 5l7 7-7 7" />
        </svg>
      </button>
    </div>

    <div class="divider"></div>

    <!-- Copy -->
    <button
      class="tool-btn"
      class:active={pressed === 'copy'}
      title="Copy (Ctrl+C)"
      on:click={copyPress}
    >
      <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linecap="round" stroke-linejoin="round">
        <rect x="9" y="9" width="12" height="12" rx="2" />
        <path d="M6 15H4.5A1.5 1.5 0 0 1 3 13.5v-9A1.5 1.5 0 0 1 4.5 3h9A1.5 1.5 0 0 1 15 4.5V6" />
      </svg>
    </button>

    <!-- Paste - fixed icon sizing -->
    <button
      class="tool-btn"
      class:active={pressed === 'paste'}
      title="Paste (Ctrl+V)"
      on:click={pastePress}
    >
      <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linecap="round" stroke-linejoin="round" style="width:18px;height:18px;">
        <rect x="6" y="4" width="12" height="16" rx="2" />
        <path d="M9 4V3.5A1.5 1.5 0 0 1 10.5 2h3A1.5 1.5 0 0 1 15 3.5V4" />
        <path d="M9 10h6M9 13h6" />
      </svg>
    </button>

    <!-- Undo -->
    <button
      class="tool-btn"
      class:active={pressed === 'undo'}
      title="Undo (Ctrl+Z)"
      on:click={undoPress}
    >
      <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linecap="round" stroke-linejoin="round">
        <path d="M3 7v6h6" />
        <path d="M21 17a9 9 0 0 0-9-9 9 9 0 0 0-6 2.3L3 13" />
      </svg>
    </button>

    <!-- Redo -->
    <button
      class="tool-btn"
      class:active={pressed === 'redo'}
      title="Redo (Ctrl+Y)"
      on:click={redoPress}
    >
      <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linecap="round" stroke-linejoin="round">
        <path d="M21 7v6h-6" />
        <path d="M3 17a9 9 0 0 1 9-9 9 9 0 0 1 6 2.3L21 13" />
      </svg>
    </button>

    <!-- Alt+Tab -->
    <button
      class="tool-btn"
      class:active={pressed === 'alttab'}
      title="Switch windows (Alt+Tab)"
      on:click={altTabPress}
    >
      <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linecap="round" stroke-linejoin="round">
        <rect x="3" y="5" width="18" height="14" rx="2" />
        <path d="M3 10h18M3 14h18" />
        <path d="M8 7v2M16 7v2" />
      </svg>
    </button>

    <div class="divider"></div>

    <!-- Mouse hold button -->
    <button
      class="tool-btn lockable"
      class:active={mouseHeld}
      title={mouseHeld ? 'Release mouse button' : 'Hold mouse button (drag mode)'}
      on:click={toggleMouseHold}
    >
      <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linecap="round" stroke-linejoin="round">
        <rect x="5" y="3" width="14" height="18" rx="7" />
        <path d="M12 3v5M9 6l3-3 3 3" />
      </svg>
      <svg class="badge" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
        <circle cx="12" cy="12" r="3" />
      </svg>
    </button>

    <div class="divider"></div>

    <!-- On-screen keyboard toggle -->
   

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
  </aside>

  {#if lockNotice}
    <div class="lock-notice">{lockNotice}</div>
  {/if}
</div>

<style>
  :global(html), :global(body) {
    margin: 0;
    background: transparent;
    overflow: hidden;
    color-scheme: dark;
  }

  /* This element's rendered box == the OS window's box, kept in sync by
     the ResizeObserver in the script above. No fixed/absolute positioning,
     no full-screen overlay, no pointer-events:none — every pixel here is
     real toolbar, and everything outside it is genuinely outside the
     window, so the OS naturally passes clicks through to whatever's below. */
  .root {
    display: inline-flex;
    flex-direction: column;
    align-items: flex-start;
    gap: 8px;
    font-family: -apple-system, 'Segoe UI', Inter, sans-serif;
  }

  /* ===================== Toolbar ===================== */
  .toolbar {
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
    margin: 0 10px 2px;
  }
  .grip:active { cursor: grabbing; }
  .grip svg { width: 15px; height: 15px; }

  .divider {
    width: 28px;
    height: 1px;
    background: rgba(255, 255, 255, 0.08);
    margin: 5px 0;
  }

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
      border-color 0.12s ease, transform 0.08s ease, opacity 0.2s ease;
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

  /* Drag button group */
  .drag-btn-group {
    display: flex;
    flex-direction: column;
    gap: 2px;
    align-items: center;
  }

  .drag-btn-group .drag-btn {
    width: 38px;
    height: 32px;
    transition: all 0.2s ease;
  }

  .drag-btn.drag-active {
    background: #3a7bd5;
    border-color: #3a7bd5;
    color: #fff;
    animation: pulse 0.6s ease-in-out infinite alternate;
  }

  @keyframes pulse {
    0% { transform: scale(1); }
    100% { transform: scale(1.08); }
  }

  /* During drag mode, other buttons fade */
  .toolbar.dragging .tool-btn:not(.drag-active) {
    opacity: 0.4;
  }

  /* ===================== Lock notice ===================== */
  .lock-notice {
    max-width: 220px;
    padding: 7px 10px;
    border-radius: 8px;
    background: rgba(20, 20, 22, 0.9);
    border: 1px solid rgba(255, 255, 255, 0.1);
    color: rgba(240, 240, 242, 0.92);
    font-size: 11px;
    line-height: 1.35;
    box-shadow: 0 10px 26px rgba(0, 0, 0, 0.45);
  }
</style>