// Drag a screenshot from the assets panel onto a device on the canvas: the
// device's `screenshot` key is set to the screenshot's key (one undo step,
// through `element_patch` like the inspector field, so the view's variant
// scope applies the same way).
//
// Screenshots are referenced by key and resolved per language and variant,
// so the file a row stands for only gives its key: a file from another
// language or variant folder sets the same key, and every language shows
// its own file of it (or the fallback).
//
// Pointer events, not HTML5 drag & drop: the Tauri window keeps native
// file drops on (`tauri://drag-drop`, importer.js), and in-page HTML5
// drag & drop does not work alongside it on every platform (WebView2).
// Pointer capture also keeps the canvas from treating the drag as a hover.
//
// Images (assets/) are draggable too, but a device shows screenshots only
// (no image field in the model): dropping one on a device explains that.

import { api } from '../api.js';
import { h, icon } from '../dom.js';
import { editor } from '../editor.js';
import { t } from '../i18n.js';
import { currentTarget, store, viewArgs } from '../store.js';
import { deviceAtPoint, notifyImported } from './importer.js';

const THRESHOLD = 4;

/**
 * Makes `node` a drag source. `item`: `{ kind: 'screenshot', key, lang?,
 * variant? }` or `{ kind: 'image', path }`. `thumb()` returns the image
 * element to show in the drag ghost (optional).
 */
export function makeDraggable(node, item, thumb) {
  node.classList.add('drag-source');
  node.addEventListener('pointerdown', (e) => {
    if (e.button !== 0 || e.target.closest('.as-remove, .as-user, .btn')) return;
    start(node, item, thumb, e);
  });
}

function start(node, item, thumb, e0) {
  let ghost = null;
  let device = null;
  const pointerId = e0.pointerId;

  const label = () => {
    if (item.kind === 'image') return device ? t('assets.drag.image_refused') : item.path.replace(/^assets\//, '');
    if (!device) return item.key;
    return t('assets.drag.onto').replace('{key}', item.key).replace('{id}', device.id);
  };

  function begin() {
    try {
      node.setPointerCapture(pointerId);
    } catch {
      /* already released */
    }
    const img = thumb?.();
    ghost = h(
      'div',
      { class: 'drag-ghost', role: 'status' },
      img?.src ? h('img', { src: img.src, alt: '', draggable: 'false' }) : h('span', { class: 'drag-ghost-icon' }, icon(item.kind === 'image' ? 'image' : 'device')),
      h('span', { class: 'drag-ghost-label' }),
    );
    document.body.append(ghost);
    document.body.classList.add('shot-dragging');
  }

  function update(x, y) {
    const d = deviceAtPoint(x, y);
    device = d;
    const id = d && item.kind === 'screenshot' ? d.id : null;
    if (store.get().dropTarget !== id) store.set({ dropTarget: id });
    ghost.classList.toggle('refused', Boolean(d) && item.kind === 'image');
    ghost.classList.toggle('over', Boolean(id));
    ghost.lastChild.textContent = label();
    ghost.style.left = `${x + 14}px`;
    ghost.style.top = `${y + 14}px`;
  }

  function end() {
    window.removeEventListener('pointermove', move, true);
    window.removeEventListener('pointerup', up, true);
    window.removeEventListener('pointercancel', cancel, true);
    window.removeEventListener('keydown', key, true);
    ghost?.remove();
    document.body.classList.remove('shot-dragging');
    if (store.get().dropTarget) store.set({ dropTarget: null });
  }

  function move(e) {
    if (e.pointerId !== pointerId) return;
    if (!ghost) {
      if (Math.hypot(e.clientX - e0.clientX, e.clientY - e0.clientY) < THRESHOLD) return;
      begin();
    }
    e.preventDefault();
    update(e.clientX, e.clientY);
  }

  function up(e) {
    if (e.pointerId !== pointerId) return;
    const dragged = Boolean(ghost);
    if (dragged) update(e.clientX, e.clientY);
    const target = device;
    end();
    if (!dragged) return;
    // The release must not also click the thumbnail (image preview).
    window.addEventListener('click', swallow, { capture: true, once: true });
    setTimeout(() => window.removeEventListener('click', swallow, true), 0);
    if (target) drop(item, target).catch((err) => editor.toast(err.message, true));
  }

  function cancel() {
    end();
  }

  function key(e) {
    if (e.key === 'Escape' && ghost) {
      e.preventDefault();
      e.stopPropagation();
      device = null;
      end();
    }
  }

  window.addEventListener('pointermove', move, true);
  window.addEventListener('pointerup', up, true);
  window.addEventListener('pointercancel', cancel, true);
  window.addEventListener('keydown', key, true);
}

function swallow(e) {
  e.preventDefault();
  e.stopPropagation();
}

async function drop(item, device) {
  if (item.kind === 'image') {
    editor.toast(t('assets.drag.image_hint'), true);
    return;
  }
  const ids = [device.id];
  const res = await editor.apply(api.elementPatch(viewArgs(), ids, { screenshot: item.key }), { touched: ids, select: ids });
  if (!res) return; // error toast shown by apply
  if (!res.applied) {
    editor.toast(t('assets.drag.same').replace('{id}', device.id).replace('{key}', item.key));
    return;
  }
  // The assets panel lists which devices use each key.
  notifyImported([]);
  const st = store.get();
  const variant = currentTarget(st)?.screenshots ?? null;
  // A file from another language / variant folder: say that the key counts.
  const elsewhere = (item.lang && item.lang !== st.lang) || (item.variant && variant && item.variant !== variant);
  const msg = t('assets.drag.done').replace('{id}', device.id).replace('{key}', item.key);
  editor.toast(elsewhere ? `${msg} ${t('assets.drag.by_key').replace('{key}', item.key)}` : msg);
}
