// Font family picker (ADR 0028): a search field and a grouped list –
// Vitrine Studio's bundled families (with their category), the project's
// fonts (incl. system fonts stored in the project) and the fonts installed
// on this computer (when the app setting "Use installed fonts" is on).
// Bundled families win over installed ones of the same name, so those are
// not listed twice. Typing filters the list; the list itself commits.
//
// `fonts` is the `catalog_fonts` result.

import { h } from '../dom.js';
import { t } from '../i18n.js';

/** Grouped options `[{value, label, group}]` of every family the picker offers. */
export function fontPickerOptions(fonts) {
  const info = fonts.bundled_info ?? fonts.bundled.map((family) => ({ family }));
  const bundled = new Set(fonts.bundled.map((f) => f.toLowerCase()));
  const project = (fonts.project ?? []).filter((f) => !bundled.has(f.toLowerCase()));
  const taken = new Set([...bundled, ...project.map((f) => f.toLowerCase())]);
  const stored = new Set((fonts.stored ?? []).map((f) => f.toLowerCase()));
  const installed = (fonts.installed ?? []).filter((f) => !taken.has(f.family.toLowerCase()));
  return [
    ...info.map((f) => ({ value: f.family, label: f.category ? `${f.family} · ${t(`fonts.category.${f.category}`)}` : f.family, group: t('fonts.group.bundled') })),
    ...project.map((f) => ({ value: f, label: stored.has(f.toLowerCase()) ? `${f} · ${t('fonts.stored')}` : f, group: t('fonts.group.project') })),
    ...installed.map((f) => ({ value: f.family, label: f.family, group: t('fonts.group.system') })),
  ];
}

/**
 * `{ value, fonts, emptyLabel, onchange }` → element. `emptyLabel` is the
 * label of the "no font set" choice (`''`).
 */
export function fontPicker({ value, fonts, emptyLabel, onchange }) {
  const all = fontPickerOptions(fonts);
  const current = value ?? '';
  const known = !current || all.some((o) => o.value.toLowerCase() === current.toLowerCase());
  const sel = h('select', { class: 'field font-picker-list', title: t('fonts.picker.title') });
  const search = h('input', { class: 'field font-picker-search', type: 'search', placeholder: t('fonts.picker.search'), spellcheck: 'false' });
  const count = h('span', { class: 'muted small font-picker-count' });

  function fill(query) {
    const q = query.trim().toLowerCase();
    const shown = q ? all.filter((o) => o.value.toLowerCase().includes(q)) : all;
    const nodes = [h('option', { value: '' }, emptyLabel)];
    if (!known) nodes.push(h('option', { value: current }, `${current} · ${t('fonts.missing')}`));
    const groups = new Map();
    for (const o of shown) {
      if (!groups.has(o.group)) groups.set(o.group, h('optgroup', { label: o.group }));
      groups.get(o.group).append(h('option', { value: o.value }, o.label));
    }
    sel.replaceChildren(...nodes, ...groups.values());
    // Keep the current choice selected even when the filter hides it.
    if (current && !shown.some((o) => o.value === current) && known) {
      sel.prepend(h('option', { value: current }, current));
    }
    sel.value = current;
    count.textContent = q ? t('fonts.picker.matches').replace('{n}', String(shown.length)) : '';
  }

  search.addEventListener('input', () => fill(search.value));
  search.addEventListener('keydown', (e) => {
    // Enter picks the first match.
    if (e.key !== 'Enter') return;
    const q = search.value.trim().toLowerCase();
    const first = all.find((o) => o.value.toLowerCase().includes(q));
    if (q && first && first.value !== current) onchange(first.value);
  });
  sel.addEventListener('change', () => onchange(sel.value));
  fill('');
  return h('div', { class: 'font-picker' }, search, sel, count);
}
