// Targets editor (task 2.9): the project's targets per strip, adding from
// catalog presets (grouped by store/platform), and a form for every target
// field. Changes go through `target_patch` / `target_add_from_preset` /
// `target_remove` (ops are built in Rust) and `editor.apply`. Store-rule
// hints come from validation findings of the target.

import { api } from '../api.js';
import { h, icon } from '../dom.js';
import { editor } from '../editor.js';
import { t } from '../i18n.js';
import { store } from '../store.js';
import { checkbox, confirmDialog, errorLine, input, openModal, row, segmented, select } from './modal.js';

/** Finding codes shown as hints in the targets editor. */
const HINT_CODES = new Set([
  'store_size', 'store_count', 'store_format', 'unknown_store_slot', 'unverified_store_slot', 'store_slot_duplicate',
  'unknown_gallery', 'no_gallery', 'unknown_screenshot_variant', 'unknown_mapped_device',
  'region_out_of_range', 'partial_image', 'variant_suggested', 'unknown_variant',
  'store_decorations', 'screenshot_only_empty', 'text_outside_safe_area',
]);

let catalogs = null;

async function loadCatalogs() {
  if (!catalogs) {
    const [presets, galleries, devices] = await Promise.all([api.catalogTargets(), api.catalogGalleries(), api.catalogDevices()]);
    catalogs = { presets, galleries, devices };
  }
  return catalogs;
}

const sizeLabel = (s) => `${s[0] ?? s.w}×${s[1] ?? s.h}`;
const asPair = (s) => (Array.isArray(s) ? s : [s.w, s.h]);
// Apple creative assets carry a placement: `apple:<display type>#<placement>`.
const slotKey = (slot) =>
  slot?.apple ? `apple:${slot.apple.display_type}${slot.apple.placement ? `#${slot.apple.placement}` : ''}` : slot?.google ? `google:${slot.google.image_type}` : '';
function slotFromKey(key) {
  if (!key) return null;
  const [store, value] = [key.slice(0, key.indexOf(':')), key.slice(key.indexOf(':') + 1)];
  if (store !== 'apple') return { google: { image_type: value } };
  const [display_type, placement] = value.split('#');
  return { apple: placement ? { display_type, placement } : { display_type } };
}

/** Opens the targets editor; `focus` = target id to select first. */
export async function openTargets(focus) {
  const cat = await loadCatalogs();
  const err = errorLine();
  const list = h('div', { class: 'tg-list' });
  const pane = h('div', { class: 'tg-pane' });
  let selected = focus ?? store.get().target;
  let mode = 'edit';

  const m = openModal({
    title: t('targets.title'),
    size: 'large',
    cls: 'dlg-targets',
    body: [h('div', { class: 'tg-layout' }, list, h('div', { class: 'tg-right' }, err.el, pane))],
    footer: [h('span', { class: 'spacer' }), h('button', { class: 'btn btn-primary', type: 'button', onclick: () => m.close() }, t('project.dialog.done'))],
  });

  const unsub = store.select((s) => s.summary, () => renderList());
  m.closed.then(unsub);

  /** Runs a backend change, keeps the view context valid, then refreshes. */
  async function commit(call, { newId, removed } = {}) {
    let result;
    try {
      result = await call();
    } catch (e) {
      err.show(e);
      return null;
    }
    err.show(null);
    const st = store.get();
    if (removed && st.target === removed) {
      const other = st.summary.targets.find((x) => x.strip === st.strip && x.id !== removed);
      store.set({ target: other?.id ?? null, variant: undefined });
    } else if (newId && st.target === selected) {
      store.set({ target: newId });
    }
    await editor.apply(Promise.resolve(result));
    if (newId) selected = newId;
    if (removed) selected = store.get().summary.targets[0]?.id ?? null;
    if (!store.get().target) {
      const s2 = store.get();
      const first = s2.summary.targets.find((x) => x.strip === s2.strip);
      if (first) await editor.setContext({ target: first.id });
    }
    renderList();
    await renderPane();
    return result;
  }

  function renderList() {
    const s = store.get().summary;
    if (!s) return;
    const groups = s.strips.map((strip) => {
      const items = s.targets.filter((x) => x.strip === strip.id);
      return h(
        'div',
        { class: 'tg-group' },
        h('div', { class: 'section-title' }, `${t('targets.strip')}: ${strip.id}`),
        h(
          'ul',
          { class: 'list' },
          items.map((x) =>
            h(
              'li',
              {
                class: 'list-item',
                tabindex: '0',
                'aria-selected': String(mode === 'edit' && x.id === selected),
                onclick: () => {
                  selected = x.id;
                  mode = 'edit';
                  renderList();
                  renderPane();
                },
              },
              h('span', { class: 'grow' }, x.id),
              h('span', { class: 'muted small' }, x.kind === 'banner' ? t('targets.kind.banner') : sizeLabel(x.size)),
            ),
          ),
          items.length ? null : h('li', { class: 'placeholder' }, t('targets.none')),
        ),
      );
    });
    list.replaceChildren(
      h(
        'div',
        { class: 'tg-list-head' },
        h(
          'button',
          {
            class: `btn btn-small ${mode === 'add' ? 'btn-primary' : ''}`,
            type: 'button',
            onclick: () => {
              mode = 'add';
              renderList();
              renderPane();
            },
          },
          icon('plus'),
          t('targets.add'),
        ),
      ),
      ...groups,
    );
  }

  // ----- add from preset ------------------------------------------------------
  function renderAdd() {
    const s = store.get().summary;
    let strip = store.get().strip ?? s.strips[0]?.id;
    let filter = '';
    const results = h('div', { class: 'tg-presets' });
    const STORES = { apple: 'App Store', google: 'Google Play', none: t('targets.store.none') };
    function draw() {
      const q = filter.trim().toLowerCase();
      const byGroup = new Map();
      for (const p of cat.presets) {
        if (q && !`${p.id} ${p.label} ${p.platform}`.toLowerCase().includes(q)) continue;
        const key = `${STORES[p.store] ?? p.store} · ${p.platform}`;
        if (!byGroup.has(key)) byGroup.set(key, []);
        byGroup.get(key).push(p);
      }
      results.replaceChildren(
        ...[...byGroup].map(([group, presets]) =>
          h(
            'section',
            { class: 'tg-preset-group' },
            h('div', { class: 'section-title' }, group),
            presets.map((p) => {
              const sizes = [
                ...p.portrait.map((x) => ({ size: asPair(x), o: 'portrait' })),
                ...p.landscape.map((x) => ({ size: asPair(x), o: 'landscape' })),
              ];
              return h(
                'div',
                { class: 'tg-preset', title: p.note ?? '' },
                h(
                  'div',
                  { class: 'tg-preset-main' },
                  h('span', { class: 'tg-preset-label' }, p.label),
                  h('span', { class: 'muted small mono' }, p.id),
                  h('span', { class: `tag tag-${p.status}` }, t(`targets.status.${p.status}`)),
                  p.requirement ? h('span', { class: `tag tag-${p.requirement}` }, t(`targets.req.${p.requirement}`)) : null,
                  p.kind === 'banner' ? h('span', { class: 'tag' }, t('targets.kind.banner')) : null,
                ),
                h(
                  'div',
                  { class: 'tg-preset-sizes' },
                  sizes.slice(0, 6).map(({ size, o }) =>
                    h(
                      'button',
                      {
                        class: 'btn btn-small',
                        type: 'button',
                        title: `${t('targets.add.size')} (${t(`targets.orientation.${o}`)})`,
                        onclick: async () => {
                          const r = await commit(() => api.targetAddFromPreset(p.id, strip, { size }));
                          if (r?.created?.[0]) {
                            selected = r.created[0];
                            mode = 'edit';
                            renderList();
                            await renderPane();
                          }
                        },
                      },
                      icon('plus'),
                      sizeLabel(size),
                    ),
                  ),
                ),
              );
            }),
          ),
        ),
      );
    }
    draw();
    pane.replaceChildren(
      h('h3', { class: 'tg-title' }, t('targets.add.title')),
      h(
        'div',
        { class: 'tg-add-bar' },
        row(t('targets.strip'), select({ options: s.strips.map((x) => x.id), value: strip, onchange: (v) => (strip = v) })),
        input({
          placeholder: t('targets.add.filter'),
          cls: 'tg-filter',
          oninput: (v) => {
            filter = v;
            draw();
          },
        }),
      ),
      results,
    );
  }

  // ----- edit form ------------------------------------------------------------
  async function renderPane() {
    if (mode === 'add') return renderAdd();
    if (!selected || !store.get().summary.targets.some((x) => x.id === selected)) {
      selected = store.get().summary.targets[0]?.id ?? null;
    }
    if (!selected) {
      pane.replaceChildren(h('div', { class: 'placeholder' }, t('targets.empty')));
      return;
    }
    const id = selected;
    let model;
    let report;
    try {
      [model, report] = await Promise.all([api.targetGet(id), api.validateAll({ targets: [id], preview: true })]);
    } catch (e) {
      err.show(e);
      return;
    }
    if (id !== selected || mode !== 'edit') return;
    pane.replaceChildren(form(model, report));
  }

  function form({ target: tg, pages, variants }, report) {
    const s = store.get().summary;
    const id = tg.id;
    const patch = (p) => commit(() => api.targetPatch(id, p));
    const banner = tg.kind === 'banner';
    const size = asPair(tg.size);

    // size presets: sizes of presets with the same store slot / screenshot variant, plus the current one
    const sizeOptions = [];
    const seen = new Set();
    for (const p of cat.presets) {
      const related = (slotKey(p.slot) && slotKey(p.slot) === slotKey(tg.store)) || (p.kind === tg.kind && p.screenshots && p.screenshots === tg.screenshots);
      if (!related) continue;
      for (const x of [...p.portrait, ...p.landscape].map(asPair)) {
        const k = sizeLabel(x);
        if (!seen.has(k)) {
          seen.add(k);
          sizeOptions.push({ value: k, label: `${k} · ${p.label}` });
        }
      }
    }
    const currentKey = sizeLabel(size);
    if (!seen.has(currentKey)) sizeOptions.unshift({ value: currentKey, label: `${currentKey} · ${t('targets.size.custom')}` });
    const w = input({ type: 'number', value: size[0], min: 1, cls: 'num', onchange: (v) => v > 0 && patch({ size: [v, size[1]] }) });
    const hh = input({ type: 'number', value: size[1], min: 1, cls: 'num', onchange: (v) => v > 0 && patch({ size: [size[0], v] }) });
    const sizeCtl = h(
      'div',
      { class: 'inline' },
      select({ options: sizeOptions, value: currentKey, onchange: (v) => patch({ size: v.split('×').map(Number) }) }),
      w,
      h('span', { class: 'muted' }, '×'),
      hh,
      h('button', { class: 'btn btn-small', type: 'button', title: t('targets.size.swap'), onclick: () => patch({ size: [size[1], size[0]] }) }, '⇄'),
    );

    // gallery
    const gal = typeof tg.gallery === 'string' ? { preset: tg.gallery } : tg.gallery ?? null;
    const profile = cat.galleries.find((g) => g.id === gal?.preset);
    const galleryCtl = h(
      'div',
      { class: 'stack' },
      h(
        'div',
        { class: 'inline' },
        select({
          options: [{ value: '', label: t('targets.none.value') }, ...cat.galleries.map((g) => ({ value: g.id, label: g.label }))],
          value: gal?.preset ?? '',
          onchange: (v) => patch({ gallery: v ? { ...(gal ?? {}), preset: v } : null }),
        }),
        profile ? h('span', { class: `tag ${profile.measured === 'no' ? 'tag-placeholder' : 'tag-verified'}`, title: profile.note ?? profile.source }, t(`targets.measured.${profile.measured}`)) : null,
      ),
      gal
        ? h(
            'div',
            { class: 'inline' },
            h('span', { class: 'muted small' }, t('targets.gallery.gap')),
            input({ type: 'number', value: gal.gap, min: 0, placeholder: profile ? String(profile.gap) : '', cls: 'num', onchange: (v) => patch({ gallery: { ...gal, gap: v ?? undefined } }) }),
            h('span', { class: 'muted small' }, t('targets.gallery.radius')),
            input({ type: 'number', value: gal.radius, min: 0, placeholder: profile ? String(profile.radius_x) : '', cls: 'num', onchange: (v) => patch({ gallery: { ...gal, radius: v ?? undefined } }) }),
          )
        : null,
    );

    // device map
    const deviceOptions = cat.devices.map((d) => ({ value: d.id, label: d.label, group: d.class }));
    const mapEntries = Object.entries(tg.device_map ?? {});
    const setMap = (entries) => patch({ device_map: Object.fromEntries(entries.filter(([a, b]) => a && b)) });
    const mapCtl = h(
      'div',
      { class: 'stack' },
      mapEntries.map(([from, to], i) =>
        h(
          'div',
          { class: 'inline' },
          select({ options: deviceOptions, value: from, onchange: (v) => setMap(mapEntries.map((e, j) => (j === i ? [v, e[1]] : e))) }),
          h('span', { class: 'muted' }, '→'),
          select({ options: deviceOptions, value: to, onchange: (v) => setMap(mapEntries.map((e, j) => (j === i ? [e[0], v] : e))) }),
          h('button', { class: 'icon-btn', type: 'button', title: t('targets.remove.entry'), onclick: () => setMap(mapEntries.filter((_, j) => j !== i)) }, icon('close')),
        ),
      ),
      h(
        'div',
        {},
        h(
          'button',
          {
            class: 'btn btn-small',
            type: 'button',
            onclick: () => {
              const used = new Set(mapEntries.map((e) => e[0]));
              const from = cat.devices.find((d) => !used.has(d.id));
              if (from) setMap([...mapEntries, [from.id, from.id]]);
            },
          },
          icon('plus'),
          t('targets.device_map.add'),
        ),
      ),
    );

    // screenshots variant
    const shotVariants = [...new Set([...cat.devices.map((d) => d.screenshot_variant), ...cat.presets.map((p) => p.screenshots)].filter(Boolean))].sort();
    if (tg.screenshots && !shotVariants.includes(tg.screenshots)) shotVariants.unshift(tg.screenshots);

    // languages
    const allLangs = s.languages.available;
    const langs = tg.langs ?? null;
    const langCtl = h(
      'div',
      { class: 'inline wrap' },
      checkbox({ label: t('targets.langs.all'), checked: !langs, onchange: (v) => patch({ langs: v ? null : [...allLangs] }) }),
      langs
        ? allLangs.map((l) =>
            checkbox({
              label: l,
              checked: langs.includes(l),
              onchange: (v) => {
                const next = allLangs.filter((x) => (x === l ? v : langs.includes(x)));
                if (next.length) patch({ langs: next });
              },
            }),
          )
        : null,
    );

    // banner region
    let bannerRows = [];
    if (banner) {
      const region = tg.region ?? null;
      const rmode = region?.pages ? 'pages' : region?.rect ? 'rect' : 'all';
      const regionCtl = h('div', { class: 'stack' });
      const modeSel = select({
        options: [
          { value: 'all', label: t('targets.region.all') },
          { value: 'pages', label: t('targets.region.pages') },
          { value: 'rect', label: t('targets.region.rect') },
        ],
        value: rmode,
        onchange: (v) =>
          patch({
            region: v === 'all' ? null : v === 'pages' ? { pages: [0, Math.max(0, pages - 1)] } : { rect: { page: 0, x: 0, y: 0, width: 1000, height: 600 } },
          }),
      });
      regionCtl.append(modeSel);
      if (rmode === 'pages') {
        const [a, b] = region.pages;
        regionCtl.append(
          h(
            'div',
            { class: 'inline' },
            h('span', { class: 'muted small' }, t('targets.region.from')),
            input({ type: 'number', value: a, min: 0, max: pages - 1, cls: 'num', onchange: (v) => v != null && patch({ region: { pages: [v, Math.max(v, b)] } }) }),
            h('span', { class: 'muted small' }, t('targets.region.to')),
            input({ type: 'number', value: b, min: 0, max: pages - 1, cls: 'num', onchange: (v) => v != null && patch({ region: { pages: [Math.min(a, v), v] } }) }),
            h('span', { class: 'muted small' }, `(0–${pages - 1})`),
          ),
        );
      } else if (rmode === 'rect') {
        const r = region.rect;
        const field = (k) =>
          h('label', { class: 'mini' }, h('span', { class: 'muted small' }, t(`targets.rect.${k}`)), input({ type: 'number', value: r[k] ?? 0, cls: 'num', onchange: (v) => v != null && patch({ region: { rect: { ...r, [k]: v } } }) }));
        regionCtl.append(h('div', { class: 'inline wrap' }, ['page', 'x', 'y', 'width', 'height'].map(field)));
      }
      bannerRows = [
        row(t('targets.region'), regionCtl),
        row(t('targets.fit'), segmented({ options: ['cover', 'contain'].map((v) => ({ value: v, label: t(`targets.fit.${v}`) })), value: tg.fit ?? 'cover', onchange: (v) => patch({ fit: v }) })),
        row(t('targets.gaps'), segmented({ options: ['none', 'as_store'].map((v) => ({ value: v, label: t(`targets.gaps.${v}`) })), value: tg.gaps ?? 'none', onchange: (v) => patch({ gaps: v }) })),
      ];
    }

    // store slot
    const slots = new Map();
    for (const p of cat.presets) if (p.slot) slots.set(slotKey(p.slot), { value: slotKey(p.slot), label: slotKey(p.slot).split(':')[1], group: p.slot.apple ? 'App Store' : 'Google Play' });
    if (tg.store && !slots.has(slotKey(tg.store))) slots.set(slotKey(tg.store), { value: slotKey(tg.store), label: slotKey(tg.store).split(':')[1] });

    // hints
    const hints = (report?.findings ?? []).filter((f) => HINT_CODES.has(f.code));
    const uniq = [...new Map(hints.map((f) => [`${f.code}|${f.message}`, f])).values()];
    const hintEl = uniq.length
      ? h(
          'div',
          { class: 'tg-hints' },
          uniq.map((f) => h('div', { class: `hint hint-${f.level}` }, icon(f.level === 'error' ? 'error' : 'warning'), h('span', { class: 'mono small' }, f.code), h('span', {}, f.message))),
        )
      : h('div', { class: 'tg-hints' }, h('div', { class: 'hint hint-ok' }, icon('check'), t('targets.hints.ok')));

    // safe area (insets in % of the image)
    const safe = tg.safe_area ?? null;
    const safeCtl = safe
      ? h(
          'div',
          { class: 'inline wrap' },
          ['top', 'right', 'bottom', 'left'].map((k) =>
            h('label', { class: 'mini' }, h('span', { class: 'muted small' }, t(`targets.safe_area.${k}`)), input({ type: 'number', value: safe[k] ?? 0, min: 0, max: 49, step: 0.5, cls: 'num', onchange: (v) => v != null && v >= 0 && v < 50 && patch({ safe_area: { ...safe, [k]: v } }) })),
          ),
          h('button', { class: 'btn btn-small', type: 'button', onclick: () => patch({ safe_area: null }) }, t('targets.safe_area.remove')),
        )
      : h('button', { class: 'btn btn-small', type: 'button', onclick: () => patch({ safe_area: { top: 0, right: 0, bottom: 0, left: 0 } }) }, t('targets.safe_area.add'));

    const format = tg.format ?? 'png';
    return h(
      'div',
      { class: 'tg-form' },
      h(
        'div',
        { class: 'tg-form-head' },
        h('h3', { class: 'tg-title' }, id),
        h('span', { class: 'spacer' }),
        h(
          'button',
          {
            class: 'btn btn-small',
            type: 'button',
            onclick: async () => {
              if (await confirmDialog({ title: t('targets.remove'), message: `${t('targets.remove.confirm')} „${id}“?`, confirm: t('targets.remove'), danger: true })) {
                await commit(() => api.targetRemove(id), { removed: id });
              }
            },
          },
          t('targets.remove'),
        ),
      ),
      hintEl,
      h(
        'div',
        { class: 'form' },
        row(t('targets.id'), input({ value: id, onchange: (v) => v && v !== id && commit(() => api.targetPatch(id, {}, v), { newId: v }) })),
        row(
          t('targets.kind'),
          segmented({
            options: ['pages', 'banner'].map((v) => ({ value: v, label: t(`targets.kind.${v}`) })),
            value: tg.kind ?? 'pages',
            onchange: (v) => patch(v === 'banner' ? { kind: v, group: null, gallery: null, screenshot_only: false } : { kind: v, region: null, fit: null, gaps: null }),
          }),
        ),
        row(t('targets.strip'), select({ options: s.strips.map((x) => x.id), value: tg.strip, onchange: (v) => patch({ strip: v, variant: null }) })),
        row(t('targets.size'), sizeCtl),
        banner ? null : row(t('targets.group'), input({ type: 'number', value: tg.group ?? 1, min: 1, max: 6, cls: 'num', onchange: (v) => v >= 1 && patch({ group: v === 1 ? null : v }) }), t('targets.group.hint')),
        row(
          t('targets.variant'),
          select({ options: [{ value: '', label: t('targets.none.value') }, ...variants], value: tg.variant ?? '', onchange: (v) => patch({ variant: v || null }) }),
        ),
        banner ? null : row(t('targets.gallery'), galleryCtl),
        banner ? null : row(t('targets.screenshot_only'), checkbox({ label: t('targets.screenshot_only.label'), checked: !!tg.screenshot_only, onchange: (v) => patch({ screenshot_only: v }) }), t('targets.screenshot_only.hint')),
        ...bannerRows,
        row(t('targets.screenshots'), select({ options: [{ value: '', label: t('targets.none.value') }, ...shotVariants], value: tg.screenshots ?? '', onchange: (v) => patch({ screenshots: v || null }) })),
        row(t('targets.device_map'), mapCtl),
        row(t('targets.langs'), langCtl),
        row(t('targets.safe_area'), safeCtl, t('targets.safe_area.hint')),
        row(
          t('targets.format'),
          h(
            'div',
            { class: 'inline' },
            segmented({ options: [{ value: 'png', label: 'PNG' }, { value: 'jpeg', label: 'JPEG' }], value: format, onchange: (v) => patch(v === 'png' ? { format: null, quality: null } : { format: 'jpeg', quality: tg.quality ?? 90 }) }),
            format === 'jpeg'
              ? h('label', { class: 'mini' }, h('span', { class: 'muted small' }, t('targets.quality')), input({ type: 'number', value: tg.quality ?? 90, min: 1, max: 100, cls: 'num', onchange: (v) => v >= 1 && v <= 100 && patch({ quality: v }) }))
              : null,
          ),
        ),
        row(
          t('targets.store'),
          select({ options: [{ value: '', label: t('targets.none.value') }, ...slots.values()], value: slotKey(tg.store), onchange: (v) => patch({ store: slotFromKey(v) }) }),
          t('targets.store.hint'),
        ),
      ),
    );
  }

  renderList();
  await renderPane();
  return m;
}
