// Store upload configuration (Phase 5.1, ADR 0016).
//
// - App settings → "Stores": app-wide accounts. Keys are imported from a
//   file picked in a native dialog; the backend validates the file and puts
//   the secret into the system keychain (the UI never sees key material).
//   Test asks the store who we are (`not_implemented` until the store
//   clients are wired up), remove deletes the keychain entry after a
//   confirmation.
// - Project settings → "Stores" tab: Apple app id / bundle id / platform,
//   Google package name, account per store, language → store locale map
//   with the built-in defaults shown. Every change is one
//   `project_store_patch` (one undo step).

import { api } from '../api.js';
import { h, icon } from '../dom.js';
import { t } from '../i18n.js';
import { choiceDialog, confirmDialog, input, row, select } from './modal.js';

const KINDS = ['apple', 'google'];

// ----- app settings: accounts --------------------------------------------------------

/** Rows of the "Stores" section of the app settings. */
export function storeAccountRows() {
  const list = h('div', { class: 'store-accounts' });
  const keychain = h('div', { class: 'form-hint' });
  const error = h('div', { class: 'form-error', role: 'alert', hidden: true });

  function showError(e) {
    error.textContent = e ? (e.message ?? String(e)) : '';
    error.hidden = !e;
  }

  async function refresh() {
    let data;
    try {
      data = await api.storeAccountsList();
    } catch (e) {
      showError(e);
      return;
    }
    const kc = data.keychain;
    if (kc.error) keychain.textContent = `${t('stores.keychain.error')}: ${kc.error}`;
    else if (kc.backend === 'mock') keychain.textContent = t('stores.keychain.mock');
    else keychain.textContent = t('stores.keychain.system').replace('{service}', kc.service);
    keychain.classList.toggle('warn', Boolean(kc.error) || kc.backend === 'mock');
    list.replaceChildren(...(data.accounts.length ? data.accounts.map(accountItem) : [h('div', { class: 'muted small' }, t('stores.accounts.none'))]));
  }

  function accountItem(a) {
    const status = h('span', { class: 'store-account-status small' });
    const facts = a.kind === 'apple' ? `${t('stores.apple.key_id')} ${a.apple?.key_id ?? ''}` : (a.google?.client_email ?? '');
    return h(
      'div',
      { class: 'store-account' },
      h('span', { class: `tag store-kind store-kind-${a.kind}` }, t(`stores.kind.${a.kind}`)),
      h('div', { class: 'store-account-main' }, h('strong', {}, a.label), h('span', { class: 'mono small muted selectable' }, `${a.id} · ${facts}`), status),
      h(
        'button',
        {
          class: 'btn btn-small',
          type: 'button',
          onclick: async () => {
            status.dataset.state = 'busy';
            status.textContent = t('stores.test.running');
            try {
              const r = await api.storeAccountTest(a.id);
              status.dataset.state = 'ok';
              status.textContent = `${t('stores.test.ok')}: ${r.summary}`;
            } catch (e) {
              const notYet = e.code === 'not_implemented';
              status.dataset.state = notYet ? 'warn' : 'error';
              status.textContent = notYet ? t('stores.test.not_implemented') : e.message;
            }
          },
        },
        t('stores.test'),
      ),
      h(
        'button',
        {
          class: 'icon-btn',
          type: 'button',
          title: t('stores.remove'),
          onclick: async () => {
            const ok = await confirmDialog({ title: t('stores.remove'), message: t('stores.remove.confirm').replace('{label}', a.label), confirm: t('stores.remove'), danger: true });
            if (!ok) return;
            try {
              await api.storeAccountRemove(a.id);
              showError(null);
            } catch (e) {
              showError(e);
            }
            refresh();
          },
        },
        icon('trash'),
      ),
    );
  }

  async function add(kind) {
    showError(null);
    const files = await api.pickFiles({ title: t(`stores.pick.${kind}`), extensions: [kind === 'apple' ? 'p8' : 'json'], multiple: false });
    const path = files?.[0];
    if (!path) return;
    const details = await importDetails(kind, path);
    if (!details) return;
    try {
      await api.storeAccountImport({ kind, path, ...details });
    } catch (e) {
      showError(e);
    }
    refresh();
  }

  refresh();
  return [
    h('div', { class: 'form-section' }, icon('export'), t('stores.title')),
    row(
      t('stores.accounts'),
      h(
        'div',
        { class: 'store-accounts-box' },
        error,
        list,
        h(
          'div',
          { class: 'store-accounts-add' },
          ...KINDS.map((k) => h('button', { class: 'btn', type: 'button', onclick: () => add(k) }, t(`stores.add.${k}`))),
        ),
        keychain,
      ),
      t('stores.accounts.hint'),
    ),
  ];
}

/** Label (and Apple's issuer/key id) for an import; null when cancelled. */
function importDetails(kind, path) {
  const name = path.split(/[\\/]/).pop() ?? '';
  const keyFromName = /^AuthKey_([A-Z0-9]{10})\.p8$/.exec(name)?.[1] ?? '';
  const label = h('input', { class: 'field', type: 'text', spellcheck: 'false', placeholder: t('stores.import.label.placeholder') });
  const issuer = h('input', { class: 'field mono', type: 'text', spellcheck: 'false', placeholder: '57246542-96fe-1a63-e053-0824d011072a' });
  const keyId = h('input', { class: 'field mono', type: 'text', spellcheck: 'false', placeholder: 'ABCDE12345' });
  keyId.value = keyFromName;
  const err = h('div', { class: 'form-error', role: 'alert', hidden: true });
  const fields = [
    h('p', { class: 'dlg-text' }, t(`stores.import.intro.${kind}`)),
    h('div', { class: 'mono small selectable' }, path),
    h('label', { class: 'dlg-field' }, h('span', {}, t('stores.import.label')), label),
  ];
  if (kind === 'apple') {
    fields.push(h('label', { class: 'dlg-field' }, h('span', {}, t('stores.apple.issuer_id')), issuer), h('label', { class: 'dlg-field' }, h('span', {}, t('stores.apple.key_id')), keyId));
  }
  fields.push(err);
  const check = () => {
    let msg = null;
    if (kind === 'apple' && !/^[0-9a-fA-F]{8}(-[0-9a-fA-F]{4}){3}-[0-9a-fA-F]{12}$/.test(issuer.value.trim())) msg = t('stores.import.issuer_invalid');
    else if (kind === 'apple' && !/^[A-Z0-9]{10}$/.test(keyId.value.trim())) msg = t('stores.import.key_invalid');
    err.textContent = msg ?? '';
    err.hidden = !msg;
    return !msg;
  };
  return choiceDialog({
    title: t(`stores.add.${kind}`),
    size: 'medium',
    body: h('div', { class: 'store-import' }, ...fields),
    buttons: [
      { label: t('project.dialog.cancel'), value: null },
      {
        label: t('stores.import.ok'),
        primary: true,
        check: async () => check(),
        value: () => {
          const out = { label: label.value.trim() || null };
          if (kind === 'apple') {
            out.issuer_id = issuer.value.trim();
            out.key_id = keyId.value.trim();
          }
          return out;
        },
      },
    ],
  });
}

// ----- project settings: mapping ------------------------------------------------------

/**
 * The "Stores" tab of the project settings. `commit(call)` applies a
 * backend result through the editor and re-renders the dialog.
 */
export async function storesPane(commit) {
  const info = await api.projectStoreGet();
  const s = info.store;
  const accounts = (kind) => info.accounts.filter((a) => a.kind === kind);
  const patch = (p) => commit(() => api.projectStorePatch(p));

  const warnings = info.missing_accounts.map((m) =>
    h('div', { class: 'hint hint-warn' }, icon('warning'), t(`stores.account.${m.problem}`).replace('{account}', m.account).replace('{store}', t(`stores.kind.${m.store}`))),
  );

  function accountSelect(kind, value, onchange) {
    const options = [{ value: '', label: t('stores.account.none') }, ...accounts(kind).map((a) => ({ value: a.id, label: `${a.label} (${a.id})` }))];
    if (value && !options.some((o) => o.value === value)) options.push({ value, label: `${value} – ${t('stores.account.missing')}` });
    return select({ options, value: value ?? '', onchange });
  }

  function appleSection() {
    const a = s.apple;
    if (!a) return setupButton('apple');
    const set = (field, v) => patch({ apple: { ...a, [field]: v === '' ? null : v } });
    return h(
      'div',
      { class: 'form' },
      row(t('stores.account'), accountSelect('apple', a.account, (v) => set('account', v)), accounts('apple').length ? null : t('stores.account.add_hint')),
      row(t('stores.apple.app_id'), input({ value: a.app_id ?? '', cls: 'mono', placeholder: '1234567890', onchange: (v) => set('app_id', v.trim()) }), t('stores.apple.app_id.hint')),
      row(t('stores.apple.bundle_id'), input({ value: a.bundle_id ?? '', cls: 'mono', placeholder: 'com.example.app', onchange: (v) => set('bundle_id', v.trim()) })),
      row(
        t('stores.apple.platform'),
        select({
          options: ['ios', 'macos'].map((p) => ({ value: p, label: t(`stores.apple.platform.${p}`) })),
          value: a.platform ?? 'ios',
          onchange: (v) => set('platform', v),
        }),
      ),
      removeRow('apple'),
    );
  }

  function googleSection() {
    const g = s.google;
    if (!g) return setupButton('google');
    const set = (field, v) => patch({ google: { ...g, [field]: v === '' ? null : v } });
    return h(
      'div',
      { class: 'form' },
      row(t('stores.account'), accountSelect('google', g.account, (v) => set('account', v)), accounts('google').length ? null : t('stores.account.add_hint')),
      row(t('stores.google.package_name'), input({ value: g.package_name ?? '', cls: 'mono', placeholder: 'com.example.app', onchange: (v) => set('package_name', v.trim()) })),
      removeRow('google'),
    );
  }

  function setupButton(kind) {
    return h('div', { class: 'store-setup' }, h('span', { class: 'muted small' }, t(`stores.not_mapped.${kind}`)), h('button', { class: 'btn', type: 'button', onclick: () => patch({ [kind]: {} }) }, t(`stores.setup.${kind}`)));
  }

  function removeRow(kind) {
    return h(
      'div',
      { class: 'store-remove' },
      h(
        'button',
        {
          class: 'btn btn-small',
          type: 'button',
          onclick: async () => {
            if (await confirmDialog({ title: t(`stores.unmap.${kind}`), message: t('stores.unmap.confirm'), confirm: t(`stores.unmap.${kind}`), danger: true })) patch({ [kind]: null });
          },
        },
        t(`stores.unmap.${kind}`),
      ),
    );
  }

  /** Language → store locale; empty choice = built-in default. */
  function localeTable(kind) {
    const explicit = s.locales?.[kind] ?? {};
    const codes = info.locales[kind];
    const rows = info.effective[kind].map((e) => {
      const def = e.default ? t('stores.locale.default').replace('{code}', e.default) : t('stores.locale.no_default');
      const options = [{ value: '', label: def }, ...codes.map((c) => ({ value: c.code, label: `${c.code} – ${c.name}` }))];
      const value = explicit[e.lang] ?? '';
      if (value && !codes.some((c) => c.code === value)) options.push({ value, label: `${value} – ${t('stores.locale.unknown')}` });
      const state = e.locale ? (e.source === 'project' ? 'project' : 'default') : 'none';
      return h(
        'div',
        { class: 'store-locale', dataset: { state } },
        h('span', { class: 'mono' }, e.lang),
        select({
          options,
          value,
          onchange: (v) => {
            const map = { ...explicit };
            if (v) map[e.lang] = v;
            else delete map[e.lang];
            patch({ locales: { [kind]: map } });
          },
        }),
        h('span', { class: 'small muted' }, e.locale ? `→ ${e.locale}` : t('stores.locale.none')),
      );
    });
    return h('div', { class: 'store-locales' }, ...rows);
  }

  return h(
    'div',
    { class: 'ps-stores' },
    h('p', { class: 'form-hint' }, t('stores.project.intro')),
    ...warnings,
    h('div', { class: 'section-title' }, t('stores.kind.apple')),
    appleSection(),
    h('div', { class: 'section-title' }, t('stores.kind.google')),
    googleSection(),
    h('div', { class: 'section-title' }, t('stores.locales')),
    h('p', { class: 'form-hint' }, t('stores.locales.hint').replace('{date}', info.locales.checked)),
    h('div', { class: 'store-locale-cols' }, ...KINDS.map((k) => h('div', {}, h('div', { class: 'store-locale-head' }, t(`stores.kind.${k}`)), localeTable(k)))),
  );
}
