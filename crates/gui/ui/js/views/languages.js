// "All languages" view (task 2.7, KONZEPT §9.2): the output images of the
// current target in every project language, one row per language
// (`render_image`). Text overflows and emergency breaks of each language
// (from its `overlay`) are marked on the cells. A picker shows all images
// or a single one. Clicking a cell switches the view language, opens the
// canvas and selects the text element under the pointer (if any).
//
// Zoom/pan like the canvas (views/viewport.js): world units are lu, rows
// are stacked in the world; the language labels sit in a fixed gutter on
// the left. `fit()` keeps rows readable (≥ MIN_ROW px) and then scrolls.

import { api, toImageBitmap } from '../api.js';
import { h, icon, picker } from '../dom.js';
import { editor } from '../editor.js';
import { inQuad, intersects } from '../geometry.js';
import { t } from '../i18n.js';
import { currentTarget, store, viewArgs } from '../store.js';
import { applyWorld, createViewport, densityFor, makeSlot, paintSlot } from './viewport.js';

const ALL = 'all';
/** Width of the language label gutter (px). */
const GUTTER = 150;
/** Smallest row height `fit()` uses (px). */
const MIN_ROW = 140;

export function mountLanguages(root) {
  const choose = picker({
    label: t('langgrid.image'),
    onchange: (v) => {
      focus = v;
      later(0);
    },
  });
  const summary = h('span', { class: 'muted small' });
  const head = h('div', { class: 'lg-head' }, choose.el, h('span', { class: 'lg-spacer' }), summary);
  const world = h('div', { class: 'canvas-world' });
  const labels = h('div', { class: 'vp-labels' });
  const gutter = h('div', { class: 'lg-gutter' });
  const empty = h('div', { class: 'canvas-empty', hidden: true }, t('canvas.empty'));
  const stage = h('div', { class: 'canvas-view zoom-view lg-stage', tabindex: '0' }, world, labels, gutter, empty);
  const el = h('div', { class: 'lg-view' }, head, stage);
  root.append(el);

  let focus = ALL;
  let seq = 0;
  let sig = null;
  /** `[{ lang, label, status, y, slots: [{…slot, index, source, caption}] }]` */
  let rows = [];
  let content = null;
  let rowH = 1;
  let resTimer = 0;

  function exportScale() {
    const o = store.get().overlay;
    const target = currentTarget();
    if (!o?.images.length || !target) return 1;
    return target.size[0] / o.images[0].rect.w;
  }

  const vp = createViewport(stage, {
    key: 'languages',
    content: () => content,
    unit: exportScale,
    pointer: 'drag',
    padding: 24,
    inset: { left: GUTTER },
    fitFloor: () => MIN_ROW / rowH,
    onChange() {
      applyTransform();
      scheduleResolution();
    },
    onResize: () => scheduleResolution(),
  });

  function applyTransform() {
    const { zoom, panX, panY } = vp.view();
    applyWorld(world, { zoom, panX, panY });
    for (const r of rows) {
      r.label.style.top = `${r.y * zoom + panY}px`;
      for (const s of r.slots) {
        s.caption.style.left = `${(s.rect.x + s.rect.w / 2) * zoom + panX}px`;
        s.caption.style.top = `${(s.rect.y + s.rect.h) * zoom + panY + 2}px`;
      }
    }
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

  /** Overflow/emergency markers of image `img` (lu viewBox, scales with the slot). */
  function markersSvg(o, img) {
    const S = (p) => `${(p.x - img.rect.x).toFixed(1)},${(p.y - img.rect.y).toFixed(1)}`;
    const marks = [];
    for (const e of o.elements) {
      if (!e.text || !(e.text.overflow || e.text.emergency_breaks?.length)) continue;
      if (!img.pages.includes(e.page)) continue;
      const cls = e.text.overflow ? 'lg-overflow' : 'lg-emergency';
      marks.push(`<polygon class="${cls}" points="${e.quad.map(S).join(' ')}"/>`);
    }
    if (!marks.length) return null;
    const svg = document.createElementNS('http://www.w3.org/2000/svg', 'svg');
    svg.setAttribute('class', 'lg-marks');
    svg.setAttribute('viewBox', `0 0 ${img.rect.w} ${img.rect.h}`);
    svg.setAttribute('preserveAspectRatio', 'none');
    svg.innerHTML = marks.join('');
    return svg;
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

  function languages(st) {
    const l = st.summary.languages;
    return [l.default, ...l.available.filter((x) => x !== l.default)];
  }

  /** Rebuilds rows and slots (languages, images or focus changed). */
  function rebuild(langs, shown) {
    world.replaceChildren();
    labels.replaceChildren();
    gutter.replaceChildren();
    rows = [];
    content = null;
    if (!shown.length) return;
    rowH = Math.max(...shown.map((i) => i.rect.h));
    const gapX = rowH * 0.03;
    const gapY = rowH * 0.14;
    let width = 0;
    langs.forEach((lang, n) => {
      const y = n * (rowH + gapY);
      const status = h('div', { class: 'lg-status muted small' }, '…');
      const current = h('span', { class: 'lg-current', hidden: true }, t('langgrid.current'));
      const label = h('div', { class: 'lg-label' }, h('div', { class: 'lg-lang' }, h('strong', {}, lang), current), status);
      gutter.append(label);
      let x = 0;
      const slots = shown.map((img) => {
        const slot = makeSlot(world, { x, y, w: img.rect.w, h: img.rect.h }, 'lg-cell');
        slot.el.title = `${lang} · ${t('canvas.image')} ${img.index + 1}`;
        slot.index = img.index;
        slot.source = img.rect;
        slot.caption = h('div', { class: 'vp-label lg-caption' }, `${img.index + 1}`);
        labels.append(slot.caption);
        x += img.rect.w + gapX;
        return slot;
      });
      width = Math.max(width, x - gapX);
      rows.push({ lang, label, current, status, y, slots, overlay: null });
    });
    content = { x: 0, y: 0, w: width, h: langs.length * (rowH + gapY) - gapY };
  }

  async function renderSlot(row, slot) {
    const level = densityFor(vp.view().zoom, slot.rect, exportScale());
    const gen = ++slot.gen;
    slot.dirty = false;
    try {
      const view = { ...viewArgs(), lang: row.lang };
      const bitmap = await toImageBitmap(await api.renderImage(view, slot.index, Math.max(1, Math.round(slot.rect.w * level))));
      if (gen !== slot.gen) {
        bitmap.close?.();
        return;
      }
      paintSlot(slot, bitmap, level);
    } catch (e) {
      if (gen === slot.gen) slot.dirty = true;
      console.warn('render failed', e);
    }
  }

  function updateResolution() {
    const zoom = vp.view().zoom;
    for (const row of rows) {
      for (const slot of row.slots) {
        if (!vp.isVisible(slot.rect)) continue;
        if (slot.dirty || Math.abs(slot.level - densityFor(zoom, slot.rect, exportScale())) > 1e-9) renderSlot(row, slot);
      }
    }
  }

  function scheduleResolution(delay = 160) {
    clearTimeout(resTimer);
    resTimer = setTimeout(updateResolution, delay);
  }

  /** Fetches every language's overlay: status line, markers, click targets. */
  async function refreshIssues(my) {
    let total = 0;
    await Promise.all(
      rows.map(async (r) => {
        let o;
        try {
          o = await api.overlay({ ...viewArgs(), lang: r.lang });
        } catch (e) {
          if (my === seq) r.status.textContent = e.message;
          return;
        }
        if (my !== seq) return;
        r.overlay = o;
        const n = issues(o);
        total += n.overflow + n.emergency;
        r.status.replaceChildren(
          ...(n.overflow ? [h('div', { class: 'lg-bad' }, icon('error'), ` ${n.overflow} ${t('langgrid.overflow')}`)] : []),
          ...(n.emergency ? [h('div', { class: 'lg-warn' }, icon('warning'), ` ${n.emergency} ${t('langgrid.emergency')}`)] : []),
          ...(n.overflow || n.emergency ? [] : [h('div', { class: 'lg-ok' }, icon('check'), ` ${t('langgrid.ok')}`)]),
        );
        summary.textContent = total ? `${total} ${t('langgrid.issues')}` : t('langgrid.no_issues');
        for (const slot of r.slots) {
          const img = o.images.find((i) => i.index === slot.index);
          slot.el.querySelector('.lg-marks')?.remove();
          slot.el.classList.remove('has-issue');
          if (!img) continue;
          const marks = markersSvg(o, img);
          if (marks) {
            slot.el.append(marks);
            slot.el.classList.add('has-issue');
          }
        }
      }),
    );
  }

  /** Re-reads the context; `all`: every pixmap is stale. */
  function refresh() {
    const my = ++seq;
    const st = store.get();
    const o0 = st.overlay;
    if (!o0 || !st.summary) {
      rebuild([], []);
      sig = null;
      empty.hidden = false;
      return;
    }
    empty.hidden = true;
    const langs = languages(st);
    if (focus !== ALL && !o0.images.some((i) => String(i.index) === focus)) focus = ALL;
    const pageIds = (img) => img.pages.map((i) => o0.pages.find((p) => p.index === i)?.id ?? i + 1).join(' + ');
    choose.set(
      [{ value: ALL, label: t('langgrid.all') }, ...o0.images.map((i) => ({ value: String(i.index), label: `${i.index + 1} · ${pageIds(i)}` }))],
      focus,
    );
    const shown = focus === ALL ? o0.images : o0.images.filter((i) => String(i.index) === focus);
    const next = JSON.stringify([langs, focus, shown.map((i) => i.rect)]);
    if (next !== sig) {
      sig = next;
      rebuild(langs, shown);
      if (content) vp.refresh(`${st.summary.dir}|${focus}:${langs.length}:${content.w}x${content.h}`);
    }
    for (const r of rows) {
      r.label.classList.toggle('current', r.lang === st.lang);
      r.current.hidden = r.lang !== st.lang;
    }
    applyTransform();
    for (const r of rows) for (const s of r.slots) s.dirty = true;
    updateResolution();
    refreshIssues(my);
  }

  // Clicks on a cell (a drag pans; the viewport swallows its click).
  stage.addEventListener('click', (e) => {
    const cell = e.target.closest?.('.lg-cell');
    if (!cell) return;
    for (const r of rows) {
      const slot = r.slots.find((s) => s.el === cell);
      if (!slot) continue;
      const o = r.overlay ?? store.get().overlay;
      const img = o.images.find((i) => i.index === slot.index);
      if (img) open(r.lang, o, img, e, cell);
      return;
    }
  });

  let timer = 0;
  function later(delay = 80) {
    clearTimeout(timer);
    timer = setTimeout(refresh, delay);
  }
  const unsubs = [
    editor.on('invalidate', ({ all, rects }) => {
      // Only the touched images change; the issue lists may change anywhere.
      if (!all) {
        for (const r of rows) for (const s of r.slots) if ((rects ?? []).some((x) => intersects(x, s.source))) s.dirty = true;
        updateResolution();
        refreshIssues(++seq);
        return;
      }
      later();
    }),
    // Target/language switches and reloads come as `reset`.
    editor.on('reset', () => later()),
  ];
  refresh();
  return {
    el: stage,
    fit: vp.fit,
    zoomIn: vp.zoomIn,
    zoomOut: vp.zoomOut,
    zoomTo: vp.zoomTo,
    zoomActual: vp.zoomActual,
    percent: vp.percent,
    /** Fit mode: the view re-fits on resize until a manual zoom/pan. */
    get fitMode() {
      return vp.fitMode;
    },
    unmount() {
      seq++;
      clearTimeout(timer);
      clearTimeout(resTimer);
      unsubs.forEach((u) => u());
      vp.destroy();
      el.remove();
    },
  };
}
