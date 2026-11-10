// Store upload (Phase 5.5, ADR 0018): plan → confirm → progress → result.
//
// - `openUpload()`: the "Upload…" dialog (menu, export dialog link). Store
//   tabs (the stores the project is mapped to), "Check store state" toggle
//   (remote plan), plan table (target · language · locale · action · images ·
//   problems), Confirm & upload → per-set progress from `upload-progress`
//   events → result + "Show log". A run started elsewhere (MCP, another
//   window) is shown when the dialog opens.
// - `initUploadConfirm()`: an AI assistant asked for an upload over MCP
//   (`upload-confirm-requested`): a dialog shows the plan and the user
//   approves or declines (`upload_confirm`); it closes itself when the
//   request times out (`upload-confirm-resolved`).

import { api, listen } from '../api.js';
import { h, icon } from '../dom.js';
import { editor } from '../editor.js';
import { t } from '../i18n.js';
import { store } from '../store.js';
import { checkbox, confirmDialog, errorLine, openModal } from './modal.js';

const fmt = (s, vars) => s.replace(/\{(\w+)\}/g, (_, k) => String(vars[k] ?? ''));

function kb(bytes) {
  return bytes >= 1048576 ? `${(bytes / 1048576).toFixed(1)} MB` : `${Math.ceil(bytes / 1024)} KB`;
}

function storeLabel(store) {
  return store === 'apple' ? t('stores.kind.apple') : t('stores.kind.google');
}

/** Problem list (`{level, code, message, blocking}`). */
function problemList(problems, cls = '') {
  if (!problems?.length) return null;
  return h(
    'ul',
    { class: `up-problems ${cls}`.trim() },
    problems.map((p) => h('li', { class: `up-problem ${p.blocking ? 'bad' : p.level}` }, icon(p.blocking || p.level === 'error' ? 'error' : 'warning'), h('span', {}, p.message))),
  );
}

/** The plan as a table per store (also used by the MCP confirmation). */
export function planView(plan) {
  const parts = [problemList(plan.problems)];
  for (const sp of plan.stores) {
    const rows = sp.sets.map((s) =>
      h(
        'tr',
        { class: `up-row up-${s.action}` },
        h('td', {}, s.target),
        h('td', {}, s.lang),
        h('td', { class: 'mono' }, s.locale ?? '–'),
        h('td', {}, h('span', { class: `up-action up-action-${s.action}`, title: s.reason ?? '' }, t(`upload.action.${s.action}`))),
        h('td', { class: 'num' }, s.action === 'skip' ? '–' : `${s.images.length}${s.remote_count != null ? ` (${fmt(t('upload.in_store'), { n: s.remote_count })})` : ''}`),
        h('td', {}, problemList(s.problems, 'compact') ?? (s.reason ? h('span', { class: 'muted small' }, s.reason) : null)),
      ),
    );
    const remote = sp.remote
      ? h(
          'div',
          { class: `small ${sp.remote.ok ? 'muted' : 'up-warn'}` },
          sp.remote.error ?? [sp.remote.app_name, sp.remote.version ? `${t('upload.version')} ${sp.remote.version}` : null].filter(Boolean).join(' · '),
        )
      : null;
    parts.push(
      h(
        'section',
        { class: 'up-store' },
        h('h3', { class: 'up-store-title' }, storeLabel(sp.store), h('span', { class: 'muted small' }, ` ${sp.app ?? ''}${sp.account ? ` · ${sp.account}` : ''}`)),
        remote,
        problemList(sp.problems),
        sp.sets.length
          ? h(
              'table',
              { class: 'up-table' },
              h('thead', {}, h('tr', {}, ['upload.col.target', 'upload.col.lang', 'upload.col.locale', 'upload.col.action', 'upload.col.images', 'upload.col.problems'].map((k) => h('th', {}, t(k))))),
              h('tbody', {}, rows),
            )
          : h('p', { class: 'muted small' }, t('upload.no_sets')),
      ),
    );
  }
  const tt = plan.totals;
  parts.push(h('div', { class: 'up-totals' }, fmt(t('upload.totals'), { sets: tt.upload_sets, images: tt.images, size: kb(tt.bytes), unchanged: tt.unchanged_sets, skipped: tt.skipped_sets })));
  return h('div', { class: 'up-plan' }, parts.filter(Boolean));
}

export function openUpload() {
  const s = store.get().summary;
  if (!s) return null;
  let storeTab = null;
  let remote = false;
  let force = false;
  let plan = null;
  let running = false;
  let unlisten = [];

  const err = errorLine();
  const tabs = h('div', { class: 'tabs up-tabs', role: 'tablist' });
  const options = h(
    'div',
    { class: 'inline wrap up-options' },
    checkbox({ label: t('upload.remote'), checked: remote, title: t('upload.remote.hint'), onchange: (v) => ((remote = v), makePlan()) }),
    checkbox({ label: t('upload.force'), checked: force, onchange: (v) => ((force = v), makePlan()) }),
    h('button', { class: 'btn btn-small', type: 'button', onclick: () => makePlan() }, icon('reset'), t('upload.replan')),
  );
  const planBox = h('div', { class: 'up-plan-box' });
  const progress = h('div', { class: 'up-progress', hidden: true });
  const result = h('div', { class: 'up-result', hidden: true });
  const config = h('div', { class: 'up-config' }, tabs, options, h('p', { class: 'muted small' }, t('upload.intro')), planBox);
  const startBtn = h('button', { class: 'btn btn-primary', type: 'button', disabled: true, onclick: () => start() }, t('upload.start'));
  const cancelBtn = h('button', { class: 'btn', type: 'button', hidden: true, onclick: () => cancel() }, t('upload.cancel'));
  const logBtn = h('button', { class: 'btn', type: 'button', onclick: () => showLog() }, t('upload.log'));
  const closeBtn = h('button', { class: 'btn', type: 'button', onclick: () => m.close() }, t('project.dialog.close'));

  const m = openModal({
    title: t('upload.title'),
    size: 'large',
    cls: 'dlg-upload',
    dismissable: true,
    body: [err.el, config, progress, result],
    footer: [logBtn, h('span', { class: 'spacer' }), closeBtn, cancelBtn, startBtn],
    onClose: () => {
      for (const u of unlisten) u();
    },
  });

  async function init() {
    try {
      const info = await api.projectStoreGet();
      const stores = ['apple', 'google'].filter((k) => info.store?.[k]);
      if (!stores.length) {
        planBox.replaceChildren(h('p', { class: 'up-empty' }, t('upload.not_mapped')));
        options.hidden = true;
        return;
      }
      storeTab = stores[0];
      tabs.replaceChildren(
        ...stores.map((k) =>
          h(
            'button',
            {
              type: 'button',
              role: 'tab',
              'aria-selected': String(k === storeTab),
              onclick: (e) => {
                storeTab = k;
                for (const b of tabs.children) b.setAttribute('aria-selected', String(b === e.currentTarget));
                makePlan();
              },
            },
            storeLabel(k),
          ),
        ),
      );
      unlisten.push(await listen('upload-progress', onProgress));
      unlisten.push(await listen('upload-finished', onFinished));
      const status = await api.uploadStatus();
      if (status.run && (status.run.status === 'running' || status.run.status === 'paused')) showRun(status.run);
      else await makePlan();
    } catch (e) {
      err.show(e);
    }
  }

  async function makePlan() {
    if (running || !storeTab) return;
    err.show(null);
    startBtn.disabled = true;
    planBox.replaceChildren(h('p', { class: 'muted' }, remote ? t('upload.planning.remote') : t('upload.planning')));
    try {
      plan = await api.uploadPlan({ store: storeTab, remote, force });
      planBox.replaceChildren(planView(plan));
      startBtn.disabled = plan.blocked || plan.totals.upload_sets === 0;
      startBtn.title = plan.blocked ? t('upload.blocked') : plan.totals.upload_sets === 0 ? t('upload.nothing') : '';
    } catch (e) {
      planBox.replaceChildren();
      err.show(e);
    }
  }

  async function start() {
    if (!plan || plan.blocked) return;
    const ok = await confirmDialog({
      title: t('upload.confirm.title'),
      message: fmt(t('upload.confirm.message'), { sets: plan.totals.upload_sets, images: plan.totals.images, store: storeLabel(storeTab) }),
      confirm: t('upload.confirm.ok'),
    });
    if (!ok) return;
    try {
      const out = await api.uploadExecute(plan.confirm_token);
      showRun(out.run);
    } catch (e) {
      err.show(e);
      if (e?.code === 'not_confirmed') makePlan();
    }
  }

  /** Per-set rows of the running run, keyed by set key. */
  let setRows = new Map();
  let bar = null;
  let line = null;

  function showRun(run) {
    running = true;
    config.hidden = true;
    result.hidden = true;
    progress.hidden = false;
    startBtn.hidden = true;
    cancelBtn.hidden = false;
    cancelBtn.disabled = false;
    bar = h('progress', { class: 'ex-bar', max: String(run.bytes_total || 1), value: String(run.bytes_done) });
    line = h('div', { class: 'small up-line' }, t('upload.running'));
    setRows = new Map();
    const rows = run.sets
      .filter((x) => x.status !== 'skipped')
      .map((x) => {
        const status = h('span', { class: `up-status up-status-${x.status}` }, t(`upload.status.${x.status}`));
        const files = h('span', { class: 'muted small num' }, `${x.files_done}/${x.files_total}`);
        const error = h('div', { class: 'up-error small', hidden: !x.error }, x.error ?? '');
        setRows.set(x.key, { status, files, error });
        return h('tr', {}, h('td', {}, storeLabel(x.store)), h('td', {}, `${x.target} · ${x.lang}`), h('td', { class: 'mono' }, x.locale ?? ''), h('td', {}, status, error), h('td', {}, files));
      });
    progress.replaceChildren(line, bar, h('table', { class: 'up-table' }, h('tbody', {}, rows)));
  }

  function updateSet(x) {
    const r = x && setRows.get(x.key);
    if (!r) return;
    r.status.className = `up-status up-status-${x.status}`;
    r.status.textContent = t(`upload.status.${x.status}`);
    r.files.textContent = `${x.files_done}/${x.files_total}`;
    r.error.hidden = !x.error;
    r.error.textContent = x.error ?? '';
  }

  function onProgress(p) {
    if (!running) return;
    if (bar) {
      bar.max = p.bytes_total || 1;
      bar.value = p.bytes_done + (p.step === 'transfer' ? p.file?.bytes_sent ?? 0 : 0);
    }
    updateSet(p.set);
    if (line) {
      if (p.status === 'paused') line.textContent = t('upload.paused');
      else if (p.file?.file_name) line.textContent = `${t('upload.running')} ${p.file.file_name} · ${p.images_done}/${p.images_total}`;
      else if (p.step === 'commit') line.textContent = t('upload.committing');
    }
  }

  function onFinished(run) {
    if (!running) return;
    running = false;
    for (const x of run.sets) updateSet(x);
    cancelBtn.hidden = true;
    startBtn.hidden = false;
    startBtn.textContent = t('upload.again');
    startBtn.disabled = true;
    const good = run.status === 'completed';
    result.hidden = false;
    result.replaceChildren(
      h('div', { class: `ex-result ${good ? 'good' : run.status === 'cancelled' || run.status === 'partial' ? 'warn' : 'bad'}` }, icon(good ? 'check' : 'warning'), h('strong', {}, t(`upload.result.${run.status}`))),
      h('p', { class: 'small' }, run.message ?? ''),
      good ? null : h('p', { class: 'muted small' }, t('upload.resume_hint')),
      h(
        'div',
        { class: 'inline' },
        h(
          'button',
          {
            class: 'btn',
            type: 'button',
            onclick: () => {
              progress.hidden = true;
              result.hidden = true;
              config.hidden = false;
              startBtn.textContent = t('upload.start');
              makePlan();
            },
          },
          t('upload.back'),
        ),
      ),
    );
  }

  async function cancel() {
    cancelBtn.disabled = true;
    try {
      await api.uploadCancel();
    } catch (e) {
      err.show(e);
    }
  }

  async function showLog() {
    try {
      const log = await api.uploadLog({ limit: 10 });
      const runs = log.runs.map((r) =>
        h(
          'details',
          { class: 'up-log-run' },
          h('summary', {}, `#${r.id} · ${new Date(r.started_at * 1000).toLocaleString()} · ${t(`upload.result.${r.status}`)}`),
          h(
            'ul',
            { class: 'small' },
            r.sets.map((x) => h('li', {}, `${storeLabel(x.store)} ${x.locale} ${x.slot} (${x.target} · ${x.lang}): ${t(`upload.status.${x.status === 'done' ? 'done' : x.status}`)}${x.error ? ` – ${x.error}` : ''}`)),
          ),
        ),
      );
      const lm = openModal({
        title: t('upload.log'),
        size: 'medium',
        body: [runs.length ? h('div', { class: 'up-log' }, runs) : h('p', { class: 'muted' }, t('upload.log.empty')), h('p', { class: 'muted small mono' }, log.path)],
        footer: [
          h('button', { class: 'btn', type: 'button', onclick: () => api.revealPath(log.path).catch((e) => err.show(e)) }, t('upload.log.reveal')),
          h('span', { class: 'spacer' }),
          h('button', { class: 'btn', type: 'button', onclick: () => lm.close() }, t('project.dialog.close')),
        ],
      });
    } catch (e) {
      err.show(e);
    }
  }

  init();
  return m;
}

// ----- MCP confirmation (policy in crates/app/src/upload/policy.rs) -----

/** Open confirmation dialogs by request id. */
const pending = new Map();

function showConfirm(req) {
  if (pending.has(req.id)) return;
  let answered = false;
  const answer = async (approved) => {
    answered = true;
    try {
      await api.uploadConfirm(req.id, approved);
      if (approved) editor.toast(fmt(t('upload.mcp.started'), { client: req.client }));
    } catch (e) {
      editor.toast(`${e?.code ?? 'error'}: ${e?.message ?? e}`, true);
    }
    m.close();
  };
  const m = openModal({
    title: fmt(t('upload.mcp.title'), { client: req.client }),
    size: 'large',
    cls: 'dlg-upload',
    dismissable: true,
    body: [h('p', { class: 'up-mcp-intro' }, icon('warning'), h('span', {}, fmt(t('upload.mcp.intro'), { client: req.client }))), h('p', { class: 'small' }, req.summary), planView(req.plan)],
    footer: [
      h('span', { class: 'muted small' }, t('upload.mcp.timeout')),
      h('span', { class: 'spacer' }),
      h('button', { class: 'btn', type: 'button', onclick: () => answer(false) }, t('upload.mcp.decline')),
      h('button', { class: 'btn btn-primary', type: 'button', onclick: () => answer(true) }, t('upload.mcp.confirm')),
    ],
    onClose: () => {
      pending.delete(req.id);
      // Closing the dialog without a choice declines.
      if (!answered) api.uploadConfirm(req.id, false).catch(() => {});
    },
  });
  pending.set(req.id, m);
}

/** Listens for assistant upload requests (call once at start). */
export async function initUploadConfirm() {
  await listen('upload-confirm-requested', (req) => showConfirm(req));
  await listen('upload-confirm-resolved', (r) => {
    const m = pending.get(r.id);
    if (m && r.outcome === 'timeout') {
      editor.toast(t('upload.mcp.expired'), true);
      pending.delete(r.id);
      m.close();
    }
  });
  try {
    const status = await api.uploadStatus();
    for (const req of status.pending_confirmations ?? []) showConfirm(req);
  } catch {
    /* no project yet */
  }
}
