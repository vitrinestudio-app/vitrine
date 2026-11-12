// App menu (top-bar button), its mirror in the native macOS menu bar, and
// the app-level shortcuts that also work on the start screen: ⌘N new
// project, ⌘O open, ⌘E export, ⌘, app settings (Ctrl elsewhere). In a
// browser (dev bridge) ⌘N/⌘O may be taken by the browser itself; the menu
// offers the same commands.
//
// Native menu bar (macOS app only; `menu_set`, crates/gui/src/native_menu.rs):
// the application menu (About, App settings…, Services, Hide, Quit), File
// (the in-app menu's entries), Edit (Undo/Redo = project history, or the
// focused text field's own undo; Cut/Copy/Paste/Select All native), Window
// and Help (quick guide, keyboard & mouse sheet, allow-listed store setup
// docs). The in-app menu ends with the same help entries. Clicks arrive as `menu-action`
// events and run the same `handlers` as the in-app menu. Shortcut owner:
// the shortcuts of native items (⌘N ⌘O ⌘E ⌘, ⌘Z ⇧⌘Z) belong to the native
// menu there and the JS key handlers skip them (`menuOwns` in
// shortcuts.js); everywhere else the JS handlers own them.

import { api, listen } from './api.js';
import { h, icon } from './dom.js';
import { t } from './i18n.js';
import { isMac, isTyping, keyLabel, menuOwns, setMenuOwned } from './shortcuts.js';
import { store } from './store.js';
import { invoke, isTauri } from './transport.js';

/** Entries of the in-app menu (and of the native File menu). */
const ITEMS = [
  { id: 'newProject', label: 'project.menu.new', key: 'mod+n' },
  { id: 'openProject', label: 'project.menu.open', key: 'mod+o' },
  { id: 'closeProject', label: 'project.menu.close', project: true },
  { sep: true },
  { id: 'projectSettings', label: 'project.menu.project_settings', project: true },
  { id: 'targets', label: 'project.menu.targets', project: true },
  { id: 'exportDialog', label: 'project.menu.export', key: 'mod+e', project: true },
  { id: 'uploadDialog', label: 'project.menu.upload', project: true },
  { id: 'importScreenshots', label: 'import.menu', project: true },
  { sep: true },
  { id: 'appSettings', label: 'project.menu.app_settings', key: 'mod+,' },
  { id: 'checkUpdates', label: 'update.menu', app: true },
];

/**
 * Help entries: end of the in-app menu and top of the native Help menu
 * (help/guide.js, help/sheet.js). `hint`: key shown in the in-app menu only
 * (`?` is no native accelerator; the JS handler in help/sheet.js owns it).
 */
const HELP_ITEMS = [
  { id: 'guide', label: 'guide.menu' },
  { id: 'gestures', label: 'help.gestures.menu', hint: '?' },
];

/**
 * Link at the top of the Help menu (in-app help section and native Help
 * menu; allow-listed in `open_url` as `APPLE_BEZELS_URL`).
 */
const BEZELS_LINK = {
  id: 'help:apple-bezels',
  label: 'native.help.apple_bezels',
  url: 'https://developer.apple.com/design/resources/#product-bezels',
};

/** Store docs links of the native Help menu (must be allow-listed in `open_url`). */
const HELP_LINKS = [
  BEZELS_LINK,
  {
    id: 'help:apple-keys',
    label: 'native.help.apple_keys',
    url: 'https://developer.apple.com/documentation/appstoreconnectapi/creating-api-keys-for-app-store-connect-api',
  },
  { id: 'help:google-api', label: 'native.help.google_api', url: 'https://developers.google.com/android-publisher/getting_started' },
];

/**
 * `handlers`: { newProject, openProject, projectSettings, targets, exportDialog, uploadDialog, importScreenshots,
 * appSettings, checkUpdates, closeProject, guide, gestures, hasProject() }.
 */
export function createMenu(handlers) {
  // "Check for updates…" only in the app (the dev bridge cannot update).
  const items = [...ITEMS.filter((it) => !it.app || isTauri), { sep: true }, { ...BEZELS_LINK, link: true }, ...HELP_ITEMS];
  const button = h('button', { class: 'icon-btn menu-btn', type: 'button', title: t('project.menu.title'), 'aria-haspopup': 'menu' }, icon('menu'));
  let popup = null;

  function close() {
    popup?.remove();
    popup = null;
    document.removeEventListener('mousedown', onDown, true);
    document.removeEventListener('keydown', onKey, true);
  }
  function onDown(e) {
    if (!popup?.contains(e.target) && !button.contains(e.target)) close();
  }
  function onKey(e) {
    if (e.key === 'Escape') {
      e.stopPropagation();
      close();
    }
  }
  button.addEventListener('click', () => {
    if (popup) return close();
    const hasProject = handlers.hasProject();
    popup = h(
      'div',
      { class: 'popover menu', role: 'menu' },
      items.map((it) =>
        it.sep
          ? h('div', { class: 'menu-sep' })
          : h(
              'button',
              {
                type: 'button',
                role: 'menuitem',
                class: 'menu-item',
                disabled: it.project && !hasProject,
                onclick: () => {
                  close();
                  if (it.link) api.openUrl(it.url).catch((e) => console.warn('open_url', e));
                  else handlers[it.id]?.();
                },
              },
              h('span', { class: 'grow' }, t(it.label)),
              it.key || it.hint ? h('span', { class: 'muted small' }, it.key ? keyLabel(it.key) : it.hint) : null,
            ),
      ),
    );
    document.body.append(popup);
    const r = button.getBoundingClientRect();
    popup.style.top = `${r.bottom + 4}px`;
    popup.style.left = `${r.left}px`;
    document.addEventListener('mousedown', onDown, true);
    document.addEventListener('keydown', onKey, true);
  });
  return { el: button, close };
}

/** Installs the app-level shortcuts (also active without a project). */
export function bindAppShortcuts(handlers) {
  const onKey = (e) => {
    const mod = isMac ? e.metaKey : e.ctrlKey;
    if (!mod || e.altKey || document.querySelector('dialog[open]') || menuOwns(e)) return;
    const key = e.key.toLowerCase();
    let fn = null;
    if (key === 'n' && !e.shiftKey) fn = handlers.newProject;
    else if (key === 'o' && !e.shiftKey) fn = handlers.openProject;
    else if (key === 'e' && !e.shiftKey && handlers.hasProject()) fn = handlers.exportDialog;
    else if (key === ',') fn = handlers.appSettings;
    if (fn) {
      e.preventDefault();
      fn();
    }
  };
  window.addEventListener('keydown', onKey);
  return () => window.removeEventListener('keydown', onKey);
}

/** The native menu bar spec for `menu_set` (labels in the UI language). */
function nativeSpec(hasProject) {
  const item = (id, label, key, enabled = true) => ({ type: 'item', id, label, ...(key ? { key } : {}), enabled });
  const native = (it, label) => ({ type: 'predefined', item: it, label: t(label) });
  const sep = { type: 'separator' };
  // File = the in-app entries; App settings and Check for updates move to
  // the application menu.
  const file = ITEMS.filter((it) => it.id !== 'appSettings' && it.id !== 'checkUpdates')
    .filter((it, i, all) => !(it.sep && (all[i - 1]?.sep || i === all.length - 1)))
    .map((it) => (it.sep ? sep : item(it.id, t(it.label), it.key, !it.project || hasProject)));
  const settings = ITEMS.find((it) => it.id === 'appSettings');
  return {
    menus: [
      {
        label: 'Vitrine Studio',
        items: [
          native('about', 'native.about'),
          item('checkUpdates', t('update.menu')),
          sep,
          item(settings.id, t(settings.label), settings.key),
          sep,
          native('services', 'native.services'),
          sep,
          native('hide', 'native.hide'),
          native('hide_others', 'native.hide_others'),
          native('show_all', 'native.show_all'),
          sep,
          native('quit', 'native.quit'),
        ],
      },
      { label: t('native.file'), items: file },
      {
        label: t('native.edit'),
        items: [
          item('undo', t('native.undo'), 'mod+z'),
          item('redo', t('native.redo'), 'mod+shift+z'),
          sep,
          native('cut', 'native.cut'),
          native('copy', 'native.copy'),
          native('paste', 'native.paste'),
          native('select_all', 'native.select_all'),
        ],
      },
      {
        label: t('native.window'),
        role: 'window',
        items: [native('minimize', 'native.minimize'), native('maximize', 'native.zoom'), sep, native('close_window', 'native.close_window')],
      },
      {
        label: t('native.help'),
        role: 'help',
        items: [
          item(BEZELS_LINK.id, t(BEZELS_LINK.label)),
          sep,
          ...HELP_ITEMS.map((it) => item(it.id, t(it.label))),
          sep,
          ...HELP_LINKS.filter((l) => l !== BEZELS_LINK).map((l) => item(l.id, t(l.label))),
        ],
      },
    ],
  };
}

/** Shortcuts of the spec's own items (not the predefined ones). */
function specKeys(spec) {
  return spec.menus.flatMap((m) => m.items).flatMap((it) => (it.type === 'item' && it.key ? [it.key] : []));
}

/**
 * Mirrors the app menu into the native menu bar (macOS app only; the
 * backend answers `{native: false}` elsewhere, the dev bridge has no menu).
 * `handlers` as for `createMenu`, plus `undo`/`redo` (project history).
 * Rebuilt when a project is opened or closed (enabled state); a UI
 * language switch reloads the page and so rebuilds it too. Resolves to
 * true when the native menu is active.
 */
export async function initNativeMenu(handlers) {
  if (!isTauri) return false;
  const send = async (hasProject) => {
    const spec = nativeSpec(hasProject);
    const reply = await invoke('menu_set', spec);
    setMenuOwned(reply?.native ? specKeys(spec) : []);
    return Boolean(reply?.native);
  };
  try {
    if (!(await send(handlers.hasProject()))) return false;
  } catch (e) {
    console.warn('menu_set', e);
    return false;
  }
  store.select(
    (st) => Boolean(st.summary),
    (hasProject) => send(hasProject).catch((e) => console.warn('menu_set', e)),
  );
  await listen('menu-action', (id) => runNative(handlers, id));
  return true;
}

/** Runs a native menu item. */
function runNative(handlers, id) {
  if (id === 'undo' || id === 'redo') {
    // In a text field: its own undo/redo; else the project history.
    if (isTyping({ target: document.activeElement })) document.execCommand(id);
    else handlers[id]?.();
    return;
  }
  const link = HELP_LINKS.find((l) => l.id === id);
  if (link) {
    api.openUrl(link.url).catch((e) => console.warn('open_url', e));
    return;
  }
  // Help opens on top of anything (also without a project).
  if (HELP_ITEMS.some((x) => x.id === id)) {
    handlers[id]?.();
    return;
  }
  // Like the in-app shortcuts: nothing while a dialog is open.
  if (document.querySelector('dialog[open]')) return;
  const it = ITEMS.find((x) => x.id === id);
  if (!it || (it.project && !handlers.hasProject())) return;
  handlers[id]?.();
}
