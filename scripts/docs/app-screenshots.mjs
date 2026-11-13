#!/usr/bin/env node
// Takes the app screenshots of the website (website/assets/images/*.jpg and
// their *-dark.jpg twins) from the dev bridge in headless Chrome.
//
// Node >= 22, no npm dependencies: Chrome is driven over the DevTools
// protocol with Node's global WebSocket. The shot list lives in
// app-shots.mjs next to this file; website/README.md ("Screenshots")
// documents the whole procedure.
//
//   node scripts/docs/app-screenshots.mjs init --root DIR [--cli BIN] [--synthetic-bezels BIN]
//       sample workspace (needs network for the gallery): downloads the
//       gallery templates to DIR/templates, installs them into DIR/app (the
//       bridge's app folder), creates the sample projects in DIR/Projects
//       (edits from app-shots.mjs), fake store keys in DIR/keys and the
//       synthetic bezel package DIR/bezels; existing parts are kept
//       (delete a project folder to recreate it)
//   node scripts/docs/app-screenshots.mjs shoot --root DIR [options]
//       takes the shots
//   node scripts/docs/app-screenshots.mjs list
//       lists the shots
//
// shoot options:
//   --port N        dev bridge port (default 7860); a bridge already running
//                   there is used (start it with --app-dir DIR/app --mcp),
//                   otherwise one is started and stopped again
//   --bridge BIN    dev bridge binary (default $VITRINE_DEVBRIDGE or
//                   target/release/vitrine-devbridge)
//   --only a,b      only these shots (names from `list`)
//   --theme T       light | dark | both (default both)
//   --out DIR       output folder (default website/assets/images)
//   --keep          keep the bridge running afterwards (when started here)
// Chrome: $CHROME, else the usual macOS / Linux paths.

import { spawn, execFileSync } from 'node:child_process';
import crypto from 'node:crypto';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

import { projects, shots, setup as globalSetup, PRIVACY } from './app-shots.mjs';

const here = path.dirname(fileURLToPath(import.meta.url));
const repo = path.resolve(here, '../..');

// Window and output conventions (website/README.md, "Screenshots").
const WINDOW = { width: 1600, height: 1000, scale: 2 };
const JPEG_QUALITY = 84;

// ----- arguments --------------------------------------------------------------

function parseArgs(argv) {
  const [command = 'help', ...rest] = argv;
  const o = { command, port: 7860, theme: 'both', out: path.join(repo, 'website/assets/images'), only: null, keep: false };
  for (let i = 0; i < rest.length; i++) {
    const a = rest[i];
    const v = () => {
      if (i + 1 >= rest.length) fail(`${a} needs a value`);
      return rest[++i];
    };
    if (a === '--root') o.root = path.resolve(v());
    else if (a === '--port') o.port = Number(v());
    else if (a === '--bridge') o.bridge = path.resolve(v());
    else if (a === '--cli') o.cli = path.resolve(v());
    else if (a === '--synthetic-bezels') o.synthetic = path.resolve(v());
    else if (a === '--only') o.only = v().split(',').map((s) => s.trim()).filter(Boolean);
    else if (a === '--theme') o.theme = v();
    else if (a === '--out') o.out = path.resolve(v());
    else if (a === '--keep') o.keep = true;
    else fail(`unknown argument ${a}`);
  }
  return o;
}

function fail(msg) {
  console.error(`app-screenshots: ${msg}`);
  process.exit(2);
}

function usage() {
  const text = fs.readFileSync(fileURLToPath(import.meta.url), 'utf8').split('\n');
  console.log(text.slice(1, text.findIndex((l) => l.startsWith('import'))).map((l) => l.replace(/^\/\/ ?/, '')).join('\n'));
}

// ----- init: sample workspace -------------------------------------------------------

function findBinary(name, explicit, envVar) {
  const candidates = [explicit, process.env[envVar], path.join(process.env.CARGO_TARGET_DIR ?? path.join(repo, 'target'), 'release', name), path.join(repo, 'target/release', name), path.join(repo, 'target/debug', name)];
  const hit = candidates.find((c) => c && fs.existsSync(c));
  if (!hit) fail(`${name} not found; build it (see website/README.md) or pass its path / set ${envVar}`);
  return hit;
}

function init(o) {
  if (!o.root) fail('--root DIR is required');
  const cli = findBinary('vitrine', o.cli, 'VITRINE_CLI');
  const appDir = path.join(o.root, 'app');
  const env = { ...process.env, VITRINE_APP_DIR: appDir };
  const run = (...args) => {
    console.log(`$ vitrine ${args.join(' ')}`);
    execFileSync(cli, args, { stdio: 'inherit', env, cwd: o.root });
  };
  fs.mkdirSync(path.join(o.root, 'templates'), { recursive: true });
  for (const [key, p] of Object.entries(projects)) {
    const dir = path.join(o.root, 'Projects', p.name);
    if (fs.existsSync(dir)) {
      console.log(`${key}: ${dir} exists, kept`);
      continue;
    }
    const file = path.join(o.root, 'templates', `${p.template}.vitrine-template`);
    if (!fs.existsSync(file)) run('gallery', 'download', p.template, '-o', file);
    // Installed app-wide too, so "Local templates" lists it as downloaded.
    run('template', 'import', file, '--replace');
    run('new', dir, '--template', p.template, ...p.langs.flatMap((l) => ['--lang', l]));
    for (const [rel, content] of Object.entries(p.files ?? {})) {
      if (content === null) fs.rmSync(path.join(dir, rel), { force: true });
      else fs.writeFileSync(path.join(dir, rel), content);
    }
    for (const fn of p.edit ?? []) fn(dir, fs, path);
    // Fails loudly when an edit broke project.yaml (exit 2); findings (exit 1) are fine.
    try {
      execFileSync(cli, ['--project', dir, 'validate', '--quiet'], { stdio: 'ignore', env });
    } catch (e) {
      if (e.status !== 1) fail(`${key}: the project does not load after the edits`);
    }
  }
  writeKeys(o.root);
  // Synthetic bezel package for the frame-import shot (never Apple artwork).
  const bezels = path.join(o.root, 'bezels', 'Product Bezels.zip');
  const synth = [o.synthetic, path.join(path.dirname(cli), 'examples', 'synthetic_bezels')].find((c) => c && fs.existsSync(c));
  if (fs.existsSync(bezels)) console.log(`bezels: ${bezels} exists, kept`);
  else if (synth) {
    fs.mkdirSync(path.dirname(bezels), { recursive: true });
    execFileSync(synth, [bezels], { stdio: 'inherit' });
  } else console.log('bezels: synthetic_bezels example not built, the frame-import shot will fail (see website/README.md)');
  console.log(`\nSample workspace ready in ${o.root}`);
}

/** Fake store keys: well-formed, never valid at Apple or Google. */
function writeKeys(root) {
  const dir = path.join(root, 'keys');
  fs.mkdirSync(dir, { recursive: true });
  const apple = path.join(dir, `AuthKey_${globalSetup.apple.keyId}.p8`);
  if (!fs.existsSync(apple)) {
    const { privateKey } = crypto.generateKeyPairSync('ec', { namedCurve: 'P-256' });
    fs.writeFileSync(apple, privateKey.export({ type: 'pkcs8', format: 'pem' }));
  }
  const google = path.join(dir, 'service-account.json');
  if (!fs.existsSync(google)) {
    const { privateKey } = crypto.generateKeyPairSync('rsa', { modulusLength: 2048 });
    const g = globalSetup.google;
    fs.writeFileSync(
      google,
      JSON.stringify(
        {
          type: 'service_account',
          project_id: g.projectId,
          private_key_id: crypto.randomBytes(20).toString('hex'),
          private_key: privateKey.export({ type: 'pkcs8', format: 'pem' }),
          client_email: g.clientEmail,
          token_uri: 'https://oauth2.googleapis.com/token',
        },
        null,
        2,
      ),
    );
  }
}

// ----- dev bridge --------------------------------------------------------------------

async function bridgeUp(base) {
  try {
    await fetch(`${base}/`);
    return true;
  } catch {
    return false;
  }
}

async function startBridge(o) {
  const base = `http://127.0.0.1:${o.port}`;
  if (await bridgeUp(base)) {
    console.log(`using the dev bridge on ${base}`);
    return { base, proc: null };
  }
  const bin = findBinary('vitrine-devbridge', o.bridge, 'VITRINE_DEVBRIDGE');
  const args = ['--port', String(o.port), '--app-dir', path.join(o.root, 'app'), '--ui', path.join(repo, 'crates/gui/ui'), '--mcp'];
  console.log(`starting ${bin} ${args.join(' ')}`);
  const proc = spawn(bin, args, { stdio: ['ignore', 'pipe', 'inherit'], env: { ...process.env, VITRINE_KEYRING: 'mock' } });
  await new Promise((resolve, reject) => {
    let out = '';
    proc.stdout.on('data', (d) => {
      out += d;
      if (out.includes('dev bridge:')) resolve();
    });
    proc.on('exit', (code) => reject(new Error(`dev bridge exited (${code})`)));
    setTimeout(() => reject(new Error('dev bridge did not start')), 20000);
  });
  return { base, proc };
}

async function cmd(base, name, args = {}) {
  const r = await fetch(`${base}/cmd/${name}`, { method: 'POST', headers: { 'Content-Type': 'application/json' }, body: JSON.stringify(args) });
  const body = await r.json().catch(() => null);
  if (!r.ok) throw new Error(`${name}: ${body?.message ?? r.statusText}`);
  return body;
}

/** Store accounts from the fake keys (in-memory keychain of the bridge). */
async function ensureAccounts(base, root) {
  const have = await cmd(base, 'store_accounts_list');
  const list = Array.isArray(have) ? have : (have.accounts ?? []);
  const keys = path.join(root, 'keys');
  const a = globalSetup.apple;
  const g = globalSetup.google;
  if (!list.some((x) => x.label === a.label)) {
    await cmd(base, 'store_account_import', { kind: 'apple', path: path.join(keys, `AuthKey_${a.keyId}.p8`), label: a.label, issuer_id: a.issuerId });
  }
  if (!list.some((x) => x.label === g.label)) {
    await cmd(base, 'store_account_import', { kind: 'google', path: path.join(keys, 'service-account.json'), label: g.label });
  }
}

// ----- Chrome over the DevTools protocol ---------------------------------------------

function findChrome() {
  const candidates = [
    process.env.CHROME,
    '/Applications/Google Chrome.app/Contents/MacOS/Google Chrome',
    '/Applications/Chromium.app/Contents/MacOS/Chromium',
    '/usr/bin/google-chrome',
    '/usr/bin/chromium',
    '/usr/bin/chromium-browser',
  ];
  const hit = candidates.find((c) => c && fs.existsSync(c));
  if (!hit) fail('Chrome not found; set CHROME to its binary');
  return hit;
}

async function launchChrome() {
  const profile = fs.mkdtempSync(path.join(os.tmpdir(), 'vitrine-shots-chrome-'));
  const proc = spawn(
    findChrome(),
    [
      '--headless=new',
      '--remote-debugging-port=0',
      `--user-data-dir=${profile}`,
      `--window-size=${WINDOW.width},${WINDOW.height}`,
      `--force-device-scale-factor=${WINDOW.scale}`,
      '--hide-scrollbars',
      '--no-first-run',
      '--no-default-browser-check',
      '--font-render-hinting=none',
      'about:blank',
    ],
    { stdio: 'ignore' },
  );
  const portFile = path.join(profile, 'DevToolsActivePort');
  for (let i = 0; i < 100 && !fs.existsSync(portFile); i++) await sleep(100);
  const [port, wsPath] = fs.readFileSync(portFile, 'utf8').split('\n');
  const cdp = await connect(`ws://127.0.0.1:${port}${wsPath}`);
  const { targetId } = await cdp.send('Target.createTarget', { url: 'about:blank' });
  const { sessionId } = await cdp.send('Target.attachToTarget', { targetId, flatten: true });
  const page = (method, params) => cdp.send(method, params, sessionId);
  await page('Page.enable');
  await page('Runtime.enable');
  await page('Emulation.setDeviceMetricsOverride', { width: WINDOW.width, height: WINDOW.height, deviceScaleFactor: WINDOW.scale, mobile: false });
  return {
    page,
    cdp,
    close() {
      proc.kill();
      setTimeout(() => fs.rmSync(profile, { recursive: true, force: true }), 500);
    },
  };
}

function connect(url) {
  return new Promise((resolve, reject) => {
    const ws = new WebSocket(url);
    let id = 0;
    const pending = new Map();
    const waiters = [];
    ws.onmessage = (ev) => {
      const msg = JSON.parse(ev.data);
      if (msg.id && pending.has(msg.id)) {
        const { ok, ko } = pending.get(msg.id);
        pending.delete(msg.id);
        msg.error ? ko(new Error(`${msg.error.message} ${msg.error.data ?? ''}`)) : ok(msg.result);
      } else if (msg.method) {
        for (const w of [...waiters]) if (w.method === msg.method) (waiters.splice(waiters.indexOf(w), 1), w.ok(msg.params));
      }
    };
    ws.onerror = reject;
    ws.onopen = () =>
      resolve({
        send(method, params = {}, sessionId) {
          return new Promise((ok, ko) => {
            pending.set(++id, { ok, ko });
            ws.send(JSON.stringify({ id, method, params, ...(sessionId ? { sessionId } : {}) }));
          });
        },
        once(method) {
          return new Promise((ok) => waiters.push({ method, ok }));
        },
        close: () => ws.close(),
      });
  });
}

const sleep = (ms) => new Promise((r) => setTimeout(r, ms));

/** Evaluates `body` (an async function body) in the page with `ui` in scope. */
async function evaluate(page, body, arg) {
  const expression = `(async (ui, arg) => { ${body}\n })(window.__shots, ${JSON.stringify(arg ?? null)})`;
  const r = await page('Runtime.evaluate', { expression, awaitPromise: true, returnByValue: true });
  if (r.exceptionDetails) {
    const d = r.exceptionDetails;
    throw new Error(`page: ${d.exception?.description ?? d.text}`);
  }
  return r.result.value;
}

// Helpers available to the page steps as `ui.*`.
const PAGE_HELPERS = `
window.__shots = {
  sleep: (ms) => new Promise((r) => setTimeout(r, ms)),
  /** An app module, e.g. await ui.mod('dialogs/upload'). */
  mod: (name) => import('/js/' + name + '.js'),
  async store() { return (await this.mod('store')).store; },
  async editor() { return (await this.mod('editor')).editor; },
  /** Element by selector (and optional text), waiting up to 'timeout' ms. */
  async waitFor(sel, { text = null, timeout = 10000 } = {}) {
    const t0 = performance.now();
    for (;;) {
      const el = [...document.querySelectorAll(sel)].find((e) => !text || e.textContent.includes(text));
      if (el) return el;
      if (performance.now() - t0 > timeout) throw new Error('not found: ' + sel + (text ? ' "' + text + '"' : ''));
      await this.sleep(50);
    }
  },
  async click(sel, opts) { const el = await this.waitFor(sel, opts); el.click(); return el; },
  /** Waits until no pixmap slot of the view is loading. */
  async idle(timeout = 15000) {
    const t0 = performance.now();
    await this.sleep(300);
    const busy = '.slot-loading, img.loading, .tg-spinner, [aria-busy="true"]';
    while (document.querySelector(busy) && performance.now() - t0 < timeout) await this.sleep(100);
    await document.fonts.ready;
    await this.sleep(300);
  },
  /** Replaces text in text nodes, form values, titles (privacy, see PRIVACY in app-shots.mjs). */
  scrub(rules) {
    const apply = (s) => rules.reduce((acc, [from, to]) => acc.replace(new RegExp(from.source, from.flags), to), s);
    const walk = document.createTreeWalker(document.body, NodeFilter.SHOW_TEXT);
    for (let n; (n = walk.nextNode()); ) { const v = apply(n.nodeValue); if (v !== n.nodeValue) n.nodeValue = v; }
    for (const el of document.querySelectorAll('input, textarea')) { const v = apply(el.value); if (v !== el.value) el.value = v; }
    for (const el of document.querySelectorAll('[title]')) el.title = apply(el.title);
    for (const el of document.querySelectorAll('[placeholder]')) el.placeholder = apply(el.placeholder);
  },
};
`;

/** Clip rectangle in CSS px. */
async function clipRect(page, clip) {
  if (!clip || clip === 'window') return { x: 0, y: 0, width: WINDOW.width, height: WINDOW.height };
  if (typeof clip === 'object' && 'width' in clip && !clip.selector) return clip;
  if (typeof clip === 'object' && clip.script) {
    const r = await evaluate(page, clip.script);
    return { x: Math.max(0, Math.round(r.x)), y: Math.max(0, Math.round(r.y)), width: Math.round(r.width), height: Math.round(r.height) };
  }
  const spec = typeof clip === 'string' ? { selector: clip } : clip;
  const r = await evaluate(
    page,
    `const el = await ui.waitFor(arg.selector, { text: arg.text });
     const r = el.getBoundingClientRect();
     return { x: r.x, y: r.y, width: r.width, height: r.height };`,
    spec,
  );
  const pad = spec.pad ?? 0;
  const x = Math.max(0, Math.floor(r.x - pad));
  const y = Math.max(0, Math.floor(r.y - pad));
  const rect = {
    x,
    y,
    width: Math.min(WINDOW.width - x, Math.ceil(r.width + 2 * pad)),
    height: Math.min(WINDOW.height - y, Math.ceil(r.height + 2 * pad)),
  };
  if (spec.width) rect.width = spec.width;
  if (spec.height) rect.height = spec.height;
  if (spec.maxHeight) rect.height = Math.min(rect.height, spec.maxHeight);
  return rect;
}

/** Node-side input steps: { click | rightClick | dblClick | hover | wheel: selector, text?, at?: [fx, fy] }. */
async function pointer(page, step) {
  const kind = ['click', 'rightClick', 'dblClick', 'hover', 'wheel'].find((k) => k in step);
  if (!kind) throw new Error(`unknown step ${JSON.stringify(step)}`);
  const r = await evaluate(page, `const el = await ui.waitFor(arg.sel, { text: arg.text }); const r = el.getBoundingClientRect(); return { x: r.x, y: r.y, w: r.width, h: r.height };`, { sel: step[kind], text: step.text });
  const [fx, fy] = step.at ?? [0.5, 0.5];
  const x = r.x + r.w * fx;
  const y = r.y + r.h * fy;
  await page('Input.dispatchMouseEvent', { type: 'mouseMoved', x, y });
  if (kind === 'hover') return;
  if (kind === 'wheel') {
    // { wheel: sel, deltaY, deltaX?, ctrl?: true (zoom), times? }
    for (let i = 0; i < (step.times ?? 1); i++) {
      await page('Input.dispatchMouseEvent', { type: 'mouseWheel', x, y, deltaX: step.deltaX ?? 0, deltaY: step.deltaY ?? 0, modifiers: step.ctrl ? 2 : 0 });
      await sleep(60);
    }
    return;
  }
  const button = kind === 'rightClick' ? 'right' : 'left';
  const clicks = kind === 'dblClick' ? 2 : 1;
  for (let c = 1; c <= clicks; c++) {
    await page('Input.dispatchMouseEvent', { type: 'mousePressed', x, y, button, clickCount: c, buttons: button === 'right' ? 2 : 1 });
    await page('Input.dispatchMouseEvent', { type: 'mouseReleased', x, y, button, clickCount: c });
  }
}

// ----- shoot -------------------------------------------------------------------------

async function shoot(o) {
  if (!o.root) fail('--root DIR is required (see `init`)');
  const list = o.only ? o.only.map((n) => shots.find((s) => s.name === n) ?? fail(`no shot ${n}`)) : shots;
  const themes = o.theme === 'both' ? ['light', 'dark'] : [o.theme];
  const { base, proc } = await startBridge(o);
  const chrome = await launchChrome();
  const { page } = chrome;
  const rootReal = fs.realpathSync(o.root);
  const rules = [
    [path.join(rootReal, 'app', 'data'), PRIVACY.appData],
    [path.join(o.root, 'app', 'data'), PRIVACY.appData],
    [rootReal, PRIVACY.home],
    [o.root, PRIVACY.home],
    ...PRIVACY.rules,
  ];
  const escape = (s) => s.replace(/[.*+?^${}()|[\]\\]/g, '\\$&');
  const scrubRules = rules.map(([a, b]) => [a instanceof RegExp ? { source: a.source, flags: a.flags } : { source: escape(a), flags: 'g' }, b]);
  let openProject = null;
  try {
    await ensureAccounts(base, o.root);
    for (const shot of list) {
      // Project: open it (and run its one-time setup commands) when it changes.
      if (shot.project !== openProject) {
        if (shot.project) {
          const p = projects[shot.project];
          await cmd(base, 'project_open', { dir: path.join(o.root, 'Projects', p.name) });
        } else {
          await cmd(base, 'project_close').catch(() => {});
        }
        openProject = shot.project ?? null;
      }
      for (const theme of themes) {
        const params = new URLSearchParams({ lang: 'en', theme, ...(shot.query ?? {}) });
        const loaded = chrome.cdp.once('Page.loadEventFired');
        await page('Page.navigate', { url: `${base}/?${params}` });
        await loaded;
        await evaluate(page, PAGE_HELPERS + 'return true;');
        if (shot.project) await evaluate(page, `await ui.waitFor('.topbar, #app, body'); for (let i = 0; i < 200 && !(await ui.store()).get().summary; i++) await ui.sleep(50);`);
        await evaluate(page, 'await ui.idle();');
        const run = async (step) => {
          if (typeof step === 'string') await evaluate(page, step, { root: o.root, rootReal, theme });
          else if (step.cmd) await cmd(base, step.cmd, step.args ?? {});
          else if (step.key) {
            await page('Input.dispatchKeyEvent', { type: 'keyDown', key: step.key, code: step.code ?? step.key, windowsVirtualKeyCode: step.keyCode ?? 0 });
            await page('Input.dispatchKeyEvent', { type: 'keyUp', key: step.key, code: step.code ?? step.key });
          } else if (step.wait) await sleep(step.wait);
          else await pointer(page, step);
        };
        try {
          for (const step of shot.steps ?? []) await run(step);
          await evaluate(page, 'await ui.idle();');
          await sleep(shot.wait ?? 400);
          await evaluate(page, 'ui.scrub(arg.rules); for (const sel of arg.hide) for (const el of document.querySelectorAll(sel)) el.style.display = "none"; document.activeElement?.blur?.();', { rules: scrubRules, hide: shot.hide ?? [] });
          const rect = await clipRect(page, shot.clip);
          const maxWidth = shot.maxWidth ?? 1520;
          const scale = Math.min(1, maxWidth / (rect.width * WINDOW.scale));
          const { data } = await page('Page.captureScreenshot', { format: 'jpeg', quality: JPEG_QUALITY, clip: { ...rect, scale }, captureBeyondViewport: false });
          const file = path.join(o.out, `${shot.name}${theme === 'dark' ? '-dark' : ''}.jpg`);
          fs.writeFileSync(file, Buffer.from(data, 'base64'));
          console.log(`${file.startsWith(repo) ? path.relative(repo, file) : file}  ${Math.round(rect.width * WINDOW.scale * scale)}×${Math.round(rect.height * WINDOW.scale * scale)}`);
        } finally {
          // `after`: undo what the steps changed in the project (e.g. {cmd: 'undo'}).
          for (const step of shot.after ?? []) await run(step).catch((e) => console.error(`${shot.name}: after: ${e.message}`));
        }
      }
    }
  } finally {
    chrome.close();
    if (proc && !o.keep) proc.kill();
  }
}

// ----- main --------------------------------------------------------------------------

const o = parseArgs(process.argv.slice(2));
if (o.command === 'init') init(o);
else if (o.command === 'shoot') await shoot(o);
else if (o.command === 'list') for (const s of shots) console.log(`${s.name.padEnd(22)} ${s.project ?? '-'}  ${s.note ?? ''}`);
else usage();
