// App menu (top-bar button) and the app-level shortcuts that also work on
// the start screen: ⌘N new project, ⌘O open, ⌘E export, ⌘, app settings
// (Ctrl elsewhere). In a browser (dev bridge) ⌘N/⌘O may be taken by the
// browser itself; the menu offers the same commands.

import { h, icon } from './dom.js';
import { t } from './i18n.js';
import { isMac, keyLabel } from './shortcuts.js';

/**
 * `handlers`: { newProject, openProject, projectSettings, targets, exportDialog,
 * appSettings, closeProject, hasProject() }.
 */
export function createMenu(handlers) {
  const items = [
    { id: 'newProject', label: 'project.menu.new', key: 'mod+n' },
    { id: 'openProject', label: 'project.menu.open', key: 'mod+o' },
    { sep: true },
    { id: 'projectSettings', label: 'project.menu.project_settings', project: true },
    { id: 'targets', label: 'project.menu.targets', project: true },
    { id: 'exportDialog', label: 'project.menu.export', key: 'mod+e', project: true },
    { sep: true },
    { id: 'appSettings', label: 'project.menu.app_settings', key: 'mod+,' },
    { sep: true },
    { id: 'closeProject', label: 'project.menu.close', project: true },
  ];
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
                  handlers[it.id]?.();
                },
              },
              h('span', { class: 'grow' }, t(it.label)),
              it.key ? h('span', { class: 'muted small' }, keyLabel(it.key)) : null,
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
    if (!mod || e.altKey || document.querySelector('dialog[open]')) return;
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
