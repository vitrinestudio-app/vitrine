//! Element resolution: variant overrides, visibility, home page + anchor →
//! canvas, kind-specific properties, group transforms, z order.

// e, p, d, r, k: element, props, diagnostics, result, scale.
#![allow(clippy::many_single_char_names)]

use std::collections::BTreeMap;

use super::lookup::{lookup_screenshot, lookup_text};
use super::scene::{
    DeviceColor, DiagnosticCode, ImportedFrame, ResolvedDevice, ResolvedElement, ResolvedImage,
    ResolvedKind, ResolvedShadowLayer, ResolvedShape, ResolvedSpanStyle, ResolvedText,
    ScreenshotSource, TextFrame,
};
use super::style::StyleResolver;
use super::{Diags, ResolveOptions};
use crate::catalog::Device;
use crate::frame_import::{BezelChoice, FrameIndex, select_bezel};
use crate::geometry::{CanvasLayout, Rect, Vec2};
use crate::model::{
    Align, AnchorY, Clip, DeviceProps, Element, ElementId, ElementKind, ElementPatch, FitMode,
    FrameStyle, ImageProps, Orientation, PixelSize, Project, Rgba, ShapeKind, ShapeProps, Strip,
    Target, TextProps, VAlign, VariantId,
};

/// Device width when an element sets none, lu.
pub const DEFAULT_DEVICE_WIDTH: f64 = 800.0;
/// Free text box width when an element sets none, lu.
pub const DEFAULT_TEXT_WIDTH: f64 = 840.0;

pub(super) struct Inputs<'a> {
    pub project: &'a Project,
    pub target: &'a Target,
    pub strip: &'a Strip,
    pub lang: &'a str,
    pub chain: Vec<String>,
    pub layout: CanvasLayout,
    pub options: &'a ResolveOptions<'a>,
    pub styles: StyleResolver<'a>,
    pub spans: &'a BTreeMap<String, ResolvedSpanStyle>,
}

/// Transform of an enclosing group, applied on top of its children.
#[derive(Clone, Debug)]
struct GroupXf {
    id: ElementId,
    offset: Vec2,
    /// Canvas point the group rotates around.
    pivot: Vec2,
    rotation: f64,
    opacity: f64,
    z: i32,
}

struct Walker<'a, 'i> {
    inp: &'i Inputs<'a>,
    overrides: Option<&'a BTreeMap<ElementId, ElementPatch>>,
    content_scale: f64,
    frame_style: FrameStyle,
    out: Vec<ResolvedElement>,
}

pub(super) fn resolve_elements(
    inp: &Inputs<'_>,
    d: &mut Diags,
) -> (Option<VariantId>, Vec<ResolvedElement>) {
    let (variant_id, variant) = match &inp.target.variant {
        None => (None, None),
        Some(v) => {
            if let Some(var) = inp.strip.variants.get(v) {
                (Some(v.clone()), Some(var))
            } else {
                d.warn(
                    DiagnosticCode::UnknownVariant,
                    format!("strip `{}` has no variant `{v}`", inp.strip.id),
                );
                (None, None)
            }
        }
    };
    if let Some(var) = variant {
        for id in var.overrides.keys() {
            if inp.strip.element(id.as_str()).is_none() {
                d.warn(
                    DiagnosticCode::UnknownOverrideElement,
                    format!("variant override for unknown element `{id}`"),
                );
            }
        }
    }
    let mut w = Walker {
        inp,
        overrides: variant.map(|v| &v.overrides),
        content_scale: variant.and_then(|v| v.content_scale).unwrap_or(1.0),
        frame_style: inp
            .project
            .styles
            .frame
            .clone()
            .unwrap_or(FrameStyle::Detailed),
        out: Vec::new(),
    };
    for e in &inp.strip.elements {
        w.element(e, d);
    }
    d.element = None;
    // Stable: equal z keeps list (painting) order.
    w.out.sort_by_key(|e| e.z);
    (variant_id, w.out)
}

impl Walker<'_, '_> {
    fn layout(&self) -> &CanvasLayout {
        &self.inp.layout
    }

    /// The element with the variant override, then the language override
    /// applied (a copy).
    fn patched(&self, e: &Element, d: &mut Diags) -> Element {
        let e = match self.overrides.and_then(|o| o.get(&e.id)) {
            None => e.clone(),
            Some(patch) => match e.patched(patch) {
                Ok((p, _)) => p,
                Err(err) => {
                    d.warn(
                        DiagnosticCode::InvalidOverride,
                        format!("variant override not applied: {err}"),
                    );
                    e.clone()
                }
            },
        };
        match e.for_lang(self.inp.lang) {
            Ok(Some((_, p))) => p,
            Ok(None) => e,
            Err(err) => {
                d.warn(
                    DiagnosticCode::InvalidOverride,
                    format!("language override not applied: {err}"),
                );
                e
            }
        }
    }

    fn visible(&self, e: &Element) -> bool {
        if !e.visible {
            return false;
        }
        let Some(v) = &e.visible_in else {
            return true;
        };
        let lang = self.inp.lang;
        (v.targets.is_empty() || v.targets.contains(&self.inp.target.id))
            && (v.langs.is_empty()
                || v.langs.iter().any(|l| {
                    let l = l.as_str();
                    lang == l || lang.strip_prefix(l).is_some_and(|r| r.starts_with('-'))
                }))
    }

    /// Resolve `e` (and its children); returns how many elements were added.
    fn element(&mut self, e: &Element, d: &mut Diags) {
        d.element = Some(e.id.clone());
        let e = self.patched(e, d);
        if !self.visible(&e) {
            return;
        }
        if e.page >= self.inp.strip.page.count {
            d.warn(
                DiagnosticCode::PageOutOfRange,
                format!(
                    "home page {} is outside the strip ({} pages)",
                    e.page, self.inp.strip.page.count
                ),
            );
        }
        if let ElementKind::Group(g) = &e.kind {
            let start = self.out.len();
            for c in &g.children {
                self.element(c, d);
            }
            let xf = GroupXf {
                id: e.id.clone(),
                offset: Vec2::new(e.at.x, e.at.y),
                pivot: self.group_pivot(&e, d),
                rotation: e.rotation,
                opacity: e.opacity,
                z: e.z,
            };
            for r in &mut self.out[start..] {
                apply_group(r, &xf);
            }
            return;
        }
        if let Some(r) = self.leaf(&e, d) {
            self.out.push(r);
        }
    }

    /// Canvas point a group rotates around: centre of the bounding box of
    /// its children's reference points (all children, visible or not, so
    /// the pivot does not depend on language or target visibility) plus the
    /// group's `pivot` offset. Without children: the group's home page
    /// origin plus `pivot`.
    fn group_pivot(&self, g: &Element, d: &mut Diags) -> Vec2 {
        let mut pts = Vec::new();
        self.reference_points(g, d, &mut pts);
        let base = if pts.is_empty() {
            Vec2::new(self.layout().page_x(i64::from(g.page)), 0.0)
        } else {
            let (mut lo, mut hi) = (pts[0], pts[0]);
            for p in &pts {
                lo = Vec2::new(lo.x.min(p.x), lo.y.min(p.y));
                hi = Vec2::new(hi.x.max(p.x), hi.y.max(p.y));
            }
            (lo + hi) * 0.5
        };
        let off = g.pivot.map_or(Vec2::ZERO, |p| Vec2::new(p.x, p.y));
        base + off
    }

    fn reference_points(&self, g: &Element, d: &mut Diags, out: &mut Vec<Vec2>) {
        let saved = d.element.clone();
        let mut quiet = Diags::default();
        for c in g.children() {
            let c = self.patched(c, &mut quiet);
            match &c.kind {
                ElementKind::Group(_) => self.reference_points(&c, d, out),
                ElementKind::Text(t) if t.r#box.is_some() => {
                    out.push(self.text_frame(&c, t, &mut quiet).0.rect.center());
                }
                ElementKind::Background(_) => {
                    out.push(self.layout().page_rect(i64::from(c.page)).center());
                }
                _ => out.push(self.layout().to_canvas(
                    i64::from(c.page),
                    Vec2::new(c.at.x, c.at.y),
                    c.effective_anchor_y(),
                )),
            }
        }
        d.element = saved;
    }

    fn leaf(&self, e: &Element, d: &mut Diags) -> Option<ResolvedElement> {
        let layout = self.layout();
        let page = i64::from(e.page);
        let anchor = e.effective_anchor_y();
        let center = layout.to_canvas(page, Vec2::new(e.at.x, e.at.y), anchor);
        let sy = if anchor == AnchorY::Stretch {
            layout.stretch_factor()
        } else {
            1.0
        };
        let mut r = ResolvedElement {
            id: e.id.clone(),
            groups: Vec::new(),
            page: e.page,
            z: e.z,
            center,
            size: Vec2::ZERO,
            rotation: e.rotation,
            pivot: e.pivot.map_or(Vec2::ZERO, |p| Vec2::new(p.x, p.y)),
            opacity: e.opacity,
            clip: (e.effective_clip() == Clip::Page).then(|| layout.page_rect(page)),
            props: ResolvedKind::Background {
                fill: super::scene::ResolvedFill::Color {
                    color: Rgba::rgb(0, 0, 0),
                },
            },
        };
        match &e.kind {
            ElementKind::Background(b) => {
                let rect = layout.page_rect(page);
                r.center = rect.center();
                r.size = Vec2::new(rect.w, rect.h);
                r.props = ResolvedKind::Background {
                    fill: self.inp.styles.fill(&b.fill, d),
                };
            }
            ElementKind::Device(p) => {
                let (dev, size) = self.device(e, p, d)?;
                let k = self.content_scale;
                r.size = size * k;
                r.pivot = r.pivot * k;
                r.props = ResolvedKind::Device(Box::new(dev));
            }
            ElementKind::Text(t) => {
                let (frame, valign) = self.text_frame(e, t, d);
                r.center = frame.rect.center();
                r.size = Vec2::new(frame.rect.w, frame.rect.h);
                let mut text = self.text(t, frame, valign, d);
                if frame.auto_height {
                    text.default_pivot = self.text_pivot(&text, r.center);
                }
                r.pivot = r.pivot + text.default_pivot;
                r.props = ResolvedKind::Text(Box::new(text));
            }
            ElementKind::Image(p) => {
                let (img, size) = self.image(e, p, sy, d);
                r.size = size;
                r.props = ResolvedKind::Image(img);
            }
            ElementKind::Shape(p) => {
                let (shape, size) = self.shape(e, p, sy, d);
                r.size = size;
                r.props = ResolvedKind::Shape(shape);
            }
            ElementKind::Group(_) => unreachable!("groups are handled by the caller"),
        }
        Some(r)
    }

    #[allow(clippy::too_many_lines)]
    fn device(
        &self,
        e: &Element,
        p: &DeviceProps,
        d: &mut Diags,
    ) -> Option<(ResolvedDevice, Vec2)> {
        let catalog = self.inp.options.catalog;
        let mut model = p.model.clone();
        let mut mapped_from = None;
        if let Some(to) = self.inp.target.device_map.get(&p.model) {
            if catalog.device(to).is_some() {
                mapped_from = Some(p.model.clone());
                model.clone_from(to);
            } else {
                d.warn(
                    DiagnosticCode::UnknownMappedDevice,
                    format!(
                        "`device_map` maps `{}` to unknown model `{to}`; keeping `{}`",
                        p.model, p.model
                    ),
                );
            }
        }
        // Foldables: `fold` picks the model of that state (after the map,
        // so a mapped foldable folds too).
        if let Some(other) = catalog
            .device(&model)
            .and_then(|dev| dev.fold_model(p.fold))
            .filter(|o| catalog.device(o).is_some())
        {
            other.clone_into(&mut model);
        }
        let Some(device) = catalog.device(&model) else {
            d.warn(
                DiagnosticCode::UnknownDevice,
                format!("unknown device model `{model}`; element skipped"),
            );
            return None;
        };
        let mut orientation = p.orientation.unwrap_or(device.natural);
        if !device.supports(orientation) {
            d.warn(
                DiagnosticCode::UnsupportedOrientation,
                format!(
                    "`{model}` does not support {orientation:?}; using its natural orientation"
                ),
            );
            orientation = device.natural;
        }
        let width = e.width.unwrap_or_else(|| {
            d.info(
                DiagnosticCode::MissingSize,
                format!("device without `width`; using {DEFAULT_DEVICE_WIDTH} lu"),
            );
            DEFAULT_DEVICE_WIDTH
        });
        let requested = p.frame.clone().unwrap_or_else(|| self.frame_style.clone());
        let variant = self
            .inp
            .target
            .screenshots
            .as_deref()
            .or(device.screenshot.variant.as_deref());
        let screenshot = p.screenshot.as_deref().map(|key| {
            let s = lookup_screenshot(self.inp.options.assets, &self.inp.chain, variant, key);
            match s.source {
                ScreenshotSource::Missing => d.warn(
                    DiagnosticCode::ScreenshotMissing,
                    format!(
                        "screenshot `{key}` not found for `{}` (variant {})",
                        self.inp.lang,
                        variant.unwrap_or("-")
                    ),
                ),
                ScreenshotSource::Lang | ScreenshotSource::LangVariant => {}
                ScreenshotSource::Fallback | ScreenshotSource::FallbackVariant => d.info(
                    DiagnosticCode::ScreenshotFallback,
                    format!(
                        "`{}` uses fallback screenshot `{}` for `{key}`",
                        self.inp.lang,
                        s.path.as_deref().unwrap_or_default()
                    ),
                ),
            }
            s
        });
        let imported = self.imported_frame(
            p,
            &requested,
            &model,
            device,
            orientation,
            screenshot.as_ref().and_then(|s| s.size),
            d,
        );
        let (color, palette) = match &imported {
            Some(f) => (
                DeviceColor::Named(f.color.clone()),
                catalog.devices.colors.get(&f.color).cloned(),
            ),
            // Vendor colour names of an imported frame that is missing here
            // are expected: an info, not a warning.
            None => self.device_color(
                p,
                device,
                mapped_from.is_some() || matches!(requested, FrameStyle::Imported(_)),
                d,
            ),
        };
        let frame = match (&requested, &imported) {
            (FrameStyle::Imported(_), None) => FrameStyle::Detailed,
            _ => requested,
        };
        // `aspect: free` models (browser, none) take the screenshot's aspect
        // when its size is known. The screenshot is never rotated, so for a
        // rotated model the natural-orientation aspect is inverted.
        let free_aspect = device.screenshot.aspect.is_none();
        let device = match screenshot.as_ref().and_then(|s| s.size) {
            Some(px) if free_aspect && px.w > 0 && px.h > 0 => {
                let r = f64::from(px.w) / f64::from(px.h);
                device.with_display_aspect(if orientation == device.natural {
                    r
                } else {
                    1.0 / r
                })
            }
            _ => device.clone(),
        };
        let outer = device.outer_size();
        let ratio = match &imported {
            // The bezel PNG is already in its orientation.
            Some(f) if f.size.w > 0 => f64::from(f.size.h) / f64::from(f.size.w),
            _ if orientation == device.natural => outer.h / outer.w,
            _ => outer.w / outer.h,
        };
        let k = self.content_scale;
        let shadow = self
            .inp
            .styles
            .shadow(p.shadow.as_deref(), d)
            .into_iter()
            .map(|l| scale_shadow(l, k))
            .collect();
        Some((
            ResolvedDevice {
                model,
                mapped_from,
                device,
                orientation,
                frame,
                color,
                palette,
                chrome: p.chrome,
                url: p.url.clone(),
                appearance: p.appearance,
                free_aspect,
                screenshot,
                fit: p.fit.unwrap_or(FitMode::Cover),
                content_scale: k,
                shadow,
                imported,
            },
            Vec2::new(width, width * ratio),
        ))
    }

    /// The installed vendor frame for `frame: imported:<source>`, or an
    /// info when it is missing on this machine (parametric fallback).
    #[allow(clippy::too_many_arguments)]
    fn imported_frame(
        &self,
        p: &DeviceProps,
        requested: &FrameStyle,
        model: &str,
        device: &Device,
        orientation: Orientation,
        screenshot_px: Option<PixelSize>,
        d: &mut Diags,
    ) -> Option<ImportedFrame> {
        let FrameStyle::Imported(source) = requested else {
            return None;
        };
        if crate::frame_import::Rules::builtin(source).is_some_and(|r| !r.covers(model)) {
            // No vendor frame exists for this model (browser, Android …).
            return None;
        }
        let color = p.color.as_deref().filter(|c| !c.starts_with('#'));
        let candidates = self.inp.options.frames.map_or_else(Vec::new, |ix| {
            ix.candidates(
                source,
                model,
                color,
                device.default_color.as_deref(),
                orientation,
            )
        });
        // The target's page size (pages targets): a 1206×2622 target shows
        // the bezel whose screen is 1206×2622.
        let t = self.inp.target;
        let target_px = matches!(t.kind, crate::model::TargetKind::Pages)
            .then(|| PixelSize::new(t.size.w / t.group_size().max(1), t.size.h));
        let pin = p.bezel.as_deref();
        let auto = select_bezel(&candidates, None, target_px, screenshot_px);
        let chosen = select_bezel(&candidates, pin, target_px, screenshot_px);
        let Some((f, choice)) = chosen else {
            d.info(
                DiagnosticCode::ImportedFrameMissing,
                format!(
                    "no imported `{source}` frame installed for `{model}` ({}, {}); using the parametric frame",
                    color.unwrap_or("any colour"),
                    crate::frame_import::orientation_name(orientation)
                ),
            );
            return None;
        };
        let installed = self
            .inp
            .options
            .frames
            .map(|ix| ix.installed_colors(source, model))
            .unwrap_or_default();
        if let Some(c) = color
            && c != f.color
        {
            d.info(
                DiagnosticCode::ImportedColorFallback,
                format!(
                    "colour `{c}` is not installed for the imported `{source}` frame of `{model}`; using `{}` (installed: {})",
                    f.color_label,
                    installed
                        .iter()
                        .map(|c| c.label.as_str())
                        .collect::<Vec<_>>()
                        .join(", ")
                ),
            );
        }
        if let Some(pin) = pin
            && choice != BezelChoice::Pinned
        {
            d.info(
                DiagnosticCode::ImportedBezelFallback,
                format!(
                    "bezel `{pin}` is not installed for `{model}` ({}, {}); using `{}` (installed: {})",
                    f.color_label,
                    crate::frame_import::orientation_name(orientation),
                    f.apple_model,
                    candidates
                        .iter()
                        .map(|c| c.apple_model.as_str())
                        .collect::<Vec<_>>()
                        .join(", ")
                ),
            );
        } else if choice == BezelChoice::Largest && candidates.len() > 1 {
            d.info(
                DiagnosticCode::ImportedBezelFallback,
                format!(
                    "no installed `{model}` bezel matches the target or screenshot size; using the largest, `{}` (set `bezel:` to choose)",
                    f.apple_model
                ),
            );
        }
        let mut bezels: Vec<String> = candidates.iter().map(|c| c.apple_model.clone()).collect();
        bezels.sort();
        bezels.dedup();
        Some(ImportedFrame {
            source: source.clone(),
            file: FrameIndex::rel_path(source, &f.file),
            mask: FrameIndex::rel_path(source, &f.mask),
            size: f.size,
            screen: f.screen,
            color: f.color.clone(),
            bezel: f.apple_model.clone(),
            choice,
            automatic: auto.map_or_else(String::new, |(a, _)| a.apple_model.clone()),
            bezels,
            swatch: f.swatch.clone(),
            colors: installed,
        })
    }

    fn device_color(
        &self,
        p: &DeviceProps,
        device: &Device,
        mapped: bool,
        d: &mut Diags,
    ) -> (DeviceColor, Option<crate::catalog::ColorVariant>) {
        let palette = &self.inp.options.catalog.devices.colors;
        let named = |name: &str| {
            (
                DeviceColor::Named(name.to_owned()),
                palette.get(name).cloned(),
            )
        };
        let default = || match &device.default_color {
            Some(c) => named(c),
            None => (DeviceColor::None, None),
        };
        match p.color.as_deref() {
            None => default(),
            Some(hex) if hex.starts_with('#') => match Rgba::parse(hex) {
                Ok(c) => (DeviceColor::Custom(c), None),
                Err(err) => {
                    d.warn(DiagnosticCode::UnknownColor, err);
                    default()
                }
            },
            Some(name) if device.has_color(name) => named(name),
            Some(name) => {
                let msg = format!(
                    "`{}` has no colour `{name}`; using its default colour",
                    device.id
                );
                if mapped {
                    d.info(DiagnosticCode::DeviceColorFallback, msg);
                } else {
                    d.warn(DiagnosticCode::DeviceColorFallback, msg);
                }
                default()
            }
        }
    }

    /// Text box on the canvas and the effective vertical alignment.
    fn text_frame(&self, e: &Element, t: &TextProps, d: &mut Diags) -> (TextFrame, VAlign) {
        let layout = self.layout();
        let anchor = e.effective_anchor_y();
        if let Some(b) = &t.r#box {
            // Margins relative to the home page (column). Both edges are
            // y coordinates of the reference page and follow the anchor.
            let page = layout.page_rect(i64::from(e.page));
            let left = page.x + b.left.unwrap_or(0.0);
            let right = page.right() - b.right.unwrap_or(0.0);
            let top = layout.anchor_y(b.top.unwrap_or(0.0), anchor);
            let bottom = b.bottom.map_or(page.bottom(), |m| {
                layout.anchor_y(layout.reference_page_height - m, anchor)
            });
            let frame = TextFrame {
                rect: Rect::new(left, top, right - left, bottom - top),
                auto_height: b.bottom.is_none(),
                margins: true,
            };
            (frame, t.valign.unwrap_or(VAlign::Top))
        } else {
            let c = layout.to_canvas(i64::from(e.page), Vec2::new(e.at.x, e.at.y), anchor);
            let w = e.width.unwrap_or_else(|| {
                d.info(
                    DiagnosticCode::MissingSize,
                    format!("free text box without `width`; using {DEFAULT_TEXT_WIDTH} lu"),
                );
                DEFAULT_TEXT_WIDTH
            });
            let sy = if anchor == AnchorY::Stretch {
                layout.stretch_factor()
            } else {
                1.0
            };
            let h = e.height.map(|h| h * sy);
            let frame = TextFrame {
                rect: Rect::centered(c, w, h.unwrap_or(0.0)),
                auto_height: h.is_none(),
                margins: false,
            };
            (frame, t.valign.unwrap_or(VAlign::Center))
        }
    }

    /// Default pivot of an auto-height text box as offset from `center`
    /// (the frame centre): the centre of the laid-out content. Without
    /// text, the centre of the top edge for margin boxes (where the text
    /// would start), the frame centre (`at`) for free boxes.
    fn text_pivot(&self, text: &ResolvedText, center: Vec2) -> Vec2 {
        let measured = match self.inp.options.text {
            Some(m) => m.content_rect(text, self.inp.spans),
            None => crate::layout::measure_with_bundled_fonts(text, self.inp.spans),
        };
        match measured {
            Some(rect) => rect.center() - center,
            None if text.frame.margins => Vec2::new(0.0, -text.frame.rect.h / 2.0),
            None => Vec2::ZERO,
        }
    }

    fn text(&self, t: &TextProps, frame: TextFrame, valign: VAlign, d: &mut Diags) -> ResolvedText {
        let key = t.text.as_str();
        let found = lookup_text(self.inp.project, &self.inp.chain, key);
        match &found {
            None => d.warn(
                DiagnosticCode::TextMissing,
                format!(
                    "text `{key}` missing in `{}` and all fallbacks",
                    self.inp.lang
                ),
            ),
            Some((_, l)) if l.as_str() != self.inp.lang => d.info(
                DiagnosticCode::TextFallback,
                format!("text `{key}` for `{}` uses `{l}`", self.inp.lang),
            ),
            Some(_) => {}
        }
        let (text, text_lang) = found.map_or((None, None), |(s, l)| (Some(s.to_owned()), Some(l)));
        ResolvedText {
            key: t.text.clone(),
            text,
            text_lang,
            style: self.inp.styles.text_style(t, d),
            frame,
            align: t.align.unwrap_or(Align::Left),
            valign,
            shrink_to: t.fit.and_then(|f| f.shrink_to),
            max_lines: t.fit.and_then(|f| f.max_lines),
            hyphenate: self.inp.styles.hyphenate(t),
            font_fallbacks: self.inp.project.styles.font_fallbacks(),
            default_pivot: Vec2::ZERO,
        }
    }

    fn image(&self, e: &Element, p: &ImageProps, sy: f64, d: &mut Diags) -> (ResolvedImage, Vec2) {
        let w = e.width.unwrap_or_else(|| {
            d.warn(
                DiagnosticCode::MissingSize,
                "image without `width`; using 0".into(),
            );
            0.0
        });
        let h = e.height.map(|h| h * sy);
        (
            ResolvedImage {
                asset: p.asset.clone(),
                fit: p.fit.unwrap_or(FitMode::Contain),
                auto_height: h.is_none(),
                shadow: self.inp.styles.shadow(p.shadow.as_deref(), d),
            },
            Vec2::new(w, h.unwrap_or(w)),
        )
    }

    fn shape(&self, e: &Element, p: &ShapeProps, sy: f64, d: &mut Diags) -> (ResolvedShape, Vec2) {
        let mut dim = |v: Option<f64>, name: &str, optional: bool| {
            v.unwrap_or_else(|| {
                if !optional {
                    d.warn(
                        DiagnosticCode::MissingSize,
                        format!("shape without `{name}`; using 0"),
                    );
                }
                0.0
            })
        };
        let w = dim(e.width, "width", false);
        let h = dim(e.height, "height", p.shape == ShapeKind::Line) * sy;
        (
            ResolvedShape {
                shape: p.shape,
                radius: p.radius.unwrap_or(0.0),
                fill: p.fill.as_ref().map(|f| self.inp.styles.fill(f, d)),
                stroke: p.stroke.as_ref().map(|s| self.inp.styles.stroke(s, d)),
                shadow: self.inp.styles.shadow(p.shadow.as_deref(), d),
            },
            Vec2::new(w, h),
        )
    }
}

/// `screenshot_only` targets: per page, the largest device homed on it
/// becomes a bare screenshot filling the page (catalog model `none`, square
/// corners, no shadow, rotation or group transform); everything else is
/// dropped. Pages without a device stay empty (warning). Diagnostics of
/// dropped elements are removed.
pub(super) fn screenshot_only(
    elements: &[ResolvedElement],
    layout: &CanvasLayout,
    count: u32,
    catalog: &crate::catalog::Catalog,
    d: &mut Diags,
) -> Vec<ResolvedElement> {
    let Some(bare) = catalog.device("none") else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for page in 0..count {
        let main = elements
            .iter()
            .filter(|e| e.page == page && matches!(e.props, ResolvedKind::Device(_)))
            .fold(None::<&ResolvedElement>, |best, e| match best {
                Some(b) if b.size.x * b.size.y >= e.size.x * e.size.y => Some(b),
                _ => Some(e),
            });
        let Some(e) = main else {
            d.element = None;
            d.warn(
                DiagnosticCode::ScreenshotOnlyEmpty,
                format!(
                    "page {} has no device: its screenshot-only image stays empty",
                    page + 1
                ),
            );
            continue;
        };
        let ResolvedKind::Device(dev) = &e.props else {
            continue;
        };
        let rect = layout.page_rect(i64::from(page));
        let mut device = bare.with_display_aspect(rect.w / rect.h);
        device.body.radius = 0.0;
        device.display.radius = 0.0;
        let mut dev = (**dev).clone();
        dev.mapped_from = Some(dev.model.clone());
        dev.model.clone_from(&device.id);
        dev.orientation = device.natural;
        dev.device = device;
        dev.frame = FrameStyle::Stylized;
        dev.free_aspect = true;
        dev.imported = None;
        dev.chrome = None;
        dev.shadow = Vec::new();
        dev.content_scale = 1.0;
        out.push(ResolvedElement {
            id: e.id.clone(),
            groups: Vec::new(),
            page,
            z: 0,
            center: rect.center(),
            size: Vec2::new(rect.w, rect.h),
            rotation: 0.0,
            pivot: Vec2::ZERO,
            opacity: 1.0,
            clip: Some(rect),
            props: ResolvedKind::Device(Box::new(dev)),
        });
    }
    let kept: Vec<ElementId> = out.iter().map(|e| e.id.clone()).collect();
    d.items
        .retain(|x| x.element.as_ref().is_none_or(|id| kept.contains(id)));
    out
}

fn scale_shadow(l: ResolvedShadowLayer, k: f64) -> ResolvedShadowLayer {
    ResolvedShadowLayer {
        color: l.color,
        blur: l.blur * k,
        x: l.x * k,
        y: l.y * k,
    }
}

/// Apply a group transform: the child's rotation pivot (a canvas point) is
/// offset by the group's `at` and rotated around the group pivot; the
/// child's own rotation, opacity and z are combined with the group's.
fn apply_group(r: &mut ResolvedElement, g: &GroupXf) {
    let world_pivot = r.center + r.pivot;
    let moved = world_pivot.rotated_around(g.pivot, g.rotation) + g.offset;
    let delta = moved - world_pivot;
    r.center = r.center + delta;
    if let ResolvedKind::Text(t) = &mut r.props {
        t.frame.rect.x += delta.x;
        t.frame.rect.y += delta.y;
    }
    r.rotation += g.rotation;
    r.opacity *= g.opacity;
    r.z += g.z;
    r.groups.insert(0, g.id.clone());
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn group_transform_composes_rotation_around_group_pivot() {
        let mut r = ResolvedElement {
            id: ElementId::new("c"),
            groups: Vec::new(),
            page: 0,
            z: 1,
            center: Vec2::new(110.0, 100.0),
            size: Vec2::new(20.0, 20.0),
            rotation: 10.0,
            pivot: Vec2::ZERO,
            opacity: 0.5,
            clip: None,
            props: ResolvedKind::Background {
                fill: super::super::scene::ResolvedFill::Color {
                    color: Rgba::rgb(0, 0, 0),
                },
            },
        };
        let g = GroupXf {
            id: ElementId::new("g"),
            offset: Vec2::new(5.0, 0.0),
            pivot: Vec2::new(100.0, 100.0),
            rotation: 90.0,
            opacity: 0.5,
            z: 2,
        };
        apply_group(&mut r, &g);
        assert!((r.center.x - 105.0).abs() < 1e-9 && (r.center.y - 110.0).abs() < 1e-9);
        assert!((r.rotation - 100.0).abs() < 1e-9);
        assert!((r.opacity - 0.25).abs() < 1e-9);
        assert_eq!(r.z, 3);
        assert_eq!(r.groups, [ElementId::new("g")]);
    }
}
