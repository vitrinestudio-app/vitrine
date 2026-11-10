// App settings dialog (task 2.10): UI language (applies right away by
// reloading the web view; the backend session stays open), defaults for
// new projects (language, frame style), a note on store credentials
// (Phase 5).

import { api } from '../api.js';
import { h } from '../dom.js';
import { t } from '../i18n.js';
import { openFramePackages } from './frame-packages.js';
import { checkbox, errorLine, input, openModal, row, select } from './modal.js';

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

export async function openAppSettings() {
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
        row(t('settings.store'), h('span', { class: 'muted' }, t('settings.store.note'))),
        row(t('settings.file'), h('span', { class: 'mono small selectable' }, info.settings_file)),
      ),
    ],
    footer: [saved, h('span', { class: 'spacer' }), h('button', { class: 'btn btn-primary', type: 'button', onclick: () => m.close() }, t('project.dialog.done'))],
  });
  return m;
}
