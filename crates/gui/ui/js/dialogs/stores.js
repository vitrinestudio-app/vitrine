// Store upload configuration (Phase 5.1, ADR 0016).
//
// - App settings → "Stores": app-wide accounts. Keys are imported from a
//   file picked in a native dialog; the backend validates the file and puts
//   the secret into the system keychain (the UI never sees key material).
//   Test asks the store who we are (`not_implemented` until the store
//   clients are wired up), remove deletes the keychain entry after a
//   confirmation. "How do I get this?" panels (also in the import dialog)
//   explain how to create the keys; links open in the system browser via
//   `open_url` (allow-listed in crates/app/src/commands/os.rs).
// - Project settings → "Stores" tab: Apple app id / bundle id / platform,
//   Google package name, account per store, language → store locale map
//   with the built-in defaults shown. Every change is one
//   `project_store_patch` (one undo step). "Choose app…" lists the apps the
//   Apple key can see (`store_apps_list`, cached per session) and fills
//   Apple ID + bundle id; Google Play has no app list, so the package name
//   is typed and "Check" verifies it (`store_package_check`).

import { api } from '../api.js';
import { h, icon } from '../dom.js';
import { t } from '../i18n.js';
import { choiceDialog, confirmDialog, input, row, select } from './modal.js';

const KINDS = ['apple', 'google'];

/** Steps and links of the "How do I get this?" panels (texts: `stores.help.<kind>.<n>`). */
const HELP = {
  apple: {
    steps: 6,
    links: [
      ['stores.help.link.asc_keys', 'https://appstoreconnect.apple.com/access/integrations/api'],
      ['stores.help.link.apple_docs', 'https://developer.apple.com/documentation/appstoreconnectapi/creating-api-keys-for-app-store-connect-api'],
    ],
  },
  google: {
    steps: 8,
    links: [
      ['stores.help.link.enable_api', 'https://console.cloud.google.com/apis/library/androidpublisher.googleapis.com'],
      ['stores.help.link.service_accounts', 'https://console.cloud.google.com/iam-admin/serviceaccounts'],
      ['stores.help.link.play_console', 'https://play.google.com/console'],
      ['stores.help.link.google_docs', 'https://developers.google.com/android-publisher/getting_started'],
    ],
  },
};

/** Collapsible "How do I get this?" panel for `kind`. */
export function storeHelp(kind, { open = false } = {}) {
  const help = HELP[kind];
  const err = h('div', { class: 'form-error', role: 'alert', hidden: true });
  const link = ([key, url]) =>
    h(
      'button',
      {
        class: 'btn btn-small store-help-link',
        type: 'button',
        title: url,
        onclick: async () => {
          err.hidden = true;
          try {
            await api.openUrl(url);
          } catch (e) {
            err.textContent = `${e.message ?? e} – ${url}`;
            err.hidden = false;
          }
        },
      },
      t(key),
    );
  return h(
    'details',
    { class: 'store-help', open },
    h('summary', {}, t(`stores.help.${kind}.title`)),
    h('ol', { class: 'store-help-steps' }, Array.from({ length: help.steps }, (_, i) => h('li', {}, t(`stores.help.${kind}.${i + 1}`)))),
    h('p', { class: 'form-hint' }, t(`stores.help.${kind}.note`)),
    h('div', { class: 'store-help-links' }, help.links.map(link)),
    err,
  );
}

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
    h('div', { class: 'form-section store-section' }, icon('export'), t('stores.title')),
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
        ...KINDS.map((k) => storeHelp(k)),
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
  fields.push(storeHelp(kind), err);
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

/** Accounts whose app list was loaded this session (re-render shows it again from the cache). */
const pickerLoaded = new Set();

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
      row(t('stores.apple.app'), applePicker(a), t('stores.apple.app.hint')),
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

  /** "Choose app…": the apps the account's key can see → Apple ID + bundle id. */
  function applePicker(a) {
    const account = a.account && accounts('apple').some((x) => x.id === a.account) ? a.account : null;
    const box = h('div', { class: 'store-picker' });
    const status = h('div', { class: 'store-check small', role: 'status' });
    if (!account) {
      box.append(h('span', { class: 'muted small' }, t('stores.apple.app.need_account')));
      return box;
    }
    const load = async (refresh) => {
      status.dataset.state = 'busy';
      status.textContent = t('stores.apple.app.loading');
      try {
        const r = await api.storeAppsList(account, refresh);
        pickerLoaded.add(account);
        show(r.apps);
      } catch (e) {
        status.dataset.state = 'error';
        status.textContent = e.message ?? String(e);
      }
    };
    const show = (apps) => {
      status.textContent = '';
      delete status.dataset.state;
      if (!apps.length) {
        status.dataset.state = 'warn';
        status.textContent = t('stores.apple.app.none');
      }
      const current = apps.find((x) => x.id === a.app_id || (!a.app_id && x.bundle_id === a.bundle_id));
      const options = [{ value: '', label: current ? '' : t('stores.apple.app.choose') }, ...apps.map((x) => ({ value: x.id, label: `${x.name} – ${x.bundle_id}` }))].filter((o) => o.label);
      const sel = select({
        options,
        value: current?.id ?? '',
        onchange: (id) => {
          const app = apps.find((x) => x.id === id);
          if (app) patch({ apple: { ...a, app_id: app.id, bundle_id: app.bundle_id } });
        },
      });
      const facts = current ? h('span', { class: 'muted small mono selectable' }, [current.sku ? `SKU ${current.sku}` : null, current.primary_locale].filter(Boolean).join(' · ')) : null;
      box.replaceChildren(
        sel,
        h('button', { class: 'icon-btn', type: 'button', title: t('stores.apple.app.refresh'), onclick: () => load(true) }, icon('reset')),
        facts ?? '',
        status,
      );
    };
    if (pickerLoaded.has(account)) load(false);
    else box.append(h('button', { class: 'btn btn-small', type: 'button', onclick: () => load(false) }, t('stores.apple.app.load')), status);
    return box;
  }

  /** "Check" next to the Play package name (temporary edit, deleted again). */
  function packageCheck(g) {
    const status = h('div', { class: 'store-check small', role: 'status' });
    const account = g.account && accounts('google').some((x) => x.id === g.account) ? g.account : null;
    const btn = h(
      'button',
      {
        class: 'btn btn-small',
        type: 'button',
        disabled: !account || !g.package_name,
        title: !account ? t('stores.google.check.need_account') : !g.package_name ? t('stores.google.check.need_package') : t('stores.google.check.hint'),
        onclick: async () => {
          btn.disabled = true;
          status.dataset.state = 'busy';
          status.textContent = t('stores.google.check.running');
          try {
            const r = await api.storePackageCheck(account, g.package_name);
            status.dataset.state = 'ok';
            status.textContent = r.title ? t('stores.google.check.ok_title').replace('{title}', r.title).replace('{lang}', r.default_language ?? '') : t('stores.google.check.ok');
          } catch (e) {
            status.dataset.state = 'error';
            status.textContent = e.message ?? String(e);
          }
          btn.disabled = false;
        },
      },
      t('stores.google.check'),
    );
    return h('div', { class: 'store-picker' }, btn, status);
  }

  function googleSection() {
    const g = s.google;
    if (!g) return setupButton('google');
    const set = (field, v) => patch({ google: { ...g, [field]: v === '' ? null : v } });
    return h(
      'div',
      { class: 'form' },
      row(t('stores.account'), accountSelect('google', g.account, (v) => set('account', v)), accounts('google').length ? null : t('stores.account.add_hint')),
      row(t('stores.google.package_name'), input({ value: g.package_name ?? '', cls: 'mono', placeholder: 'com.example.app', onchange: (v) => set('package_name', v.trim()) }), t('stores.google.package_name.hint')),
      row('', packageCheck(g), t('stores.google.check.note')),
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
