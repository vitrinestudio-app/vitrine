// App settings dialog (task 2.10): UI language (applies right away by
// reloading the web view; the backend session stays open), defaults for
// new projects (language, frame style), AI assistants (MCP) (ADR 0015:
// generic first – switch, status, port, autostart, how any MCP client
// connects – then the client-specific setup: "Add to Claude Desktop",
// Claude Code commands), store accounts (Phase 5.1, `stores.js`).

import { api } from '../api.js';
import { clientLabel, onAssistantChange, refreshAssistantStatus } from '../assistant.js';
import { h, icon } from '../dom.js';
import { t } from '../i18n.js';
import { openFramePackages } from './frame-packages.js';
import { checkbox, choiceDialog, errorLine, input, openModal, row, select } from './modal.js';
import { storeAccountRows } from './stores.js';

/** Reloads the UI in `lang`, keeping the view context (see main.js). */
export function reloadInLanguage(lang) {
  const url = new URL(location.href);
  if (url.searchParams.has('lang')) {
    if (lang) url.searchParams.set('lang', lang);
    else url.searchParams.delete('lang');
  }
  window.dispatchEvent(new Event('arrangia:before-reload'));
  location.replace(url.toString());
}

/** App settings; `section: 'stores'` scrolls to the store accounts. */
export async function openAppSettings({ section } = {}) {
  const [info, frames] = await Promise.all([api.settingsGet(), api.framesList().catch(() => null)]);
  const frameCount = frames?.sources?.reduce((n, s) => n + s.frames.length, 0) ?? 0;
  const err = errorLine();
  const saved = h('span', { class: 'muted small' });
  let settings = info.settings;

  async function save(patch) {
    try {
      settings = await api.settingsSet(patch);
      err.show(null);
      saved.textContent = t('settings.saved');
      return true;
    } catch (e) {
      err.show(e);
      return false;
    }
  }

  const m = openModal({
    title: t('settings.title'),
    size: 'medium',
    body: [
      err.el,
      h(
        'div',
        { class: 'form' },
        row(
          t('settings.ui_language'),
          select({
            options: [{ value: '', label: t('settings.ui_language.system') }, ...info.ui_languages.map((l) => ({ value: l, label: t(`settings.ui_language.${l}`) }))],
            value: settings.ui_language ?? '',
            onchange: async (v) => {
              if (await save({ ui_language: v || null })) reloadInLanguage(v || null);
            },
          }),
          t('settings.ui_language.hint'),
        ),
        row(
          t('settings.default_language'),
          input({ value: settings.default_project_language, cls: 'num-wide mono', onchange: (v) => v.trim() && save({ default_project_language: v.trim() }) }),
          t('settings.default_language.hint'),
        ),
        row(
          t('settings.frame'),
          select({
            options: [...['detailed', 'stylized'].map((f) => ({ value: f, label: t(`settings.frame.${f}`) })), { value: 'imported:apple', label: t('frames.source.apple') }],
            value: settings.default_frame_style,
            onchange: (v) => save({ default_frame_style: v }),
          }),
        ),
        row(
          t('fonts.system_setting'),
          checkbox({
            label: t('fonts.system_setting.on'),
            checked: !!settings.system_font_fallback,
            // The backend rebuilds the fonts and reports a `fonts` change.
            onchange: (v) => save({ system_font_fallback: v }),
          }),
          t('fonts.system_setting.hint'),
        ),
        row(
          t('frames.title'),
          h(
            'div',
            { class: 'fp-settings-row' },
            h('span', {}, frameCount ? t('frames.count').replace('{n}', String(frameCount)) : t('frames.none_short')),
            h('button', { class: 'btn', type: 'button', onclick: () => openFramePackages() }, t('frames.manage')),
          ),
          t('frames.settings_hint'),
        ),
        ...mcpRows(save, () => settings),
        ...storeAccountRows(),
        row(t('settings.file'), h('span', { class: 'mono small selectable' }, info.settings_file)),
      ),
    ],
    footer: [saved, h('span', { class: 'spacer' }), h('button', { class: 'btn btn-primary', type: 'button', onclick: () => m.close() }, t('project.dialog.done'))],
    onClose: () => unsubMcp?.(),
  });
  if (section === 'stores') requestAnimationFrame(() => m.body.querySelector('.store-section')?.scrollIntoView({ block: 'start' }));
  return m;
}

let unsubMcp = null;

/** Read-only text field with a copy button. */
function copyField(text) {
  const field = h('input', { class: 'field mono small assistant-cmd', type: 'text', readonly: true, spellcheck: 'false' });
  field.value = text;
  field.addEventListener('focus', () => field.select());
  const btn = h('button', { class: 'icon-btn', type: 'button', title: t('mcp.copy') }, icon('copy'));
  btn.addEventListener('click', async () => {
    field.select();
    try {
      await navigator.clipboard.writeText(text);
    } catch {
      document.execCommand?.('copy');
    }
    btn.classList.add('done');
    setTimeout(() => btn.classList.remove('done'), 1200);
  });
  return h('div', { class: 'assistant-copy' }, field, btn);
}

function statusText(st) {
  if (!st?.hosted) return t('mcp.state.unavailable');
  if (!st.enabled) return t('mcp.state.off');
  if (!st.running) return st.error ? `${t('mcp.state.error')}: ${st.error}` : t('mcp.state.off');
  const now = Date.now();
  const clients = (st.clients ?? []).filter((c) => c.sessions > 0 || now - c.last_seen < 5 * 60 * 1000);
  const parts = [t('mcp.settings.listening').replace('{port}', String(st.port))];
  parts.push(clients.length ? t('mcp.settings.clients').replace('{clients}', clients.map((c) => clientLabel(c)).join(', ')) : t('mcp.settings.no_clients'));
  if (st.last_activity) parts.push(t('mcp.settings.last').replace('{time}', new Date(st.last_activity).toLocaleTimeString()));
  return parts.join(' · ');
}

/** Rows of the "AI assistants (MCP)" section: generic first, then client-specific. */
function mcpRows(save, current) {
  const toggle = h('span');
  const status = h('div', { class: 'assistant-status small' });
  const code = h('div', { class: 'mcp-connect' });
  const other = h('div', { class: 'mcp-connect' });
  let st = null;

  function render() {
    toggle.replaceChildren(
      checkbox({
        label: t('mcp.settings.allow'),
        checked: st ? st.enabled : current().mcp_enabled !== false,
        disabled: st?.hosted === false,
        onchange: async (v) => {
          try {
            st = await api.mcpSetEnabled(v);
          } catch (e) {
            status.textContent = e.message;
          }
          await refreshAssistantStatus();
        },
      }),
    );
    status.dataset.state = !st?.running ? 'off' : 'on';
    status.textContent = statusText(st);
    const items = [
      h('div', { class: 'mcp-connect-head' }, h('strong', {}, t('claude.settings.code_stdio')), h('span', { class: 'badge ok' }, t('mcp.settings.recommended'))),
      copyField(st?.claude_code?.stdio ?? ''),
    ];
    if (st?.claude_code?.http) {
      items.push(h('div', { class: 'mcp-connect-head' }, h('strong', {}, t('claude.settings.code_http'))), copyField(st.claude_code.http), h('div', { class: 'form-hint' }, t('claude.settings.code_http.hint')));
    }
    code.replaceChildren(...items);

    const generic = [
      h('div', { class: 'mcp-connect-head' }, h('strong', {}, t('mcp.settings.other.stdio')), h('span', { class: 'badge ok' }, t('mcp.settings.recommended'))),
      copyField(st?.generic?.stdio ?? ''),
    ];
    if (st && !st.cli_exists) generic.push(h('div', { class: 'form-hint warn' }, t('mcp.settings.cli_missing').replace('{path}', st.cli_path)));
    if (st?.url && st?.generic?.http_header) {
      generic.push(
        h('div', { class: 'mcp-connect-head' }, h('strong', {}, t('mcp.settings.other.http'))),
        copyField(st.url),
        copyField(st.generic.http_header),
        h('div', { class: 'form-hint' }, t('mcp.settings.other.http.hint')),
      );
    }
    other.replaceChildren(...generic);
  }

  async function refresh() {
    try {
      st = await api.mcpStatus();
    } catch {
      st = null;
    }
    render();
  }
  unsubMcp?.();
  unsubMcp = onAssistantChange(() => refresh());
  refresh();

  return [
    h('div', { class: 'form-section' }, icon('sparkle'), t('mcp.settings.title')),
    row(t('mcp.settings.connect'), h('div', { class: 'assistant-toggle' }, toggle, status), t('mcp.settings.connect.hint')),
    row(
      t('mcp.settings.port'),
      input({
        type: 'number',
        min: 1024,
        max: 65535,
        value: current().mcp_port ?? '',
        placeholder: t('mcp.settings.port.auto'),
        cls: 'num-wide mono',
        onchange: (v) => save({ mcp_port: v || null }),
      }),
      t('mcp.settings.port.hint'),
    ),
    row(
      t('mcp.settings.autostart'),
      checkbox({ label: t('mcp.settings.autostart.on'), checked: current().mcp_autostart_gui !== false, onchange: (v) => save({ mcp_autostart_gui: v }) }),
      t('mcp.settings.autostart.hint'),
    ),
    row(t('mcp.settings.other'), other, t('mcp.settings.other.hint')),
    row(
      t('claude.settings.desktop'),
      h('div', {}, h('button', { class: 'btn', type: 'button', onclick: () => openClaudeDesktop() }, t('claude.desktop.button'))),
      t('claude.settings.desktop.hint'),
    ),
    row(t('claude.settings.code'), code),
  ];
}

/**
 * "Add to Claude Desktop": shows the file, the entry and the state, asks
 * for confirmation, writes (with backup) and says to restart Claude Desktop.
 */
export async function openClaudeDesktop() {
  const plan = await api.claudeDesktopPlan();
  const stateText = t(`claude.desktop.state.${plan.state}`);
  const body = h(
    'div',
    { class: 'claude-desktop' },
    h('p', { class: 'dlg-text' }, t('claude.desktop.intro')),
    h('div', { class: 'form' }, row(t('claude.desktop.file'), h('span', { class: 'mono small selectable' }, plan.config_path)), row(t('claude.desktop.status'), h('span', { class: `claude-state ${plan.state}` }, stateText))),
    plan.error ? h('div', { class: 'form-error' }, plan.error) : null,
    !plan.cli_exists ? h('div', { class: 'form-hint warn' }, t('mcp.settings.cli_missing').replace('{path}', plan.cli_path)) : null,
    h('div', { class: 'section-title' }, t(plan.preview ? 'claude.desktop.after' : 'claude.desktop.entry')),
    // The whole file as it will be written (other entries unchanged).
    h('pre', { class: 'claude-json mono small selectable' }, plan.preview ?? JSON.stringify({ mcpServers: { arrangia: plan.entry } }, null, 2)),
    plan.current && plan.state === 'different' ? h('div', { class: 'form-hint' }, `${t('claude.desktop.current')}: ${JSON.stringify(plan.current)}`) : null,
    plan.config_exists && plan.state !== 'same' ? h('div', { class: 'form-hint' }, t('claude.desktop.backup')) : null,
  );
  const canWrite = plan.state === 'missing' || plan.state === 'different';
  const choice = await choiceDialog({
    title: t('claude.desktop.title'),
    size: 'medium',
    body,
    buttons: canWrite
      ? [
          { label: t('project.dialog.cancel'), value: null },
          { label: t('claude.desktop.write'), primary: true, value: 'write' },
        ]
      : [{ label: t('project.dialog.done'), primary: true, value: null }],
  });
  if (choice !== 'write') return;
  try {
    const r = await api.claudeDesktopInstall();
    await choiceDialog({
      title: t('claude.desktop.title'),
      body: h(
        'div',
        {},
        h('p', { class: 'dlg-text' }, r.written ? t('claude.desktop.done') : t('claude.desktop.state.same')),
        r.backup ? h('div', { class: 'form-hint mono selectable' }, `${t('claude.desktop.backup_at')}: ${r.backup}`) : null,
        h('p', { class: 'dlg-text strong' }, t('claude.desktop.restart')),
      ),
      buttons: [{ label: t('project.dialog.ok'), primary: true, value: null }],
    });
  } catch (e) {
    await choiceDialog({ title: t('claude.desktop.title'), body: h('div', { class: 'form-error' }, e.message), buttons: [{ label: t('project.dialog.ok'), primary: true, value: null }] });
  }
}
