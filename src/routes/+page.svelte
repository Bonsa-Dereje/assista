<script>
  import { onMount, onDestroy } from 'svelte';

  // ---------- Tauri window handle ----------
  let appWindow = null;
  let tauriReady = false;
  let invoke = null;
  let keyboardWindow = null;

  let lockNotice = '';
  let lockNoticeTimer = null;
  function flashLockNotice(text) {
    lockNotice = text;
    clearTimeout(lockNoticeTimer);
    lockNoticeTimer = setTimeout(() => (lockNotice = ''), 3500);
  }

  let unlistenForceUnlock = null;
  let unlistenKeyboardClosed = null;

  let rootEl;
  let resizeObserver;

  function syncWindowSize() {
    if (!tauriReady || !rootEl) return;
    const rect = rootEl.getBoundingClientRect();
    if (rect.width > 0 && rect.height > 0) {
      appWindow.setSize(new (window.__tauriLogicalSize)(Math.ceil(rect.width), Math.ceil(rect.height)));
    }
  }

  // ---------- Settings state ----------
  let showSettings = false;
  let isLightMode = false;

  // Button visibility settings
  const buttonSettings = {
    arrowUp: { label: '↑ Up', visible: true },
    arrowDown: { label: '↓ Down', visible: true },
    arrowLeft: { label: '← Left', visible: true },
    arrowRight: { label: '→ Right', visible: true },
    copy: { label: 'Copy', visible: true },
    paste: { label: 'Paste', visible: true },
    undo: { label: 'Undo', visible: true },
    redo: { label: 'Redo', visible: true },
    altTab: { label: 'Alt+Tab', visible: true },
    mouseHold: { label: 'Mouse Hold', visible: true },
    keyboardLock: { label: 'Key Lock', visible: true },
    touchpadLock: { label: 'Touch Lock', visible: true },
  };

  function toggleButtonVisibility(key) {
    buttonSettings[key].visible = !buttonSettings[key].visible;
  }

  function toggleLightMode() {
    isLightMode = !isLightMode;
    document.documentElement.classList.toggle('light-mode', isLightMode);
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

  function handleDragStart(e, direction) {
    isDragging = true;
    dragStartX = e.clientX;
    dragStartY = e.clientY;
    currentDirection = direction;
    
    if (tauriReady) {
      invoke('send_modified_key', { key: direction, shift: true }).catch(console.error);
      
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
    
    const deltaX = e.clientX - dragStartX;
    const deltaY = e.clientY - dragStartY;
    
    let newDirection = currentDirection;
    if (Math.abs(deltaX) > 30 || Math.abs(deltaY) > 30) {
      const absX = Math.abs(deltaX);
      const absY = Math.abs(deltaY);
      
      if (absX > absY) {
        newDirection = deltaX > 0 ? '→' : '←';
      } else {
        newDirection = deltaY > 0 ? '↓' : '↑';
      }
      
      if (newDirection !== currentDirection) {
        currentDirection = newDirection;
        if (tauriReady) {
          invoke('send_modified_key', { key: newDirection, shift: true }).catch(console.error);
        }
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
    
    if (tauriReady) {
      invoke('send_key', { key: currentDirection, ctrl: false, alt: false, shift: false }).catch(console.error);
    }
    
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

  // ---------- Mouse hold ----------
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
  <!-- ===================== TOOLBAR AND SETTINGS CONTAINER ===================== -->
  <div class="toolbar-container">
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

      <!-- Directional buttons -->
      {#if buttonSettings.arrowUp.visible}
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
      {/if}

      {#if buttonSettings.arrowUp.visible || buttonSettings.copy.visible}
      <div class="divider"></div>
      {/if}

      <!-- Copy -->
      {#if buttonSettings.copy.visible}
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
      {/if}

      <!-- Paste -->
      {#if buttonSettings.paste.visible}
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
      {/if}

      <!-- Undo -->
      {#if buttonSettings.undo.visible}
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
      {/if}

      <!-- Redo -->
      {#if buttonSettings.redo.visible}
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
      {/if}

      <!-- Alt+Tab -->
      {#if buttonSettings.altTab.visible}
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
      {/if}

      {#if buttonSettings.altTab.visible || buttonSettings.mouseHold.visible}
      <div class="divider"></div>
      {/if}

      <!-- Mouse hold -->
      {#if buttonSettings.mouseHold.visible}
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
      {/if}

      {#if buttonSettings.mouseHold.visible || buttonSettings.keyboardLock.visible}
      <div class="divider"></div>
      {/if}

      <!-- Lock keyboard -->
      {#if buttonSettings.keyboardLock.visible}
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
      {/if}

      <!-- Lock touchpad -->
      {#if buttonSettings.touchpadLock.visible}
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
      {/if}

      <div class="divider"></div>

      <!-- Settings button -->
      <button
        class="tool-btn"
        class:active={showSettings}
        title="Settings"
        on:click={() => showSettings = !showSettings}
      >
        <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linecap="round" stroke-linejoin="round">
          <path d="M12.22 2h-.44a2 2 0 0 0-2 2v.18a2 2 0 0 1-1 1.73l-.43.25a2 2 0 0 1-2 0l-.15-.08a2 2 0 0 0-2.73.73l-.22.38a2 2 0 0 0 .73 2.73l.15.1a2 2 0 0 1 1 1.72v.51a2 2 0 0 1-1 1.74l-.15.09a2 2 0 0 0-.73 2.73l.22.38a2 2 0 0 0 2.73.73l.15-.08a2 2 0 0 1 2 0l.43.25a2 2 0 0 1 1 1.73V20a2 2 0 0 0 2 2h.44a2 2 0 0 0 2-2v-.18a2 2 0 0 1 1-1.73l.43-.25a2 2 0 0 1 2 0l.15.08a2 2 0 0 0 2.73-.73l.22-.39a2 2 0 0 0-.73-2.73l-.15-.08a2 2 0 0 1-1-1.74v-.5a2 2 0 0 1 1-1.74l.15-.09a2 2 0 0 0 .73-2.73l-.22-.38a2 2 0 0 0-2.73-.73l-.15.08a2 2 0 0 1-2 0l-.43-.25a2 2 0 0 1-1-1.73V4a2 2 0 0 0-2-2z" />
          <circle cx="12" cy="12" r="3" />
        </svg>
      </button>
    </aside>

    <!-- Settings panel - positioned to the right -->
    {#if showSettings}
    <div class="settings-panel">
      <div class="settings-header">
        <span class="settings-title">Settings</span>
        <button class="settings-close" on:click={() => showSettings = false}>✕</button>
      </div>
      
      <div class="settings-section">
        <div class="settings-label">Theme</div>
        <button class="theme-toggle" on:click={toggleLightMode}>
          {#if isLightMode}
            <svg class="theme-icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
              <path d="M21 12.79A9 9 0 1 1 11.21 3 7 7 0 0 0 21 12.79z" />
            </svg>
            Dark Mode
          {:else}
            <svg class="theme-icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
              <circle cx="12" cy="12" r="5" />
              <line x1="12" y1="1" x2="12" y2="3" />
              <line x1="12" y1="21" x2="12" y2="23" />
              <line x1="4.22" y1="4.22" x2="5.64" y2="5.64" />
              <line x1="18.36" y1="18.36" x2="19.78" y2="19.78" />
              <line x1="1" y1="12" x2="3" y2="12" />
              <line x1="21" y1="12" x2="23" y2="12" />
              <line x1="4.22" y1="19.78" x2="5.64" y2="18.36" />
              <line x1="18.36" y1="5.64" x2="19.78" y2="4.22" />
            </svg>
            Light Mode
          {/if}
        </button>
      </div>

      <div class="settings-divider"></div>

      <div class="settings-section">
        <div class="settings-label">Visible Buttons</div>
        <div class="settings-grid">
          {#each Object.entries(buttonSettings) as [key, setting]}
            <label class="settings-item">
              <input
                type="checkbox"
                checked={setting.visible}
                on:change={() => toggleButtonVisibility(key)}
              />
              <span>{setting.label}</span>
            </label>
          {/each}
        </div>
      </div>
    </div>
    {/if}
  </div>

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

  :global(.light-mode) {
    color-scheme: light;
  }

  .root {
    display: inline-flex;
    flex-direction: column;
    align-items: flex-start;
    gap: 8px;
    font-family: -apple-system, 'Segoe UI', Inter, sans-serif;
    padding: 4px; /* Add padding to prevent clipping */
  }

  /* ===================== Toolbar Container ===================== */
  .toolbar-container {
    display: flex;
    flex-direction: row;
    align-items: flex-start;
    gap: 12px;
    max-width: 100%; /* Prevent overflow */
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
    flex-shrink: 0;
  }

  :global(.light-mode) .toolbar {
    background: #f5f5f7;
    border: 1px solid rgba(0, 0, 0, 0.08);
    box-shadow:
      0 14px 34px rgba(0, 0, 0, 0.12),
      inset 0 1px 0 rgba(255, 255, 255, 0.8);
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

  :global(.light-mode) .grip {
    color: rgba(0, 0, 0, 0.25);
  }

  .divider {
    width: 28px;
    height: 1px;
    background: rgba(255, 255, 255, 0.08);
    margin: 5px 0;
  }

  :global(.light-mode) .divider {
    background: rgba(0, 0, 0, 0.08);
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

  :global(.light-mode) .tool-btn {
    background: #ffffff;
    border: 1px solid rgba(0, 0, 0, 0.06);
    color: rgba(50, 50, 55, 0.7);
  }

  .tool-btn:hover {
    background: #2b2b2f;
    border-color: rgba(255, 255, 255, 0.12);
    color: #f2f2f4;
  }

  :global(.light-mode) .tool-btn:hover {
    background: #f0f0f2;
    border-color: rgba(0, 0, 0, 0.12);
    color: #1a1a1e;
  }

  .tool-btn:active { transform: scale(0.94); }

  .tool-btn.active {
    background: #f2f2f4;
    border-color: #f2f2f4;
    color: #17171a;
  }

  :global(.light-mode) .tool-btn.active {
    background: #1a1a1e;
    border-color: #1a1a1e;
    color: #f5f5f7;
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

  :global(.light-mode) .tool-btn.lockable .badge {
    background: #f5f5f7;
    color: rgba(0, 0, 0, 0.4);
  }

  .tool-btn.lockable.active .badge {
    background: #17171a;
    color: #e14a4a;
  }

  :global(.light-mode) .tool-btn.lockable.active .badge {
    background: #f5f5f7;
    color: #d63030;
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

  :global(.light-mode) .drag-btn.drag-active {
    background: #3a7bd5;
    border-color: #3a7bd5;
    color: #fff;
  }

  @keyframes pulse {
    0% { transform: scale(1); }
    100% { transform: scale(1.08); }
  }

  /* ===================== Settings Panel ===================== */
  .settings-panel {
    min-width: 200px;
    max-width: 240px;
    padding: 16px;
    border-radius: 12px;
    background: #1b1b1d;
    border: 1px solid rgba(255, 255, 255, 0.07);
    box-shadow: 0 14px 34px rgba(0, 0, 0, 0.55);
    flex-shrink: 0;
    overflow: hidden; /* Prevent content from spilling out */
  }

  :global(.light-mode) .settings-panel {
    background: #f5f5f7;
    border: 1px solid rgba(0, 0, 0, 0.08);
    box-shadow: 0 14px 34px rgba(0, 0, 0, 0.12);
  }

  .settings-header {
    display: flex;
    justify-content: space-between;
    align-items: center;
    margin-bottom: 12px;
  }

  .settings-title {
    color: rgba(230, 230, 233, 0.9);
    font-size: 13px;
    font-weight: 600;
  }

  :global(.light-mode) .settings-title {
    color: rgba(30, 30, 35, 0.9);
  }

  .settings-close {
    background: none;
    border: none;
    color: rgba(230, 230, 233, 0.5);
    cursor: pointer;
    font-size: 16px;
    padding: 0 4px;
    transition: color 0.15s ease;
  }

  .settings-close:hover {
    color: rgba(230, 230, 233, 0.9);
  }

  :global(.light-mode) .settings-close {
    color: rgba(0, 0, 0, 0.4);
  }

  :global(.light-mode) .settings-close:hover {
    color: rgba(0, 0, 0, 0.8);
  }

  .settings-section {
    margin: 8px 0;
  }

  .settings-label {
    color: rgba(230, 230, 233, 0.6);
    font-size: 10px;
    text-transform: uppercase;
    letter-spacing: 0.6px;
    margin-bottom: 8px;
  }

  :global(.light-mode) .settings-label {
    color: rgba(0, 0, 0, 0.5);
  }

  .settings-divider {
    height: 1px;
    background: rgba(255, 255, 255, 0.06);
    margin: 10px 0;
  }

  :global(.light-mode) .settings-divider {
    background: rgba(0, 0, 0, 0.06);
  }

  .theme-toggle {
    width: 100%;
    padding: 6px 12px;
    background: #232326;
    border: 1px solid rgba(255, 255, 255, 0.05);
    border-radius: 8px;
    color: rgba(230, 230, 233, 0.8);
    cursor: pointer;
    font-size: 12px;
    transition: background 0.15s ease;
    display: flex;
    align-items: center;
    justify-content: center;
    gap: 8px;
  }

  .theme-toggle:hover {
    background: #2b2b2f;
  }

  :global(.light-mode) .theme-toggle {
    background: #ffffff;
    border: 1px solid rgba(0, 0, 0, 0.06);
    color: rgba(30, 30, 35, 0.8);
  }

  :global(.light-mode) .theme-toggle:hover {
    background: #f0f0f2;
  }

  .theme-icon {
    width: 16px;
    height: 16px;
    flex-shrink: 0;
  }

  .settings-grid {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 4px 8px;
  }

  .settings-item {
    display: flex;
    align-items: center;
    gap: 6px;
    color: rgba(230, 230, 233, 0.75);
    font-size: 12px;
    cursor: pointer;
    padding: 3px 0;
    transition: color 0.15s ease;
    min-width: 0; /* Prevent overflow */
  }

  .settings-item:hover {
    color: rgba(230, 230, 233, 0.95);
  }

  :global(.light-mode) .settings-item {
    color: rgba(30, 30, 35, 0.7);
  }

  :global(.light-mode) .settings-item:hover {
    color: rgba(30, 30, 35, 0.95);
  }

  .settings-item input[type="checkbox"] {
    appearance: none;
    width: 14px;
    height: 14px;
    border-radius: 4px;
    background: #232326;
    border: 1px solid rgba(255, 255, 255, 0.1);
    cursor: pointer;
    position: relative;
    flex-shrink: 0;
    transition: all 0.15s ease;
  }

  .settings-item input[type="checkbox"]:checked {
    background: #3a7bd5;
    border-color: #3a7bd5;
  }

  .settings-item input[type="checkbox"]:checked::after {
    content: '✓';
    position: absolute;
    top: 50%;
    left: 50%;
    transform: translate(-50%, -50%);
    color: white;
    font-size: 10px;
  }

  :global(.light-mode) .settings-item input[type="checkbox"] {
    background: #ffffff;
    border: 1px solid rgba(0, 0, 0, 0.15);
  }

  :global(.light-mode) .settings-item input[type="checkbox"]:checked {
    background: #3a7bd5;
    border-color: #3a7bd5;
  }

  .settings-item input[type="checkbox"]:hover {
    border-color: rgba(255, 255, 255, 0.2);
  }

  :global(.light-mode) .settings-item input[type="checkbox"]:hover {
    border-color: rgba(0, 0, 0, 0.3);
  }

  .settings-item span {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
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

  :global(.light-mode) .lock-notice {
    background: rgba(245, 245, 247, 0.95);
    border: 1px solid rgba(0, 0, 0, 0.08);
    color: rgba(30, 30, 35, 0.92);
    box-shadow: 0 10px 26px rgba(0, 0, 0, 0.15);
  }
</style>