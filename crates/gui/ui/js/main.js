// Entry point: theme, UI language, backend connection, shell, shortcuts.
//
// Debug query parameters (dev bridge): `?theme=dark|light` forces a theme,
// `?lang=de|en` the UI language.

import { actions } from './actions.js';
import { api, listen } from './api.js';
import { editor } from './editor.js';
import { applyTranslations, initI18n } from './i18n.js';
import { mountShell } from './shell.js';
import { bindShortcuts } from './shortcuts.js';
import { store } from './store.js';
import { isTauri } from './transport.js';

const params = new URLSearchParams(location.search);
const theme = params.get('theme');
if (theme === 'dark' || theme === 'light') document.documentElement.dataset.theme = theme;

let shell = null;

async function showProject() {
  const open = await editor.load();
  document.getElementById('app').hidden = !open;
  document.getElementById('welcome').hidden = open;
  if (open && !shell) {
    shell = mountShell();
    bindShortcuts({
      ...actions,
      reorder: (order) => actions.reorder(order),
      nudge: (dx, dy) => shell.canvas?.nudge(dx, dy),
      escape: () => {
        if (!shell.canvas?.cancel()) actions.clearSelection();
      },
      zoomIn: () => shell.canvas?.zoomIn(),
      zoomOut: () => shell.canvas?.zoomOut(),
      fit: () => shell.canvas?.fit(),
      zoomActual: () => shell.canvas?.zoomActual(),
    });
  }
}

async function start() {
  await initI18n(params.get('lang') ?? undefined);
  applyTranslations();
  const info = await api.appInfo();
  document.getElementById('app-version').textContent = `${info.version}${isTauri ? '' : ' · dev bridge'}`;
  document.getElementById('open-project').addEventListener('click', async () => {
    try {
      const s = await api.openDialog();
      if (s) await showProject();
    } catch (e) {
      editor.toast(e.message, true);
    }
  });
  await listen('project-changed', (change) => {
    if (store.get().summary) editor.externalChange(change).catch(report);
    else showProject().catch(report);
  });
  await showProject();
}

function report(error) {
  console.error(error);
  editor.toast(`${error.code ?? 'error'}: ${error.message}`, true);
}

window.addEventListener('unhandledrejection', (e) => report(e.reason));
start().catch(report);
