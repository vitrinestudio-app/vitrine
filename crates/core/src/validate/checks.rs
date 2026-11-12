//! The individual checks. `target_checks` and `frame_checks` run once per
//! target, `scene_checks` once per target × language.

// s, e, t, d, l: scope, element, text, device, layout.
#![allow(clippy::many_single_char_names)]

use std::collections::BTreeSet;

use super::{Ctx, FindingCode as C, Severity, Sink, ValidateMode, ValidateOptions};
use crate::catalog::{DeviceClass, PresetStatus, SizePolicy, TargetPreset};
use crate::geometry::{Rect, Vec2};
use crate::layout::{TextDiagnostic, TextLayouter, layout_text, span_styles};
use crate::model::{
    Canvas, Chrome, ElementId, FitMode, ImageFormat, PixelSize, Project, Target, TargetKind,
};
use crate::resolve::{
    DiagnosticCode, Frame, Level, ResolvedDevice, ResolvedElement, ResolvedKind, ResolvedScene,
    ResolvedText, ScreenshotSource,
};

/// Tolerance (lu) before a text counts as leaving its clip rect.
const CLIP_EPS: f64 = 0.5;

pub(super) struct Scope<'a> {
    pub project: &'a Project,
    pub target: &'a Target,
    pub canvas: &'a Canvas,
    pub options: &'a ValidateOptions<'a>,
}

impl Scope<'_> {
    pub(super) fn page_of(&self, id: &ElementId) -> Option<u32> {
        self.canvas.element(id.as_str()).map(|e| e.page)
    }
}

// ------------------------------------------------------------ per target

/// Checks that need only the target definition.
pub(super) fn target_checks(s: &Scope<'_>, ctx: &Ctx, out: &mut Sink) {
    let catalog = s.options.catalog;
    if let Some(v) = s.target.screenshots.as_deref() {
        let known: BTreeSet<&str> = catalog
            .devices
            .devices
            .iter()
            .filter_map(|d| d.screenshot.variant.as_deref())
            .chain(
                catalog
                    .targets
                    .presets
                    .iter()
                    .filter_map(|p| p.screenshots.as_deref()),
            )
            .collect();
        if !known.contains(v) {
            out.push(ctx.finding(
                Severity::Info,
                C::UnknownScreenshotVariant,
                false,
                None,
                None,
                format!(
                    "screenshot variant `{v}` is not a catalog variant (expected one of {})",
                    known.into_iter().collect::<Vec<_>>().join(", ")
                ),
            ));
        }
    }
}

/// The catalog presets for the target's store slot (empty = none / no slot).
fn slot_presets<'c>(s: &Scope<'c>) -> Vec<&'c TargetPreset> {
    let Some(slot) = &s.target.store else {
        return Vec::new();
    };
    s.options
        .catalog
        .targets
        .presets
        .iter()
        .filter(|p| p.slot.as_ref() == Some(slot))
        .collect()
}

/// Checks on the target frame (language independent): store rules,
/// anchor deviation.
pub(super) fn frame_checks(s: &Scope<'_>, scene: &ResolvedScene, ctx: &Ctx, out: &mut Sink) {
    store_checks(s, &scene.frame, ctx, out);
    decoration_checks(s, scene, ctx, out);
    let layout = &scene.frame.layout;
    if s.target.kind == TargetKind::Pages && s.target.variant.is_none() && !s.target.screenshot_only
    {
        let dev = layout.page_height / layout.reference_page_height - 1.0;
        if dev.abs() > s.options.thresholds.variant_aspect {
            out.push(ctx.finding(
                Severity::Info,
                C::VariantSuggested,
                false,
                None,
                None,
                format!(
                    "page height {:.0} lu differs from the canvas's {:.0} lu by {:+.0} %; \
                     anchors alone may not be enough, consider a variant",
                    layout.page_height,
                    layout.reference_page_height,
                    dev * 100.0
                ),
            ));
        }
    }
}

#[allow(clippy::too_many_lines)]
fn store_checks(s: &Scope<'_>, frame: &Frame, ctx: &Ctx, out: &mut Sink) {
    let Some(slot) = &s.target.store else { return };
    let catalog = s.options.catalog;
    let (value, known, store) = match (&slot.apple, &slot.google) {
        (Some(a), _) => (
            a.display_type.as_str(),
            &catalog.targets.store_slots.apple,
            "App Store display type",
        ),
        (None, Some(g)) => (
            g.image_type.as_str(),
            &catalog.targets.store_slots.google,
            "Google Play image type",
        ),
        (None, None) => return,
    };
    let presets = slot_presets(s);
    let mut push =
        |level, code, message| out.push(ctx.finding(level, code, false, None, None, message));
    if !known.iter().any(|k| k == value) {
        if presets.is_empty() {
            push(
                Severity::Warn,
                C::UnknownStoreSlot,
                format!("unknown {store} `{value}`; store rules are not checked"),
            );
            return;
        }
        if presets.iter().any(|p| p.status == PresetStatus::Unverified) {
            push(
                Severity::Info,
                C::UnverifiedStoreSlot,
                format!("{store} `{value}` is not confirmed by the store API yet"),
            );
        }
    }
    let Some(rules) = presets
        .iter()
        .find_map(|p| p.rules.as_deref())
        .and_then(|r| catalog.store_rules(r))
    else {
        return;
    };
    let format = s.target.format.unwrap_or(ImageFormat::Png);
    if !rules.formats.contains(&format) {
        push(
            Severity::Error,
            C::StoreFormat,
            format!("{} does not accept {format:?} images", rules.label),
        );
    }
    let size = s.target.size;
    match rules.sizes {
        SizePolicy::Exact => {
            if !presets.iter().any(|p| p.has_size(size)) {
                let accepted: BTreeSet<String> =
                    presets.iter().flat_map(|p| p.sizes.all()).map(px).collect();
                push(
                    Severity::Error,
                    C::StoreSize,
                    format!(
                        "{} is not an accepted size for `{value}` (accepted: {})",
                        px(size),
                        accepted.into_iter().collect::<Vec<_>>().join(", ")
                    ),
                );
            }
        }
        SizePolicy::Range => {
            for problem in rules.check_size(size) {
                push(
                    Severity::Error,
                    C::StoreSize,
                    format!("{}: {problem}", rules.label),
                );
            }
        }
    }
    if let Some(count) = rules.count {
        let n = match frame.kind {
            TargetKind::Pages => frame.images.len(),
            TargetKind::Banner => 1,
        } as u32;
        if n > count.max {
            push(
                Severity::Error,
                C::StoreCount,
                format!(
                    "{n} images; {} accepts at most {} per set",
                    rules.label, count.max
                ),
            );
        } else if n < count.min {
            push(
                Severity::Warn,
                C::StoreCount,
                format!(
                    "{n} image(s); {} expects at least {} per set",
                    rules.label, count.min
                ),
            );
        }
    }
    // Alpha: exports are always written without alpha channel (PNG 24-bit,
    // JPEG), so `alpha: false` rules are met by construction.
}

/// Stores that want the bare app UI (`decorations: false`, Wear OS):
/// report frames, texts, backgrounds and graphics of targets that are not
/// `screenshot_only` (info: the user may know better).
fn decoration_checks(s: &Scope<'_>, scene: &ResolvedScene, ctx: &Ctx, out: &mut Sink) {
    if s.target.screenshot_only {
        return;
    }
    let catalog = s.options.catalog;
    let Some(rules) = slot_presets(s)
        .iter()
        .find_map(|p| p.rules.as_deref())
        .and_then(|r| catalog.store_rules(r))
        .filter(|r| !r.decorations)
    else {
        return;
    };
    let mut found = BTreeSet::new();
    if !scene.backgrounds.is_empty() {
        found.insert("backgrounds");
    }
    for e in &scene.elements {
        match &e.props {
            ResolvedKind::Background { .. } => found.insert("backgrounds"),
            ResolvedKind::Text(_) => found.insert("texts"),
            ResolvedKind::Device(d) if d.device.class != DeviceClass::None => {
                found.insert("device frames")
            }
            ResolvedKind::Device(_) => false,
            ResolvedKind::Image(_) | ResolvedKind::Shape(_) => found.insert("graphics"),
        };
    }
    if found.is_empty() {
        return;
    }
    out.push(ctx.finding(
        Severity::Info,
        C::StoreDecorations,
        false,
        None,
        None,
        format!(
            "{} wants the app interface only, without {}; switch on `screenshot_only` to export the bare screenshots",
            rules.label,
            found.into_iter().collect::<Vec<_>>().join(", ")
        ),
    ));
}

fn px(s: PixelSize) -> String {
    format!("{}x{}", s.w, s.h)
}

// ------------------------------------------------- design (per target)

/// Smallest uncovered strip (lu) [`page_cover_checks`] reports.
const COVER_MIN_STRIP: f64 = 2.0;

/// Language-independent layout checks on the first resolved scene of a
/// pages target: devices too small for their page, page-filling pictures
/// or panels that leave a strip uncovered (ADR 0029).
pub(super) fn design_checks(s: &Scope<'_>, scene: &ResolvedScene, ctx: &Ctx, out: &mut Sink) {
    if scene.frame.kind != TargetKind::Pages || s.target.screenshot_only {
        return;
    }
    device_size_checks(s, scene, ctx, out);
    page_cover_checks(scene, ctx, out);
    seam_split_checks(scene, ctx, out);
}

/// `seam_split`: an element clipped at the right edge of page `n` and one
/// of the same kind and size clipped at the left edge of page `n + 1`,
/// at the same height – one shape built as two halves. In the store the
/// gap pulls the halves apart; one element with `clip: none` stays
/// continuous behind the gap in every target.
fn seam_split_checks(scene: &ResolvedScene, ctx: &Ctx, out: &mut Sink) {
    let pages = &scene.frame.pages;
    let near = |a: f64, b: f64, tol: f64| (a - b).abs() <= tol;
    for a in &scene.elements {
        if a.clip.is_none() || matches!(a.props, ResolvedKind::Background { .. }) {
            continue;
        }
        let (Some(pa), Some(pb)) = (pages.get(a.page as usize), pages.get(a.page as usize + 1))
        else {
            continue;
        };
        // Only where the store shows a gap between the two pages.
        if pa.image == pb.image || (pb.rect.x - pa.rect.right()).abs() < 0.5 {
            continue;
        }
        if !near(a.center.x, pa.rect.right(), a.size.x * 0.25) {
            continue;
        }
        let twin = scene.elements.iter().find(|b| {
            b.page == a.page + 1
                && b.clip.is_some()
                && std::mem::discriminant(&b.props) == std::mem::discriminant(&a.props)
                && near(b.center.x, pb.rect.x, b.size.x * 0.25)
                && near(b.center.y, a.center.y, a.size.y * 0.1)
                && near(b.size.x, a.size.x, a.size.x * 0.05)
                && near(b.size.y, a.size.y, a.size.y * 0.05)
        });
        if let Some(b) = twin {
            out.push(ctx.finding(
                Severity::Info,
                C::SeamSplit,
                false,
                Some(a.page),
                Some(&a.id),
                format!(
                    "`{}` and `{}` are two halves of one shape at the border of pages {} and {}: \
                     the store gap ({:.0} lu here) pulls them apart; use one element with \
                     `clip: none` across the border – it stays continuous behind the gap in \
                     every target",
                    a.id,
                    b.id,
                    a.page + 1,
                    b.page + 1,
                    pb.rect.x - pa.rect.right()
                ),
            ));
        }
    }
}

/// `device_small`: the largest device whose home page is `p` fills less
/// than [`super::Thresholds::device_small`] of the page height (or width,
/// for wide devices). Pages with several devices count their largest one.
fn device_size_checks(s: &Scope<'_>, scene: &ResolvedScene, ctx: &Ctx, out: &mut Sink) {
    let min = s.options.thresholds.device_small;
    if min <= 0.0 {
        return;
    }
    for page in &scene.frame.pages {
        let largest = scene
            .elements
            .iter()
            .filter(|e| e.page == page.index && matches!(e.props, ResolvedKind::Device(_)))
            .map(|e| {
                let b = rotated_bounds(Rect::centered(e.center, e.size.x, e.size.y), e);
                (e, (b.h / page.rect.h).max(b.w / page.rect.w))
            })
            .max_by(|a, b| a.1.total_cmp(&b.1));
        if let Some((e, share)) = largest
            && share < min
        {
            out.push(ctx.finding(
                Severity::Info,
                C::DeviceSmall,
                false,
                Some(e.page),
                Some(&e.id),
                format!(
                    "the largest device of page {} fills only {:.0} % of the page (≥ {:.0} % \
                     reads better in the store's small thumbnails)",
                    page.index + 1,
                    share * 100.0,
                    min * 100.0
                ),
            ));
        }
    }
}

/// `page_not_covered`: an unrotated picture or panel covering ≥ 85 % of
/// its home page and reaching three page edges misses the fourth by a
/// strip of at most 15 % of the page (typical after a variant changed the
/// page height).
fn page_cover_checks(scene: &ResolvedScene, ctx: &Ctx, out: &mut Sink) {
    for e in &scene.elements {
        if !matches!(e.props, ResolvedKind::Image(_) | ResolvedKind::Shape(_))
            || e.rotation.rem_euclid(360.0) != 0.0
            || e.opacity < 0.99
        {
            continue;
        }
        let Some(page) = scene.frame.pages.get(e.page as usize) else {
            continue;
        };
        let p = page.rect;
        let r = Rect::centered(e.center, e.size.x, e.size.y);
        let r = match e.clip {
            Some(c) => match intersection(&r, &c) {
                Some(r) => r,
                None => continue,
            },
            None => r,
        };
        let Some(i) = intersection(&r, &p) else {
            continue;
        };
        if i.w * i.h < 0.85 * p.w * p.h {
            continue;
        }
        let strips = [
            ("left", i.x - p.x, p.w),
            ("top", i.y - p.y, p.h),
            ("right", p.right() - i.right(), p.w),
            ("bottom", p.bottom() - i.bottom(), p.h),
        ];
        let open: Vec<_> = strips
            .iter()
            .filter(|(_, gap, _)| *gap > COVER_MIN_STRIP)
            .collect();
        if let [(side, gap, len)] = open[..]
            && *gap <= 0.15 * len
        {
            out.push(ctx.finding(
                Severity::Info,
                C::PageNotCovered,
                false,
                Some(e.page),
                Some(&e.id),
                format!(
                    "covers page {} except a {gap:.0} lu strip at the {side}; the canvas \
                     background shows there (resize it in this target's variant, or set \
                     anchor_y: stretch)",
                    e.page + 1
                ),
            ));
        }
    }
}

// ---------------------------------------------------- per target × lang

pub(super) fn scene_checks(
    s: &Scope<'_>,
    scene: &ResolvedScene,
    layouter: &mut TextLayouter,
    ctx: &Ctx,
    out: &mut Sink,
) {
    resolve_diagnostics(s, scene, ctx, out);
    let spans = span_styles(&scene.spans);
    let pages: Vec<Rect> = scene.frame.pages.iter().map(|p| p.rect).collect();
    for e in &scene.elements {
        let bounds = match &e.props {
            ResolvedKind::Background { .. } => continue,
            ResolvedKind::Text(t) => text_checks(s, scene, e, t, layouter, &spans, ctx, out),
            ResolvedKind::Device(d) => {
                screenshot_checks(s, scene, e, d, ctx, out);
                Some(rotated_bounds(
                    Rect::centered(e.center, e.size.x, e.size.y),
                    e,
                ))
            }
            ResolvedKind::Image(_) | ResolvedKind::Shape(_) => Some(rotated_bounds(
                Rect::centered(e.center, e.size.x, e.size.y),
                e,
            )),
        };
        if let Some(b) = bounds
            && !pages.iter().any(|p| touches(&b, p))
        {
            out.push(ctx.finding(
                Severity::Warn,
                C::ElementOutside,
                false,
                Some(e.page),
                Some(&e.id),
                "element lies entirely outside every page; it appears in no export".into(),
            ));
        }
    }
}

/// Resolve diagnostics → findings. Text and screenshot diagnostics are
/// rebuilt from the scene (they need the language they came from).
fn resolve_diagnostics(s: &Scope<'_>, scene: &ResolvedScene, ctx: &Ctx, out: &mut Sink) {
    use DiagnosticCode as D;
    for d in &scene.diagnostics {
        let keep = match d.level {
            Level::Info => Severity::Info,
            Level::Warn => Severity::Warn,
        };
        let (code, level, lang_dependent) = match d.code {
            D::TextFallback | D::TextMissing | D::ScreenshotFallback | D::ScreenshotMissing => {
                continue;
            }
            D::LangNotAvailable => (C::LangNotAvailable, Severity::Warn, true),
            D::UnknownVariant => (C::UnknownVariant, Severity::Warn, false),
            D::UnknownOverrideElement => (C::UnknownOverrideElement, Severity::Warn, false),
            D::InvalidOverride => (C::InvalidOverride, Severity::Warn, false),
            D::UnknownDevice => (C::UnknownDevice, Severity::Error, false),
            D::UnknownMappedDevice => (C::UnknownMappedDevice, Severity::Warn, false),
            D::DeviceColorFallback => (C::UnknownDeviceColor, keep, false),
            D::UnsupportedOrientation => (C::UnsupportedOrientation, Severity::Warn, false),
            D::UnknownColor => (C::UnknownColor, Severity::Error, false),
            D::UnknownTextStyle => (C::UnknownTextStyle, Severity::Warn, false),
            D::UnknownShadow => (C::UnknownShadow, Severity::Warn, false),
            D::UnknownGallery => (C::UnknownGallery, Severity::Warn, false),
            D::NoGallery => (C::NoGallery, Severity::Warn, false),
            D::PageOutOfRange => (C::PageOutOfRange, Severity::Error, false),
            D::MissingSize => (C::MissingSize, keep, false),
            D::PartialImage => (C::PartialImage, Severity::Info, false),
            D::RegionOutOfRange => (C::RegionOutOfRange, Severity::Warn, false),
            D::ImportedFrameMissing => (C::ImportedFrameMissing, Severity::Info, false),
            D::ImportedBezelFallback => (C::ImportedBezelFallback, Severity::Info, false),
            D::ImportedColorFallback => (C::ImportedColorFallback, Severity::Info, false),
            D::ScreenshotOnlyEmpty => (C::ScreenshotOnlyEmpty, Severity::Warn, false),
            D::BackgroundHidden => (C::BackgroundHidden, Severity::Warn, false),
        };
        let page = d.element.as_ref().and_then(|id| s.page_of(id));
        out.push(ctx.finding(
            level,
            code,
            lang_dependent,
            page,
            d.element.as_ref(),
            d.message.clone(),
        ));
    }
}

/// Text checks; returns the text's canvas bounds (rotated, unclipped).
#[allow(clippy::too_many_arguments, clippy::too_many_lines)]
fn text_checks(
    s: &Scope<'_>,
    scene: &ResolvedScene,
    e: &ResolvedElement,
    t: &ResolvedText,
    layouter: &mut TextLayouter,
    spans: &std::collections::BTreeMap<String, crate::layout::ResolvedSpanStyle>,
    ctx: &Ctx,
    out: &mut Sink,
) -> Option<Rect> {
    let lang = scene.lang.as_str();
    let key = t.key.as_str();
    let mut push = |level, code, message: String| {
        out.push(ctx.finding(level, code, true, Some(e.page), Some(&e.id), message));
    };
    match &t.text_lang {
        None => {
            let anywhere = s.project.strings.values().any(|m| m.contains_key(key));
            push(
                Severity::Error,
                C::TextMissing,
                if anywhere {
                    format!("text `{key}` is missing in `{lang}` and its fallback languages")
                } else {
                    format!("text `{key}` exists in no language")
                },
            );
        }
        Some(from) if from.as_str() != lang => {
            // Falling back to the default language means "not translated"
            // (warning); a prefix or configured fallback is deliberate.
            let level = if *from == s.project.languages.default {
                Severity::Warn
            } else {
                Severity::Info
            };
            push(
                level,
                C::TextFallback,
                format!("`{lang}` has no text for `{key}`; using `{from}`"),
            );
        }
        Some(_) => {}
    }
    let placed = layout_text(layouter, t, spans)?;
    let l = &placed.layout;
    let mut overflow = Vec::new();
    for d in &l.diagnostics {
        let (level, code) = match d {
            TextDiagnostic::Markup { .. } => (Severity::Warn, C::InvalidMarkup),
            TextDiagnostic::UnknownSpanStyle { .. } => (Severity::Warn, C::UnknownSpanStyle),
            TextDiagnostic::UnknownFont { .. } => (Severity::Warn, C::UnknownFont),
            TextDiagnostic::MissingGlyphs { .. } => (Severity::Warn, C::MissingGlyphs),
            TextDiagnostic::EmergencyBreak { .. } => (Severity::Warn, C::EmergencyBreak),
            TextDiagnostic::Hyphenated { .. } => (Severity::Info, C::Hyphenated),
            d if d.is_overflow() => {
                overflow.push(d.to_string());
                continue;
            }
            _ => continue,
        };
        push(level, code, format!("text `{key}`: {d}"));
    }
    if l.overflow || !overflow.is_empty() {
        let detail = if overflow.is_empty() {
            "does not fit its box".to_owned()
        } else {
            overflow.join("; ")
        };
        push(
            Severity::Error,
            C::TextOverflow,
            format!(
                "text `{key}` overflows in `{lang}` at {:.1} lu: {detail}",
                l.font_size
            ),
        );
        if t.hyphenate
            && let Some(tl) = &t.text_lang
            && crate::layout::hyphen::pattern_set_for(tl.as_str()).is_none()
        {
            push(
                Severity::Info,
                C::HyphenationUnavailable,
                format!("no hyphenation patterns for `{tl}`; `hyphenate` has no effect"),
            );
        }
    }
    let content = placed.content_rect();
    if content.w <= 0.0 || content.h <= 0.0 {
        return None;
    }
    // The hard-edged outline counts; soft glows and shadows fading out at
    // a page edge or into a gap are not reported.
    let bounds = rotated_bounds(placed.ink_rect(), e);
    let visible = match e.clip {
        Some(clip) => {
            if !inside(&bounds, &clip) {
                push(
                    Severity::Warn,
                    C::TextClipped,
                    format!("text `{key}` extends beyond its page and is cut off"),
                );
            }
            intersection(&bounds, &clip)
        }
        None => Some(bounds),
    };
    if let Some(v) = visible
        && let Some(g) = scene.frame.gaps.iter().find(|g| v.intersects(g))
    {
        push(
            Severity::Warn,
            C::TextInGap,
            format!(
                "text `{key}` reaches into the store gap at x {:.0}–{:.0} lu and is cut off in the exports",
                g.x,
                g.right()
            ),
        );
    }
    if let (Some(area), Some(v)) = (s.target.safe_area, visible) {
        for img in &scene.frame.images {
            let Some(part) = intersection(&v, &img.source) else {
                continue;
            };
            if !inside(&part, &area.inside(img.source)) {
                push(
                    Severity::Warn,
                    C::TextOutsideSafeArea,
                    format!(
                        "text `{key}` lies outside the safe area of image {} (keep key content inside it)",
                        img.index + 1
                    ),
                );
                break;
            }
        }
    }
    Some(bounds)
}

fn screenshot_checks(
    s: &Scope<'_>,
    scene: &ResolvedScene,
    e: &ResolvedElement,
    d: &ResolvedDevice,
    ctx: &Ctx,
    out: &mut Sink,
) {
    let Some(shot) = &d.screenshot else { return };
    let lang = scene.lang.as_str();
    let key = &shot.key;
    let th = &s.options.thresholds;
    let mut push = |level, code, lang_dependent, message: String| {
        out.push(ctx.finding(
            level,
            code,
            lang_dependent,
            Some(e.page),
            Some(&e.id),
            message,
        ));
    };
    match shot.source {
        ScreenshotSource::Missing => {
            let level = match s.options.mode {
                ValidateMode::Export => Severity::Error,
                ValidateMode::Preview => Severity::Warn,
            };
            push(
                level,
                C::ScreenshotMissing,
                true,
                format!(
                    "screenshot `{key}` not found for `{lang}` (variant {}); a placeholder is shown",
                    shot.variant.as_deref().unwrap_or("-")
                ),
            );
            return;
        }
        ScreenshotSource::Fallback | ScreenshotSource::FallbackVariant => {
            let from = shot.lang.as_ref().map_or("?", |l| l.as_str());
            let which = if shot.lang.as_ref() == Some(&s.project.languages.default) {
                "default"
            } else {
                "fallback"
            };
            push(
                Severity::Info,
                C::ScreenshotFallback,
                true,
                format!(
                    "`{lang}` uses the {which} `{from}` screenshot for `{key}` ({})",
                    shot.path.as_deref().unwrap_or_default()
                ),
            );
        }
        ScreenshotSource::Lang | ScreenshotSource::LangVariant => {}
    }
    // Aspect, upscale and colour profile depend on the file, not the
    // language: findings name the file and are recorded without language,
    // so languages that fall back to the same file share one finding.
    let file = shot.path.as_deref().unwrap_or_default();
    for (code, message) in file_notes(s, file) {
        // A sample may serve only some languages: per language.
        push(Severity::Info, code, code == C::SampleScreenshot, message);
    }
    let Some(size) = shot.size.filter(|p| p.w > 0 && p.h > 0) else {
        return;
    };
    let (sw, sh) = (f64::from(size.w), f64::from(size.h));
    let (dw_lu, dh_lu) = screen_lu(s, e, d);
    if !d.free_aspect {
        let want = if dh_lu > 0.0 {
            dw_lu / dh_lu
        } else {
            d.device.display_ratio(d.orientation)
        };
        let dev = (sw / sh) / want - 1.0;
        if dev.abs() > th.screenshot_aspect {
            push(
                Severity::Warn,
                C::ScreenshotAspect,
                false,
                format!(
                    "screenshot `{file}` ({}x{}, {:.3}) differs from the `{}` display ({want:.3}) by {:+.1} %; wrong device?",
                    size.w,
                    size.h,
                    sw / sh,
                    d.model,
                    dev * 100.0
                ),
            );
        }
    }
    let scale = image_scale(&scene.frame, e.page);
    let (dw, dh) = (dw_lu * scale, dh_lu * scale);
    let (kx, ky) = (dw / sw, dh / sh);
    let factor = match d.fit {
        FitMode::Contain => kx.min(ky),
        FitMode::Cover | FitMode::Stretch => kx.max(ky),
    };
    if factor > th.max_upscale {
        push(
            Severity::Warn,
            C::ScreenshotUpscale,
            false,
            format!(
                "screenshot `{file}` ({}x{}) is enlarged {factor:.2}x to fill the {dw:.0}x{dh:.0} px display; it may look blurry",
                size.w, size.h
            ),
        );
    }
}

/// A screenshot whose colour profile cannot be used (drawn as sRGB).
fn color_check(s: &Scope<'_>, file: &str) -> Option<String> {
    if file.is_empty() {
        return None;
    }
    let note = s.options.assets.color_note(file)?;
    Some(format!("screenshot `{file}`: {note}"))
}

/// Info findings about the screenshot file itself: an unusable colour
/// profile, a sample screenshot from a template.
fn file_notes(s: &Scope<'_>, file: &str) -> Vec<(C, String)> {
    let mut out = Vec::new();
    if let Some(message) = color_check(s, file) {
        out.push((C::ColorProfileUnsupported, message));
    }
    if !file.is_empty() && s.options.assets.is_sample(file) {
        out.push((
            C::SampleScreenshot,
            format!(
                "screenshot `{file}` is a sample from a template; replace it with your own (store uploads refuse samples)"
            ),
        ));
    }
    out
}

/// Size in lu of the area the screenshot fills: the display, minus the
/// browser bars of `chrome: browser` on a laptop / desktop (as drawn by
/// the renderer).
fn screen_lu(s: &Scope<'_>, e: &ResolvedElement, d: &ResolvedDevice) -> (f64, f64) {
    if let Some(f) = &d.imported {
        // The detected screen rect of the imported bezel.
        let (bw, bh) = (f64::from(f.size.w.max(1)), f64::from(f.size.h.max(1)));
        return (
            e.size.x * f64::from(f.screen.w) / bw,
            e.size.y * f64::from(f.screen.h) / bh,
        );
    }
    let (fx, fy) = d.device.display_fraction(d.orientation);
    let (w, h) = (e.size.x * fx, e.size.y * fy);
    if d.chrome == Some(Chrome::Browser)
        && matches!(d.device.class, DeviceClass::Laptop | DeviceClass::Desktop)
    {
        let bars = s.options.catalog.devices.browser_chrome.in_display(w, h).1;
        return (w, h - bars);
    }
    (w, h)
}

/// px per lu of the export image showing `page`.
fn image_scale(frame: &Frame, page: u32) -> f64 {
    let image = frame
        .pages
        .get(page as usize)
        .and_then(|p| p.image)
        .unwrap_or(0);
    frame
        .images
        .get(image as usize)
        .or(frame.images.first())
        .map_or(1.0, |i| i.viewport.scale)
}

// ------------------------------------------------------------- geometry

/// Axis-aligned bounds of `r` rotated by the element's rotation around its
/// pivot point (`center + pivot`).
fn rotated_bounds(r: Rect, e: &ResolvedElement) -> Rect {
    if e.rotation.rem_euclid(360.0) == 0.0 {
        return r;
    }
    let pivot = e.center + e.pivot;
    let corners = [
        Vec2::new(r.x, r.y),
        Vec2::new(r.right(), r.y),
        Vec2::new(r.x, r.bottom()),
        Vec2::new(r.right(), r.bottom()),
    ]
    .map(|c| c.rotated_around(pivot, e.rotation));
    let (mut lo, mut hi) = (corners[0], corners[0]);
    for c in &corners[1..] {
        lo = Vec2::new(lo.x.min(c.x), lo.y.min(c.y));
        hi = Vec2::new(hi.x.max(c.x), hi.y.max(c.y));
    }
    Rect::new(lo.x, lo.y, hi.x - lo.x, hi.y - lo.y)
}

fn inside(a: &Rect, b: &Rect) -> bool {
    a.x >= b.x - CLIP_EPS
        && a.y >= b.y - CLIP_EPS
        && a.right() <= b.right() + CLIP_EPS
        && a.bottom() <= b.bottom() + CLIP_EPS
}

fn intersection(a: &Rect, b: &Rect) -> Option<Rect> {
    let x = a.x.max(b.x);
    let y = a.y.max(b.y);
    let r = a.right().min(b.right());
    let btm = a.bottom().min(b.bottom());
    (r > x && btm > y).then(|| Rect::new(x, y, r - x, btm - y))
}

/// Overlap test that also works for degenerate (zero-sized) rects: those
/// count by their centre.
fn touches(a: &Rect, page: &Rect) -> bool {
    if a.w <= 0.0 || a.h <= 0.0 {
        let c = a.center();
        return c.x >= page.x && c.x <= page.right() && c.y >= page.y && c.y <= page.bottom();
    }
    a.intersects(page)
}
