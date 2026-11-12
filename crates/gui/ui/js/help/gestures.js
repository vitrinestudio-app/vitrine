// The one list of keyboard shortcuts, mouse gestures and modifiers of the
// editor, grouped by area. The "Keyboard & mouse" help sheet
// (help/sheet.js) is generated from it, so keep it complete: whoever adds,
// changes or removes a gesture (shortcuts.js, views/*.js, panels/*.js,
// menu.js …) updates its entry here in the same change. Only list what
// really exists.
//
// Notation of `keys` (alternatives of one entry; each a `+`-joined combo):
//   modifiers  mod (⌘ on macOS, Ctrl elsewhere), shift, alt (⌥/Alt), space
//   mouse      click, dblclick, drag, middle-drag, right-click, wheel, pinch,
//              drop, scrollbar
//   keys       enter, esc, backspace, delete, tab, f2, up, down, left, right,
//              arrows (all four), or the key itself: a, z, [, ], ?, 0, = …
// Labels of the named tokens: `help.key.<token>` in both i18n catalogs.
//
// Gestures that change with a pending feature carry `when: '<feature>'`
// (shown only when it is on) or `unless: '<feature>'` (shown only while it
// is off); flip the flag in `FEATURES` when the feature lands.

/**
 * Pending canvas gesture changes (canvas view, views/canvas.js +
 * views/viewport.js). Set a flag to true in the change that implements it.
 */
export const FEATURES = {
  /** Plain drag on an empty canvas area pans; Shift+drag draws the selection frame. */
  canvasDragPans: true,
  /** The canvas view has scrollbars. */
  canvasScrollbars: true,
  /** Right-click on a canvas element opens its context menu. */
  elementContextMenu: true,
};

/**
 * `[{ id, title, items: [{ keys: [combo…], text, when?, unless? }] }]`;
 * `title`/`text` are i18n keys.
 */
export const GESTURES = [
  {
    id: 'navigation',
    title: 'help.group.navigation',
    items: [
      { keys: ['wheel'], text: 'help.g.scroll' },
      { keys: ['shift+wheel'], text: 'help.g.scroll_h' },
      { keys: ['mod+wheel', 'pinch'], text: 'help.g.zoom_wheel' },
      { keys: ['drag'], text: 'help.g.pan_canvas_drag', when: 'canvasDragPans' },
      { keys: ['space+drag', 'middle-drag'], text: 'help.g.pan' },
      { keys: ['drag'], text: 'help.g.pan_views', unless: 'canvasDragPans' },
      { keys: ['scrollbar'], text: 'help.g.scrollbars', when: 'canvasScrollbars' },
      { keys: ['mod+=', 'mod+-'], text: 'help.g.zoom_keys' },
      { keys: ['mod+0'], text: 'help.g.zoom_fit' },
      { keys: ['mod+1'], text: 'help.g.zoom_actual' },
    ],
  },
  {
    id: 'selection',
    title: 'help.group.selection',
    items: [
      { keys: ['click'], text: 'help.g.select' },
      { keys: ['dblclick'], text: 'help.g.select_deeper' },
      { keys: ['mod+click'], text: 'help.g.select_direct' },
      { keys: ['dblclick'], text: 'help.g.select_background' },
      { keys: ['shift+click'], text: 'help.g.select_toggle' },
      { keys: ['drag'], text: 'help.g.marquee', unless: 'canvasDragPans' },
      { keys: ['shift+drag'], text: 'help.g.marquee_add', unless: 'canvasDragPans' },
      { keys: ['shift+drag'], text: 'help.g.marquee_shift', when: 'canvasDragPans' },
      { keys: ['mod+a'], text: 'help.g.select_all' },
      { keys: ['esc'], text: 'help.g.escape' },
    ],
  },
  {
    id: 'arrange',
    title: 'help.group.arrange',
    items: [
      { keys: ['drag'], text: 'help.g.move' },
      { keys: ['shift+drag'], text: 'help.g.move_axis' },
      { keys: ['alt+drag'], text: 'help.g.no_snap' },
      { keys: ['shift+drag'], text: 'help.g.resize_aspect' },
      { keys: ['drag'], text: 'help.g.rotate' },
      { keys: ['shift+drag'], text: 'help.g.rotate_free' },
      { keys: ['arrows'], text: 'help.g.nudge' },
      { keys: ['shift+arrows'], text: 'help.g.nudge_big' },
      { keys: ['mod+d'], text: 'help.g.duplicate' },
      { keys: ['mod+g'], text: 'help.g.group' },
      { keys: ['mod+shift+g'], text: 'help.g.ungroup' },
      { keys: ['backspace', 'delete'], text: 'help.g.delete' },
      { keys: ['[', ']'], text: 'help.g.order' },
      { keys: ['shift+[', 'shift+]'], text: 'help.g.order_end' },
      { keys: ['mod+z'], text: 'help.g.undo' },
      { keys: ['mod+shift+z', 'mod+y'], text: 'help.g.redo' },
      { keys: ['right-click'], text: 'help.g.context_menu', when: 'elementContextMenu' },
      { keys: ['shift+f10'], text: 'help.g.context_menu_key', when: 'elementContextMenu' },
      { keys: ['mod+c', 'mod+x', 'mod+v'], text: 'help.g.clipboard' },
      { keys: ['drop'], text: 'help.g.drop_device' },
      { keys: ['drop'], text: 'help.g.drop_import' },
    ],
  },
  {
    id: 'text',
    title: 'help.group.text',
    items: [
      { keys: ['dblclick'], text: 'help.g.text_edit' },
      { keys: ['enter'], text: 'help.g.text_commit' },
      { keys: ['shift+enter'], text: 'help.g.text_break' },
      { keys: ['esc'], text: 'help.g.text_cancel' },
      { keys: ['mod+enter'], text: 'help.g.text_inspector' },
      { keys: ['click', 'enter'], text: 'help.g.translations_cell' },
    ],
  },
  {
    id: 'panels',
    title: 'help.group.panels',
    items: [
      { keys: ['drag'], text: 'help.g.scrub' },
      { keys: ['up', 'down'], text: 'help.g.num_step' },
      { keys: ['shift+up', 'shift+down'], text: 'help.g.num_step_big' },
      { keys: ['enter'], text: 'help.g.num_apply' },
      { keys: ['esc'], text: 'help.g.num_revert' },
      { keys: ['shift+click'], text: 'help.g.layers_range' },
      { keys: ['mod+click'], text: 'help.g.layers_toggle' },
      { keys: ['up', 'down'], text: 'help.g.layers_walk' },
      { keys: ['dblclick', 'enter', 'f2'], text: 'help.g.layers_rename' },
      { keys: ['drag'], text: 'help.g.layers_reorder' },
      { keys: ['drag', 'dblclick'], text: 'help.g.inspector_resize' },
      { keys: ['click', 'enter'], text: 'help.g.asset_preview' },
      { keys: ['left', 'right'], text: 'help.g.preview_step' },
    ],
  },
  {
    id: 'app',
    title: 'help.group.app',
    items: [
      { keys: ['?'], text: 'help.g.help' },
      { keys: ['mod+n'], text: 'help.g.new' },
      { keys: ['mod+o'], text: 'help.g.open' },
      { keys: ['mod+e'], text: 'help.g.export' },
      { keys: ['mod+,'], text: 'help.g.settings' },
      { keys: ['esc'], text: 'help.g.dialog_close' },
      { keys: ['enter'], text: 'help.g.dialog_confirm' },
      { keys: ['drop'], text: 'help.g.drop_folder' },
      { keys: ['alt+right-click'], text: 'help.g.debug_menu' },
    ],
  },
];

/** The groups with only the entries that apply to the current features. */
export function activeGestures(features = FEATURES) {
  const on = (it) => (!it.when || features[it.when]) && (!it.unless || !features[it.unless]);
  return GESTURES.map((g) => ({ ...g, items: g.items.filter(on) })).filter((g) => g.items.length);
}
