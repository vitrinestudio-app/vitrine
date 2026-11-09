// Inspector panel (right sidebar). Minimal until task 2.6: shows the
// selection (id, kind, home page, size, rotation) and, in a variant view,
// the overridden fields with reset buttons.

import { h, icon } from '../dom.js';
import { effectiveVariant } from '../store.js';

const fmt = (v) => (Math.round(v * 10) / 10).toLocaleString();

/** @param {HTMLElement} el  @param {import('./index.js').PanelContext} ctx */
export function mount(el, { store, editor, api, t, viewArgs }) {
  const header = h('div', { class: 'panel-header' }, t('panel.inspector'));
  const body = h('div', { class: 'panel-body' });
  el.append(header, body);

  function row(label, value) {
    return [h('dt', {}, label), h('dd', { title: String(value) }, value)];
  }

  function render() {
    const st = store.get();
    const o = st.overlay;
    const sel = st.selection;
    if (!o || !sel.length) {
      body.replaceChildren(h('p', { class: 'placeholder' }, t('inspector.none')));
      return;
    }
    if (sel.length > 1) {
      body.replaceChildren(
        h('div', { class: 'section-title' }, `${sel.length} ${t('inspector.selected')}`),
        h('ul', { class: 'list' }, ...sel.map((id) => h('li', { class: 'list-item' }, h('span', { class: 'grow mono small' }, id)))),
      );
      return;
    }
    const id = sel[0];
    const e = o.elements.find((x) => x.id === id);
    const g = o.groups.find((x) => x.id === id);
    const page = o.pages.find((p) => p.index === (e?.page ?? -1));
    const props = h('dl', { class: 'props' });
    props.append(...row(t('inspector.id'), id));
    props.append(...row(t('inspector.kind'), e ? t(`kind.${e.kind}`) : t('kind.group')));
    if (e) {
      props.append(...row(t('inspector.page'), page ? `${page.index + 1} · ${page.id}` : e.page + 1));
      props.append(...row(t('inspector.size'), `${fmt(e.rect.w)} × ${fmt(e.rect.h)} lu`));
      props.append(...row(t('inspector.center'), `${fmt(e.move_point.x)}, ${fmt(e.move_point.y)}`));
      props.append(...row(t('inspector.rotation'), `${fmt(e.rotation)}°`));
      if (e.text) {
        props.append(...row(t('inspector.lines'), e.text.lines));
        props.append(...row(t('inspector.font_size'), `${fmt(e.text.font_size)} lu`));
        if (e.text.overflow) props.append(...row(t('inspector.status'), t('inspector.overflow')));
        else if (e.text.emergency_breaks?.length) props.append(...row(t('inspector.status'), t('inspector.emergency')));
      }
      if (e.in_gap) props.append(...row(t('inspector.status'), t('inspector.in_gap')));
    } else if (g) {
      props.append(...row(t('inspector.members'), g.members.length));
      props.append(...row(t('inspector.size'), `${fmt(g.bounds.w)} × ${fmt(g.bounds.h)} lu`));
    }
    const parts = [h('div', { class: 'section-title' }, t('inspector.element')), props];

    const variant = effectiveVariant(st);
    const overridden = (e ?? g)?.overridden ?? [];
    if (variant) {
      const reset = (field) =>
        editor.apply(api.resetOverride(viewArgs(), id, field), { touched: [id] });
      parts.push(h('div', { class: 'section-title' }, `${t('inspector.overrides')} · ${variant}`));
      if (overridden.length) {
        parts.push(
          h(
            'div',
            { class: 'props-chips' },
            ...overridden.map((f) =>
              h('span', { class: 'chip' }, f, h('button', { type: 'button', title: t('inspector.reset_field'), onclick: () => reset(f) }, icon('close'))),
            ),
            h('button', { class: 'btn btn-small', type: 'button', onclick: () => reset(undefined) }, icon('reset'), t('inspector.reset_all')),
          ),
        );
      } else parts.push(h('p', { class: 'placeholder' }, t('inspector.no_overrides')));
    }
    body.replaceChildren(...parts);
  }

  const unsub = store.select((s) => [s.selection, s.overlay, s.variant], render);
  render();
  return { unmount: () => unsub() };
}
