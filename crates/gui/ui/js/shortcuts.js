// Keyboard shortcuts. One window-level listener maps key combinations to
// named commands; `bindShortcuts(handlers)` supplies the functions. Keys
// are ignored while typing in inputs. `mod` = ⌘ on macOS, Ctrl elsewhere.
// In the macOS app ⌘Z / ⇧⌘Z belong to the native Edit menu (`menuOwns`);
// it runs the same undo/redo (menu.js).
//
//   ⌘Z / ⇧⌘Z (⌘Y)   undo / redo
//   ⌫ / Delete       delete selection
//   ⌘C / ⌘X / ⌘V     copy / cut / paste elements (element clipboard; the
//                    DOM copy/cut/paste events cover the macOS Edit menu)
//   ⌘D               duplicate
//   ⌘G / ⇧⌘G         group / ungroup
//   [ / ]            send backward / bring forward (⇧: to back / to front)
//   arrows           nudge 1 lu (⇧: 10 lu)
//   ⌘A               select all, Esc clears (or cancels a drag)
//   ⇧F10 / menu key  context menu of the selection (canvas)
//   ⌘+ / ⌘− / ⌘0     zoom in / out / fit; ⌘1 = 100 % (export pixels)

export const isMac = /Mac|iPhone|iPad/.test(navigator.platform || navigator.userAgent);

/** Human-readable shortcut for tooltips (`mod+shift+z` → `⇧⌘Z` / `Ctrl+Shift+Z`). */
export function keyLabel(combo) {
  const parts = combo.split('+');
  if (isMac) {
    const map = { mod: '⌘', shift: '⇧', alt: '⌥', delete: '⌫', escape: '⎋' };
    return parts.map((p) => map[p] ?? p.toUpperCase()).join('');
  }
  const map = { mod: 'Ctrl', shift: 'Shift', alt: 'Alt', delete: 'Del', escape: 'Esc' };
  return parts.map((p) => map[p] ?? p.toUpperCase()).join('+');
}

/** Is the event's target a text field (input, textarea, select, contenteditable)? */
export function isTyping(e) {
  const tag = e.target?.tagName;
  return tag === 'INPUT' || tag === 'TEXTAREA' || tag === 'SELECT' || e.target?.isContentEditable;
}

/**
 * Shortcuts owned by the native menu bar (macOS app, see menu.js): one
 * owner per shortcut, so the JS handlers skip these.
 */
let menuOwned = new Set();

/** Hands `combos` (`mod+shift+z` notation) to the native menu bar. */
export function setMenuOwned(combos) {
  menuOwned = new Set(combos);
}

/** Is the shortcut of keydown `e` owned by the native menu bar? */
export function menuOwns(e) {
  if (!menuOwned.size) return false;
  const parts = [];
  if (isMac ? e.metaKey : e.ctrlKey) parts.push('mod');
  if (e.altKey) parts.push('alt');
  if (e.shiftKey) parts.push('shift');
  parts.push(e.key.toLowerCase());
  return menuOwned.has(parts.join('+'));
}

/**
 * Installs the shortcut listener. `handlers`: { undo, redo, remove,
 * duplicate, group, ungroup, reorder(order), nudge(dx, dy), selectAll,
 * escape, zoomIn, zoomOut, fit, zoomActual, copy, cut, paste, contextMenu }
 * (missing ones are skipped).
 * Returns an uninstall function.
 */
export function bindShortcuts(handlers) {
  const call = (name, ...args) => {
    const fn = handlers[name];
    if (!fn) return false;
    fn(...args);
    return true;
  };

  const onKey = (e) => {
    if (isTyping(e) || menuOwns(e)) return;
    const mod = isMac ? e.metaKey : e.ctrlKey;
    const key = e.key.toLowerCase();
    let handled = false;
    if (mod) {
      if (key === 'z') handled = call(e.shiftKey ? 'redo' : 'undo');
      else if (key === 'y') handled = call('redo');
      else if (key === 'd') handled = call('duplicate');
      else if (key === 'c' && !e.shiftKey && !textSelected()) handled = call('copy');
      else if (key === 'x' && !e.shiftKey && !textSelected()) handled = call('cut');
      else if (key === 'v' && !e.shiftKey) handled = call('paste');
      else if (key === 'g') handled = call(e.shiftKey ? 'ungroup' : 'group');
      else if (key === 'a') handled = call('selectAll');
      else if (key === '=' || key === '+') handled = call('zoomIn');
      else if (key === '-' || key === '_') handled = call('zoomOut');
      else if (key === '0') handled = call('fit');
      else if (key === '1') handled = call('zoomActual');
    } else if (!e.altKey) {
      const step = e.shiftKey ? 10 : 1;
      if (key === 'backspace' || key === 'delete') handled = call('remove');
      else if (key === 'escape') handled = call('escape');
      else if ((key === 'f10' && e.shiftKey) || key === 'contextmenu') handled = call('contextMenu');
      else if (key === 'arrowleft') handled = call('nudge', -step, 0);
      else if (key === 'arrowright') handled = call('nudge', step, 0);
      else if (key === 'arrowup') handled = call('nudge', 0, -step);
      else if (key === 'arrowdown') handled = call('nudge', 0, step);
      else if (e.code === 'BracketLeft') handled = call('reorder', e.shiftKey ? 'back' : 'backward');
      else if (e.code === 'BracketRight') handled = call('reorder', e.shiftKey ? 'front' : 'forward');
    }
    if (handled) e.preventDefault();
  };
  // The macOS Edit menu (native Cut/Copy/Paste items) sends these events
  // instead of a keydown; a handled keydown above prevents them.
  const onClipboard = (e) => {
    if (isTyping(e) || (e.type !== 'paste' && textSelected())) return;
    if (call(e.type === 'copy' ? 'copy' : e.type === 'cut' ? 'cut' : 'paste')) e.preventDefault();
  };
  window.addEventListener('keydown', onKey);
  for (const type of ['copy', 'cut', 'paste']) document.addEventListener(type, onClipboard);
  return () => {
    window.removeEventListener('keydown', onKey);
    for (const type of ['copy', 'cut', 'paste']) document.removeEventListener(type, onClipboard);
  };
}

/** Is there selected page text (then ⌘C copies that text, not elements)? */
function textSelected() {
  const sel = window.getSelection();
  return Boolean(sel && !sel.isCollapsed && sel.toString().trim());
}
