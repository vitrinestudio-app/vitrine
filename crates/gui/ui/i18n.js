// Minimal UI translation loader. Catalogs live in ./i18n/<lang>.json as flat
// key → string maps. Unknown languages fall back to English.

const FALLBACK = 'en';
const SUPPORTED = ['en', 'de'];

let catalog = {};
let fallbackCatalog = {};

async function loadCatalog(lang) {
  const response = await fetch(`./i18n/${lang}.json`);
  if (!response.ok) {
    throw new Error(`Cannot load UI catalog ${lang}: ${response.status}`);
  }
  return response.json();
}

/** Picks the best supported language from the browser preferences. */
export function detectLanguage() {
  for (const tag of navigator.languages ?? [navigator.language]) {
    const base = tag.toLowerCase().split('-')[0];
    if (SUPPORTED.includes(base)) return base;
  }
  return FALLBACK;
}

/** Loads the catalog for `lang` (plus the English fallback). */
export async function initI18n(lang = detectLanguage()) {
  fallbackCatalog = await loadCatalog(FALLBACK);
  catalog = lang === FALLBACK ? fallbackCatalog : await loadCatalog(lang);
  document.documentElement.lang = lang;
  return lang;
}

/** Translates `key`, falling back to English and finally to the key itself. */
export function t(key) {
  return catalog[key] ?? fallbackCatalog[key] ?? key;
}

/** Fills every element carrying a `data-i18n` attribute below `root`. */
export function applyTranslations(root = document) {
  for (const el of root.querySelectorAll('[data-i18n]')) {
    el.textContent = t(el.dataset.i18n);
  }
}
