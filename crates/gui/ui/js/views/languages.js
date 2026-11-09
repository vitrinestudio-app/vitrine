// "All languages" view (task 2.7, KONZEPT §9.2): the output images of the
// current target in every project language, one row per language
// (`render_image` at small size). Text overflows and emergency breaks of
// each language (from its `overlay`) are marked on the cells. A picker
// shows all images or a single one larger. Clicking a cell switches the
// view language, opens the canvas and selects the text element under the
// pointer (if any).

import { api, toImageBitmap } from '../api.js';
import { h, icon, picker } from '../dom.js';
import { editor } from '../editor.js';
import { inQuad } from '../geometry.js';
import { t } from '../i18n.js';
import { store, viewArgs } from '../store.js';

const ALL = 'all';

export function mountLanguages(root) {
  const choose = picker({ label: t('langgrid.image'), onchange: (v) => {
    focus = v;
    render();
  } });
  const summary = h('span', { class: 'muted small' });
  const head = h('div', { class: 'lg-head' }, choose.el, h('span', { class: 'lg-spacer' }), summary);
  const grid = h('div', { class: 'lg-grid' });
  const el = h('div', { class: 'lg-view' }, head, grid);
  root.append(el);
  let focus = ALL;
  let seq = 0;

  function markersSvg(o, img, w, hgt) {
    const k = w / img.rect.w;
    const S = (p) => `${((p.x - img.rect.x) * k).toFixed(1)},${((p.y - img.rect.y) * (hgt / img.rect.h)).toFixed(1)}`;
    const marks = [];
    for (const e of o.elements) {
      if (!e.text || !(e.text.overflow || e.text.emergency_breaks?.length)) continue;
      const pages = img.pages;
      if (!pages.includes(e.page)) continue;
      const cls = e.text.overflow ? 'lg-overflow' : 'lg-emergency';
      marks.push(`<polygon class="${cls}" points="${e.quad.map(S).join(' ')}"/>`);
    }
    if (!marks.length) return null;
    const svg = document.createElementNS('http://www.w3.org/2000/svg', 'svg');
    svg.setAttribute('class', 'lg-marks');
    svg.setAttribute('viewBox', `0 0 ${w} ${hgt}`);
    svg.style.width = `${w}px`;
    svg.style.height = `${hgt}px`;
    svg.innerHTML = marks.join('');
    return svg;
  }

  function issues(o) {
    let overflow = 0;
    let emergency = 0;
    for (const e of o.elements) {
      if (!e.text) continue;
      if (e.text.overflow) overflow++;
      else if (e.text.emergency_breaks?.length) emergency++;
    }
    return { overflow, emergency };
  }

  async function open(lang, o, img, e, cell) {
    const r = cell.getBoundingClientRect();
    const p = {
      x: img.rect.x + ((e.clientX - r.left) / r.width) * img.rect.w,
      y: img.rect.y + ((e.clientY - r.top) / r.height) * img.rect.h,
    };
    const hit = [...o.elements].reverse().find((x) => x.kind === 'text' && img.pages.includes(x.page) && inQuad(p, x.quad));
    await editor.setContext({ lang });
    store.set({ view: 'canvas', ...(hit ? { selection: [hit.id] } : {}) });
  }

  async function render() {
    const my = ++seq;
    const st = store.get();
    const o0 = st.overlay;
    if (!o0 || !st.summary) {
      grid.replaceChildren(h('div', { class: 'canvas-empty' }, t('canvas.empty')));
      return;
    }
    const langs = [st.summary.languages.default, ...st.summary.languages.available.filter((l) => l !== st.summary.languages.default)];
    if (focus !== ALL && !o0.images.some((i) => String(i.index) === focus)) focus = ALL;
    const pageIds = (img) => img.pages.map((i) => o0.pages.find((p) => p.index === i)?.id ?? i + 1).join(' + ');
    choose.set(
      [{ value: ALL, label: t('langgrid.all') }, ...o0.images.map((i) => ({ value: String(i.index), label: `${i.index + 1} · ${pageIds(i)}` }))],
      focus,
    );

    // Row height: fit all languages, within sensible bounds.
    const avail = Math.max(200, el.clientHeight - 60);
    const rowH = Math.round(Math.min(focus === ALL ? 320 : 640, Math.max(140, avail / langs.length - 28)));
    const dpr = window.devicePixelRatio || 1;
    const shown = focus === ALL ? o0.images : o0.images.filter((i) => String(i.index) === focus);

    const v0 = viewArgs(st);
    const rows = langs.map((lang) => {
      const label = h('div', { class: 'lg-lang' }, h('strong', {}, lang), lang === st.lang ? h('span', { class: 'lg-current' }, t('langgrid.current')) : null);
      const status = h('div', { class: 'lg-status muted small' }, '…');
      const cells = shown.map((img) => {
        const w = Math.round((img.rect.w / img.rect.h) * rowH);
        const canvas = h('canvas');
        canvas.style.width = `${w}px`;
        canvas.style.height = `${rowH}px`;
        const cell = h('figure', { class: 'lg-cell', title: `${lang} · ${t('canvas.image')} ${img.index + 1}` }, canvas, h('figcaption', {}, `${img.index + 1}`));
        return { img, canvas, cell, w };
      });
      const row = h('div', { class: `lg-row${lang === st.lang ? ' current' : ''}` }, h('div', { class: 'lg-row-head' }, label, status), h('div', { class: 'lg-cells' }, ...cells.map((c) => c.cell)));
      return { lang, row, cells, status };
    });
    grid.replaceChildren(...rows.map((r) => r.row));

    let totalIssues = 0;
    await Promise.all(
      rows.map(async (r) => {
        const view = { ...v0, lang: r.lang };
        let o;
        try {
          o = await api.overlay(view);
        } catch (e) {
          r.status.textContent = e.message;
          return;
        }
        if (my !== seq) return;
        const n = issues(o);
        totalIssues += n.overflow + n.emergency;
        r.status.replaceChildren(
          ...(n.overflow ? [h('span', { class: 'lg-bad' }, icon('error'), ` ${n.overflow} ${t('langgrid.overflow')}`)] : []),
          ...(n.emergency ? [h('span', { class: 'lg-warn' }, icon('warning'), ` ${n.emergency} ${t('langgrid.emergency')}`)] : []),
          ...(n.overflow || n.emergency ? [] : [h('span', { class: 'lg-ok' }, icon('check'), ` ${t('langgrid.ok')}`)]),
        );
        summary.textContent = totalIssues ? `${totalIssues} ${t('langgrid.issues')}` : t('langgrid.no_issues');
        for (const c of r.cells) {
          const img = o.images.find((i) => i.index === c.img.index) ?? c.img;
          const marks = markersSvg(o, img, c.w, rowH);
          if (marks) {
            c.cell.append(marks);
            c.cell.classList.add('has-issue');
          }
          c.cell.addEventListener('click', (e) => open(r.lang, o, img, e, c.canvas));
        }
        await Promise.all(
          r.cells.map(async (c) => {
            try {
              const bitmap = await toImageBitmap(await api.renderImage(view, c.img.index, Math.round(c.w * dpr)));
              if (my !== seq) return;
              c.canvas.width = bitmap.width;
              c.canvas.height = bitmap.height;
              c.canvas.getContext('2d').drawImage(bitmap, 0, 0);
              bitmap.close?.();
            } catch (e) {
              console.warn('render failed', e);
            }
          }),
        );
      }),
    );
  }

  let timer = 0;
  const later = () => {
    clearTimeout(timer);
    timer = setTimeout(render, 80);
  };
  const unsubs = [editor.on('invalidate', later), editor.on('reset', later), store.select((s) => [s.target, s.lang, s.summary], later)];
  const ro = new ResizeObserver(later);
  ro.observe(el);
  render();
  return {
    unmount() {
      seq++;
      clearTimeout(timer);
      unsubs.forEach((u) => u());
      ro.disconnect();
      el.remove();
    },
  };
}
