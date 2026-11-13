//! Reference documentation for the model, served by the `docs_get` tool
//! (and as resources `vitrine://docs/<topic>`).
//!
//! Some MCP clients (Claude Desktop) only let the *user* attach resources,
//! so the assistant cannot read `vitrine://docs/ops` by itself; a tool
//! can always be called. The element field reference is generated from the
//! model's field lists (`ElementKindTag::fields`, [`COMMON_FIELDS`],
//! [`LANG_OVERRIDE_FIELDS`]) plus a description table that a test keeps
//! complete.

use std::fmt::Write as _;

use vitrine_core::model::{COMMON_FIELDS, ElementKindTag, ElementPatch, LANG_OVERRIDE_FIELDS};

/// Topics of `docs_get`, with a one-line summary each.
pub const TOPICS: [(&str, &str); 7] = [
    (
        "overview",
        "concepts: project, canvas, pages, lu, elements, targets, workflow",
    ),
    (
        "elements",
        "every element kind with all fields, types, defaults, allowed values",
    ),
    (
        "ops",
        "JSON shape of every op for apply_ops (incl. override ops), with examples",
    ),
    (
        "project",
        "project.yaml structure: languages, styles, canvases, variants, targets, import",
    ),
    (
        "markup",
        "inline text markup in translations, auto-fit, hyphenation",
    ),
    (
        "targets",
        "targets: presets, sizes, gallery gaps, group, variants, banners",
    ),
    (
        "scopes",
        "base / variant / language overrides: resolution order, setting, resetting",
    ),
];

const OVERVIEW: &str = include_str!("docs/overview.md");
const PROJECT: &str = include_str!("docs/project.md");
const MARKUP: &str = include_str!("docs/markup.md");
const TARGETS: &str = include_str!("docs/targets.md");
const SCOPES: &str = include_str!("docs/scopes.md");

/// Markdown of `topic` (`None` for an unknown topic).
pub fn topic(topic: &str) -> Option<String> {
    Some(match topic {
        "overview" => {
            let mut s = OVERVIEW.to_owned();
            s.push_str("\n## More topics (docs_get {topic})\n");
            for (t, what) in TOPICS {
                let _ = writeln!(s, "- `{t}`: {what}");
            }
            s
        }
        "elements" => elements(),
        "ops" => crate::resources::OPS_DOC.to_owned(),
        "project" => PROJECT.to_owned(),
        "markup" => MARKUP.to_owned(),
        "targets" => TARGETS.to_owned(),
        "scopes" => SCOPES.to_owned(),
        _ => return None,
    })
}

const KINDS: [ElementKindTag; 6] = [
    ElementKindTag::Device,
    ElementKindTag::Text,
    ElementKindTag::Image,
    ElementKindTag::Shape,
    ElementKindTag::Background,
    ElementKindTag::Group,
];

/// Field descriptions: `kind.field` for a meaning specific to one kind,
/// else `field`. Format: "type – meaning (default …)".
const FIELD_DOCS: &[(&str, &str)] = &[
    // common
    (
        "page",
        "integer ≥ 0 – home page (0-based); `at` is relative to it. Always written",
    ),
    (
        "lock_page",
        "bool – keep the home page when moved onto another page (default false)",
    ),
    (
        "locked",
        "bool – locked in the GUI editor: the canvas does not select or drag it (default false; no effect on rendering)",
    ),
    (
        "at",
        "{x, y} lu – centre of the element relative to the top-left corner of its home page (default {x: 0, y: 0}!). Text with `box` ignores it",
    ),
    ("width", "number lu – width"),
    ("height", "number lu – height"),
    (
        "rotation",
        "number – degrees, positive = clockwise (default 0)",
    ),
    (
        "pivot",
        "{x, y} lu – rotation pivot as offset from the centre (default: the centre)",
    ),
    (
        "anchor_y",
        "top | bottom | center | stretch – which page edge the element keeps its distance to when a target's page height differs (default top; background: stretch)",
    ),
    ("opacity", "number 0..1 (default 1)"),
    (
        "z",
        "integer – layer; painting order is (z, list order) (default 0)",
    ),
    (
        "clip",
        "none | page – page: clipped to the home page (default page for text, none otherwise)",
    ),
    (
        "visible",
        "bool (default true); false hides it, typical in a variant override",
    ),
    (
        "visible_in",
        "{targets?: [target ids], langs?: [languages]} – only shown there (empty list = no restriction)",
    ),
    (
        "allow",
        "[finding codes] – design findings meant on purpose for this element (and a group's children), not reported by validate: text_near_device, low_contrast, text_clipped, text_in_gap, text_outside_safe_area, element_outside, screen_header_covered, screen_header_cut, screen_covered, device_cut, seam_overlap, seam_split, element_in_corner, device_small, page_not_covered, page_blends_with_store, hyphenated, variant_suggested, screenshot_upscale (e.g. `allow: [text_clipped, low_contrast]` on giant decorative type)",
    ),
    // device
    (
        "model",
        "string – device model id from catalog_devices, e.g. `iphone-island` (required)",
    ),
    (
        "orientation",
        "portrait | landscape (default: the model's natural orientation)",
    ),
    (
        "frame",
        "stylized | detailed | imported:<source> (e.g. imported:apple, see frames_list) (default: styles.frame, else detailed)",
    ),
    (
        "bezel",
        "string – pin one imported bezel (imported frames only; default: chosen by target/screenshot size)",
    ),
    (
        "device.color",
        "string – colour variant of the model (catalog_devices colors; imported frames: vendor colour names) or #RRGGBB",
    ),
    (
        "fold",
        "open | folded – foldables (e.g. iphone-duo): open = inner display, folded = outer display (default: the model's own state)",
    ),
    (
        "chrome",
        "none | browser – browser window around the screen (web/desktop) (default none)",
    ),
    ("url", "string – address bar text with chrome browser"),
    (
        "appearance",
        "light | dark – look of the browser chrome (default light)",
    ),
    (
        "screenshot",
        "string – screenshot key: shows screenshots/<lang>/[<variant>/]<key>.png (fallback language chain; placeholder when missing)",
    ),
    (
        "device.width",
        "number lu – width; the height follows the model's aspect (default 800)",
    ),
    (
        "device.height",
        "number lu – not used (the height follows the width)",
    ),
    (
        "device.fit",
        "cover | contain | stretch – screenshot in the display (default cover)",
    ),
    (
        "device.shadow",
        "string – shadow preset: soft | hard | none or a styles.shadows name (default: none)",
    ),
    // text
    (
        "text",
        "string – translation key, e.g. hero.title (required); the text lives in i18n/<lang>.yaml (strings_set)",
    ),
    (
        "style",
        "string – text style name from styles.text (font, weight, size, line height, colour …)",
    ),
    (
        "box",
        "{top?, left?, right?, bottom?} lu – margins to the home page edges; missing bottom = the box grows downwards. With `box` the text is a margin box (at/width are ignored); without it a free box centred at `at` with `width`",
    ),
    ("align", "left | center | right (default left)"),
    (
        "valign",
        "top | center | bottom – vertical alignment in the box (default top for margin boxes, center for free boxes)",
    ),
    (
        "text.fit",
        "{shrink_to?: number lu, max_lines?: integer} – auto-fit: the font shrinks down to shrink_to until the text fits the box / max_lines",
    ),
    (
        "text.color",
        "colour #RRGGBB[AA] or @name – overrides the style's colour",
    ),
    (
        "font_size",
        "number lu – overrides the style's size (the start size for auto-fit)",
    ),
    (
        "text.stroke",
        "{color, width lu, position?: outside | center (default outside), join?: round | miter | bevel (default round)} – glyph outline; width 0 switches off the style's outline",
    ),
    (
        "glow",
        "{color, radius lu (blur sigma), opacity?: 0..1 (default 1), spread?: lu (default 0)} – soft glow; opacity 0 switches off the style's glow",
    ),
    (
        "text.shadow",
        "string – shadow preset (soft | hard | none | styles.shadows name) (default: the style's)",
    ),
    (
        "hyphenate",
        "bool – automatic hyphenation when the text does not fit at fit.shrink_to (default: the style's, else false)",
    ),
    (
        "text.width",
        "number lu – width of a free box (no `box`) (default 840)",
    ),
    (
        "text.height",
        "number lu – height of a free box (default: grows with the text)",
    ),
    // image
    (
        "asset",
        "string – project-relative image path, e.g. assets/badge.png (assets_import kind asset)",
    ),
    (
        "image.fit",
        "cover | contain | stretch – image in its box (default contain)",
    ),
    ("image.width", "number lu – width (required)"),
    (
        "image.height",
        "number lu – height (default: from the image's aspect)",
    ),
    (
        "image.shadow",
        "string – shadow preset; follows the image's alpha (default: none)",
    ),
    // shape
    ("shape", "rect | ellipse | line (required)"),
    ("radius", "number lu – corner radius of rect (default 0)"),
    (
        "shape.fill",
        "fill (see Value shapes) – the inside (default: none)",
    ),
    (
        "shape.stroke",
        "{color, width lu, join?: round | miter | bevel (default miter)} – outline centred on the path",
    ),
    ("shape.shadow", "string – shadow preset (default: none)"),
    ("shape.width", "number lu – width"),
    ("shape.height", "number lu – height"),
    // background
    (
        "background.fill",
        "fill (see Value shapes) – fills the home page at every target's page height. An ordinary layer in (z, list order): give it a low z (e.g. -30) so later images do not cover it. One per page for per-page colours (canvas `backgrounds` are layers over the whole canvas, canvas_update)",
    ),
    (
        "group.at",
        "{x, y} lu – OFFSET added to all children (default {x: 0, y: 0}), not a centre; override it in a variant to move the whole group",
    ),
    (
        "group.width",
        "not used (the children have their own sizes)",
    ),
    ("group.height", "not used"),
    // group
    (
        "children",
        "list of elements – moved/rotated together; `at` of children is relative to the group's home page",
    ),
    (
        "langs",
        "{<lang>: {field: value …}} – language overrides (see below)",
    ),
];

fn doc_exact(key: &str) -> Option<&'static str> {
    FIELD_DOCS.iter().find(|(k, _)| *k == key).map(|(_, d)| *d)
}

fn doc(kind: ElementKindTag, field: &str) -> Option<&'static str> {
    doc_exact(&format!("{}.{field}", kind.as_str())).or_else(|| doc_exact(field))
}

/// The `elements` topic, generated from the model's field lists.
pub fn elements() -> String {
    let mut s = String::from(
        "# Element fields\n\n\
Elements live in `canvases[].elements` (painting order: first = bottom). One YAML map per \
element, tagged by `kind`; unknown fields are errors (the error message lists the allowed \
ones). Units: lu (a page is 1000 lu wide). Colours: `#RRGGBB`, `#RRGGBBAA` or `@name` \
(styles.colors). In element_update / update_element a field set to `null` resets it to the \
default shown here; in a variant or language override `null` removes the field from the \
override (see docs_get {topic: \"scopes\"}).\n\n## Common fields (every kind)\n",
    );
    for f in COMMON_FIELDS {
        let _ = writeln!(s, "- `{f}`: {}", doc_exact(f).unwrap_or("?"));
    }
    for kind in KINDS {
        let _ = writeln!(s, "\n## kind: {}", kind.as_str());
        let mut fields: Vec<&str> = kind.fields().to_vec();
        // Common fields with a kind-specific meaning.
        for f in COMMON_FIELDS {
            if doc_exact(&format!("{}.{f}", kind.as_str())).is_some() {
                fields.push(f);
            }
        }
        for f in fields {
            let _ = writeln!(s, "- `{f}`: {}", doc(kind, f).unwrap_or("?"));
        }
    }
    s.push_str(VALUE_SHAPES);
    let _ = write!(
        s,
        "\n## Language overrides (`langs`, text elements only)\n\
`langs: {{de: {{box: {{top: 120, left: 60, right: 60}}, font_size: 64}}}}` – fields for one \
language, applied after variant overrides; `de` also applies to `de-AT`. Allowed fields: {}. \
Set them with element_update {{scope: \"lang\", lang}}; remove with element_reset_override.\n\
\n## Patchable fields\n{}\n",
        LANG_OVERRIDE_FIELDS.join(", "),
        ElementPatch::field_names().join(", ")
    );
    s
}

const VALUE_SHAPES: &str = "
## Value shapes
- Point: `{x, y}` in lu.
- Fill (backgrounds, shapes, canvas `backgrounds`):
  - colour string `\"#0B3324\"` / `\"@primary\"` (= `{kind: color, color}`)
  - `{kind: linear-gradient, angle?: degrees (CSS: 0 = to top, 90 = to right, default 180 = to bottom), stops: [[colour, 0..1], …]}`
  - `{kind: radial-gradient, center?: {x, y} fractions of the area (default centre), radius?: fraction of the larger side (default 0.5), stops: [[colour, pos], …]}`
  - `{kind: image, asset: assets/…, fit?: cover | contain | stretch | tile (default cover), focus?: {x, y} fractions of the image kept in view when cropping (default {x: 0.5, y: 0.5})}`; `tile` repeats the image at its natural size (1 px or SVG unit = 1 lu) from the top-left of the area
- Canvas `backgrounds`: a list of layers over the whole canvas (bottom to top), not one fill per page; only layers above the last opaque one show.
- Shadow presets: `soft`, `hard`, `none` built in; more in styles.shadows (`{layers: [{color, blur, x, y}]}`).
";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_field_of_every_kind_is_documented() {
        for kind in KINDS {
            for f in kind.fields().iter().chain(COMMON_FIELDS.iter()) {
                assert!(
                    doc(kind, f).is_some(),
                    "{}.{f} is not documented",
                    kind.as_str()
                );
            }
        }
        let e = elements();
        assert!(!e.contains(": ?"), "{e}");
        for needle in [
            "focus",
            "hyphenate",
            "shrink_to",
            "kind: device",
            "glow",
            "tile",
        ] {
            assert!(e.contains(needle), "elements mention {needle}");
        }
    }

    #[test]
    fn every_topic_has_content() {
        for (t, _) in TOPICS {
            let body = topic(t).unwrap();
            assert!(body.len() > 400, "{t} is too short");
            assert!(body.starts_with("# "), "{t} starts with a heading");
        }
        assert!(topic("nope").is_none());
        let ops = topic("ops").unwrap();
        for op in [
            "set_lang_override",
            "reset_lang_override_field",
            "set_variant_override",
            "reset_override_field",
        ] {
            assert!(ops.contains(op), "ops mention {op}");
        }
    }
}
