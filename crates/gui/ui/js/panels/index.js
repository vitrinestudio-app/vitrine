// Panel registry and the context every panel receives. To add a panel:
// create `panels/<name>.js` exporting `mount(el, ctx)` and list it here
// (left sidebar tab) or mount it from main.js. See ui/README.md.

import { api } from '../api.js';
import { editor } from '../editor.js';
import { t } from '../i18n.js';
import { store, viewArgs } from '../store.js';
import * as assets from './assets.js';
import * as layers from './layers.js';
import * as canvases from './canvases.js';
import * as translations from './translations.js';

/**
 * @typedef {object} PanelContext
 * @property {typeof store} store       app state (subscribe/select/set)
 * @property {typeof editor} editor     loads, applies edits, emits invalidate/reset
 * @property {typeof api} api           typed command wrappers
 * @property {typeof t} t               UI translation
 * @property {typeof viewArgs} viewArgs view context {target, lang, variant} for commands
 */

/** @returns {PanelContext} */
export function panelContext() {
  return { store, editor, api, t, viewArgs };
}

/** Left sidebar tabs, in order: `{ id, icon, title (i18n key), module }`. */
export const SIDEBAR_TABS = [
  { id: 'canvases', icon: 'canvases', title: 'panel.canvases', module: canvases },
  { id: 'layers', icon: 'layers', title: 'panel.layers', module: layers },
  { id: 'assets', icon: 'assets', title: 'panel.assets', module: assets },
  { id: 'translations', icon: 'translations', title: 'panel.translations', module: translations },
];
