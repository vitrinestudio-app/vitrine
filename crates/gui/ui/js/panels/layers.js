// Layers panel (left sidebar, task 2.6): the current canvas's elements as a
// tree, topmost first. Groups expand/collapse; click selects (shift/cmd
// adds), double-click renames, the eye toggles `visible` (an override in a
// variant view), drag & drop reorders or moves into/out of groups
// (`edit_arrange`, one undo step, positions kept). Rows show the home page
// and, in a variant view, a marker for overridden elements.

import { h, icon } from '../dom.js';
import { currentCanvas, effectiveVariant } from '../store.js';

const KIND_ICON = { device: 'device', text: 'text', image: 'image', shape: 'shape', group: 'group', background: 'background' };
const collapsed = new Set();

/** @param {HTMLElement} el  @param {import('./index.js').PanelContext} ctx */
export function mount(el, { store, editor, api, t, viewArgs }) {
  const list = h('ul', { class: 'layers', role: 'tree', 'aria-label': t('panel.layers') });
  const empty = h('p', { class: 'placeholder' }, t('layers.empty'));
  el.append(h('div', { class: 'section-title' }, t('panel.layers')), list, empty);
  let anchor = null;
  let drag = null;

  /** `{ id → { e, children: [ids] } }` and the top-level ids, painting order. */
  function tree(canvas) {
    const nodes = new Map();
    const top = [];
    for (const e of canvas.elements) nodes.set(e.id, { e, children: [] });
    for (const e of canvas.elements) {
      if (e.parent && nodes.has(e.parent)) nodes.get(e.parent).children.push(e.id);
      else top.push(e.id);
    }
    return { nodes, top };
  }

  /** Ids in display order (topmost first), honouring collapsed groups. */
  function visibleOrder(t0) {
    const out = [];
    const walk = (ids, depth) => {
      for (const id of [...ids].reverse()) {
        out.push({ id, depth });
        const n = t0.nodes.get(id);
        if (n.children.length && !collapsed.has(id)) walk(n.children, depth + 1);
      }
    };
    walk(t0.top, 0);
    return out;
  }

  function drawnIds(o) {
    const s = new Set();
    for (const e of o?.elements ?? []) {
      s.add(e.id);
      for (const g of e.groups ?? []) s.add(g);
    }
    return s;
  }

  function overriddenOf(o, id) {
    return o?.elements.find((e) => e.id === id)?.overridden ?? o?.groups.find((g) => g.id === id)?.overridden ?? [];
  }

  function select(id, ev, order) {
    const sel = store.get().selection;
    if (ev.shiftKey && anchor && order.some((x) => x.id === anchor)) {
      const a = order.findIndex((x) => x.id === anchor);
      const b = order.findIndex((x) => x.id === id);
      const range = order.slice(Math.min(a, b), Math.max(a, b) + 1).map((x) => x.id);
      store.set({ selection: [...new Set([...sel, ...range])] });
      return;
    }
    if (ev.metaKey || ev.ctrlKey) {
      store.set({ selection: sel.includes(id) ? sel.filter((x) => x !== id) : [...sel, id] });
    } else store.set({ selection: [id] });
    anchor = id;
  }

  function rename(li, id) {
    const label = li.querySelector('.layer-name');
    const input = h('input', { class: 'layer-rename mono', type: 'text', value: id, spellcheck: 'false' });
    label.replaceWith(input);
    input.focus();
    input.select();
    let done = false;
    const finish = (ok) => {
      if (done) return;
      done = true;
      const v = input.value.trim();
      if (ok && v && v !== id) editor.apply(api.rename(id, v), { touched: [id], select: [v] });
      else render();
    };
    input.addEventListener('keydown', (e) => {
      e.stopPropagation();
      if (e.key === 'Enter') finish(true);
      if (e.key === 'Escape') finish(false);
    });
    input.addEventListener('blur', () => finish(true));
  }

  /** Drop position under the pointer: before/after a row or into a group. */
  function dropSpot(li, ev, node) {
    const r = li.getBoundingClientRect();
    const y = (ev.clientY - r.top) / r.height;
    if (node.e.kind === 'group' && y > 0.3 && y < 0.7) return 'into';
    return y < 0.5 ? 'above' : 'below';
  }

  function onDrop(t0, targetId, spot) {
    const id = drag;
    drag = null;
    if (!id || id === targetId) return render();
    // Not into itself or a descendant.
    for (let p = targetId; p; p = t0.nodes.get(p)?.e.parent) if (p === id) return render();
    const target = t0.nodes.get(targetId).e;
    let parent;
    let index;
    if (spot === 'into') {
      parent = targetId;
      index = null;
    } else {
      parent = target.parent ?? null;
      const siblings = (parent ? t0.nodes.get(parent).children : t0.top).filter((x) => x !== id);
      const i = siblings.indexOf(targetId);
      // Display is topmost first: "above" = later in painting order.
      index = spot === 'above' ? i + 1 : i;
    }
    editor.apply(api.arrange(viewArgs(), id, parent, index), { touched: [id] });
  }

  function render() {
    const st = store.get();
    const canvas = currentCanvas(st);
    if (!canvas) {
      list.replaceChildren();
      empty.hidden = false;
      return;
    }
    const t0 = tree(canvas);
    const order = visibleOrder(t0);
    empty.hidden = order.length > 0;
    const drawn = drawnIds(st.overlay);
    const variant = effectiveVariant(st);
    const sel = new Set(st.selection);
    list.replaceChildren(
      ...order.map(({ id, depth }) => {
        const n = t0.nodes.get(id);
        const e = n.e;
        const isGroup = e.kind === 'group';
        const shown = drawn.has(id);
        const over = variant ? overriddenOf(st.overlay, id) : [];
        const li = h('li', {
          class: `layer${shown ? '' : ' hidden-layer'}${over.length ? ' overridden' : ''}`,
          role: 'treeitem',
          tabindex: '0',
          draggable: 'true',
          'aria-selected': String(sel.has(id)),
          'aria-expanded': isGroup ? String(!collapsed.has(id)) : null,
          dataset: { id },
          title: `${id} · ${e.label}`,
        });
        li.style.setProperty('--depth', depth);
        const twisty = isGroup
          ? h(
              'button',
              {
                type: 'button',
                class: `layer-twisty${collapsed.has(id) ? ' closed' : ''}`,
                title: t('layers.expand'),
                onclick: (ev) => {
                  ev.stopPropagation();
                  if (collapsed.has(id)) collapsed.delete(id);
                  else collapsed.add(id);
                  render();
                },
              },
              icon('chevron'),
            )
          : h('span', { class: 'layer-twisty' });
        const eye = h(
          'button',
          {
            type: 'button',
            class: 'layer-eye',
            title: shown ? t('layers.hide') : t('layers.show'),
            onclick: (ev) => {
              ev.stopPropagation();
              editor.apply(api.elementPatch(viewArgs(), id, { visible: shown ? false : null }), { touched: [id] });
            },
          },
          icon(shown ? 'eye' : 'eye-off'),
        );
        const parts = [
          twisty,
          h('span', { class: 'layer-icon' }, icon(KIND_ICON[e.kind] ?? 'shape')),
          h('span', { class: 'layer-name' }, h('span', { class: 'layer-id' }, id), e.kind === 'text' || e.kind === 'device' || e.kind === 'image' ? h('span', { class: 'layer-label' }, e.label) : null),
          over.length ? h('span', { class: 'layer-over', title: `${t('inspector.overrides')}: ${over.join(', ')}` }) : null,
          isGroup ? null : h('span', { class: 'layer-page', title: `${t('inspector.page')} ${e.page + 1} · ${canvas.page.ids[e.page] ?? ''}` }, String(e.page + 1)),
          eye,
        ];
        li.append(...parts.filter(Boolean));
        li.addEventListener('click', (ev) => select(id, ev, order));
        li.addEventListener('dblclick', (ev) => {
          ev.preventDefault();
          rename(li, id);
        });
        li.addEventListener('keydown', (ev) => {
          if (ev.target !== li) return;
          if (ev.key === 'Enter' || ev.key === 'F2') {
            ev.preventDefault();
            rename(li, id);
          } else if (ev.key === 'ArrowDown' || ev.key === 'ArrowUp') {
            ev.preventDefault();
            ev.stopPropagation();
            const i = order.findIndex((x) => x.id === id) + (ev.key === 'ArrowDown' ? 1 : -1);
            const next = order[i];
            if (next) {
              select(next.id, ev, order);
              list.querySelector(`[data-id="${CSS.escape(next.id)}"]`)?.focus();
            }
          }
        });
        li.addEventListener('dragstart', (ev) => {
          drag = id;
          ev.dataTransfer.effectAllowed = 'move';
          ev.dataTransfer.setData('text/plain', id);
          li.classList.add('dragging');
        });
        li.addEventListener('dragend', () => {
          drag = null;
          li.classList.remove('dragging');
          for (const x of list.querySelectorAll('.drop-above,.drop-below,.drop-into')) x.classList.remove('drop-above', 'drop-below', 'drop-into');
        });
        li.addEventListener('dragover', (ev) => {
          if (!drag || drag === id) return;
          ev.preventDefault();
          const spot = dropSpot(li, ev, n);
          li.classList.toggle('drop-above', spot === 'above');
          li.classList.toggle('drop-below', spot === 'below');
          li.classList.toggle('drop-into', spot === 'into');
        });
        li.addEventListener('dragleave', () => li.classList.remove('drop-above', 'drop-below', 'drop-into'));
        li.addEventListener('drop', (ev) => {
          ev.preventDefault();
          const spot = dropSpot(li, ev, n);
          li.classList.remove('drop-above', 'drop-below', 'drop-into');
          onDrop(t0, id, spot);
        });
        return li;
      }),
    );
    // Keep the selection in view.
    const first = list.querySelector('[aria-selected="true"]');
    if (first && !list.contains(document.activeElement)) first.scrollIntoView({ block: 'nearest' });
  }

  const unsub = store.select((s) => [s.summary, s.canvas, s.selection, s.overlay, s.variant], render);
  render();
  return { unmount: () => unsub() };
}
