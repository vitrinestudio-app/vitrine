//! Device colours across frame sources: when a device switches between the
//! parametric frames (catalog palette: `black-titanium`, …) and an imported
//! one (the vendor's colours: `black`, `burgundy`, …), its colour is mapped
//! to the closest colour of the new source.

use super::store::FrameIndex;
use crate::catalog;
use crate::model::{ElementKind, ElementPatch, FrameStyle, Project};
use crate::ops::Op;

/// The colour a device of `model` with colour `color` should get when its
/// frame becomes `to`:
///
/// - `None`: keep it (no colour, a custom `#hex`, already available in the
///   new source, or nothing installed for an imported source: the
///   parametric fallback then uses it);
/// - `Some(Some(c))`: write `c`, the closest colour of the new source by
///   housing colour (measured swatch of an imported colour, catalog body
///   colour of a palette colour), else by name;
/// - `Some(None)`: clear it (no basis for a choice): the default applies.
pub fn remap_color(
    model: &str,
    color: Option<&str>,
    to: &FrameStyle,
    frames: Option<&FrameIndex>,
) -> Option<Option<String>> {
    let color = color.filter(|c| !c.starts_with('#'))?;
    let device = catalog::device(model)?;
    // Choices of the new source: (id, housing rgb).
    let choices: Vec<(String, Option<[u8; 3]>)> = match to.external_source() {
        Some(source) => {
            let installed = frames.map(|ix| ix.installed_colors(&source, model))?;
            if installed.is_empty() {
                return None;
            }
            installed
                .into_iter()
                .map(|c| {
                    let rgb = c.swatch.as_deref().and_then(parse_hex);
                    (c.id, rgb)
                })
                .collect()
        }
        None => device
            .colors
            .iter()
            .map(|c| (c.clone(), palette_rgb(c)))
            .collect(),
    };
    if choices.iter().any(|(c, _)| c == color) {
        return None;
    }
    if let Some(rgb) = housing_rgb(model, color, frames)
        && let Some((best, _)) = choices
            .iter()
            .filter_map(|(c, x)| x.map(|x| (c, distance(rgb, x))))
            .min_by(|a, b| a.1.total_cmp(&b.1))
    {
        return Some(Some(best.clone()));
    }
    let by_name = choices
        .iter()
        .map(|(c, _)| (c, name_score(color, c)))
        .filter(|(_, s)| *s > 0)
        .max_by_key(|(_, s)| *s)
        .map(|(c, _)| c.clone());
    Some(by_name)
}

/// Ops that change the project's default frame style to `frame` (`None` =
/// `detailed`) and remap the colours of every device that inherits it
/// (base elements and variant overrides, see [`remap_color`]), in an
/// order where each op is valid: colours first when leaving an imported
/// source, the styles first when entering one. Empty when nothing changes.
pub fn set_default_frame_ops(
    project: &Project,
    frame: Option<FrameStyle>,
    frames: Option<&FrameIndex>,
) -> Vec<Op> {
    if project.styles.frame == frame {
        return Vec::new();
    }
    let effective = |f: &Option<FrameStyle>| f.clone().unwrap_or(FrameStyle::Detailed);
    let (old_default, new_default) = (effective(&project.styles.frame), effective(&frame));
    let mut colors = Vec::new();
    if old_default != new_default {
        for canvas in &project.canvases {
            canvas.walk_elements(&mut |e| {
                let ElementKind::Device(d) = &e.kind else {
                    return;
                };
                // Base element.
                let mut base_color = d.color.clone();
                if d.frame.is_none()
                    && let Some(c) = remap_color(&d.model, d.color.as_deref(), &new_default, frames)
                {
                    colors.push(Op::UpdateElement {
                        id: e.id.clone(),
                        patch: ElementPatch {
                            color: Some(c.clone()),
                            ..ElementPatch::default()
                        },
                    });
                    base_color = c;
                }
                // Variant overrides that inherit the project default.
                for (vid, v) in &canvas.variants {
                    let Some(p) = v.overrides.get(&e.id) else {
                        continue;
                    };
                    let own_frame = match &p.frame {
                        Some(f) => f.clone(),
                        None => d.frame.clone(),
                    };
                    if own_frame.is_some() {
                        continue;
                    }
                    let model = match &p.model {
                        Some(Some(m)) => m.as_str(),
                        _ => d.model.as_str(),
                    };
                    let color = match &p.color {
                        Some(c) => c.clone(),
                        None => base_color.clone(),
                    };
                    let Some(c) = remap_color(model, color.as_deref(), &new_default, frames) else {
                        continue;
                    };
                    if p.color.is_none() && c == base_color {
                        continue;
                    }
                    let mut patch = p.clone();
                    patch.color = Some(c);
                    colors.push(Op::SetVariantOverride {
                        canvas: canvas.id.clone(),
                        variant: vid.clone(),
                        element: e.id.clone(),
                        patch: Some(patch),
                    });
                }
            });
        }
    }
    let mut styles = project.styles.clone();
    styles.frame = frame;
    let set = Op::SetStyles { styles };
    if new_default.is_external() {
        std::iter::once(set).chain(colors).collect()
    } else {
        colors.into_iter().chain(std::iter::once(set)).collect()
    }
}

/// Housing colour of `color`: a measured swatch of an installed frame of
/// `model`, else the catalog palette's body colour.
fn housing_rgb(model: &str, color: &str, frames: Option<&FrameIndex>) -> Option<[u8; 3]> {
    frames
        .and_then(|ix| {
            ix.sources()
                .flat_map(|s| ix.installed_colors(s, model))
                .find(|c| c.id == color)
                .and_then(|c| c.swatch.as_deref().and_then(parse_hex))
        })
        .or_else(|| palette_rgb(color))
}

fn palette_rgb(color: &str) -> Option<[u8; 3]> {
    catalog::catalog()
        .devices
        .colors
        .get(color)
        .map(|v| [v.body.r, v.body.g, v.body.b])
}

/// `#RRGGBB` → rgb.
pub fn parse_hex(s: &str) -> Option<[u8; 3]> {
    let h = s.strip_prefix('#')?;
    if h.len() != 6 {
        return None;
    }
    let c = |i: usize| u8::from_str_radix(h.get(i..i + 2)?, 16).ok();
    Some([c(0)?, c(2)?, c(4)?])
}

/// "Redmean" colour distance (cheap, perceptually better than plain RGB).
fn distance(a: [u8; 3], b: [u8; 3]) -> f64 {
    let rm = f64::midpoint(f64::from(a[0]), f64::from(b[0]));
    let d = |i: usize| f64::from(a[i]) - f64::from(b[i]);
    ((2.0 + rm / 256.0) * d(0) * d(0)
        + 4.0 * d(1) * d(1)
        + (2.0 + (255.0 - rm) / 256.0) * d(2) * d(2))
    .sqrt()
}

/// Shared words of two colour ids (`black-titanium` / `black` → 1), with a
/// few synonyms (silver ~ natural/white, black ~ midnight/space).
fn name_score(a: &str, b: &str) -> usize {
    fn group(w: &str) -> &str {
        match w {
            "silver" | "natural" | "white" | "porcelain" | "starlight" => "light",
            "black" | "midnight" | "space" | "graphite" => "dark",
            w => w,
        }
    }
    let wa: Vec<&str> = a.split('-').map(group).collect();
    let wb: Vec<&str> = b.split('-').map(group).collect();
    wa.iter().filter(|w| wb.contains(w)).count()
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;
    use std::path::Path;

    use super::*;
    use crate::frame_import::{FrameSourceIndex, InstalledFrame, PxRect};
    use crate::model::{Orientation, PixelSize};

    fn installed(color: &str, label: &str, swatch: Option<&str>) -> InstalledFrame {
        InstalledFrame {
            model: "iphone-island".into(),
            color: color.into(),
            color_label: label.into(),
            orientation: Orientation::Portrait,
            apple_model: "iPhone 18 Pro".into(),
            file: String::new(),
            mask: String::new(),
            size: PixelSize::new(10, 10),
            screen: PxRect {
                x: 1,
                y: 1,
                w: 8,
                h: 8,
            },
            radius: 0.0,
            priority: 0,
            source_file: String::new(),
            package: String::new(),
            license: None,
            imported: String::new(),
            swatch: swatch.map(Into::into),
            origin: crate::frame_import::FrameOrigin::App,
        }
    }

    fn index(swatches: bool) -> FrameIndex {
        let s = |x: &'static str| swatches.then_some(x);
        let frames = vec![
            installed("black", "Black", s("#2B2B2D")),
            installed("burgundy", "Burgundy", s("#5A1F2A")),
            installed("glacier", "Glacier", s("#C9D8E3")),
            installed("silver", "Silver", s("#DADBDD")),
        ];
        let mut m = BTreeMap::new();
        m.insert("apple".to_owned(), FrameSourceIndex { frames });
        FrameIndex::from_sources(Path::new("/x"), m)
    }

    #[test]
    fn maps_between_palette_and_installed_colours() {
        let apple = FrameStyle::Imported("apple".into());
        let ix = index(true);
        let m = |c: &str, to: &FrameStyle| remap_color("iphone-island", Some(c), to, Some(&ix));
        assert_eq!(m("black-titanium", &apple), Some(Some("black".into())));
        assert_eq!(m("natural-titanium", &apple), Some(Some("silver".into())));
        assert_eq!(m("white-titanium", &apple), Some(Some("silver".into())));
        assert_eq!(m("burgundy", &apple), None, "installed: kept");
        // Back to the palette.
        let det = FrameStyle::Detailed;
        assert_eq!(m("burgundy", &det), Some(Some("black-titanium".into())));
        assert_eq!(m("silver", &det), Some(Some("natural-titanium".into())));
        assert_eq!(m("midnight", &det), None);
        // Custom hex and no colour stay.
        assert_eq!(m("#FF0000", &det), None);
        assert_eq!(remap_color("iphone-island", None, &apple, Some(&ix)), None);
        // Nothing installed: keep (the parametric fallback draws it).
        assert_eq!(
            remap_color("iphone-island", Some("midnight"), &apple, None),
            None
        );
    }

    #[test]
    fn without_swatches_by_name_else_cleared() {
        let apple = FrameStyle::Imported("apple".into());
        let ix = index(false);
        let m = |c: &str, to: &FrameStyle| remap_color("iphone-island", Some(c), to, Some(&ix));
        assert_eq!(m("black-titanium", &apple), Some(Some("black".into())));
        // A colour with no rgb and no name match: cleared.
        assert_eq!(m("glacier", &FrameStyle::Stylized), Some(None));
    }

    #[test]
    fn default_frame_switch_remaps_inheriting_devices() {
        use crate::model::{ElementId, VariantId};
        let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data/sample");
        let mut p = Project::load(&dir).unwrap();
        let ix = index(true);
        let phone = ElementId::new("hero-phone");
        let color = |p: &Project| match &p.element("hero-phone").unwrap().1.kind {
            ElementKind::Device(d) => d.color.clone(),
            _ => unreachable!(),
        };
        p.apply(Op::UpdateElement {
            id: phone.clone(),
            patch: crate::io::from_yaml("{ color: natural-titanium }").unwrap(),
        })
        .unwrap();
        // An override colour in a variant.
        p.apply(Op::SetVariantOverride {
            canvas: p.canvases[0].id.clone(),
            variant: VariantId::new("tablet-landscape"),
            element: phone.clone(),
            patch: Some(crate::io::from_yaml("{ rotation: -4, color: black-titanium }").unwrap()),
        })
        .unwrap();
        let apple = Some(FrameStyle::Imported("apple".into()));
        let ops = set_default_frame_ops(&p, apple.clone(), Some(&ix));
        assert!(matches!(ops[0], Op::SetStyles { .. }), "styles first");
        assert_eq!(ops.len(), 3);
        p.apply_ops(ops).unwrap();
        assert_eq!(color(&p).as_deref(), Some("silver"));
        let over = &p.canvases[0].variants[&VariantId::new("tablet-landscape")].overrides[&phone];
        assert_eq!(over.color, Some(Some("black".into())));
        assert_eq!(
            set_default_frame_ops(&p, apple, Some(&ix)),
            Vec::<Op>::new()
        );
        // And back: colours first, then the styles.
        let ops = set_default_frame_ops(&p, None, Some(&ix));
        assert!(matches!(ops.last(), Some(Op::SetStyles { .. })));
        p.apply_ops(ops).unwrap();
        assert_eq!(color(&p).as_deref(), Some("natural-titanium"));
        assert!(p.styles.frame.is_none());
    }
}
