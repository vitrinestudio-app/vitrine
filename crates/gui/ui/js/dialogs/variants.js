// Variants of a strip (KONZEPT §3.4): the "New variant" dialog (name,
// based on another variant, content scale, targets that show it) and the
// remove confirmation with its impact. Used by the top-bar variant picker
// ("+ New variant…"), the strips & targets dialog, the target form and the
// strip settings in the inspector. Ops are built in Rust (`strip_variant`);
// each change is one undo step.

import { api } from '../api.js';
import { h } from '../dom.js';
import { editor } from '../editor.js';
import { t } from '../i18n.js';
import { store } from '../store.js';
import { checkbox, confirmDialog, errorLine, input, openModal, row, select } from './modal.js';

const ID_RE = /^[a-z0-9][a-z0-9_.-]*$/i;

/**
 * Opens the "New variant" dialog for `strip`. `targets`: target ids
 * pre-checked as "shown by"; `from`: variant to copy; `switchTo`: show the
 * new variant in the editor afterwards. Resolves to the new id or null.
 */
export async function openNewVariant({ strip, targets = [], from = '', switchTo = false } = {}) {
  const info = await api.stripGet(strip);
  const taken = new Set(info.variants.map((v) => v.id));
  const err = errorLine();
  let name = '';
  let base = from;
  let scale = null;
  const checked = new Set(targets);
  const suggest = () => {
    const stem = base || 'variant';
    if (!taken.has(stem) && base === '') return stem;
    for (let n = 2; ; n++) if (!taken.has(`${stem}-${n}`)) return `${stem}-${n}`;
  };
  const nameEl = input({ placeholder: suggest(), cls: 'mono', oninput: (v) => (name = v.trim()) });
  const scaleEl = input({ type: 'number', min: 0.05, max: 4, step: 0.05, placeholder: '1', cls: 'num', oninput: (v) => (scale = v) });
  const fromEl = select({
    options: [{ value: '', label: t('variant.from.empty') }, ...info.variants.map((v) => ({ value: v.id, label: `${t('variant.from.copy')} ${v.id}` }))],
    value: base,
    onchange: (v) => {
      base = v;
      nameEl.placeholder = suggest();
      const src = info.variants.find((x) => x.id === v);
      scaleEl.placeholder = src?.content_scale != null ? String(src.content_scale) : '1';
    },
  });
  if (base) fromEl.dispatchEvent(new Event('change'));
  const targetBoxes = info.targets.length
    ? h(
        'div',
        { class: 'stack' },
        info.targets.map((id) => checkbox({ label: id, checked: checked.has(id), onchange: (v) => (v ? checked.add(id) : checked.delete(id)) })),
      )
    : h('span', { class: 'muted small' }, t('variant.targets.none'));

  let result = null;
  const create = async () => {
    const id = name || nameEl.placeholder;
    if (!ID_RE.test(id)) return err.show(new Error(t('inspector.id.invalid')));
    if (taken.has(id)) return err.show(new Error(`${t('variant.exists')}: ${id}`));
    const change = { action: 'add', id, targets: [...checked] };
    if (base) change.from = base;
    if (scale != null && scale > 0) change.content_scale = scale;
    let r;
    try {
      r = await api.stripVariant(strip, change);
    } catch (e) {
      return err.show(e);
    }
    await editor.apply(Promise.resolve(r));
    result = id;
    m.close();
  };
  const createBtn = h('button', { class: 'btn btn-primary', type: 'button', onclick: create }, t('variant.create'));
  const m = openModal({
    title: `${t('variant.new.title')} · ${strip}`,
    size: 'medium',
    body: [
      h('p', { class: 'muted small' }, t('variant.hint')),
      err.el,
      h(
        'div',
        { class: 'form' },
        row(t('variant.name'), nameEl),
        row(t('variant.from'), fromEl),
        row(t('strip.variant.scale'), scaleEl, t('variant.scale.hint')),
        row(t('variant.targets'), targetBoxes, t('variant.targets.hint')),
      ),
    ],
    footer: [h('span', { class: 'spacer' }), h('button', { class: 'btn', type: 'button', onclick: () => m.close() }, t('project.dialog.cancel')), createBtn],
  });
  m.el.addEventListener('keydown', (e) => {
    if (e.key === 'Enter' && e.target?.tagName === 'INPUT' && e.target.type !== 'checkbox') {
      e.preventDefault();
      create();
    }
  });
  setTimeout(() => nameEl.focus(), 0);
  await m.closed;
  if (result && switchTo && store.get().strip === strip) {
    const st = store.get();
    await editor.setContext({ variant: checked.has(st.target) ? undefined : result });
  }
  return result;
}

/**
 * Asks before removing variant `v` (`{id, overrides, targets}` from
 * `strip_get`) of `strip`; targets showing it fall back to the base layout.
 */
export async function removeVariant(strip, v) {
  const details = [
    `${v.overrides.length} ${t('strip.variant.overrides')}`,
    ...v.targets.map((id) => `${t('variant.remove.target')}: ${id}`),
  ];
  const ok = await confirmDialog({
    title: t('strip.variant.remove'),
    message: `${t('variant.remove.confirm')} „${v.id}“?`,
    details,
    confirm: t('strip.variant.remove'),
    danger: true,
  });
  if (!ok) return false;
  await editor.apply(api.stripVariant(strip, { action: 'remove', id: v.id }));
  return true;
}
