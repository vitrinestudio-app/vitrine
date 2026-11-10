// Asset import (task 2.8): the import dialog with variant detection and
// drag & drop routing.
//
// `importFiles(sources, opts)` shows one row per file: kind (screenshot,
// image, font), and for screenshots language, variant (detected from the
// pixel size via `assets_probe`: the detected variant or the candidates
// are shown and can be overridden) and key. Sources are `{ name, path }`
// (native paths: file dialog, Tauri drops → `assets_import`) or
// `{ name, file }` (browser `File` objects: dev bridge drops →
// `assets_upload`, base64).
//
// `installDropImport(getCanvas)` routes file drops: onto a device on the
// canvas → that device's screenshot key for the current language and the
// target's screenshot variant (replacing the file); anywhere else → the
// import dialog. In the Tauri app drops arrive as native `tauri://drag-*`
// events with paths; in a browser as HTML5 drops with `File`s.

import { api, listen } from '../api.js';
import { h, icon } from '../dom.js';
import { editor } from '../editor.js';
import { t } from '../i18n.js';
import { currentTarget, store, viewArgs } from '../store.js';
import { isTauri } from '../transport.js';
import { openModal } from './modal.js';

const FONT_EXT = ['ttf', 'otf', 'ttc'];
const RASTER_EXT = ['png', 'jpg', 'jpeg'];

export const extOf = (name) => (/\.([^./\\]+)$/.exec(name)?.[1] ?? '').toLowerCase();
export const baseName = (path) => path.split(/[\\/]/).pop();

/** Same rules as the backend (`commands::assets::sanitize`). */
export function sanitize(name) {
  let out = '';
  for (const ch of name.toLowerCase()) {
    if (/[a-z0-9_]/.test(ch)) out += ch;
    else if (!out.endsWith('-')) out += '-';
  }
  out = out.replace(/^[-_]+|[-_]+$/g, '');
  return out || 'file';
}

const stem = (name) => name.replace(/\.[^.]*$/, '');

function kindsFor(ext) {
  if (FONT_EXT.includes(ext)) return ['font'];
  if (ext === 'svg') return ['asset'];
  if (RASTER_EXT.includes(ext)) return ['screenshot', 'asset'];
  return [];
}

function readBase64(file) {
  return new Promise((resolve, reject) => {
    const r = new FileReader();
    r.onload = () => resolve(String(r.result).replace(/^data:[^,]*,/, ''));
    r.onerror = () => reject(r.error);
    r.readAsDataURL(file);
  });
}

async function sizeOfFile(file) {
  try {
    const bmp = await createImageBitmap(file);
    const size = [bmp.width, bmp.height];
    bmp.close?.();
    return size;
  } catch {
    return null;
  }
}

/** Screenshot variants named by the project's targets (unique, in order). */
function targetVariants() {
  const out = [];
  for (const tg of store.get().summary?.targets ?? []) if (tg.screenshots && !out.includes(tg.screenshots)) out.push(tg.screenshots);
  return out;
}

/**
 * Import dialog for `sources`. `opts`: { kind, key (locks the key of a
 * single file), lang, variant, replace, title }. Resolves to the imported
 * files (empty when cancelled).
 */
export async function importFiles(sources, opts = {}) {
  const summary = store.get().summary;
  if (!summary || !sources.length) return [];
  const usable = sources.filter((s) => kindsFor(extOf(s.name)).length);
  const skipped = sources.filter((s) => !usable.includes(s));
  if (!usable.length) {
    editor.toast(`${t('assets.unsupported')}: ${skipped.map((s) => s.name).join(', ')}`, true);
    return [];
  }

  // Pixel sizes + variant detection.
  const sizes = await Promise.all(usable.map((s) => (s.file && RASTER_EXT.includes(extOf(s.name)) ? sizeOfFile(s.file) : null)));
  const withPath = usable.filter((s) => s.path);
  const bySize = usable.map((s, i) => (!s.path && sizes[i] ? sizes[i] : null)).filter(Boolean);
  let probed = [];
  try {
    probed = await api.assetsProbe({ paths: withPath.map((s) => s.path), sizes: bySize });
  } catch (e) {
    editor.toast(e.message, true);
  }
  let pi = 0;
  let si = withPath.length;
  const probes = usable.map((s, i) => (s.path ? probed[pi++] : !s.path && sizes[i] ? probed[si++] : null) ?? { candidates: [] });

  const langs = summary.languages.available;
  const variants = targetVariants();
  const rows = usable.map((src, i) => {
    const ext = extOf(src.name);
    const kinds = kindsFor(ext);
    const probe = probes[i];
    const kind = opts.kind && kinds.includes(opts.kind) ? opts.kind : kinds[0] === 'screenshot' && !probe.candidates.length && !opts.key ? 'asset' : kinds[0];
    const kindSel = h(
      'select',
      { class: 'as-select', 'aria-label': t('assets.kind'), disabled: kinds.length < 2 },
      ...kinds.map((k) => h('option', { value: k }, t(`assets.kind_${k}`))),
    );
    kindSel.value = kind;
    const langSel = h('select', { class: 'as-select', 'aria-label': t('assets.lang') }, ...langs.map((l) => h('option', { value: l }, l)));
    langSel.value = opts.lang ?? store.get().lang ?? summary.languages.default;
    const options = [...new Set([...(opts.variant ? [opts.variant] : []), ...probe.candidates, ...variants])];
    const varSel = h(
      'select',
      { class: 'as-select', 'aria-label': t('assets.variant') },
      ...options.map((v) => h('option', { value: v }, `${v}${probe.candidates.includes(v) ? ' ✓' : ''}`)),
      h('option', { value: '' }, t('assets.variant_none')),
    );
    varSel.value = opts.variant ?? probe.variant ?? variants[0] ?? '';
    const detection = probe.variant
      ? h('span', { class: 'as-detected' }, icon('check'), ` ${t('assets.detected')} ${probe.variant}${probe.candidates.length > 1 ? ` (${t('assets.candidates')}: ${probe.candidates.join(', ')})` : ''}`)
      : RASTER_EXT.includes(ext)
        ? h('span', { class: 'as-undetected' }, t('assets.not_detected'))
        : null;
    const keyInput = h('input', { class: 'as-key', type: 'text', value: opts.key ?? sanitize(stem(src.name)), spellcheck: 'false', disabled: Boolean(opts.key), 'aria-label': t('assets.key') });
    const size = probe.width ? `${probe.width} × ${probe.height}` : '';
    const shotFields = h('div', { class: 'as-import-fields' },
      h('label', {}, h('span', {}, t('assets.lang')), langSel),
      h('label', {}, h('span', {}, t('assets.variant')), varSel),
      h('label', {}, h('span', {}, t('assets.key')), keyInput),
    );
    const sync = () => {
      shotFields.hidden = kindSel.value !== 'screenshot';
    };
    kindSel.addEventListener('change', sync);
    sync();
    const node = h(
      'div',
      { class: 'as-import-row' },
      h('div', { class: 'as-import-name' }, h('strong', {}, src.name), size ? h('span', { class: 'muted' }, ` · ${size}`) : null),
      h('div', { class: 'as-import-line' }, kindSel, detection),
      shotFields,
    );
    return { src, node, kindSel, langSel, varSel, keyInput };
  });
  const replace = h('input', { type: 'checkbox', checked: opts.replace ?? false });
  const body = h(
    'div',
    { class: 'as-import' },
    ...rows.map((r) => r.node),
    skipped.length ? h('p', { class: 'muted small' }, `${t('assets.unsupported')}: ${skipped.map((s) => s.name).join(', ')}`) : null,
    h('label', { class: 'as-replace' }, replace, t('assets.replace')),
  );
  const keyOk = () => rows.every((r) => r.kindSel.value !== 'screenshot' || sanitize(r.keyInput.value) === r.keyInput.value);
  const go = await openModal({
    title: opts.title ?? t('assets.import_title'),
    body,
    buttons: [
      { label: t('assets.modal_cancel'), value: false },
      {
        label: t('assets.import'),
        primary: true,
        value: true,
        check: () => {
          if (keyOk()) return true;
          editor.toast(t('assets.key_invalid'), true);
          return false;
        },
      },
    ],
  });
  if (!go) return [];

  const done = [];
  for (const r of rows) {
    const kind = r.kindSel.value;
    const args = { replace: replace.checked };
    if (kind === 'screenshot') {
      args.lang = r.langSel.value;
      args.variant = r.varSel.value;
      args.key = r.keyInput.value;
    }
    try {
      const out = r.src.path
        ? await api.assetsImport([r.src.path], kind, args)
        : await api.assetsUpload(r.src.name, await readBase64(r.src.file), kind, args);
      done.push(...out);
    } catch (e) {
      editor.toast(`${r.src.name}: ${e.message}`, true);
    }
  }
  if (done.length) editor.toast(`${t('assets.imported')}: ${done.map((f) => f.path).join(', ')}`);
  for (const fn of importListeners) fn(done);
  return done;
}

const importListeners = new Set();

/** Calls `fn(importedFiles)` after every import (e.g. to refresh a listing). */
export function onImported(fn) {
  importListeners.add(fn);
  return () => importListeners.delete(fn);
}

/** File picker → import dialog. */
export async function pickAndImport(opts = {}) {
  try {
    const paths = await api.pickFiles({ title: t('assets.import_title'), kind: opts.pickKind, multiple: !opts.key });
    if (!paths.length) return [];
    return importFiles(paths.map((p) => ({ name: baseName(p), path: p })), opts);
  } catch (e) {
    editor.toast(e.message, true);
    return [];
  }
}

// ----- drag & drop ------------------------------------------------------------------

/** Import dialog options for a drop at client point (x, y). */
async function dropOptions(getCanvas, x, y) {
  const canvas = getCanvas?.();
  if (!canvas?.el.contains(document.elementFromPoint(x, y))) return {};
  const el = canvas.elementAt(x, y);
  if (el?.kind !== 'device') return {};
  try {
    const info = await api.elementInfo(viewArgs(), el.id);
    const shot = info.element.props.screenshot;
    if (!shot?.key) {
      editor.toast(t('assets.device_no_key'), true);
      return {};
    }
    return {
      kind: 'screenshot',
      key: shot.key,
      lang: store.get().lang,
      variant: shot.variant ?? currentTarget()?.screenshots ?? '',
      replace: true,
      title: `${t('assets.drop_on_device')} ${el.id}`,
      single: true,
    };
  } catch {
    return {};
  }
}

async function routeDrop(getCanvas, sources, x, y) {
  if (!sources.length || !store.get().summary) return;
  const opts = await dropOptions(getCanvas, x, y);
  await importFiles(opts.single ? sources.slice(0, 1) : sources, opts);
}

function highlight(getCanvas, x, y) {
  const canvas = getCanvas?.();
  let device = null;
  if (canvas && x != null && canvas.el.contains(document.elementFromPoint(x, y))) {
    const el = canvas.elementAt(x, y);
    if (el?.kind === 'device') device = el.id;
  }
  if (store.get().hover !== device) store.set({ hover: device });
  document.body.classList.toggle('as-dragging', x != null);
}

/** Installs the window-wide drop handling. Returns an uninstall function. */
export function installDropImport(getCanvas) {
  const hasFiles = (e) => [...(e.dataTransfer?.types ?? [])].includes('Files');
  const over = (e) => {
    if (!hasFiles(e)) return;
    e.preventDefault();
    e.dataTransfer.dropEffect = 'copy';
    highlight(getCanvas, e.clientX, e.clientY);
  };
  const leave = (e) => {
    if (!e.relatedTarget) highlight(getCanvas, null, null);
  };
  const drop = (e) => {
    if (!hasFiles(e)) return;
    e.preventDefault();
    highlight(getCanvas, null, null);
    const sources = [...e.dataTransfer.files].map((file) => ({ name: file.name, file }));
    routeDrop(getCanvas, sources, e.clientX, e.clientY).catch((err) => editor.toast(err.message, true));
  };
  window.addEventListener('dragover', over);
  window.addEventListener('dragleave', leave);
  window.addEventListener('drop', drop);
  const unlisten = [];
  if (isTauri) {
    const dpr = () => window.devicePixelRatio || 1;
    const pos = (p) => ({ x: (p?.position?.x ?? 0) / dpr(), y: (p?.position?.y ?? 0) / dpr() });
    listen('tauri://drag-over', (p) => {
      const { x, y } = pos(p);
      highlight(getCanvas, x, y);
    }).then((u) => unlisten.push(u));
    listen('tauri://drag-leave', () => highlight(getCanvas, null, null)).then((u) => unlisten.push(u));
    listen('tauri://drag-drop', (p) => {
      highlight(getCanvas, null, null);
      const { x, y } = pos(p);
      const sources = (p?.paths ?? []).map((path) => ({ name: baseName(path), path }));
      routeDrop(getCanvas, sources, x, y).catch((err) => editor.toast(err.message, true));
    }).then((u) => unlisten.push(u));
  }
  return () => {
    window.removeEventListener('dragover', over);
    window.removeEventListener('dragleave', leave);
    window.removeEventListener('drop', drop);
    unlisten.forEach((u) => u());
  };
}
