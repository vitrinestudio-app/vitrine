import { applyTranslations, initI18n } from './i18n.js';

const { invoke } = window.__TAURI__.core;

async function start() {
  await initI18n();
  applyTranslations();

  const info = await invoke('app_info');
  document.getElementById('app-version').textContent = info.version;
}

start().catch((error) => console.error(error));
