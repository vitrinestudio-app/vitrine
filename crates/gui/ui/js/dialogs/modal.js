// Modal dialogs (native <dialog>) and small form helpers shared by the
// targets editor, export, project and app settings dialogs, the start
// screen and the panels (confirm/prompt/choice dialogs; no native
// confirm/prompt, they are unreliable in web views). Styles:
// styles/dialogs.css.

import { h, icon } from '../dom.js';
import { t } from '../i18n.js';

/**
 * Opens a modal dialog. `body` is a node (or array of nodes); `footer`
 * buttons go at the bottom. Escape / the close button / backdrop click
 * close it (unless `dismissable: false`). Returns `{ el, body, close,
 * closed }` (`closed` is a promise resolving when it closes).
 */
export function openModal({ title, body, footer = [], size = 'medium', dismissable = true, onClose, cls = '' }) {
  const bodyEl = h('div', { class: 'dlg-body' }, body);
  const closeBtn = h('button', { class: 'icon-btn', type: 'button', title: t('project.dialog.close'), onclick: () => close() }, icon('close'));
  const dlg = h(
    'dialog',
    { class: `dlg dlg-${size} ${cls}`.trim() },
    h('header', { class: 'dlg-head' }, h('h2', {}, title), h('span', { class: 'spacer' }), dismissable ? closeBtn : null),
    bodyEl,
    footer.length ? h('footer', { class: 'dlg-foot' }, footer) : null,
  );
  let resolve;
  const closed = new Promise((r) => (resolve = r));
  let done = false;
  function close(value) {
    if (done) return;
    done = true;
    dlg.close();
    dlg.remove();
    onClose?.(value);
    resolve(value);
  }
  dlg.addEventListener('cancel', (e) => {
    e.preventDefault();
    if (dismissable) close();
  });
  dlg.addEventListener('mousedown', (e) => {
    if (dismissable && e.target === dlg) close();
  });
  document.body.append(dlg);
  dlg.showModal();
  return { el: dlg, body: bodyEl, close, closed };
}

/**
 * Modal with a row of buttons `[{ label, primary, danger, value, check }]`;
 * resolves with the clicked button's `value` (a function is called), or
 * null when dismissed. `check()` (may be async) can veto the click. Enter
 * outside text areas/buttons clicks the primary button.
 */
export function choiceDialog({ title, body, buttons, size = 'small' }) {
  let result = null;
  const btns = buttons.map((b) => {
    const el = h(
      'button',
      { class: `btn${b.danger ? ' btn-danger' : b.primary ? ' btn-primary' : ''}`, type: 'button' },
      b.label,
    );
    el.addEventListener('click', async () => {
      if (b.check && !(await b.check())) return;
      result = typeof b.value === 'function' ? b.value() : (b.value ?? null);
      m.close();
    });
    return el;
  });
  const m = openModal({ title, size, body, footer: [h('span', { class: 'spacer' }), ...btns] });
  const primary = btns[buttons.findIndex((b) => b.primary)];
  m.el.addEventListener('keydown', (e) => {
    const tag = e.target?.tagName;
    if (e.key === 'Enter' && !e.shiftKey && !e.isComposing && tag !== 'TEXTAREA' && tag !== 'BUTTON' && primary) {
      e.preventDefault();
      primary.click();
    }
  });
  (m.body.querySelector('input, select, textarea') ?? primary ?? btns[0])?.focus();
  return m.closed.then(() => result);
}

/**
 * Yes/no question; resolves to true when confirmed. `details`: extra lines
 * (e.g. the elements using something); `confirm` (alias `ok`): label of
 * the confirming button.
 */
export async function confirmDialog({ title, message, details = [], confirm, ok, danger = false }) {
  const body = h(
    'div',
    {},
    typeof message === 'string' ? h('p', { class: 'dlg-text' }, message) : message,
    details.length ? h('ul', { class: 'dlg-list' }, ...details.map((d) => h('li', {}, d))) : null,
  );
  const r = await choiceDialog({
    title,
    body,
    buttons: [
      { label: t('project.dialog.cancel'), value: false },
      { label: confirm ?? ok ?? t('project.dialog.ok'), primary: true, danger, value: true },
    ],
  });
  return r === true;
}

/** Asks for one line of text; `validate(value)` returns an error message or null. */
export function promptDialog({ title, label, value = '', ok, validate }) {
  const field = h('input', { class: 'field', type: 'text', value, spellcheck: 'false', 'aria-label': label });
  const error = h('div', { class: 'form-error', role: 'alert', hidden: true });
  const body = h('label', { class: 'dlg-field' }, h('span', {}, label), field, error);
  const check = () => {
    const msg = validate?.(field.value.trim()) ?? null;
    error.hidden = !msg;
    error.textContent = msg ?? '';
    return !msg;
  };
  field.addEventListener('input', () => {
    if (!error.hidden) check();
  });
  setTimeout(() => field.select(), 0);
  return choiceDialog({
    title,
    body,
    buttons: [
      { label: t('project.dialog.cancel'), value: null },
      { label: ok ?? t('project.dialog.ok'), primary: true, check, value: () => field.value.trim() },
    ],
  });
}

/** A labelled form row: `<label>label</label> control [hint]`. */
export function row(label, control, hint) {
  return h('div', { class: 'form-row' }, h('label', { class: 'form-label' }, label), h('div', { class: 'form-control' }, control, hint ? h('div', { class: 'form-hint' }, hint) : null));
}

/** `<input>` committing on change (`onchange(value)`); number inputs give numbers or null. */
export function input({ type = 'text', value, placeholder, min, max, step, cls = '', onchange, oninput, title }) {
  const el = h('input', { class: `field ${cls}`.trim(), type, placeholder, min, max, step, title, spellcheck: 'false' });
  if (value != null) el.value = String(value);
  const read = () => (type === 'number' ? (el.value === '' ? null : Number(el.value)) : el.value);
  if (onchange) el.addEventListener('change', () => onchange(read()));
  if (oninput) el.addEventListener('input', () => oninput(read()));
  return el;
}

/** `<select>` from `[{value, label, group?}]` (strings allowed). */
export function select({ options, value, onchange, cls = '', title }) {
  const el = h('select', { class: `field ${cls}`.trim(), title });
  const groups = new Map();
  for (const o of options) {
    const opt = typeof o === 'string' ? { value: o, label: o } : o;
    const node = h('option', { value: opt.value }, opt.label);
    if (opt.disabled) node.disabled = true;
    if (opt.group) {
      if (!groups.has(opt.group)) {
        const g = h('optgroup', { label: opt.group });
        groups.set(opt.group, g);
        el.append(g);
      }
      groups.get(opt.group).append(node);
    } else el.append(node);
  }
  el.value = value ?? '';
  if (onchange) el.addEventListener('change', () => onchange(el.value));
  return el;
}

/** Checkbox with label. */
export function checkbox({ label, checked, onchange, disabled, title }) {
  const box = h('input', { type: 'checkbox', disabled, title });
  box.checked = Boolean(checked);
  if (onchange) box.addEventListener('change', () => onchange(box.checked));
  return label ? h('label', { class: 'check', title }, box, h('span', {}, label)) : box;
}

/** Segmented choice `[{value, label}]`. */
export function segmented({ options, value, onchange }) {
  const el = h('div', { class: 'segmented dlg-seg', role: 'group' });
  const render = (v) =>
    el.replaceChildren(
      ...options.map((o) =>
        h(
          'button',
          {
            type: 'button',
            'aria-pressed': String(o.value === v),
            onclick: () => {
              render(o.value);
              onchange(o.value);
            },
          },
          o.label,
        ),
      ),
    );
  render(value);
  return el;
}

/** Shows `error.message` as a toast-like line inside `el` (or clears it). */
export function errorLine() {
  const el = h('div', { class: 'form-error', role: 'alert', hidden: true });
  return {
    el,
    show(e) {
      el.textContent = e ? (e.message ?? String(e)) : '';
      el.hidden = !e;
    },
  };
}
