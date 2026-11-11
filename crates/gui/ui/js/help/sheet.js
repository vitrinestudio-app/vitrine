// "Keyboard & mouse" help sheet: every shortcut, mouse gesture and
// modifier of the editor, generated from the list in help/gestures.js with
// platform-aware modifier names (⌘ ⌥ ⇧ on macOS, Ctrl / Alt / Shift
// elsewhere). Opened from the Help menu (in-app + native macOS), the `?`
// key (`bindHelpKey`, not while typing) and the "?" button of the bottom
// bar (`helpButton`). Styles: styles/help.css.

import { h } from '../dom.js';
import { t } from '../i18n.js';
import { isMac, isTyping } from '../shortcuts.js';
import { openModal } from '../dialogs/modal.js';
import { activeGestures } from './gestures.js';

/** Modifier tokens → label (macOS symbols / names elsewhere). */
const MODIFIERS = isMac ? { mod: '⌘', shift: '⇧', alt: '⌥', ctrl: '⌃' } : { mod: 'Ctrl', shift: 'Shift', alt: 'Alt', ctrl: 'Ctrl' };

/** Tokens with a translated label (`help.key.<token>`). */
const NAMED = new Set([
  'space',
  'click',
  'dblclick',
  'drag',
  'middle-drag',
  'right-click',
  'wheel',
  'pinch',
  'drop',
  'scrollbar',
  'enter',
  'esc',
  'backspace',
  'delete',
  'tab',
  'arrows',
  'up',
  'down',
  'left',
  'right',
]);

/** Readable label of one token (`mod` → ⌘ / Ctrl, `wheel` → Scroll wheel …). */
export function tokenLabel(token) {
  if (MODIFIERS[token]) return MODIFIERS[token];
  if (NAMED.has(token)) return t(`help.key.${token}`);
  if (token === 'f2') return 'F2';
  return token.length === 1 ? token.toUpperCase() : token;
}

/** One combo (`mod+shift+z`) as <kbd> chips joined by "+". */
function comboEl(combo) {
  const parts = combo.split('+').map((tok) => h('kbd', { class: 'help-kbd', title: MODIFIERS[tok] && isMac ? t(`help.key.${tok}`) : null }, tokenLabel(tok)));
  const out = [];
  parts.forEach((p, i) => {
    if (i) out.push(h('span', { class: 'help-plus' }, '+'));
    out.push(p);
  });
  return h('span', { class: 'help-combo' }, out);
}

/** The keys cell: alternatives separated by "/". */
function keysEl(keys) {
  const out = [];
  keys.forEach((k, i) => {
    if (i) out.push(h('span', { class: 'help-or' }, '/'));
    out.push(comboEl(k));
  });
  return h('div', { class: 'help-keys' }, out);
}

let open = null;

/** Opens the sheet (focuses it when it is open already). */
export function openGestureSheet() {
  if (open) return open;
  const groups = activeGestures().map((g) =>
    h(
      'section',
      { class: 'help-group' },
      h('div', { class: 'section-title' }, t(g.title)),
      h(
        'div',
        { class: 'help-rows' },
        g.items.map((it) => h('div', { class: 'help-row' }, keysEl(it.keys), h('div', { class: 'help-text' }, t(it.text)))),
      ),
    ),
  );
  const m = openModal({
    title: t('help.gestures.title'),
    size: 'large',
    cls: 'help-dlg',
    body: [h('p', { class: 'muted small help-intro' }, t(isMac ? 'help.gestures.intro.mac' : 'help.gestures.intro')), h('div', { class: 'help-grid' }, groups)],
    onClose: () => {
      open = null;
    },
  });
  open = m;
  return m;
}

/**
 * `?` opens the sheet: not while typing, not with ⌘/Ctrl/Alt, not while a
 * dialog is open. Returns an uninstall function.
 */
export function bindHelpKey() {
  const onKey = (e) => {
    if (e.key !== '?' || e.metaKey || e.ctrlKey || e.altKey || isTyping(e)) return;
    if (document.querySelector('dialog[open]')) return;
    e.preventDefault();
    openGestureSheet();
  };
  window.addEventListener('keydown', onKey);
  return () => window.removeEventListener('keydown', onKey);
}

/** Small "?" button (bottom bar) opening the sheet. */
export function helpButton() {
  return h('button', { class: 'icon-btn help-btn', type: 'button', title: `${t('help.gestures.title')} (?)`, onclick: () => openGestureSheet() }, '?');
}
