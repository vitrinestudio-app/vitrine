// Form controls of the inspector and the strip settings (task 2.6):
// numeric inputs with scrub-drag labels, colour inputs with project and
// catalog swatches, selects, segmented controls, toggles, text inputs and
// the fill editor (colour, gradients with stops, image).
//
// Every control commits through its `onCommit`/`onChange` callback (one
// undo step); typing only changes the input. Inputs carry a `data-key`
// so the inspector can keep focus and unsaved typing across re-renders.

import { h, icon } from '../dom.js';

/** Rounds for display: at most `precision` decimals, no trailing zeros. */
export function fmtNum(v, precision = 2) {
  if (v == null || !Number.isFinite(v)) return '';
  const f = 10 ** precision;
  return String(Math.round(v * f) / f);
}

const clamp = (v, min, max) => Math.min(max ?? Infinity, Math.max(min ?? -Infinity, v));

/**
 * Numeric input with a scrub-drag label: drag the label (shift = ×10),
 * arrow keys ±step (shift ±big), Enter/blur commits, Escape reverts.
 *
 * opts: `{ key, label, title, value, step=1, big=10, min, max, precision=2,
 * scale=1 (display = value × scale), optional (empty = null), placeholder,
 * unit, onCommit(value|null) }`.
 */
export function numberInput(opts) {
  const { step = 1, big = 10, precision = 2, scale = 1 } = opts;
  let committed = opts.value ?? null;
  const input = h('input', {
    class: 'num-input',
    type: 'text',
    inputmode: 'decimal',
    spellcheck: 'false',
    autocomplete: 'off',
    placeholder: opts.placeholder ?? '',
    title: opts.title ?? opts.label ?? '',
    'aria-label': opts.title ?? opts.label ?? '',
    dataset: { key: opts.key ?? '' },
  });
  const show = (v) => {
    input.value = v == null ? '' : fmtNum(v * scale, precision);
    delete input.dataset.dirty;
  };
  show(committed);

  /** The typed value in model units; `undefined` = not a number. */
  const parse = () => {
    const s = input.value.trim().replace(',', '.');
    if (s === '') return null;
    const n = Number(s);
    return Number.isFinite(n) ? n / scale : undefined;
  };
  const tidy = (v) => {
    const f = 10 ** (precision + 2);
    return clamp(Math.round(v * f) / f, opts.min, opts.max);
  };
  const commit = (v) => {
    clearTimeout(timer);
    if (v === undefined || (v == null && !opts.optional)) {
      show(committed);
      return;
    }
    if (v != null) v = tidy(v);
    show(v);
    if (v === committed) return;
    committed = v;
    opts.onCommit?.(v);
  };

  // Arrow keys: change at once, commit after a short pause (one undo step
  // per burst of key presses).
  let timer = 0;
  const bump = (delta) => {
    const base = parse() ?? committed ?? 0;
    const v = tidy(base + delta / scale);
    input.value = fmtNum(v * scale, precision);
    input.dataset.dirty = '1';
    clearTimeout(timer);
    timer = setTimeout(() => commit(v), 450);
  };

  input.addEventListener('input', () => {
    input.dataset.dirty = '1';
  });
  input.addEventListener('keydown', (e) => {
    if (e.key === 'Enter') {
      e.preventDefault();
      commit(parse());
      input.select();
    } else if (e.key === 'Escape') {
      e.preventDefault();
      e.stopPropagation();
      clearTimeout(timer);
      show(committed);
      input.blur();
    } else if (e.key === 'ArrowUp' || e.key === 'ArrowDown') {
      e.preventDefault();
      bump((e.key === 'ArrowUp' ? 1 : -1) * (e.shiftKey ? big : step) * scale);
    }
  });
  input.addEventListener('blur', () => {
    if (input.dataset.dirty) commit(parse());
  });
  input.addEventListener('focus', () => input.select());

  const label = opts.label
    ? h('span', { class: 'num-scrub', title: opts.title ?? opts.label }, opts.label)
    : null;
  if (label) {
    label.addEventListener('pointerdown', (e) => {
      e.preventDefault();
      label.setPointerCapture(e.pointerId);
      const x0 = e.clientX;
      const v0 = parse() ?? committed ?? 0;
      let v = v0;
      document.body.classList.add('scrubbing');
      const move = (ev) => {
        const steps = Math.round((ev.clientX - x0) / 3);
        v = tidy(v0 + (steps * (ev.shiftKey ? big : step)) / 1);
        input.value = fmtNum(v * scale, precision);
        input.dataset.dirty = '1';
      };
      const up = () => {
        label.removeEventListener('pointermove', move);
        label.removeEventListener('pointerup', up);
        label.removeEventListener('pointercancel', up);
        document.body.classList.remove('scrubbing');
        if (v !== v0) commit(v);
      };
      label.addEventListener('pointermove', move);
      label.addEventListener('pointerup', up);
      label.addEventListener('pointercancel', up);
    });
  }
  const el = h('label', { class: 'num' }, label, input, opts.unit ? h('span', { class: 'num-unit' }, opts.unit) : null);
  return {
    el,
    input,
    set(v) {
      committed = v ?? null;
      show(committed);
    },
  };
}

/** Native select. `options`: `[{ value, label, group? }]`. */
export function selectInput({ key, options, value, onChange, title }) {
  const groups = new Map();
  const nodes = [];
  for (const o of options) {
    const opt = h('option', { value: o.value }, o.label);
    if (o.group) {
      if (!groups.has(o.group)) {
        const g = h('optgroup', { label: o.group });
        groups.set(o.group, g);
        nodes.push(g);
      }
      groups.get(o.group).append(opt);
    } else nodes.push(opt);
  }
  const select = h('select', { class: 'insp-select', title: title ?? '', 'aria-label': title ?? '', dataset: { key: key ?? '' } }, ...nodes);
  select.value = value ?? '';
  select.addEventListener('change', () => onChange(select.value));
  return select;
}

/** Segmented control. `options`: `[{ value, label?, icon?, title? }]`. */
export function segmentedInput({ options, value, onChange, title }) {
  return h(
    'div',
    { class: 'insp-seg', role: 'group', 'aria-label': title ?? '' },
    ...options.map((o) =>
      h(
        'button',
        {
          type: 'button',
          title: o.title ?? o.label ?? '',
          'aria-pressed': String(o.value === value),
          onclick: () => o.value !== value && onChange(o.value),
        },
        o.icon ? icon(o.icon) : null,
        o.label && !o.icon ? o.label : null,
      ),
    ),
  );
}

/** Checkbox with a label. */
export function checkInput({ key, checked, label, onChange, title }) {
  const box = h('input', { type: 'checkbox', checked: !!checked, dataset: { key: key ?? '' } });
  box.addEventListener('change', () => onChange(box.checked));
  return h('label', { class: 'insp-check', title: title ?? '' }, box, label ? h('span', {}, label) : null);
}

let listSeq = 0;

/**
 * Text input committing on Enter/blur (Escape reverts). `list`: values
 * offered in a datalist; `validate(v)` → error message or null.
 */
export function textInput({ key, value, placeholder, list, onCommit, mono, title, validate }) {
  let committed = value ?? '';
  const input = h('input', {
    class: `insp-text${mono ? ' mono' : ''}`,
    type: 'text',
    spellcheck: 'false',
    autocomplete: 'off',
    placeholder: placeholder ?? '',
    title: title ?? '',
    'aria-label': title ?? placeholder ?? '',
    dataset: { key: key ?? '' },
  });
  input.value = committed;
  let datalist = null;
  if (list?.length) {
    const id = `insp-list-${++listSeq}`;
    datalist = h('datalist', { id }, ...list.map((v) => h('option', { value: v })));
    input.setAttribute('list', id);
  }
  const commit = () => {
    const v = input.value.trim();
    delete input.dataset.dirty;
    if (v === committed) return;
    const err = validate?.(v);
    if (err) {
      input.classList.add('invalid');
      input.title = err;
      return;
    }
    input.classList.remove('invalid');
    committed = v;
    onCommit(v);
  };
  input.addEventListener('input', () => {
    input.dataset.dirty = '1';
    input.classList.remove('invalid');
  });
  input.addEventListener('keydown', (e) => {
    if (e.key === 'Enter') {
      e.preventDefault();
      commit();
    } else if (e.key === 'Escape') {
      e.preventDefault();
      e.stopPropagation();
      input.value = committed;
      delete input.dataset.dirty;
      input.classList.remove('invalid');
      input.blur();
    }
  });
  input.addEventListener('blur', () => input.dataset.dirty && commit());
  // Picking from the datalist fires `change` without Enter.
  input.addEventListener('change', () => input.dataset.dirty && commit());
  return datalist ? h('span', { class: 'insp-text-wrap' }, input, datalist) : input;
}

// ----- colours ------------------------------------------------------------------

/** `#RGB`/`#RRGGBB`/`#RRGGBBAA`, `@name` or a catalog colour name → CSS colour (or null). */
export function cssColor(value, palette = {}, swatches = []) {
  if (!value) return null;
  if (value.startsWith('@')) return palette[value.slice(1)] ?? null;
  if (/^#([0-9a-f]{3}|[0-9a-f]{6}|[0-9a-f]{8})$/i.test(value)) return value;
  return swatches.find((s) => s.name === value)?.hex ?? null;
}

const HEX = /^#([0-9a-f]{3}|[0-9a-f]{6}|[0-9a-f]{8})$/i;

/** Normalises `#abc` → `#AABBCC` (upper-case like the project files). */
export function normHex(v) {
  let s = v.trim();
  if (/^[0-9a-f]{6}$/i.test(s)) s = `#${s}`;
  if (/^#[0-9a-f]{3}$/i.test(s)) s = `#${[...s.slice(1)].map((c) => c + c).join('')}`;
  return s.toUpperCase();
}

let openPopover = null;
function closePopover() {
  openPopover?.remove();
  openPopover = null;
}
document.addEventListener('pointerdown', (e) => {
  if (openPopover && !openPopover.contains(e.target) && !e.target.closest?.('.color-swatch')) closePopover();
});
document.addEventListener('keydown', (e) => {
  if (e.key === 'Escape' && openPopover) {
    e.stopPropagation();
    closePopover();
  }
}, true);

/**
 * Colour input: a swatch (opens a popover with the project palette as
 * `@name` references, extra `swatches` and a custom colour picker) and a
 * text field (`#RRGGBB[AA]`, `@name` or a swatch name).
 *
 * opts: `{ key, value, palette: {name: hex}, swatches: [{name,label,hex}],
 * optional (empty = null), placeholder, onCommit(value|null), t }`.
 */
export function colorInput(opts) {
  const { palette = {}, swatches = [], t } = opts;
  let value = opts.value ?? '';
  const chip = h('button', { type: 'button', class: 'color-swatch', title: t?.('inspector.f.color.pick') ?? '' });
  const paint = () => {
    const c = cssColor(value, palette, swatches);
    chip.classList.toggle('empty', !c);
    chip.style.setProperty('--swatch', c ?? 'transparent');
  };
  const commit = (v) => {
    closePopover();
    value = v ?? '';
    paint();
    text.value = value;
    opts.onCommit(v);
  };
  const validate = (v) => {
    if (v === '') return opts.optional ? null : (t?.('inspector.f.color.invalid') ?? 'invalid');
    if (v.startsWith('@')) return palette[v.slice(1)] ? null : (t?.('inspector.f.color.unknown') ?? 'unknown');
    if (HEX.test(normHex(v))) return null;
    if (swatches.some((s) => s.name === v)) return null;
    return t?.('inspector.f.color.invalid') ?? 'invalid';
  };
  const text = h('input', {
    class: 'insp-text mono color-text',
    type: 'text',
    spellcheck: 'false',
    autocomplete: 'off',
    placeholder: opts.placeholder ?? '',
    dataset: { key: opts.key ?? '' },
    'aria-label': t?.('inspector.f.color') ?? 'colour',
  });
  text.value = value;
  const commitText = () => {
    delete text.dataset.dirty;
    let v = text.value.trim();
    if (!v.startsWith('@') && !swatches.some((s) => s.name === v) && v) v = normHex(v);
    if (v === value) return;
    const err = validate(v);
    text.classList.toggle('invalid', !!err);
    if (err) {
      text.title = err;
      return;
    }
    commit(v === '' ? null : v);
  };
  text.addEventListener('input', () => {
    text.dataset.dirty = '1';
  });
  text.addEventListener('keydown', (e) => {
    if (e.key === 'Enter') {
      e.preventDefault();
      commitText();
    } else if (e.key === 'Escape') {
      text.value = value;
      delete text.dataset.dirty;
      text.blur();
    }
  });
  text.addEventListener('blur', () => text.dataset.dirty && commitText());

  chip.addEventListener('click', () => {
    if (openPopover && openPopover.parentElement === wrap) {
      closePopover();
      return;
    }
    closePopover();
    const sw = (label, css, v, mark) =>
      h(
        'button',
        { type: 'button', class: `pop-swatch${v === value ? ' current' : ''}`, title: label, onclick: () => commit(v) },
        h('span', { class: 'pop-chip', dataset: { c: css } }),
        h('span', { class: 'pop-label' }, label),
        mark ? h('span', { class: 'pop-mark mono' }, mark) : null,
      );
    const items = [];
    const names = Object.keys(palette);
    if (names.length) {
      items.push(h('div', { class: 'pop-title' }, t?.('inspector.f.color.project') ?? 'Project'));
      for (const n of names) items.push(sw(`@${n}`, palette[n], `@${n}`, palette[n]));
    }
    if (swatches.length) {
      items.push(h('div', { class: 'pop-title' }, t?.('inspector.f.color.catalog') ?? 'Catalog'));
      for (const s of swatches) items.push(sw(s.label, s.hex, s.name, s.hex));
    }
    const picker = h('input', { type: 'color', class: 'pop-native' });
    const cur = cssColor(value, palette, swatches);
    if (cur && /^#[0-9a-f]{6}/i.test(cur)) picker.value = cur.slice(0, 7);
    picker.addEventListener('change', () => commit(picker.value.toUpperCase()));
    items.push(h('div', { class: 'pop-title' }, t?.('inspector.f.color.custom') ?? 'Custom'), h('div', { class: 'pop-custom' }, picker));
    if (opts.optional) {
      items.push(h('button', { type: 'button', class: 'btn btn-small pop-none', onclick: () => commit(null) }, t?.('inspector.f.color.none') ?? 'None'));
    }
    const pop = h('div', { class: 'color-pop' }, ...items);
    for (const c of pop.querySelectorAll('.pop-chip')) c.style.setProperty('--swatch', c.dataset.c);
    wrap.append(pop);
    openPopover = pop;
  });

  const wrap = h('span', { class: 'color-input' }, chip, text);
  paint();
  return wrap;
}

// ----- fills ------------------------------------------------------------------------

const FILL_KINDS = ['color', 'linear-gradient', 'radial-gradient', 'image'];

/** Kind of a fill value (`"#hex"`/`"@name"` strings are colours). */
export function fillKind(fill) {
  if (fill == null) return null;
  if (typeof fill === 'string') return 'color';
  return fill.kind;
}

function fillColor(fill) {
  if (typeof fill === 'string') return fill;
  if (fill?.kind === 'color') return fill.color;
  return fill?.stops?.[0]?.[0] ?? '#FFFFFF';
}

/** CSS preview of a fill (gradient bar or colour). */
export function fillCss(fill, palette) {
  const c = (v) => cssColor(v, palette) ?? '#FF00FF';
  switch (fillKind(fill)) {
    case 'color':
      return c(fillColor(fill));
    case 'linear-gradient':
      return `linear-gradient(90deg, ${fill.stops.map(([col, p]) => `${c(col)} ${p * 100}%`).join(', ')})`;
    case 'radial-gradient':
      return `radial-gradient(circle, ${fill.stops.map(([col, p]) => `${c(col)} ${p * 100}%`).join(', ')})`;
    default:
      return 'repeating-linear-gradient(45deg, var(--bg-hover) 0 4px, transparent 4px 8px)';
  }
}

/**
 * Fill editor: kind, colour, gradient angle/centre/radius, stops (add,
 * remove, move, colour, position), image asset and fit.
 *
 * opts: `{ key, fill, palette, images: [paths], optional, onCommit(fill|null), t }`.
 */
export function fillEditor(opts) {
  const { palette = {}, t } = opts;
  const fill = opts.fill;
  const kind = fillKind(fill);
  const k = opts.key ?? 'fill';
  const root = h('div', { class: 'fill-editor' });
  const commit = (f) => opts.onCommit(f);

  const kinds = [...(opts.optional ? [{ value: '', label: t('inspector.f.fill.none') }] : []), ...FILL_KINDS.map((x) => ({ value: x, label: t(`inspector.f.fill.${x}`) }))];
  const switchKind = (to) => {
    if (to === '') return commit(null);
    const col = fillColor(fill) ?? '#FFFFFF';
    const stops = fill?.stops ?? [
      [col, 0],
      [col === '#FFFFFF' ? '#000000' : '#FFFFFF', 1],
    ];
    if (to === 'color') return commit(col);
    if (to === 'linear-gradient') return commit({ kind: to, angle: fill?.angle ?? 180, stops });
    if (to === 'radial-gradient') return commit({ kind: to, stops });
    const asset = opts.images?.[0];
    if (!asset) {
      opts.onError?.(t('inspector.f.fill.no_images'));
      return;
    }
    return commit({ kind: 'image', asset, fit: 'cover' });
  };
  const preview = h('span', { class: 'fill-preview' });
  preview.style.background = fillCss(fill, palette);
  root.append(h('div', { class: 'fill-head' }, preview, selectInput({ key: `${k}.kind`, options: kinds, value: kind ?? '', onChange: switchKind, title: t('inspector.f.fill.kind') })));

  if (kind === 'color') {
    root.append(colorInput({ key: `${k}.color`, value: fillColor(fill), palette, t, onCommit: (v) => commit(v ?? '#FFFFFF') }));
  } else if (kind === 'linear-gradient' || kind === 'radial-gradient') {
    const geo = h('div', { class: 'fill-geo' });
    if (kind === 'linear-gradient') {
      geo.append(numberInput({ key: `${k}.angle`, label: '∠', title: t('inspector.f.fill.angle'), value: fill.angle ?? 180, unit: '°', precision: 1, step: 1, big: 15, onCommit: (v) => commit({ ...fill, angle: v }) }).el);
    } else {
      const c = fill.center ?? { x: 0.5, y: 0.5 };
      geo.append(
        numberInput({ key: `${k}.cx`, label: 'X', title: t('inspector.f.fill.center'), value: c.x, scale: 100, unit: '%', precision: 1, min: -1, max: 2, step: 0.01, big: 0.1, onCommit: (v) => commit({ ...fill, center: { x: v, y: c.y } }) }).el,
        numberInput({ key: `${k}.cy`, label: 'Y', title: t('inspector.f.fill.center'), value: c.y, scale: 100, unit: '%', precision: 1, min: -1, max: 2, step: 0.01, big: 0.1, onCommit: (v) => commit({ ...fill, center: { x: c.x, y: v } }) }).el,
        numberInput({ key: `${k}.r`, label: 'R', title: t('inspector.f.fill.radius'), value: fill.radius ?? 0.5, scale: 100, unit: '%', precision: 1, min: 0.01, max: 4, step: 0.01, big: 0.1, onCommit: (v) => commit({ ...fill, radius: v }) }).el,
      );
    }
    root.append(geo);
    const stops = fill.stops ?? [];
    const setStops = (s) => commit({ ...fill, stops: s });
    const list = h('div', { class: 'stops' });
    stops.forEach(([col, pos], i) => {
      const upd = (c2, p2) => setStops(stops.map((s, j) => (j === i ? [c2, p2] : s)));
      const move = (d) => {
        const s = [...stops];
        const j = i + d;
        if (j < 0 || j >= s.length) return;
        // Swap colours, keep the positions in order.
        [s[i], s[j]] = [[s[j][0], s[i][1]], [s[i][0], s[j][1]]];
        setStops(s);
      };
      list.append(
        h(
          'div',
          { class: 'stop' },
          colorInput({ key: `${k}.stop${i}.c`, value: col, palette, t, onCommit: (v) => upd(v ?? '#000000', pos) }),
          numberInput({ key: `${k}.stop${i}.p`, value: pos, scale: 100, unit: '%', precision: 1, min: 0, max: 1, step: 0.01, big: 0.1, title: t('inspector.f.fill.position'), onCommit: (v) => upd(col, v) }).el,
          h('span', { class: 'stop-btns' },
            h('button', { type: 'button', class: 'mini-btn', title: t('inspector.f.move_up'), disabled: i === 0, onclick: () => move(-1) }, icon('up')),
            h('button', { type: 'button', class: 'mini-btn', title: t('inspector.f.move_down'), disabled: i === stops.length - 1, onclick: () => move(1) }, icon('down')),
            h('button', { type: 'button', class: 'mini-btn', title: t('inspector.f.fill.remove_stop'), disabled: stops.length <= 2, onclick: () => setStops(stops.filter((_, j) => j !== i)) }, icon('trash')),
          ),
        ),
      );
    });
    const add = () => {
      // Between the two stops with the largest gap.
      let best = 0;
      for (let i = 1; i + 1 < stops.length; i++) {
        if (stops[i + 1][1] - stops[i][1] > stops[best + 1][1] - stops[best][1]) best = i;
      }
      const a = stops[best] ?? ['#FFFFFF', 0];
      const b = stops[best + 1] ?? a;
      const s = [...stops];
      s.splice(best + 1, 0, [a[0], Math.round(((a[1] + b[1]) / 2) * 1000) / 1000]);
      setStops(s);
    };
    root.append(list, h('button', { type: 'button', class: 'btn btn-small', onclick: add }, icon('plus'), t('inspector.f.fill.add_stop')));
  } else if (kind === 'image') {
    root.append(
      textInput({ key: `${k}.asset`, value: fill.asset, list: opts.images, mono: true, title: t('inspector.f.fill.asset'), onCommit: (v) => v && commit({ ...fill, asset: v }) }),
      selectInput({
        key: `${k}.fit`,
        title: t('inspector.fit'),
        options: ['cover', 'contain', 'stretch', 'tile'].map((x) => ({ value: x, label: t(`inspector.f.fit.${x}`) })),
        value: fill.fit ?? 'cover',
        onChange: (v) => commit({ ...fill, fit: v }),
      }),
    );
  }
  return root;
}
