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

      // The whole window IS the toolbar now, so there's nothing left to make
      // click-through — no setIgnoreCursorEvents anywhere in this file.

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

      // If the keyboard window is closed via its own [x] button, keep our
      // toggle state (and icon) in sync.
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

  async function toggleKeyboardPanel() {
    keyboardVisible = !keyboardVisible;
    if (!tauriReady || !keyboardWindow) return;

    if (keyboardVisible) {
      // Place the keyboard window just to the right of the toolbar, using
      // the toolbar's own real on-screen position/size.
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
    keyboardLocked = next; // optimistic; reverted below on failure
    try {
      await invoke('set_keyboard_lock', { locked: next });
    } catch (err) {
      keyboardLocked = !next; // revert
      console.error('set_keyboard_lock failed:', err);
      flashLockNotice(typeof err === 'string' ? err : 'Could not lock keyboard on this OS.');
    }
  }

  async function toggleTouchpadLock() {
    if (!tauriReady) return;
    const next = !touchpadLocked;
    touchpadLocked = next; // optimistic; reverted below on failure
    try {
      await invoke('set_touchpad_lock', { locked: next });
    } catch (err) {
      touchpadLocked = !next; // revert
      console.error('set_touchpad_lock failed:', err);
      flashLockNotice(typeof err === 'string' ? err : 'Could not lock touchpad on this OS.');
    }
  }

  onDestroy(() => {
    clearTimeout(lockNoticeTimer);
    if (unlistenForceUnlock) unlistenForceUnlock();
    if (unlistenKeyboardClosed) unlistenKeyboardClosed();
    if (resizeObserver) resizeObserver.disconnect();
  });

  // ---------- Directional buttons ----------
  let pressed = '';
  function keyPress(k) {
    pressed = k;
    setTimeout(() => { if (pressed === k) pressed = ''; }, 120);
    if (tauriReady) {
      // main is now a non-activating window (see lib.rs), so this click
      // never took focus away in the first place — send_key lands on
      // whatever window the user was actually directing input at.
      invoke('send_key', { key: k, ctrl: false, alt: false, shift: false }).catch((err) =>
        console.error('send_key failed:', err)
      );
    }
  }

  // ---------- Copy / paste ----------
  // Same "press" flash as the directional keys, but backed by dedicated
  // Rust commands (copy_shortcut / paste_shortcut) instead of the generic
  // send_key, so they read clearly at the call site and stay easy to find.
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

    <button
      class="tool-btn"
      class:active={pressed === '↑'}
      title="Up"
      on:click={() => keyPress('↑')}
    >
      <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linecap="round" stroke-linejoin="round">
        <path d="M12 19V5M5 12l7-7 7 7" />
      </svg>
    </button>

    <button
      class="tool-btn"
      class:active={pressed === '↓'}
      title="Down"
      on:click={() => keyPress('↓')}
    >
      <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linecap="round" stroke-linejoin="round">
        <path d="M12 5v14M5 12l7 7 7-7" />
      </svg>
    </button>

    <button
      class="tool-btn"
      class:active={pressed === '←'}
      title="Left"
      on:click={() => keyPress('←')}
    >
      <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linecap="round" stroke-linejoin="round">
        <path d="M19 12H5M12 5l-7 7 7 7" />
      </svg>
    </button>

    <button
      class="tool-btn"
      class:active={pressed === '→'}
      title="Right"
      on:click={() => keyPress('→')}
    >
      <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linecap="round" stroke-linejoin="round">
        <path d="M5 12h14M12 5l7 7-7 7" />
      </svg>
    </button>

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

    <!-- Paste -->
    <button
      class="tool-btn"
      class:active={pressed === 'paste'}
      title="Paste (Ctrl+V)"
      on:click={pastePress}
    >
      <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linecap="round" stroke-linejoin="round">
        <rect x="6" y="4" width="12" height="17" rx="2" />
        <path d="M9 4V3.5A1.5 1.5 0 0 1 10.5 2h3A1.5 1.5 0 0 1 15 3.5V4" />
        <path d="M9 12h6M9 16h6" />
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

  /* ===================== Lock notice ===================== */
  /* Normal flow now (not position:absolute) so it grows the window itself
     rather than needing pre-reserved dead space around the toolbar. */
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