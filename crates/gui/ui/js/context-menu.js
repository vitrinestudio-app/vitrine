// Context menu (right-click, ⇧F10 or the context-menu key): a popover at a
// screen point with items `{ label, icon?, key?, action, disabled? }` and
// `'-'` separators. Keyboard: ↑/↓ (Home/End) move, Enter/Space run, Esc or
// Tab close; a click outside, scrolling, resizing or losing the window
// closes it too. Focus returns to where it was. Only one menu is open at
// a time. Styles: `.popover.menu` (styles/dialogs.css).

import { h, icon } from './dom.js';
import { keyLabel } from './shortcuts.js';

let current = null;

/** Closes the open context menu (if any). */
export function closeContextMenu() {
  current?.close();
}

/**
 * Opens a context menu at client point (`x`, `y`). Returns `{ el, close }`.
 * `label`: accessible name of the menu.
 */
export function openContextMenu({ x, y, items, label }) {
  closeContextMenu();
  const returnFocus = document.activeElement;
  const buttons = [];
  const menu = h('div', { class: 'popover menu ctx-menu', role: 'menu', 'aria-label': label ?? null });
  for (const it of items) {
    if (!it) continue;
    if (it === '-') {
      if (menu.lastChild && !menu.lastChild.classList.contains('menu-sep')) menu.append(h('div', { class: 'menu-sep', role: 'separator' }));
      continue;
    }
    const b = h(
      'button',
      {
        type: 'button',
        role: 'menuitem',
        class: 'menu-item',
        tabindex: '-1',
        disabled: Boolean(it.disabled),
        'aria-disabled': it.disabled ? 'true' : null,
        onclick: () => {
          close();
          it.action?.();
        },
      },
      it.icon ? icon(it.icon) : h('span', { class: 'ctx-icon' }),
      h('span', { class: 'grow' }, it.label),
      it.key ? h('span', { class: 'muted small ctx-key' }, keyLabel(it.key)) : null,
    );
    buttons.push(b);
    menu.append(b);
  }
  if (menu.lastChild?.classList.contains('menu-sep')) menu.lastChild.remove();
  document.body.append(menu);

  // Keep it inside the window.
  const r = menu.getBoundingClientRect();
  const left = Math.max(4, Math.min(x, window.innerWidth - r.width - 4));
  const top = y + r.height > window.innerHeight - 4 ? Math.max(4, y - r.height) : y;
  menu.style.left = `${left}px`;
  menu.style.top = `${top}px`;

  const enabled = () => buttons.filter((b) => !b.disabled);
  function focusAt(i) {
    const list = enabled();
    if (!list.length) return;
    list[(i + list.length) % list.length].focus();
  }
  const index = () => enabled().indexOf(document.activeElement);

  function onKey(e) {
    if (e.key === 'Escape' || e.key === 'Tab') {
      e.preventDefault();
      e.stopPropagation();
      close();
    } else if (e.key === 'ArrowDown') {
      e.preventDefault();
      e.stopPropagation();
      focusAt(index() + 1);
    } else if (e.key === 'ArrowUp') {
      e.preventDefault();
      e.stopPropagation();
      focusAt(index() < 0 ? -1 : index() - 1);
    } else if (e.key === 'Home' || e.key === 'End') {
      e.preventDefault();
      e.stopPropagation();
      focusAt(e.key === 'Home' ? 0 : -1);
    } else if (e.key === 'Enter' || e.key === ' ') {
      if (menu.contains(document.activeElement)) {
        e.preventDefault();
        e.stopPropagation();
        document.activeElement.click();
      }
    } else if (!e.metaKey && !e.ctrlKey) {
      // Other keys (shortcuts) must not act behind the menu.
      e.stopPropagation();
    }
  }
  function onDown(e) {
    if (!menu.contains(e.target)) close();
  }
  const onBlur = () => close();

  let closed = false;
  function close() {
    if (closed) return;
    closed = true;
    menu.remove();
    document.removeEventListener('keydown', onKey, true);
    document.removeEventListener('pointerdown', onDown, true);
    window.removeEventListener('blur', onBlur);
    window.removeEventListener('resize', onBlur);
    window.removeEventListener('wheel', onDown, true);
    if (current?.el === menu) current = null;
    if (returnFocus instanceof HTMLElement && document.contains(returnFocus) && !menu.contains(returnFocus)) {
      returnFocus.focus({ preventScroll: true });
    }
  }
  document.addEventListener('keydown', onKey, true);
  document.addEventListener('pointerdown', onDown, true);
  window.addEventListener('blur', onBlur);
  window.addEventListener('resize', onBlur);
  window.addEventListener('wheel', onDown, true);
  // A right-click inside the menu opens nothing else.
  menu.addEventListener('contextmenu', (e) => e.preventDefault());

  focusAt(0);
  current = { el: menu, close };
  return current;
}
