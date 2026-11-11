// Start screen (task 2.10), shown while no project is open: new project
// (name, folder, template, languages), open…, recent projects (missing ones
// greyed, removable), and in Tauri dropping a folder onto the window.
// `openNewProject` / `openProjectDialog` are also used by the app menu.

import { api, listen } from './api.js';
import { h, icon } from './dom.js';
import { errorLine, input, openModal, row, select } from './dialogs/modal.js';
import { t } from './i18n.js';
import { isTauri } from './transport.js';

let onOpenedHandler = async () => {};

/** Called with the project summary whenever a project was opened from here. */
export function setOnOpened(fn) {
  onOpenedHandler = fn;
}

async function opened(summary) {
  if (summary) await onOpenedHandler(summary);
}

/** Native folder dialog → open project. */
export async function openProjectDialog() {
  return opened(await api.openDialog());
}

function joinPath(dir, name) {
  const sep = dir.includes('\\') && !dir.includes('/') ? '\\' : '/';
  return dir.endsWith(sep) ? `${dir}${name}` : `${dir}${sep}${name}`;
}

const folderName = (name) => name.trim().replace(/[/\\:*?"<>|]+/g, '-');

/** The "New project" dialog. */
export async function openNewProject() {
  let settings = { default_project_language: 'en' };
  try {
    settings = (await api.settingsGet()).settings;
  } catch {
    /* defaults */
  }
  const err = errorLine();
  let name = '';
  let location = '';
  let template = 'phone';
  let langs = settings.default_project_language;
  const target = h('div', { class: 'mono small muted path-preview' });
  const createBtn = h('button', { class: 'btn btn-primary', type: 'button', onclick: () => create() }, t('start.create'));
  const locInput = input({
    placeholder: t('start.location.placeholder'),
    cls: 'grow',
    oninput: (v) => {
      location = v.trim();
      update();
    },
  });
  function dir() {
    return location && name.trim() ? joinPath(location, folderName(name)) : '';
  }
  function update() {
    target.textContent = dir() || t('start.location.missing');
    target.classList.toggle('mono', Boolean(dir()));
    createBtn.disabled = !dir();
  }
  async function create() {
    createBtn.disabled = true;
    try {
      const list = langs.split(/[\s,]+/).filter(Boolean);
      const summary = await api.projectNew({ dir: dir(), name: name.trim(), template, langs: list });
      m.close();
      await opened(summary);
    } catch (e) {
      err.show(e);
      createBtn.disabled = false;
    }
  }
  const m = openModal({
    title: t('start.new'),
    size: 'medium',
    body: [
      err.el,
      h(
        'div',
        { class: 'form' },
        row(
          t('start.name'),
          input({
            placeholder: t('start.name.placeholder'),
            oninput: (v) => {
              name = v;
              update();
            },
          }),
        ),
        row(
          t('start.location'),
          h(
            'div',
            { class: 'inline' },
            locInput,
            h(
              'button',
              {
                class: 'btn btn-small',
                type: 'button',
                onclick: async () => {
                  try {
                    const d = await api.pickFolder(t('start.location.pick'));
                    if (d) {
                      location = d;
                      locInput.value = d;
                      update();
                    }
                  } catch (e) {
                    err.show(e);
                  }
                },
              },
              t('export.folder.choose'),
            ),
          ),
          target,
        ),
        row(
          t('start.template'),
          select({
            options: [
              { value: 'phone', label: t('start.template.phone') },
              { value: 'minimal', label: t('start.template.minimal') },
            ],
            value: template,
            onchange: (v) => (template = v),
          }),
        ),
        row(t('start.langs'), input({ value: langs, cls: 'mono', onchange: (v) => (langs = v) }), t('start.langs.hint')),
      ),
    ],
    footer: [h('span', { class: 'spacer' }), h('button', { class: 'btn', type: 'button', onclick: () => m.close() }, t('project.dialog.cancel')), createBtn],
  });
  update();
  m.el.querySelector('input')?.focus();
  return m;
}

/** Renders the start screen into `root` (the `#welcome` element). */
export function mountStart(root, { version, onSettings }) {
  const err = errorLine();
  const recent = h('ul', { class: 'start-recent' });

  async function renderRecent() {
    let list = [];
    try {
      list = await api.recentProjects();
    } catch (e) {
      err.show(e);
    }
    if (!list.length) {
      recent.replaceChildren(h('li', { class: 'placeholder' }, t('start.recent.none')));
      return;
    }
    recent.replaceChildren(
      ...list.map((p) =>
        h(
          'li',
          { class: `start-item ${p.exists ? '' : 'missing'}`, title: p.exists ? p.path : `${p.path} – ${t('start.recent.missing')}` },
          h(
            'button',
            {
              class: 'start-open',
              type: 'button',
              disabled: !p.exists,
              onclick: async () => {
                try {
                  await opened(await api.projectOpen(p.path));
                } catch (e) {
                  err.show(e);
                }
              },
            },
            h('span', { class: 'start-name' }, p.name),
            h('span', { class: 'start-path mono' }, p.path),
            p.exists ? null : h('span', { class: 'tag' }, t('start.recent.missing')),
          ),
          h(
            'button',
            {
              class: 'icon-btn',
              type: 'button',
              title: t('start.recent.remove'),
              onclick: async () => {
                await api.recentRemove(p.path).catch((e) => err.show(e));
                renderRecent();
              },
            },
            icon('close'),
          ),
        ),
      ),
    );
  }

  root.replaceChildren(
    h(
      'div',
      { class: 'start' },
      h(
        'div',
        { class: 'start-brand' },
        h('span', { class: 'start-logo' }, icon('logo')),
        h('h1', {}, t('app.title')),
        h('p', { class: 'muted' }, t('app.tagline')),
        h(
          'div',
          { class: 'start-actions' },
          h('button', { class: 'btn btn-primary btn-large', type: 'button', onclick: () => openNewProject() }, icon('plus'), t('start.new')),
          h(
            'button',
            {
              class: 'btn btn-large',
              type: 'button',
              id: 'open-project',
              onclick: () => openProjectDialog().catch((e) => err.show(e)),
            },
            t('project.open'),
          ),
        ),
        err.el,
        h('button', { class: 'btn btn-small start-settings', type: 'button', onclick: () => onSettings?.() }, t('project.menu.app_settings')),
        h('p', { class: 'muted small' }, isTauri ? t('start.drop') : ''),
        h('p', { class: 'muted small' }, `${t('app.version')} ${version}`),
      ),
      h('div', { class: 'start-side' }, h('div', { class: 'section-title' }, t('start.recent')), recent),
    ),
  );
  renderRecent();
  return { refresh: renderRecent };
}

/** Tauri: dropping a folder onto the window opens it as a project. */
export async function bindFolderDrop() {
  if (!isTauri) return;
  await listen('tauri://drag-drop', async (p) => {
    const path = p?.paths?.[0];
    // Only on the start screen: with a project open, drops are asset imports.
    if (!path || document.getElementById('welcome')?.hidden) return;
    try {
      await opened(await api.projectOpen(path));
    } catch (e) {
      window.dispatchEvent(new CustomEvent('vitrine:error', { detail: e }));
    }
  });
}
