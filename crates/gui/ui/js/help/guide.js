// Quick guide "How Vitrine Studio is organised": one screen for users who
// do not read manuals. A diagram-like overview (project → canvases →
// targets × languages × variants → images), what is managed centrally
// (project settings, app settings) and where each thing is edited. Labels
// of the places come from the UI's own i18n keys, so the guide follows
// renames of the controls it points to.
//
// Shown from the Help menu (in-app + native macOS), a link on the start
// screen, and once on the first project open (`maybeShowGuide`; remembered
// per viewer in the app setting `guide_seen`, not per project).
// Styles: styles/help.css.

import { api } from '../api.js';
import { h, icon } from '../dom.js';
import { t } from '../i18n.js';
import { isTauri } from '../transport.js';
import { openModal } from '../dialogs/modal.js';
import { openGestureSheet } from './sheet.js';

/** `[place i18n key, UI label i18n key | null]` → "Top bar: Target". */
function where(list) {
  return h(
    'ul',
    { class: 'guide-where' },
    list.map(([place, label]) => h('li', {}, h('span', { class: 'guide-place' }, t(place)), label ? h('span', { class: 'guide-ui' }, t(label)) : null)),
  );
}

/** One box of the diagram. */
function card(cls, iconName, title, text, places) {
  return h(
    'div',
    { class: `guide-card ${cls}` },
    h('div', { class: 'guide-card-head' }, iconName ? icon(iconName) : null, h('span', {}, t(title))),
    h('p', { class: 'guide-card-text' }, t(text)),
    places ? where(places) : null,
  );
}

/** Operator between boxes of the formula row. */
const op = (sym) => h('div', { class: 'guide-op', 'aria-hidden': 'true' }, sym);

let open = null;

/** Opens the quick guide. `onClose` runs after it closes. */
export function openGuide({ onClose } = {}) {
  if (open) return open;
  const formula = h(
    'div',
    { class: 'guide-formula' },
    card('guide-canvas', 'canvas', 'guide.canvas', 'guide.canvas.text', [
      ['guide.place.topbar', 'top.canvas'],
      ['guide.place.sidebar', 'panel.canvases'],
      ['guide.place.menu', 'project.menu.targets'],
      ['guide.place.inspector', 'guide.inspector.nothing'],
    ]),
    op('→'),
    card('guide-target', 'targets', 'guide.target', 'guide.target.text', [
      ['guide.place.topbar', 'top.target'],
      ['guide.place.menu', 'project.menu.targets'],
    ]),
    op('×'),
    card('guide-lang', 'languages', 'guide.lang', 'guide.lang.text', [
      ['guide.place.topbar', 'top.lang'],
      ['guide.place.sidebar', 'panel.translations'],
      ['guide.place.project', 'project.tab.languages'],
    ]),
    op('×'),
    card('guide-variant', 'pages', 'guide.variant', 'guide.variant.text', [
      ['guide.place.topbar', 'top.variant'],
      ['guide.place.menu', 'project.menu.targets'],
    ]),
    op('='),
    card('guide-output', 'export', 'guide.output', 'guide.output.text', [
      ['guide.place.view', 'view.pages'],
      ['guide.place.view', 'view.gallery'],
      ['guide.place.menu', 'project.menu.export'],
      ['guide.place.menu', 'project.menu.upload'],
    ]),
  );
  const elements = card('guide-elements', 'layers', 'guide.elements', 'guide.elements.text', [
    ['guide.place.canvas', null],
    ['guide.place.topbar', 'add.button'],
    ['guide.place.sidebar', 'panel.layers'],
    ['guide.place.sidebar', 'panel.assets'],
    ['guide.place.right', 'panel.inspector'],
  ]);
  const project = h(
    'div',
    { class: 'guide-project' },
    h('div', { class: 'guide-project-head' }, h('strong', {}, t('guide.project')), h('span', { class: 'muted small' }, t('guide.project.text'))),
    formula,
    elements,
  );
  const central = h(
    'div',
    { class: 'guide-central' },
    card('guide-settings', null, 'guide.project_settings', 'guide.project_settings.text', [
      ['guide.place.menu', 'project.menu.project_settings'],
      ['guide.place.tabs', 'guide.project_settings.tabs'],
    ]),
    card('guide-settings', null, 'guide.app_settings', 'guide.app_settings.text', [
      ['guide.place.menu', 'project.menu.app_settings'],
      ['guide.place.sections', 'guide.app_settings.sections'],
    ]),
  );
  const done = h('button', { class: 'btn btn-primary', type: 'button', onclick: () => m.close() }, t('guide.done'));
  const m = openModal({
    title: t('guide.title'),
    size: 'large',
    cls: 'help-dlg guide-dlg',
    body: [h('p', { class: 'guide-intro' }, t('guide.intro')), project, central, h('p', { class: 'muted small' }, t('guide.more'))],
    footer: [
      h(
        'button',
        {
          class: 'btn',
          type: 'button',
          onclick: () => {
            m.close();
            openGestureSheet();
          },
        },
        t('help.gestures.menu'),
      ),
      h('span', { class: 'spacer' }),
      done,
    ],
    onClose: () => {
      open = null;
      onClose?.();
    },
  });
  open = m;
  done.focus();
  return m;
}

/** Shown in this session already (avoids a second automatic show). */
let shownAuto = false;

/**
 * Shows the guide once on the first project open of this viewer (app
 * setting `guide_seen`); closing it sets the setting. Nothing while
 * another dialog is open. Only in the app: the dev bridge (often with a
 * fresh `--app-dir`, driven by scripts) shows it automatically only with
 * the debug parameter `?guide=first`.
 */
export async function maybeShowGuide() {
  if (shownAuto || document.querySelector('dialog[open]')) return;
  if (!isTauri && new URLSearchParams(location.search).get('guide') !== 'first') return;
  let seen = true;
  try {
    seen = Boolean((await api.settingsGet()).settings?.guide_seen);
  } catch {
    return;
  }
  if (seen || shownAuto) return;
  shownAuto = true;
  openGuide({
    onClose: () => api.settingsSet({ guide_seen: true }).catch((e) => console.warn('settings_set', e)),
  });
}
