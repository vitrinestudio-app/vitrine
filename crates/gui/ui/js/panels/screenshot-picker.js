// Screenshot key field of the device inspector: a text field (any key,
// also a new one) with a thumbnail of the screenshot the view shows for
// that key and a ▾ button listing the keys to pick from.
//
// Choices come from `assets_list` (no extra command): the keys with a file
// for the view's language (or a language of its fallback chain) and the
// target's screenshot variant (or the language folder for all variants),
// then the other keys – files of other variants only, or keys devices use
// without any file. Picking commits like the text field (`onCommit`), so
// undo and the variant scope are those of every inspector field.

import { api } from '../api.js';
import { h, icon } from '../dom.js';
import { t } from '../i18n.js';
import { openContextMenu } from '../context-menu.js';
import { keyLabel } from '../shortcuts.js';
import { textInput } from './fields.js';

const THUMB = 48;
const thumbCache = new Map(); // `path|size` → Promise<object URL | null>

function thumbUrl(path, stamp) {
  const k = `${path}|${stamp}`;
  if (!thumbCache.has(k)) {
    thumbCache.set(
      k,
      api
        .assetThumbnail(path, Math.round(THUMB * (window.devicePixelRatio || 1)))
        .then((p) => URL.createObjectURL(new Blob([p.data], { type: 'image/png' })))
        .catch(() => null),
    );
  }
  return thumbCache.get(k);
}

function thumb(file, cls) {
  const img = h('img', { class: cls, alt: '', draggable: 'false' });
  if (file) thumbUrl(file.path, `${file.width}x${file.height}`).then((u) => (u ? (img.src = u) : img.classList.add('broken')));
  else img.classList.add('empty');
  return img;
}

/**
 * The file `key` shows in `lang` for screenshot variant `variant`: the
 * first language of the fallback chain with a file in the variant folder
 * or in the language folder (all variants). `{ file, lang }` or null.
 */
export function resolveShot(listing, key, lang, variant) {
  const chain = listing?.screenshot_langs?.[lang]?.chain ?? [lang];
  for (const l of chain) {
    const files = listing.screenshots.filter((f) => f.lang === l && f.key === key);
    const file = (variant && files.find((f) => f.variant === variant)) || files.find((f) => f.variant == null);
    if (file) return { file, lang: l };
  }
  return null;
}

/**
 * `{ key, value, listing, lang, variant, onCommit }` → element. `listing`
 * is the `assets_list` result (may be null while loading).
 */
export function screenshotPicker({ key, value, listing, lang, variant, onCommit }) {
  const field = textInput({ key, value: value ?? '', mono: true, placeholder: t('inspector.screenshot.none'), title: t('inspector.screenshot'), onCommit });
  const current = value ? resolveShot(listing, value, lang, variant) : null;
  const preview = thumb(current?.file, 'shot-pick-thumb');
  preview.title = current ? current.file.path : value ? t('inspector.screenshot.no_file') : '';
  const button = h('button', { type: 'button', class: 'icon-btn shot-pick-btn', title: `${t('inspector.screenshot.pick')} (${keyLabel('alt+down')})`, 'aria-label': t('inspector.screenshot.pick'), 'aria-haspopup': 'menu' }, icon('chevron'));
  button.addEventListener('click', () => open());
  const input = field.matches?.('input') ? field : field.querySelector('input');
  input?.addEventListener('keydown', (e) => {
    if (e.key === 'ArrowDown' && e.altKey) {
      e.preventDefault();
      open();
    }
  });

  function open() {
    const all = [...new Set([...(listing?.screenshots ?? []).map((f) => f.key), ...Object.keys(listing?.screenshot_keys ?? {})])].sort();
    const here = [];
    const other = [];
    for (const k of all) {
      const r = resolveShot(listing, k, lang, variant);
      (r ? here : other).push({ k, r });
    }
    const users = (k) => listing?.screenshot_keys?.[k] ?? [];
    const row = ({ k, r }) => ({
      label: h(
        'span',
        { class: 'shot-pick-row' },
        thumb(r?.file, 'shot-pick-thumb'),
        h(
          'span',
          { class: 'shot-pick-text' },
          h('span', { class: 'mono' }, k),
          h(
            'span',
            { class: 'muted small' },
            [r && r.lang !== lang ? t('inspector.screenshot.from').replace('{lang}', r.lang) : null, r ? null : t('inspector.screenshot.no_file'), users(k).length ? users(k).join(', ') : t('assets.unused')].filter(Boolean).join(' · '),
          ),
        ),
      ),
      title: r?.file.path ?? null,
      current: k === value,
      action: () => k !== value && onCommit(k),
    });
    const where = variant ? `${lang} · ${variant}` : lang;
    const r = button.getBoundingClientRect();
    openContextMenu({
      x: r.right - 280,
      y: r.bottom + 4,
      cls: 'shot-menu',
      label: t('inspector.screenshot.pick'),
      items: [
        { heading: t('inspector.screenshot.available').replace('{where}', where) },
        ...(here.length ? here.map(row) : [{ label: t('inspector.screenshot.none_here'), disabled: true }]),
        other.length ? { heading: t('inspector.screenshot.other') } : null,
        ...other.map(row),
        '-',
        { label: t('inspector.screenshot.clear'), icon: 'close', disabled: !value, action: () => onCommit('') },
        { heading: t('inspector.screenshot.type_hint') },
      ],
    });
  }

  return h('span', { class: 'shot-pick' }, preview, field, button);
}
