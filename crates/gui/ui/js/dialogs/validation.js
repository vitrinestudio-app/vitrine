// Validation panel (task 2.9): a popover under the top-bar badge with all
// findings of the project, grouped target → language → page, filters by
// level and code, "preview mode" (missing screenshots are warnings) and
// click-to-navigate (switch target/lang, select the element, zoom to its
// page). Refreshes itself (throttled) after edits and external changes
// while open.

import { api } from '../api.js';
import { h, icon } from '../dom.js';
import { editor } from '../editor.js';
import { t } from '../i18n.js';
import { store } from '../store.js';
import { checkbox, select } from './modal.js';

const LEVELS = ['error', 'warn', 'info'];
const REFRESH_MS = 700;

let open = null;

/** Toggles the panel below `anchor`; `shell` gives access to the canvas view. */
export function toggleValidation(anchor, shell) {
  if (open) {
    open.close();
    return;
  }
  open = mountPanel(anchor, shell);
}

function mountPanel(anchor, shell) {
  const levels = new Set(['error', 'warn']);
  let code = '';
  let report = null;
  let seq = 0;
  let timer = 0;

  const counts = h('div', { class: 'vp-levels' });
  const codeSel = h('span');
  const status = h('span', { class: 'muted small' });
  const body = h('div', { class: 'vp-body' });
  const preview = checkbox({
    label: t('validation.preview'),
    title: t('validation.preview.hint'),
    checked: store.get().validationPreview !== false,
    onchange: (v) => {
      store.set({ validationPreview: v });
      editor.scheduleValidation();
      refresh();
    },
  });
  const el = h(
    'div',
    { class: 'popover vp', role: 'dialog', 'aria-label': t('validation.title') },
    h(
      'header',
      { class: 'vp-head' },
      h('strong', {}, t('validation.title')),
      h('span', { class: 'spacer' }),
      status,
      h('button', { class: 'icon-btn', type: 'button', title: t('validation.refresh'), onclick: () => refresh() }, icon('reset')),
      h('button', { class: 'icon-btn', type: 'button', title: t('project.dialog.close'), onclick: () => close() }, icon('close')),
    ),
    h('div', { class: 'vp-filters' }, counts, codeSel, preview),
    body,
  );
  document.body.append(el);
  place();

  function place() {
    const r = anchor.getBoundingClientRect();
    el.style.top = `${r.bottom + 6}px`;
    el.style.right = `${Math.max(8, window.innerWidth - r.right - 40)}px`;
  }

  async function refresh() {
    const my = ++seq;
    status.textContent = t('validation.running');
    try {
      const r = await api.validateAll({ preview: store.get().validationPreview !== false });
      if (my !== seq) return;
      if (!report && !r.errors && !r.warnings) levels.add('info');
      report = r;
      status.textContent = '';
      render();
    } catch (e) {
      if (my === seq) status.textContent = e.message;
    }
  }

  function schedule() {
    clearTimeout(timer);
    timer = setTimeout(refresh, REFRESH_MS);
  }

  function render() {
    const findings = report.findings;
    const n = { error: report.errors, warn: report.warnings, info: report.infos };
    counts.replaceChildren(
      ...LEVELS.map((l) =>
        h(
          'button',
          {
            type: 'button',
            class: `vp-level vp-${l}`,
            'aria-pressed': String(levels.has(l)),
            onclick: () => {
              if (levels.has(l)) levels.delete(l);
              else levels.add(l);
              render();
            },
          },
          icon(l === 'error' ? 'error' : l === 'warn' ? 'warning' : 'check'),
          `${n[l]} ${t(`validation.level.${l}`)}`,
        ),
      ),
    );
    const codes = [...new Set(findings.map((f) => f.code))].sort();
    codeSel.replaceChildren(
      select({
        options: [{ value: '', label: t('validation.codes.all') }, ...codes.map((c) => ({ value: c, label: `${c} (${findings.filter((f) => f.code === c).length})` }))],
        value: codes.includes(code) ? code : '',
        onchange: (v) => {
          code = v;
          render();
        },
      }),
    );
    const shown = findings.filter((f) => levels.has(f.level) && (!code || f.code === code));
    if (!shown.length) {
      body.replaceChildren(h('div', { class: 'placeholder' }, findings.length ? t('validation.filtered') : t('validation.none')));
      return;
    }
    // target → lang → page
    const tree = new Map();
    for (const f of shown) {
      const tk = f.target ?? '';
      const lk = f.lang ?? '';
      if (!tree.has(tk)) tree.set(tk, new Map());
      const langs = tree.get(tk);
      if (!langs.has(lk)) langs.set(lk, []);
      langs.get(lk).push(f);
    }
    const nodes = [];
    for (const [tk, langs] of tree) {
      const total = [...langs.values()].reduce((a, b) => a + b.length, 0);
      const group = h('details', { class: 'vp-target', open: true }, h('summary', {}, h('strong', {}, tk || t('validation.project')), h('span', { class: 'muted small' }, ` · ${total}`)));
      for (const [lk, list] of langs) {
        group.append(h('div', { class: 'vp-lang' }, lk ? `${t('validation.lang')} ${lk}` : t('validation.all_langs')));
        list.sort((a, b) => (a.page ?? -1) - (b.page ?? -1));
        for (const f of list) {
          group.append(
            h(
              'button',
              { type: 'button', class: `vp-item vp-${f.level}`, title: t('validation.goto'), onclick: () => goto(f) },
              icon(f.level === 'error' ? 'error' : f.level === 'warn' ? 'warning' : 'check'),
              h('span', { class: 'vp-where' }, f.page != null ? `${t('canvas.page')} ${f.page + 1}` : ''),
              h('span', { class: 'vp-msg' }, f.element ? h('span', { class: 'mono' }, `${f.element}: `) : null, f.message),
              h('span', { class: 'vp-code mono' }, f.code),
            ),
          );
        }
      }
      nodes.push(group);
    }
    body.replaceChildren(...nodes);
  }

  async function goto(f) {
    const st = store.get();
    const tg = f.target && st.summary.targets.find((x) => x.id === f.target);
    const patch = {};
    if (tg && tg.strip !== st.strip) patch.strip = tg.strip;
    if (tg && (tg.id !== st.target || patch.strip)) patch.target = tg.id;
    if (f.lang && f.lang !== st.lang) patch.lang = f.lang;
    if (Object.keys(patch).length) await editor.setContext(patch);
    if (store.get().view !== 'canvas') store.set({ view: 'canvas' });
    const overlay = store.get().overlay;
    if (f.element && overlay) {
      const known = overlay.elements.some((e) => e.id === f.element) || overlay.groups.some((g) => g.id === f.element);
      if (known) store.set({ selection: [f.element] });
    }
    const page = f.page ?? overlay?.elements.find((e) => e.id === f.element)?.page;
    const rect = overlay?.pages.find((p) => p.index === page)?.rect;
    if (rect) requestAnimationFrame(() => shell?.canvas?.zoomToRect?.(rect));
  }

  const offs = [editor.on('invalidate', schedule), editor.on('reset', schedule)];
  const onResize = () => place();
  window.addEventListener('resize', onResize);
  const onDown = (e) => {
    if (!el.contains(e.target) && !anchor.contains(e.target) && !e.target.closest?.('dialog')) close();
  };
  setTimeout(() => document.addEventListener('mousedown', onDown), 0);
  const onKey = (e) => e.key === 'Escape' && close();
  document.addEventListener('keydown', onKey);

  function close() {
    clearTimeout(timer);
    seq++;
    for (const off of offs) off();
    window.removeEventListener('resize', onResize);
    document.removeEventListener('mousedown', onDown);
    document.removeEventListener('keydown', onKey);
    el.remove();
    open = null;
  }

  refresh();
  return { close };
}
