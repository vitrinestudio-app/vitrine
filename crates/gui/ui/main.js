import { applyTranslations, initI18n, t } from './i18n.js';
import { invoke, isTauri, listen, toImageBitmap } from './js/transport.js';

const $ = (id) => document.getElementById(id);

let summary = null;

function fill(select, values, labels = values) {
  const current = select.value;
  select.replaceChildren(
    ...values.map((v, i) => Object.assign(document.createElement('option'), { value: v, textContent: labels[i] })),
  );
  if (values.includes(current)) select.value = current;
}

function imageChoices() {
  const target = summary.targets.find((x) => x.id === $('target').value);
  const strip = summary.strips.find((s) => s.id === target?.strip);
  if (!target || !strip) return [];
  const images = target.kind === 'banner' ? 1 : Math.ceil(strip.page.count / target.group);
  return [...Array(images).keys()];
}

async function renderPreview() {
  const started = performance.now();
  const image = await invoke('render_image', {
    target: $('target').value,
    lang: $('lang').value,
    image: Number($('image').value),
    max_edge: 640,
  });
  const bitmap = await toImageBitmap(image);
  const canvas = $('preview');
  canvas.width = bitmap.width;
  canvas.height = bitmap.height;
  canvas.getContext('2d').drawImage(bitmap, 0, 0);
  const ms = Math.round(performance.now() - started);
  $('preview-info').textContent = `${image.width}×${image.height} px · ${image.format} · ${(image.data.byteLength / 1024).toFixed(0)} KiB · ${ms} ms`;
}

async function showProject(s) {
  summary = s;
  $('project').hidden = false;
  $('no-project').hidden = true;
  $('project-name').textContent = s.name;
  fill($('target'), s.targets.map((x) => x.id));
  fill($('lang'), s.languages.available);
  if (!s.languages.available.includes($('lang').value)) $('lang').value = s.languages.default;
  const images = imageChoices();
  fill($('image'), images.map(String), images.map((i) => `${i + 1}`));
  await renderPreview();
}

async function loadProject() {
  try {
    await showProject(await invoke('project_summary'));
  } catch (e) {
    if (e.code !== 'no_project') throw e;
    $('project').hidden = true;
    $('no-project').hidden = false;
  }
}

async function start() {
  await initI18n();
  applyTranslations();

  const info = await invoke('app_info');
  $('app-version').textContent = `${info.version}${isTauri ? '' : ' · dev bridge'}`;

  for (const id of ['lang', 'image']) {
    $(id).addEventListener('change', () => renderPreview().catch(report));
  }
  $('target').addEventListener('change', () => showProject(summary).catch(report));
  $('open').addEventListener('click', async () => {
    const s = await invoke('dialog_open_project').catch(report);
    if (s) await showProject(s).catch(report);
  });
  await listen('project-changed', (change) => {
    $('status').textContent = `${t('project.changed')} (${change.reason}, r${change.revision})`;
    loadProject().catch(report);
  });
  await loadProject();
}

function report(error) {
  console.error(error);
  $('status').textContent = `${error.code ?? 'error'}: ${error.message}`;
}

start().catch(report);
