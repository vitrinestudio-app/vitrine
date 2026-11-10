//! `resolve(project, target, lang)` → [`ResolvedScene`] (KONZEPT §6.1,
//! ADR 0004): everything layout and rasterizer need, in canvas lu and with
//! concrete values, free of YAML concepts.
//!
//! Steps:
//! 1. **Frame** ([`Frame`]): page height from the target aspect (and
//!    `group`), gaps from the gallery profile (or `gaps:` on banners),
//!    output images with their canvas rects and viewports.
//! 2. **Variant**: the target's variant patches elements (on copies; the
//!    project is never mutated) and scales devices by `content_scale`.
//! 3. **Elements**: visibility (`visible`, `visible_in`), home page +
//!    anchor → canvas, `device_map`, colour variants, frame style, style
//!    tokens, texts along the language chain, screenshots along the lookup
//!    chain, group transforms; flattened and sorted by `z`.
//! 4. **Diagnostics**: fallbacks, missing assets and unknown references
//!    are collected as infos/warnings (reused by `core::validate`).

mod elements;
mod frame;
mod lookup;
mod scene;
mod style;

use crate::catalog::Catalog;
use crate::model::{ElementId, Lang, Project};

pub use lookup::{
    AssetLookup, FsAssets, NoAssets, SCREENSHOT_EXTENSIONS, lang_chain, lookup_screenshot,
    lookup_text,
};
pub use scene::{
    DeviceColor, Diagnostic, DiagnosticCode, Frame, GalleryInfo, ImportedFrame, Level, OutputImage,
    PageFrame, ResolvedDevice, ResolvedElement, ResolvedFill, ResolvedGlow, ResolvedImage,
    ResolvedKind, ResolvedScene, ResolvedScreenshot, ResolvedShadowLayer, ResolvedShape,
    ResolvedSpanStyle, ResolvedStop, ResolvedStroke, ResolvedText, ResolvedTextStroke,
    ResolvedTextStyle, ScreenshotSource, TextEffects, TextFrame,
};
pub use style::{DEFAULT_FONT, StyleResolver, UNKNOWN_COLOR, builtin_shadow, default_text_style};

/// Inputs besides the project.
#[derive(Clone, Copy)]
pub struct ResolveOptions<'a> {
    /// Device models and gallery profiles.
    pub catalog: &'a Catalog,
    /// Screenshot file existence.
    pub assets: &'a dyn AssetLookup,
    /// Lays out auto-height texts to find their rotation pivot (the centre
    /// of the laid-out content, ADR 0004). `None` = a per-thread layouter
    /// with the bundled fonts only; pass the project's fonts (the render
    /// context, a [`crate::layout::TextLayouter`]) so pivots match the
    /// rendered lines.
    pub text: Option<&'a dyn TextMeasure>,
    /// Installed vendor frames (`frame: imported:<source>`, ADR 0012);
    /// `None` = none installed (parametric fallback + info).
    pub frames: Option<&'a crate::frame_import::FrameIndex>,
}

impl<'a> ResolveOptions<'a> {
    /// Built-in catalog with the given asset lookup.
    pub fn new(assets: &'a dyn AssetLookup) -> Self {
        Self {
            catalog: crate::catalog::catalog(),
            assets,
            text: None,
            frames: None,
        }
    }

    /// The same options with installed vendor frames.
    #[must_use]
    pub fn with_frames(mut self, frames: Option<&'a crate::frame_import::FrameIndex>) -> Self {
        self.frames = frames;
        self
    }

    /// The same options measuring texts with `text`.
    #[must_use]
    pub fn with_text(mut self, text: &'a dyn TextMeasure) -> Self {
        self.text = Some(text);
        self
    }
}

/// Measures laid-out text during resolve: the default rotation pivot of an
/// auto-height text box is the centre of its laid-out content, which only
/// layout knows (ADR 0004, *Texts*). Implemented by
/// `RefCell`/`Mutex<TextLayouter>` and by the renderer's context.
pub trait TextMeasure {
    /// Canvas rect (unrotated) of the laid-out content of `text` placed in
    /// its frame, as [`crate::layout::PlacedText::content_rect`]; `None`
    /// when it has no text.
    fn content_rect(
        &self,
        text: &ResolvedText,
        spans: &std::collections::BTreeMap<String, ResolvedSpanStyle>,
    ) -> Option<crate::geometry::Rect>;
}

impl std::fmt::Debug for ResolveOptions<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ResolveOptions").finish_non_exhaustive()
    }
}

/// Errors that make resolving impossible. Everything else (dangling
/// references, missing texts or assets) becomes a [`Diagnostic`].
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum ResolveError {
    #[error("unknown target `{0}`")]
    UnknownTarget(String),
    #[error("target `{target}` refers to unknown strip `{strip}`")]
    UnknownStrip { target: String, strip: String },
    #[error("invalid language code `{0}`")]
    InvalidLang(String),
}

/// Resolve `target_id` in language `lang`.
pub fn resolve(
    project: &Project,
    target_id: &str,
    lang: &str,
    options: &ResolveOptions<'_>,
) -> Result<ResolvedScene, ResolveError> {
    let target = project
        .target(target_id)
        .ok_or_else(|| ResolveError::UnknownTarget(target_id.to_owned()))?;
    let strip = project
        .strip(target.strip.as_str())
        .ok_or_else(|| ResolveError::UnknownStrip {
            target: target_id.to_owned(),
            strip: target.strip.to_string(),
        })?;
    let lang_id = Lang::parse(lang).map_err(|_| ResolveError::InvalidLang(lang.to_owned()))?;

    let mut d = Diags::default();
    if !project.languages.is_available(lang) {
        d.warn(
            DiagnosticCode::LangNotAvailable,
            format!("language `{lang}` is not listed in `languages.available`"),
        );
    }
    let frame = frame::frame(target, strip, options.catalog, &mut d);
    let styles = StyleResolver::new(&project.styles);
    let spans = styles.spans(&mut d);
    let backgrounds = strip
        .backgrounds
        .iter()
        .map(|f| styles.fill(f, &mut d))
        .collect();
    let (variant, elements) = elements::resolve_elements(
        &elements::Inputs {
            project,
            target,
            strip,
            lang,
            chain: lang_chain(project, lang),
            layout: frame.layout,
            options,
            styles,
            spans: &spans,
        },
        &mut d,
    );
    let (backgrounds, elements) =
        if target.screenshot_only && target.kind == crate::model::TargetKind::Pages {
            let only = elements::screenshot_only(
                &elements,
                &frame.layout,
                strip.page.count,
                options.catalog,
                &mut d,
            );
            (Vec::new(), only)
        } else {
            (backgrounds, elements)
        };
    Ok(ResolvedScene {
        target: target.id.clone(),
        strip: strip.id.clone(),
        lang: lang_id,
        variant,
        frame,
        backgrounds,
        elements,
        spans,
        diagnostics: d.items,
    })
}

/// Diagnostics sink; `element` is attached to every diagnostic while set.
#[derive(Debug, Default)]
pub struct Diags {
    pub items: Vec<Diagnostic>,
    pub element: Option<ElementId>,
}

impl Diags {
    fn push(&mut self, level: Level, code: DiagnosticCode, message: String) {
        let d = Diagnostic {
            level,
            code,
            element: self.element.clone(),
            message,
        };
        if !self.items.contains(&d) {
            self.items.push(d);
        }
    }

    pub fn info(&mut self, code: DiagnosticCode, message: String) {
        self.push(Level::Info, code, message);
    }

    pub fn warn(&mut self, code: DiagnosticCode, message: String) {
        self.push(Level::Warn, code, message);
    }
}
