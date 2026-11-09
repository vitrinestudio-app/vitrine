// Small modal dialogs for the panels (no native confirm/prompt: they are
// unreliable in web views). Styles: `.ar-modal*` in styles/assets.css.
//
//   await confirmDialog({ title, message, details: [..], ok, danger })  → boolean
//   await promptDialog({ title, label, value, ok, validate(v) → error|null }) → string|null
//   openModal({ title, body: Node, buttons: [{ label, primary, danger, value }] }) → value|null

import { h } from '../dom.js';
import { t } from '../i18n.js';

/** Shows a modal; resolves with the clicked button's `value` (null = cancelled). */
export function openModal({ title, body, buttons, onKey }) {
  return new Promise((resolve) => {
    const prevFocus = document.activeElement;
    const done = (value) => {
      document.removeEventListener('keydown', key, true);
      backdrop.remove();
      prevFocus?.focus?.({ preventScroll: true });
      resolve(value);
    };
    const btns = buttons.map((b) =>
      h(
        'button',
        {
          type: 'button',
          class: `btn${b.primary ? ' btn-primary' : ''}${b.danger ? ' btn-danger' : ''}`,
          onclick: async () => {
            if (b.check && !(await b.check())) return;
            done(typeof b.value === 'function' ? b.value() : b.value ?? null);
          },
        },
        b.label,
      ),
    );
    const dialog = h(
      'div',
      { class: 'ar-modal', role: 'dialog', 'aria-modal': 'true', 'aria-label': title },
      h('div', { class: 'ar-modal-title' }, title),
      h('div', { class: 'ar-modal-body' }, body),
      h('div', { class: 'ar-modal-buttons' }, ...btns),
    );
    const backdrop = h('div', { class: 'ar-modal-backdrop' }, dialog);
    backdrop.addEventListener('pointerdown', (e) => {
      if (e.target === backdrop) done(null);
    });
    const key = (e) => {
      if (e.key === 'Escape') {
        e.preventDefault();
        e.stopPropagation();
        done(null);
      } else if (e.key === 'Enter' && !e.shiftKey && e.target?.tagName !== 'TEXTAREA' && e.target?.tagName !== 'BUTTON') {
        const primary = btns[buttons.findIndex((b) => b.primary)];
        if (primary) {
          e.preventDefault();
          e.stopPropagation();
          primary.click();
        }
      } else onKey?.(e);
    };
    document.addEventListener('keydown', key, true);
    document.body.append(backdrop);
    (dialog.querySelector('input, select, textarea') ?? btns.find((_, i) => buttons[i].primary) ?? btns[0])?.focus();
  });
}

/** Yes/no question; `details`: extra lines (e.g. elements using something). */
export async function confirmDialog({ title, message, details = [], ok = t('assets.modal_ok'), danger = false }) {
  const body = h(
    'div',
    {},
    h('p', {}, message),
    details.length ? h('ul', { class: 'ar-modal-list' }, ...details.map((d) => h('li', {}, d))) : null,
  );
  const r = await openModal({
    title,
    body,
    buttons: [
      { label: t('assets.modal_cancel'), value: false },
      { label: ok, primary: true, danger, value: true },
    ],
  });
  return r === true;
}

/** Asks for one line of text; `validate(value)` returns an error message or null. */
export async function promptDialog({ title, label, value = '', ok = t('assets.modal_ok'), validate }) {
  const input = h('input', { class: 'ar-modal-input', type: 'text', value, spellcheck: 'false', 'aria-label': label });
  const error = h('div', { class: 'ar-modal-error', hidden: true });
  const body = h('label', { class: 'ar-modal-field' }, h('span', {}, label), input, error);
  const check = () => {
    const msg = validate?.(input.value.trim()) ?? null;
    error.hidden = !msg;
    error.textContent = msg ?? '';
    return !msg;
  };
  input.addEventListener('input', () => {
    if (!error.hidden) check();
  });
  setTimeout(() => input.select(), 0);
  return openModal({
    title,
    body,
    buttons: [
      { label: t('assets.modal_cancel'), value: null },
      { label: ok, primary: true, check, value: () => input.value.trim() },
    ],
  });
}
