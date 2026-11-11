// Entry point: theme, UI language, backend connection, start screen,
// shell, app menu and shortcuts.
//
// Debug query parameters (dev bridge): `?theme=dark|light` forces a theme,
// `?lang=de|en` the UI language (otherwise the app setting, else the system).

import { actions } from './actions.js';
import { api, listen } from './api.js';
import { createAssistantIndicator, initAssistant } from './assistant.js';
import { openAppSettings } from './dialogs/app-settings.js';
import { openExport } from './dialogs/export.js';
import { initUploadConfirm, openUpload } from './dialogs/upload.js';
import { openScreenshotImport } from './dialogs/screenshot-import.js';
import { openProjectSettings } from './dialogs/project-settings.js';
import { openTargets } from './dialogs/targets.js';
import { openNewVariant } from './dialogs/variants.js';
import { editor } from './editor.js';
import { applyTranslations, initI18n } from './i18n.js';
import { bindAppShortcuts, createMenu, initNativeMenu } from './menu.js';
import { installDropImport } from './panels/importer.js';
import { mountShell } from './shell.js';
import { bindShortcuts } from './shortcuts.js';
import { bindFolderDrop, mountStart, openNewProject, openProjectDialog, setOnOpened } from './start.js';
import { store } from './store.js';
import { isTauri } from './transport.js';

const params = new URLSearchParams(location.search);
const theme = params.get('theme');
if (theme === 'dark' || theme === 'light') document.documentElement.dataset.theme = theme;

/** Text fields whose native context menu (cut/copy/paste, spelling) stays. */
const TEXT_FIELDS =
  'textarea, [contenteditable]:not([contenteditable="false"]), input:not([type]), ' +
  'input[type="text"], input[type="search"], input[type="url"], input[type="email"], input[type="number"], input[type="password"]';

/**
 * No web-view context menu ("Reload", "Inspect Element") on the app's own
 * surfaces: only text fields and selected text keep it. Custom context
 * menus call `preventDefault` themselves and are unaffected. Holding ⌥/Alt
 * while right-clicking shows the native menu anywhere (debugging: "Inspect
 * Element" in debug builds, which have the dev tools).
 */
function suppressContextMenu() {
  document.addEventListener('contextmenu', (e) => {
    if (e.defaultPrevented || e.altKey) return;
    const el = e.target instanceof Element ? e.target : e.target?.parentElement;
    if (el?.closest(TEXT_FIELDS)) return;
    const sel = window.getSelection();
    if (sel && !sel.isCollapsed && el && sel.containsNode(el, true)) return;
    e.preventDefault();
  });
}
suppressContextMenu();

/** View context kept across a UI reload (language switch). */
const CONTEXT_KEY = 'vitrine.context';

let shell = null;
let startScreen = null;

const appActions = {
  newProject: () => openNewProject().catch(report),
  openProject: () => openProjectDialog().catch(report),
  projectSettings: () => openProjectSettings(),
  targets: () => openTargets().catch(report),
  exportDialog: () => openExport(),
  uploadDialog: () => openUpload(),
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
      assistant: createAssistantIndicator({ openSettings: appActions.appSettings }),
      // "+ Add…" entries of the top-bar pickers: open the place where that thing is created.
      addCanvas: () => openTargets(null, { mode: 'new-canvas' }).catch(report),
      addTarget: () => openTargets(null, { mode: 'add', canvas: store.get().canvas }).catch(report),
      addLanguage: () => openProjectSettings('languages', { focusAdd: true }),
      addVariant: () => openNewVariant({ canvas: store.get().canvas, targets: [], switchTo: true }).catch(report),
    });
    bindShortcuts({
      ...actions,
      reorder: (order) => actions.reorder(order),
      nudge: (dx, dy) => shell.canvas?.nudge(dx, dy),
      escape: () => {
        if (!shell.canvas?.cancel()) actions.clearSelection();
      },
      zoomIn: () => shell.view?.zoomIn?.(),
      zoomOut: () => shell.view?.zoomOut?.(),
      fit: () => shell.view?.fit?.(),
      zoomActual: () => shell.view?.zoomActual?.(),
    });
    installDropImport(() => shell.canvas);
  }
}

async function closeProject() {
  await api.projectClose();
  await projectClosed();
}

/** The backend has no project any more (closed here or by an assistant). */
async function projectClosed() {
  store.set({ summary: null, overlay: null, canvas: null, target: null, variant: undefined, selection: [], hover: null, validation: null });
  await showProject();
}

/** A project was opened (start screen, menu, or by an assistant over MCP). */
async function projectOpened() {
  store.set({ selection: [], hover: null, variant: undefined, canvas: null, target: null, lang: null, validation: null });
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
    const view = { canvas: st.canvas, target: st.target, lang: st.lang, selection: st.selection };
    if (st.variant !== undefined) view.variant = st.variant;
    api.viewSet(view).catch((e) => console.warn('view_set', e));
  };
  store.select(
    (st) => [st.summary, st.canvas, st.target, st.lang, st.variant, st.selection],
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
  window.addEventListener('vitrine:before-reload', () => {
    const { canvas, target, lang, view } = store.get();
    try {
      sessionStorage.setItem(CONTEXT_KEY, JSON.stringify({ canvas, target, lang, view }));
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
  // AI assistants over MCP (ADR 0015): their edits show live, projects they open
  // replace the start screen.
  await initAssistant({ onProjectOpened: projectOpened, onProjectClosed: projectClosed });
  // Store uploads requested by an assistant need the user's click (ADR 0018).
  await initUploadConfirm();
  // Native menu bar (macOS app): before the key handlers, which skip its shortcuts.
  await initNativeMenu({
    ...appActions,
    undo: () => appActions.hasProject() && actions.undo(),
    redo: () => appActions.hasProject() && actions.redo(),
  });
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
window.addEventListener('vitrine:error', (e) => report(e.detail));
start().catch(report);
