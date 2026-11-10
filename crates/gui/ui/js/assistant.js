// Live collaboration with Claude (ADR 0015). The app hosts an MCP endpoint;
// Claude's edits run through this app's session. This module
//
//   - keeps the endpoint status (`mcp_status` + `mcp-status` events),
//   - shows Claude's steps as they happen: refreshes the editor, briefly
//     outlines the touched elements on the canvas and stacks a small toast
//     "Claude: <label>" (click selects the elements),
//   - keeps the activity list behind the top-bar indicator (time, label,
//     undo for the latest step),
//   - follows projects Claude opens or closes (`project-opened`/`-closed`).

import { actions } from './actions.js';
import { api, listen } from './api.js';
import { h, icon } from './dom.js';
import { editor } from './editor.js';
import { t } from './i18n.js';
import { store } from './store.js';

/** A client counts as connected this long after its last request (as in Rust). */
const ACTIVE_WINDOW_MS = 5 * 60 * 1000;
const MAX_ACTIVITY = 60;
const TOAST_MS = 4500;
const MAX_TOASTS = 4;

/** Known MCP client names → display names. */
const CLIENTS = { 'claude-ai': 'Claude Desktop', 'claude-code': 'Claude Code' };

let status = null;
/** Claude's steps, newest first: `{ time, label, action, ids, client, revision }`. */
const activity = [];
const subscribers = new Set();

function notify() {
  for (const fn of [...subscribers]) fn();
}

/** Calls `fn()` when status or activity change; returns an unsubscribe. */
export function onClaudeChange(fn) {
  subscribers.add(fn);
  return () => subscribers.delete(fn);
}

export function clientLabel(name) {
  return CLIENTS[name] ?? name;
}

/** The endpoint status (or null before the first answer). */
export function claudeStatus() {
  return status;
}

/** Clients active right now, most recently active first. */
export function connectedClients(st = status) {
  const now = Date.now();
  return (st?.clients ?? []).filter((c) => c.sessions > 0 || now - c.last_seen < ACTIVE_WINDOW_MS).sort((a, b) => b.last_seen - a.last_seen);
}

/** `off` | `ready` | `connected` | `error`. */
export function claudeState(st = status) {
  if (!st?.hosted || !st.enabled) return 'off';
  if (!st.running) return st.error ? 'error' : 'off';
  return connectedClients(st).length ? 'connected' : 'ready';
}

export function activityList() {
  return activity;
}

export async function refreshClaudeStatus() {
  try {
    status = await api.mcpStatus();
  } catch {
    status = null;
  }
  notify();
  return status;
}

/**
 * Wires the events. `onProjectOpened()` / `onProjectClosed()` switch
 * between start screen and editor (main.js).
 */
export async function initClaude({ onProjectOpened, onProjectClosed }) {
  await refreshClaudeStatus();
  await listen('mcp-status', (s) => {
    // Events never carry the token or the Claude Code commands.
    status = { ...(status ?? {}), ...s };
    notify();
  });
  await listen('project-edited', (ev) => {
    if (ev.actor?.kind === 'claude') onClaudeEdit(ev).catch((e) => console.error(e));
  });
  await listen('project-opened', (ev) => {
    if (ev.actor?.kind !== 'claude') return;
    record({ label: `${t('claude.opened')} ${ev.name}`, action: 'open', ids: [], client: ev.actor.client });
    Promise.resolve(onProjectOpened?.(ev)).catch((e) => console.error(e));
  });
  await listen('project-closed', (ev) => {
    if (ev.actor?.kind === 'claude') Promise.resolve(onProjectClosed?.()).catch((e) => console.error(e));
  });
  // "Connected" expires without events.
  setInterval(notify, 30_000);
}

function record({ label, action, ids, client, revision }) {
  activity.unshift({ time: Date.now(), label, action, ids, client, revision });
  activity.length = Math.min(activity.length, MAX_ACTIVITY);
  notify();
}

async function onClaudeEdit(ev) {
  const ids = ev.changes?.elements ?? [];
  record({ label: ev.label, action: ev.action, ids, client: ev.actor.client, revision: ev.revision });
  showToast(ev.label, ids);
  await editor.remoteEdit(ev);
  editor.highlight(ids.filter((id) => store.get().overlay?.elements.some((e) => e.id === id) || store.get().overlay?.groups.some((g) => g.id === id)));
}

// ----- toasts ----------------------------------------------------------------------

function toastHost() {
  let el = document.getElementById('claude-toasts');
  if (!el) {
    el = h('div', { id: 'claude-toasts', class: 'claude-toasts', role: 'status', 'aria-live': 'polite' });
    document.body.append(el);
  }
  return el;
}

function selectable(ids) {
  const o = store.get().overlay;
  if (!o) return [];
  return ids.filter((id) => o.elements.some((e) => e.id === id) || o.groups.some((g) => g.id === id));
}

/** A small stacked toast "Claude: …"; click selects `ids`. */
export function showToast(label, ids = []) {
  const host = toastHost();
  const el = h(
    'button',
    {
      class: 'claude-toast',
      type: 'button',
      title: ids.length ? t('claude.toast.select') : label,
      onclick: () => {
        const sel = selectable(ids);
        if (sel.length) {
          store.set({ selection: sel });
          editor.highlight(sel);
        }
        dismiss();
      },
    },
    icon('sparkle'),
    h('span', { class: 'claude-toast-text' }, label),
  );
  host.append(el);
  while (host.children.length > MAX_TOASTS) host.firstElementChild.remove();
  requestAnimationFrame(() => el.classList.add('in'));
  let timer = setTimeout(dismiss, TOAST_MS);
  el.addEventListener('mouseenter', () => clearTimeout(timer));
  el.addEventListener('mouseleave', () => {
    timer = setTimeout(dismiss, TOAST_MS / 2);
  });
  function dismiss() {
    clearTimeout(timer);
    el.classList.remove('in');
    setTimeout(() => el.remove(), 200);
  }
}

// ----- top-bar indicator + activity popover --------------------------------------------

function clock(ms) {
  return new Date(ms).toLocaleTimeString([], { hour: '2-digit', minute: '2-digit', second: '2-digit' });
}

function tooltip(st) {
  const state = claudeState(st);
  if (state === 'off') return st?.hosted === false ? t('claude.state.unavailable') : t('claude.state.off');
  if (state === 'error') return `${t('claude.state.error')}: ${st.error}`;
  if (state === 'ready') return t('claude.state.ready').replace('{port}', String(st.port));
  const names = connectedClients(st).map((c) => clientLabel(c.name)).join(', ');
  return t('claude.state.connected').replace('{clients}', names);
}

/**
 * The top-bar indicator: a sparkle with the state (off / ready / connected
 * with client name). Click opens the activity list. `openSettings()`
 * opens the app settings.
 */
export function createClaudeIndicator({ openSettings } = {}) {
  const label = h('span', { class: 'claude-ind-label' });
  const el = h('button', { class: 'claude-ind', type: 'button', onclick: () => toggleActivity(el, openSettings) }, icon('sparkle'), label);
  const render = () => {
    const state = claudeState();
    el.dataset.state = state;
    el.title = tooltip(status);
    const clients = connectedClients();
    label.textContent = state === 'connected' ? clientLabel(clients[0].name) : '';
    label.hidden = state !== 'connected';
  };
  onClaudeChange(render);
  render();
  return el;
}

let popover = null;

function toggleActivity(anchor, openSettings) {
  if (popover) {
    popover.close();
    return;
  }
  popover = mountActivity(anchor, openSettings);
}

function mountActivity(anchor, openSettings) {
  const head = h('div', { class: 'ca-status' });
  const list = h('div', { class: 'ca-list' });
  const el = h(
    'div',
    { class: 'popover claude-activity', role: 'dialog', 'aria-label': t('claude.activity') },
    h(
      'header',
      { class: 'vp-head' },
      icon('sparkle'),
      h('strong', {}, t('claude.activity')),
      h('span', { class: 'spacer' }),
      h('button', { class: 'btn btn-small', type: 'button', onclick: () => (close(), openSettings?.()) }, t('claude.settings')),
      h('button', { class: 'icon-btn', type: 'button', title: t('project.dialog.close'), onclick: () => close() }, icon('close')),
    ),
    head,
    list,
  );
  document.body.append(el);

  function place() {
    const r = anchor.getBoundingClientRect();
    el.style.top = `${r.bottom + 6}px`;
    el.style.right = `${Math.max(8, window.innerWidth - r.right - 60)}px`;
  }

  function render() {
    const st = status;
    const clients = connectedClients(st);
    head.replaceChildren(
      h('span', { class: `ca-dot ${claudeState(st)}` }),
      h('span', {}, tooltip(st)),
      ...(clients.length ? [h('span', { class: 'muted small' }, `· ${clients.map((c) => `${clientLabel(c.name)} (${c.calls})`).join(', ')}`)] : []),
    );
    if (!activity.length) {
      list.replaceChildren(h('p', { class: 'placeholder' }, t('claude.activity.empty')));
      return;
    }
    const history = store.get().history;
    const lastUndo = history?.undo?.[history.undo.length - 1];
    list.replaceChildren(
      ...activity.map((a, i) => {
        const canUndo = i === 0 && a.action !== 'undo' && a.action !== 'open' && lastUndo === a.label;
        return h(
          'div',
          { class: 'ca-item' },
          h('span', { class: 'ca-time mono small' }, clock(a.time)),
          h(
            'button',
            {
              class: 'ca-label',
              type: 'button',
              title: a.ids.length ? t('claude.toast.select') : '',
              onclick: () => {
                const sel = selectable(a.ids);
                if (sel.length) {
                  store.set({ selection: sel });
                  editor.highlight(sel);
                }
              },
            },
            a.label,
          ),
          canUndo ? h('button', { class: 'btn btn-small', type: 'button', onclick: () => actions.undo() }, icon('undo'), t('top.undo')) : null,
        );
      }),
    );
  }

  const unsub = onClaudeChange(render);
  const unsubHistory = store.select((s) => s.history, render);
  const onDown = (e) => {
    if (!el.contains(e.target) && !anchor.contains(e.target)) close();
  };
  const onKey = (e) => {
    if (e.key === 'Escape') close();
  };
  const onResize = () => place();
  setTimeout(() => document.addEventListener('mousedown', onDown), 0);
  document.addEventListener('keydown', onKey);
  window.addEventListener('resize', onResize);
  place();
  render();

  function close() {
    unsub();
    unsubHistory();
    document.removeEventListener('mousedown', onDown);
    document.removeEventListener('keydown', onKey);
    window.removeEventListener('resize', onResize);
    el.remove();
    popover = null;
  }
  return { close };
}
