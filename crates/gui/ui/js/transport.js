// Transport shim: the same `invoke`/`listen` API on top of Tauri IPC (in the
// app) or the dev HTTP bridge (`arrangia-devbridge`, in a normal browser).
// Commands take one JSON object of arguments; image commands resolve to a
// preview `{ width, height, format, originX, originY, scale, data }` where
// `data` is an ArrayBuffer holding PNG or raw RGBA (straight alpha).

const tauri = window.__TAURI__;

/** True inside the Tauri app. */
export const isTauri = Boolean(tauri?.core?.invoke);

/** Commands answering with a binary preview image. */
export const IMAGE_COMMANDS = new Set(['render_image', 'render_canvas', 'render_gallery', 'render_preview_edit']);

const FORMATS = { 0: 'rgba', 1: 'png' };

/** Error of a command: `code` (e.g. `no_project`), message, details. */
export class CommandError extends Error {
  constructor({ code = 'internal', message = 'unknown error', details } = {}) {
    super(message);
    this.name = 'CommandError';
    this.code = code;
    this.details = details;
  }
}

function toError(e) {
  if (e instanceof CommandError) return e;
  if (e && typeof e === 'object' && 'code' in e) return new CommandError(e);
  return new CommandError({ code: 'internal', message: String(e?.message ?? e) });
}

/** Parses the Tauri image payload (40-byte header + data, see preview.rs). */
function parseTauriImage(buffer) {
  const view = new DataView(buffer);
  const magic = String.fromCharCode(...new Uint8Array(buffer, 0, 4));
  if (magic !== 'ARPX') throw new CommandError({ message: 'malformed image payload' });
  return {
    width: view.getUint32(4, true),
    height: view.getUint32(8, true),
    format: FORMATS[view.getUint32(12, true)],
    originX: view.getFloat64(16, true),
    originY: view.getFloat64(24, true),
    scale: view.getFloat64(32, true),
    data: buffer.slice(40),
  };
}

/** Runs backend command `cmd` with `args`. */
export async function invoke(cmd, args = {}) {
  if (isTauri) {
    try {
      const result = await tauri.core.invoke(cmd, { args });
      return IMAGE_COMMANDS.has(cmd) ? parseTauriImage(result) : result;
    } catch (e) {
      throw toError(e);
    }
  }
  const response = await fetch(`/cmd/${encodeURIComponent(cmd)}`, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify(args),
  });
  if (!response.ok) {
    const body = await response.json().catch(() => ({ message: response.statusText }));
    throw toError({ code: 'internal', ...body });
  }
  if (IMAGE_COMMANDS.has(cmd)) {
    const h = (name) => response.headers.get(name);
    return {
      width: Number(h('X-Width')),
      height: Number(h('X-Height')),
      format: h('X-Format'),
      originX: Number(h('X-Origin-X')),
      originY: Number(h('X-Origin-Y')),
      scale: Number(h('X-Scale')),
      data: await response.arrayBuffer(),
    };
  }
  return response.json();
}

/** Decodes a preview from `invoke` into an ImageBitmap. */
export function toImageBitmap(image) {
  if (image.format === 'rgba') {
    const pixels = new Uint8ClampedArray(image.data);
    return createImageBitmap(new ImageData(pixels, image.width, image.height));
  }
  return createImageBitmap(new Blob([image.data], { type: 'image/png' }));
}

let source = null;

/**
 * Calls `callback(payload)` for every backend event `event`
 * (`project-changed`, `export-progress` …). Resolves to an unlisten function.
 */
export async function listen(event, callback) {
  if (isTauri) {
    return tauri.event.listen(event, (e) => callback(e.payload));
  }
  source ??= new EventSource('/events');
  const handler = (e) => callback(JSON.parse(e.data));
  source.addEventListener(event, handler);
  return () => source.removeEventListener(event, handler);
}
