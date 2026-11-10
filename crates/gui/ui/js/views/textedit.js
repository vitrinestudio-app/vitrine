// Inline text editor of the canvas (task 2.7): double-click a text element
// to edit the raw markup string of the current language.
//
// The editor is an HTML box attached to the text's content quad (same
// width and rotation), placed just below it (above when there is no room),
// so the live preview of the text itself stays visible: while typing, the
// affected output images are re-rendered from a scratch copy with the
// uncommitted string (`render_preview_string`, throttled, self-paced).
//
// Keys: Enter commits (one `set_string` step), Shift+Enter inserts a line
// break, Esc cancels; clicking elsewhere commits. Toolbar: bold `**`,
// italic `_`, named span `[name]…[/]`, soft hyphen `\-`. Unbalanced markers
// are listed below the field (`markup_check`).

import { api, toImageBitmap } from '../api.js';
import { h, icon } from '../dom.js';
import { editor } from '../editor.js';
import { bounds, inflate, intersects } from '../geometry.js';
import { t } from '../i18n.js';
import { store, viewArgs } from '../store.js';

const PREVIEW_INTERVAL = 90;
const CHECK_DELAY = 150;
const SOFT_HYPHEN = '­';
const MIN_WIDTH = 260;

/** Byte offset (UTF-8) → string index. */
function charIndex(text, byteOffset) {
  const enc = new TextEncoder();
  let bytes = 0;
  for (let i = 0; i < text.length; i++) {
    if (bytes >= byteOffset) return i;
    const cp = text.codePointAt(i);
    bytes += enc.encode(String.fromCodePoint(cp)).length;
    if (cp > 0xffff) i++;
  }
  return text.length;
}

/**
 * Opens the editor for text element `id`. `canvas` is the canvas view's
 * hook object: `{ el, view(), slots(), drawPreview(slot, bitmap),
 * clearPreviews() }`. Returns `{ close(commit) }` or null.
 */
export async function openTextEditor(canvas, id) {
  const view = viewArgs();
  let info;
  try {
    info = await api.elementInfo(view, id);
  } catch (e) {
    editor.toast(e.message, true);
    return null;
  }
  const props = info.element.props;
  if (props.kind !== 'text') return null;
  const st = store.get();
  const el = st.overlay?.elements.find((x) => x.id === id);
  if (!el?.text) return null;

  const lang = info.lang;
  const key = props.key;
  const isDefault = lang === st.summary.languages.default;
  const original = info.own_text ?? '';
  const shown = (info.own_text ?? props.text ?? '').replaceAll(SOFT_HYPHEN, '\\-');

  // ----- DOM ------------------------------------------------------------------
  const input = h('textarea', {
    class: 'textedit-input',
    spellcheck: 'true',
    rows: '2',
    'aria-label': `${t('textedit.title')} ${key}`,
  });
  input.value = shown;
  const diag = h('div', { class: 'textedit-diag', hidden: true });
  const spans = st.summary.styles.spans ?? [];
  const spanSelect = h(
    'select',
    { class: 'textedit-span', title: t('textedit.span'), disabled: spans.length === 0 },
    h('option', { value: '' }, t('textedit.span')),
    ...spans.map((s) => h('option', { value: s }, s)),
  );
  const tool = (label, title, onclick, cls = '') =>
    h('button', { type: 'button', class: `textedit-btn ${cls}`.trim(), title, onclick }, label);
  const toolbar = h(
    'div',
    { class: 'textedit-toolbar' },
    tool('B', `${t('textedit.bold')} (**…**)`, () => wrap('**', '**'), 'bold'),
    tool('I', `${t('textedit.italic')} (_…_)`, () => wrap('_', '_'), 'italic'),
    spanSelect,
    tool('\\-', t('textedit.soft_hyphen'), () => insert('\\-'), 'mono'),
    h('span', { class: 'textedit-spacer' }),
    h('span', { class: 'textedit-key', title: key }, `${key} · ${lang}`),
    tool(icon('check'), t('textedit.commit'), () => close(true)),
    tool(icon('close'), t('textedit.cancel'), () => close(false)),
  );
  const note = info.own_text == null ? h('div', { class: 'textedit-note' }, `${t('textedit.fallback')} ${props.text_lang ?? '—'}`) : null;
  const hint = h('div', { class: 'textedit-hint' }, t('textedit.hint'));
  const box = h('div', { class: 'textedit', role: 'dialog' }, toolbar, note, input, diag, hint);
  canvas.el.append(box);
  canvas.el.classList.add('text-editing');

  // Keep canvas gestures and shortcuts away from the editor.
  for (const type of ['pointerdown', 'dblclick', 'wheel']) box.addEventListener(type, (e) => e.stopPropagation());
  // Toolbar clicks must not blur (and thereby commit) the field.
  toolbar.addEventListener('mousedown', (e) => {
    if (e.target !== spanSelect) e.preventDefault();
  });

  // ----- placement --------------------------------------------------------------
  function place() {
    const { zoom, panX, panY } = canvas.view();
    const q = (store.get().overlay?.elements.find((x) => x.id === id) ?? el).text.content_quad;
    const S = (p) => ({ x: p.x * zoom + panX, y: p.y * zoom + panY });
    const [a, b, , d] = q.map(S);
    const width = Math.max(MIN_WIDTH, Math.hypot(b.x - a.x, b.y - a.y));
    const height = Math.hypot(d.x - a.x, d.y - a.y);
    const rot = el.rotation || 0;
    const rad = (rot * Math.PI) / 180;
    // Downward unit vector of the (rotated) box.
    const v = { x: -Math.sin(rad), y: Math.cos(rad) };
    const cx = (a.x + b.x) / 2;
    const cy = (a.y + b.y) / 2;
    const boxH = box.offsetHeight || 120;
    const below = { x: cx + v.x * (height + 8), y: cy + v.y * (height + 8) };
    const above = { x: cx - v.x * (boxH + 8), y: cy - v.y * (boxH + 8) };
    const room = canvas.el.clientHeight - (below.y + boxH);
    const at = room < 0 && above.y > 0 ? above : below;
    box.style.width = `${width}px`;
    // Rotated around its top centre (CSS transform-origin), which sits at `at`.
    box.style.left = `${at.x - width / 2}px`;
    box.style.top = `${at.y}px`;
    box.style.transform = rot ? `rotate(${rot}deg)` : '';
    // Font size of the field follows the text a little (readable range).
    const px = Math.min(22, Math.max(12, props.style.size * zoom));
    input.style.fontSize = `${px}px`;
  }

  // ----- editing helpers ------------------------------------------------------------
  function wrap(open, close_) {
    const { selectionStart: s, selectionEnd: e, value } = input;
    const inner = value.slice(s, e);
    input.setRangeText(`${open}${inner}${close_}`, s, e, 'select');
    if (!inner) input.setSelectionRange(s + open.length, s + open.length);
    else input.setSelectionRange(s + open.length, s + open.length + inner.length);
    input.focus();
    changed();
  }

  function insert(text) {
    input.setRangeText(text, input.selectionStart, input.selectionEnd, 'end');
    input.focus();
    changed();
  }

  spanSelect.addEventListener('change', () => {
    const name = spanSelect.value;
    spanSelect.value = '';
    if (name) wrap(`[${name}]`, '[/]');
  });

  function autosize() {
    input.style.height = 'auto';
    input.style.height = `${Math.min(240, input.scrollHeight + 2)}px`;
  }

  // ----- live preview + markup check --------------------------------------------------
  const pump = { inflight: false, sent: null, last: 0, timer: 0 };
  let checkTimer = 0;
  let closed = false;
  const area = inflate(bounds(el.quad), 40);
  const touched = new Set();

  function changed() {
    autosize();
    place();
    schedulePreview();
    clearTimeout(checkTimer);
    checkTimer = setTimeout(check, CHECK_DELAY);
  }

  async function check() {
    const value = input.value;
    try {
      const r = await api.markupCheck(value);
      if (closed || value !== input.value) return;
      diag.hidden = r.diagnostics.length === 0;
      diag.replaceChildren(
        ...r.diagnostics.map((d) => {
          const i = charIndex(value, d.offset);
          return h('div', {}, icon('warning'), ` ${t('textedit.unbalanced')} ${d.marker} (${t('textedit.at')} ${i + 1})`);
        }),
      );
      const unknown = r.spans.filter((s) => !spans.includes(s));
      if (unknown.length) {
        diag.hidden = false;
        diag.append(h('div', {}, icon('warning'), ` ${t('textedit.unknown_span')} ${unknown.join(', ')}`));
      }
    } catch {
      // A failed check only hides the hints.
    }
  }

  function schedulePreview() {
    if (pump.inflight || closed) return;
    clearTimeout(pump.timer);
    pump.timer = setTimeout(sendPreview, Math.max(0, PREVIEW_INTERVAL - (performance.now() - pump.last)));
  }

  async function sendPreview() {
    if (closed) return;
    const value = input.value;
    if (value === pump.sent) return;
    pump.inflight = true;
    pump.sent = value;
    pump.last = performance.now();
    const slots = canvas.slots().filter((s) => s.kind === 'image' && (intersects(area, s.rect) || touched.has(s.index)));
    await Promise.all(
      slots.map(async (slot) => {
        try {
          const width = Math.max(1, slot.real.width || 400);
          const img = await api.renderPreviewString(view, key, value, slot.index, width, width > 600);
          const bitmap = await toImageBitmap(img);
          if (closed) return;
          canvas.drawPreview(slot, bitmap);
          touched.add(slot.index);
        } catch (e) {
          console.warn('text preview failed', e);
        }
      }),
    );
    pump.inflight = false;
    if (!closed && input.value !== pump.sent) schedulePreview();
  }

  // ----- commit / cancel --------------------------------------------------------------
  async function close(commit) {
    if (closed) return;
    closed = true;
    clearTimeout(pump.timer);
    clearTimeout(checkTimer);
    unsubs.forEach((u) => u());
    box.remove();
    canvas.el.classList.remove('text-editing');
    canvas.el.focus({ preventScroll: true });
    const value = input.value;
    const unchanged = value === shown || value === original;
    if (!commit || unchanged) {
      canvas.clearPreviews();
      return;
    }
    // Empty in a non-default language = no own translation (fallback).
    const next = value === '' && !isDefault ? null : value;
    const result = await editor.apply(api.setString(lang, key, next, `Edit text ${key} (${lang})`), { touched: [id] });
    if (!result?.applied) canvas.clearPreviews();
  }

  input.addEventListener('input', changed);
  input.addEventListener('keydown', (e) => {
    if (e.key === 'Enter' && !e.shiftKey && !e.isComposing) {
      e.preventDefault();
      close(true);
    } else if (e.key === 'Escape') {
      e.preventDefault();
      e.stopPropagation();
      close(false);
    }
  });
  input.addEventListener('blur', () => {
    // Focus moving into the toolbar (span picker) keeps the editor open.
    setTimeout(() => {
      if (!closed && !box.contains(document.activeElement)) close(true);
    }, 0);
  });
  spanSelect.addEventListener('blur', () => {
    setTimeout(() => {
      if (!closed && !box.contains(document.activeElement)) close(true);
    }, 0);
  });

  const unsubs = [
    store.select((s) => [s.zoom, s.panX, s.panY], place),
    editor.on('reset', () => close(false)),
  ];

  autosize();
  place();
  requestAnimationFrame(place);
  input.focus();
  input.select();
  check();
  return { close };
}
