// Updates and the command line tool (ADR 0026).
//
// Updates (GUI only, `update_*` commands in crates/gui/src/updater.rs): at
// start the app checks GitHub Releases once a day when the app setting
// `update_check` is on; "Check for updates…" (app menu, native app menu,
// app settings) checks right away. A found update opens a dialog: Later /
// Release notes / Install and restart (download with progress, signature
// checked by the updater, then restart). deb/rpm installs only get the
// notice with a download link; dev/debug builds and builds without the
// real public key get no updates at all. Nothing is installed without the
// user's click.
//
// Command line tool (`cli_install_*`, crates/app/src/cli_install.rs):
// app settings rows to install `vitrine` for terminals.

import { api, listen } from './api.js';
import { h, icon } from './dom.js';
import { t } from './i18n.js';
import { checkbox, choiceDialog, confirmDialog, openModal, row } from './dialogs/modal.js';
import { isTauri } from './transport.js';

/** True when this front end can update itself (the Tauri app). */
export const canUpdate = isTauri;

function fmt(key, vars = {}) {
  return Object.entries(vars).reduce((s, [k, v]) => s.replace(`{${k}}`, String(v)), t(key));
}

function info(title, message) {
  return choiceDialog({ title, body: h('p', { class: 'dlg-text' }, message), buttons: [{ label: t('project.dialog.ok'), primary: true, value: null }] });
}

/** Automatic check at start (when due and switched on). Quiet on errors. */
export async function initUpdates() {
  if (!canUpdate) return;
  // Let the start screen / project load first.
  await new Promise((r) => setTimeout(r, 3000));
  try {
    const reply = await api.updateCheck(true);
    if (reply.update) await showUpdate(reply);
  } catch (e) {
    console.warn('update_check', e);
  }
}

/** "Check for updates…": checks now and always answers. */
export async function checkForUpdates() {
  const title = t('update.title');
  if (!canUpdate) return info(title, t('update.reason.bridge'));
  let status;
  try {
    status = await api.updateStatus();
  } catch (e) {
    return info(title, e.message);
  }
  if (status.mode === 'off') return info(title, fmt(`update.reason.${status.reason}`, { version: status.current_version }));
  try {
    const reply = await api.updateCheck(false);
    if (reply.update) return showUpdate(reply, status);
    return info(title, fmt('update.none', { version: status.current_version }));
  } catch (e) {
    return info(title, `${t('update.error')}: ${e.message}`);
  }
}

/** The "update available" dialog. */
async function showUpdate(reply, status = null) {
  const st = status ?? (await api.updateStatus());
  const u = reply.update;
  const notes = (u.notes ?? '').trim();
  const body = h(
    'div',
    { class: 'update-dialog' },
    h('p', { class: 'dlg-text' }, fmt('update.available', { version: u.version, current: u.current_version })),
    u.date ? h('div', { class: 'form-hint' }, fmt('update.date', { date: new Date(u.date).toLocaleDateString() })) : null,
    notes ? h('pre', { class: 'update-notes small selectable' }, notes) : null,
    h('p', { class: 'form-hint' }, t(reply.mode === 'install' ? 'update.install.hint' : 'update.notify.hint')),
  );
  const buttons = [
    { label: t('update.later'), value: null },
    { label: t('update.notes'), value: 'notes' },
    reply.mode === 'install' ? { label: t('update.install'), primary: true, value: 'install' } : { label: t('update.download'), primary: true, value: 'notes' },
  ];
  const choice = await choiceDialog({ title: t('update.title'), size: 'medium', body, buttons });
  if (choice === 'notes') {
    api.openUrl(st.releases_url).catch((e) => console.warn('open_url', e));
  } else if (choice === 'install') {
    await install(u);
  }
}

/** Downloads + installs with a progress bar; the app restarts on success. */
async function install(u) {
  const bar = h('progress', { class: 'update-progress', max: 1, value: 0 });
  const text = h('div', { class: 'form-hint' }, t('update.downloading'));
  const m = openModal({ title: fmt('update.installing', { version: u.version }), size: 'small', dismissable: false, body: [bar, text] });
  const unlisten = await listen('update-progress', ({ downloaded, total }) => {
    if (total) {
      bar.max = total;
      bar.value = downloaded;
      text.textContent = `${t('update.downloading')} ${Math.round((downloaded / total) * 100)} %`;
    } else {
      bar.removeAttribute('value');
    }
  });
  try {
    await api.updateInstall();
    // Normally not reached: the app restarts (Windows: the installer runs).
    text.textContent = t('update.restarting');
  } catch (e) {
    m.close();
    await info(t('update.title'), `${t('update.error')}: ${e.message}`);
  } finally {
    unlisten?.();
  }
}

/** App settings rows: automatic check switch, "Check now", state. */
export function updateRows(save, current) {
  const state = h('div', { class: 'small muted' });
  const refresh = async () => {
    if (!canUpdate) {
      state.textContent = t('update.reason.bridge');
      return;
    }
    try {
      const st = await api.updateStatus();
      const parts = [fmt('update.version', { version: st.current_version })];
      if (st.mode === 'off') parts.push(fmt(`update.reason.${st.reason}`, { version: st.current_version }));
      else if (st.checked_at) parts.push(fmt('update.last', { time: new Date(st.checked_at * 1000).toLocaleString() }));
      state.textContent = parts.join(' · ');
    } catch (e) {
      state.textContent = e.message;
    }
  };
  refresh();
  return [
    h('div', { class: 'form-section' }, icon('update'), t('update.settings.title')),
    row(
      t('update.settings.auto'),
      h(
        'div',
        { class: 'assistant-toggle' },
        checkbox({ label: t('update.settings.auto.on'), checked: current().update_check !== false, onchange: (v) => save({ update_check: v }) }),
        h('div', { class: 'fp-settings-row' }, h('button', { class: 'btn', type: 'button', onclick: () => checkForUpdates().then(refresh) }, t('update.menu')), state),
      ),
      t('update.settings.auto.hint'),
    ),
  ];
}

function cliText(st) {
  const loc = st.location ?? '';
  switch (st.method) {
    case 'package':
      return fmt('cli.state.package', { path: loc });
    case 'unavailable':
      return t('cli.state.unavailable');
    case 'user_path':
      return st.installed ? fmt('cli.state.path_on', { path: loc }) : fmt('cli.state.path_off', { path: loc });
    default:
      if (st.installed) return fmt('cli.state.link_on', { path: loc });
      if (st.conflict) return fmt('cli.state.conflict', { path: loc });
      return fmt('cli.state.link_off', { path: loc });
  }
}

/** App settings rows: "Command line tool" (install / remove `vitrine`). */
export function cliRows() {
  const box = h('div', { class: 'assistant-toggle' });
  const render = (st, error) => {
    const items = [h('div', { class: 'small' }, cliText(st))];
    if (!st.cli_exists && st.method !== 'unavailable') items.push(h('div', { class: 'form-hint warn' }, fmt('mcp.settings.cli_missing', { path: st.cli_path })));
    if (error) items.push(h('div', { class: 'form-error' }, error));
    if (st.method === 'symlink' || st.method === 'user_path') {
      const remove = st.installed;
      items.push(
        h(
          'div',
          {},
          h(
            'button',
            {
              class: 'btn',
              type: 'button',
              disabled: (!remove && (!st.cli_exists || st.conflict)) || undefined,
              onclick: () => change(st, remove),
            },
            t(remove ? 'cli.remove' : 'cli.install'),
          ),
        ),
      );
    }
    box.replaceChildren(...items);
  };
  async function change(st, remove) {
    const ok = await confirmDialog({
      title: t('cli.title'),
      message: fmt(remove ? `cli.confirm.remove.${st.method}` : `cli.confirm.install.${st.method}`, { path: st.location ?? '', cli: st.cli_path }),
      confirm: t(remove ? 'cli.remove' : 'cli.install'),
    });
    if (!ok) return;
    try {
      render(await api.cliInstall(remove));
    } catch (e) {
      render(st, e.code === 'cancelled' ? null : e.message);
    }
  }
  api
    .cliInstallStatus()
    .then((st) => render(st))
    .catch((e) => box.replaceChildren(h('div', { class: 'form-error' }, e.message)));
  return [h('div', { class: 'form-section' }, icon('terminal'), t('cli.title')), row(t('cli.row'), box, t('cli.hint'))];
}
