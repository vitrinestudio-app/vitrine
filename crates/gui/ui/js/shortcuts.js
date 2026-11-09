// Keyboard shortcuts. One window-level listener maps key combinations to
// named commands; `bindShortcuts(handlers)` supplies the functions. Keys
// are ignored while typing in inputs. `mod` = ⌘ on macOS, Ctrl elsewhere.
//
//   ⌘Z / ⇧⌘Z (⌘Y)   undo / redo
//   ⌫ / Delete       delete selection
//   ⌘D               duplicate
//   ⌘G / ⇧⌘G         group / ungroup
//   [ / ]            send backward / bring forward (⇧: to back / to front)
//   arrows           nudge 1 lu (⇧: 10 lu)
//   ⌘A               select all, Esc clears (or cancels a drag)
//   ⌘+ / ⌘− / ⌘0     zoom in / out / fit; ⌘1 = 100 % (export pixels)

export const isMac = /Mac|iPhone|iPad/.test(navigator.platform || navigator.userAgent);

/** Human-readable shortcut for tooltips (`mod+shift+z` → `⇧⌘Z` / `Ctrl+Shift+Z`). */
export function keyLabel(combo) {
  const parts = combo.split('+');
  if (isMac) {
    const map = { mod: '⌘', shift: '⇧', alt: '⌥' };
    return parts.map((p) => map[p] ?? p.toUpperCase()).join('');
  }
  const map = { mod: 'Ctrl', shift: 'Shift', alt: 'Alt' };
  return parts.map((p) => map[p] ?? p.toUpperCase()).join('+');
}

function isTyping(e) {
  const tag = e.target?.tagName;
  return tag === 'INPUT' || tag === 'TEXTAREA' || tag === 'SELECT' || e.target?.isContentEditable;
}

/**
 * Installs the shortcut listener. `handlers`: { undo, redo, remove,
 * duplicate, group, ungroup, reorder(order), nudge(dx, dy), selectAll,
 * escape, zoomIn, zoomOut, fit, zoomActual } (missing ones are skipped).
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
    if (isTyping(e)) return;
    const mod = isMac ? e.metaKey : e.ctrlKey;
    const key = e.key.toLowerCase();
    let handled = false;
    if (mod) {
      if (key === 'z') handled = call(e.shiftKey ? 'redo' : 'undo');
      else if (key === 'y') handled = call('redo');
      else if (key === 'd') handled = call('duplicate');
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
      else if (key === 'arrowleft') handled = call('nudge', -step, 0);
      else if (key === 'arrowright') handled = call('nudge', step, 0);
      else if (key === 'arrowup') handled = call('nudge', 0, -step);
      else if (key === 'arrowdown') handled = call('nudge', 0, step);
      else if (e.code === 'BracketLeft') handled = call('reorder', e.shiftKey ? 'back' : 'backward');
      else if (e.code === 'BracketRight') handled = call('reorder', e.shiftKey ? 'front' : 'forward');
    }
    if (handled) e.preventDefault();
  };
  window.addEventListener('keydown', onKey);
  return () => window.removeEventListener('keydown', onKey);
}
