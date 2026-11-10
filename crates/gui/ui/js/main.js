// Entry point: theme, UI language, backend connection, start screen,
// shell, app menu and shortcuts.
//
// Debug query parameters (dev bridge): `?theme=dark|light` forces a theme,
// `?lang=de|en` the UI language (otherwise the app setting, else the system).

import { actions } from './actions.js';
import { api, listen } from './api.js';
import { createClaudeIndicator, initClaude } from './claude.js';
import { openAppSettings } from './dialogs/app-settings.js';
import { openExport } from './dialogs/export.js';
import { openScreenshotImport } from './dialogs/screenshot-import.js';
import { openProjectSettings } from './dialogs/project-settings.js';
import { openTargets } from './dialogs/targets.js';
import { editor } from './editor.js';
import { applyTranslations, initI18n } from './i18n.js';
import { bindAppShortcuts, createMenu } from './menu.js';
import { installDropImport } from './panels/importer.js';
import { mountShell } from './shell.js';
import { bindShortcuts } from './shortcuts.js';
import { bindFolderDrop, mountStart, openNewProject, openProjectDialog, setOnOpened } from './start.js';
import { store } from './store.js';
import { isTauri } from './transport.js';

const params = new URLSearchParams(location.search);
const theme = params.get('theme');
if (theme === 'dark' || theme === 'light') document.documentElement.dataset.theme = theme;

/** View context kept across a UI reload (language switch). */
const CONTEXT_KEY = 'arrangia.context';

let shell = null;
let startScreen = null;

const appActions = {
  newProject: () => openNewProject().catch(report),
  openProject: () => openProjectDialog().catch(report),
  projectSettings: () => openProjectSettings(),
  targets: () => openTargets().catch(report),
  exportDialog: () => openExport(),
  importScreenshots: () => openScreenshotImport().catch(report),
  appSettings: () => openAppSettings().catch(report),
  closeProject: () => closeProject().catch(report),
  hasProject: () => Boolean(store.get().summary),
};

async function showProject() {
  const open = await editor.load();
  document.getElementById('app').hidden = !open;
  document.getElementById('welcome').hidden = open;
  if (!open) startScreen?.refresh();
  if (open && !shell) {
    shell = mountShell({
      menu: createMenu(appActions).el,
      targets: appActions.targets,
      exportDialog: appActions.exportDialog,
      claude: createClaudeIndicator({ openSettings: appActions.appSettings }),
    });
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
    installDropImport(() => shell.canvas);
  }
}

async function closeProject() {
  await api.projectClose();
  await projectClosed();
}

/** The backend has no project any more (closed here or by Claude). */
async function projectClosed() {
  store.set({ summary: null, overlay: null, strip: null, target: null, variant: undefined, selection: [], hover: null, validation: null });
  await showProject();
}

/** A project was opened (start screen, menu, or by Claude over MCP). */
async function projectOpened() {
  store.set({ selection: [], hover: null, variant: undefined, strip: null, target: null, lang: null, validation: null });
  await showProject();
}

/** Debounce of `view_set` after context/selection changes (ms). */
const VIEW_SYNC_MS = 250;

/**
 * Pushes the view context and selection to the backend (`view_set`), so
 * other front ends (MCP `view_get`) know what the user is looking at.
 */
function syncView() {
  let timer = null;
  const push = () => {
    timer = null;
    const st = store.get();
    if (!st.summary) return;
    const view = { strip: st.strip, target: st.target, lang: st.lang, selection: st.selection };
    if (st.variant !== undefined) view.variant = st.variant;
    api.viewSet(view).catch((e) => console.warn('view_set', e));
  };
  store.select(
    (st) => [st.summary, st.strip, st.target, st.lang, st.variant, st.selection],
    () => {
      clearTimeout(timer);
      timer = setTimeout(push, VIEW_SYNC_MS);
    },
  );
}

function restoreContext() {
  try {
    const saved = JSON.parse(sessionStorage.getItem(CONTEXT_KEY) ?? 'null');
    sessionStorage.removeItem(CONTEXT_KEY);
    if (saved) store.set(saved);
  } catch {
    /* not essential */
  }
  window.addEventListener('arrangia:before-reload', () => {
    const { strip, target, lang, view } = store.get();
    try {
      sessionStorage.setItem(CONTEXT_KEY, JSON.stringify({ strip, target, lang, view }));
    } catch {
      /* not essential */
    }
  });
}

async function start() {
  let settings = {};
  try {
    settings = (await api.settingsGet()).settings;
  } catch {
    /* defaults */
  }
  await initI18n(params.get('lang') ?? settings.ui_language ?? undefined);
  applyTranslations();
  const info = await api.appInfo();
  startScreen = mountStart(document.getElementById('welcome'), {
    version: `${info.version}${isTauri ? '' : ' · dev bridge'}`,
    onSettings: appActions.appSettings,
  });
  setOnOpened(projectOpened);
  // Claude over MCP (ADR 0015): its edits show live, projects it opens
  // replace the start screen.
  await initClaude({ onProjectOpened: projectOpened, onProjectClosed: projectClosed });
  bindAppShortcuts(appActions);
  await bindFolderDrop();
  await listen('project-changed', (change) => {
    if (store.get().summary) editor.externalChange(change).catch(report);
    else showProject().catch(report);
  });
  restoreContext();
  syncView();
  await showProject();
}

function report(error) {
  console.error(error);
  editor.toast(`${error?.code ?? 'error'}: ${error?.message ?? error}`, true);
}

window.addEventListener('unhandledrejection', (e) => report(e.reason));
window.addEventListener('arrangia:error', (e) => report(e.detail));
start().catch(report);
