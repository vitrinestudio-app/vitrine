//! Project validation (KONZEPT §3.3, §3.4, §4.3, §5, §6.4; ADR 0005):
//! every target × language is resolved and its texts are laid out; the
//! findings of resolve and layout are reused and the checks that need the
//! whole picture are added (text in gaps, overflow, screenshot aspect and
//! upscaling, store rules, anchor deviation, elements off the pages).
//!
//! ```
//! use arrangia_core::{Project, model::Lang, resolve::NoAssets};
//! use arrangia_core::validate::{ValidateOptions, validate};
//!
//! let project = Project::new("demo", Lang::new("en"));
//! let report = validate(&project, &ValidateOptions::new(&NoAssets));
//! assert!(!report.has_errors());
//! ```
//!
//! Findings carry a [`Severity`], a stable [`FindingCode`] (snake case in
//! JSON/YAML, see [`FindingCode::as_str`]) and where they apply (target,
//! language, page, element). Front ends (CLI exit code, MCP, GUI markers)
//! only consume the report.

mod checks;
mod store;
#[cfg(test)]
mod tests;

use std::collections::HashSet;

use serde::{Deserialize, Serialize};

use crate::catalog::{ASPECT_TOLERANCE, Catalog};
use crate::layout::{FontLibrary, TextLayouter};
use crate::model::{ElementId, Lang, Project, Target, TargetId};
use crate::resolve::{AssetLookup, ResolveError, ResolveOptions, resolve};

/// Default for [`Thresholds::max_upscale`]: a screenshot may be enlarged by
/// up to 15 % before it is reported as blurry. Store screenshots are
/// usually viewed downscaled, so a little upscaling is invisible; beyond
/// that, text in the screenshot gets soft.
pub const DEFAULT_MAX_UPSCALE: f64 = 1.15;

/// Default for [`Thresholds::variant_aspect`]: page height deviation
/// between target and strip (relative) above which a variant is suggested
/// (KONZEPT §3.4). 15 % covers iPhone 6.9″ ↔ Google Play 1:2 (8 %) with
/// anchors alone but flags grouped tablet columns (≈ 30 %).
pub const DEFAULT_VARIANT_ASPECT: f64 = 0.15;

/// Severity of a finding.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Severity {
    Info,
    Warn,
    /// Blocks export/upload (CLI `validate` exits non-zero).
    Error,
}

macro_rules! finding_codes {
    ($( $(#[$m:meta])* $v:ident = $s:literal, )*) => {
        /// What a finding is about. The string forms are stable (CLI, MCP,
        /// GUI rely on them); new codes may be added.
        #[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
        pub enum FindingCode {
            $( $(#[$m])* #[serde(rename = $s)] $v, )*
        }

        impl FindingCode {
            /// All codes in declaration order.
            pub const ALL: &'static [FindingCode] = &[$(Self::$v),*];

            /// The stable snake-case name (`text_overflow`).
            pub fn as_str(self) -> &'static str {
                match self { $(Self::$v => $s,)* }
            }
        }
    };
}

finding_codes! {
    // ---- references and structure
    /// A target filter names an unknown target.
    UnknownTarget = "unknown_target",
    /// A target refers to an unknown strip (the target is skipped).
    UnknownStrip = "unknown_strip",
    /// A language code is invalid.
    InvalidLang = "invalid_lang",
    /// A language is not in `languages.available`.
    LangNotAvailable = "lang_not_available",
    UnknownVariant = "unknown_variant",
    /// A variant override names an element that does not exist.
    UnknownOverrideElement = "unknown_override_element",
    /// A variant override does not apply to its element.
    InvalidOverride = "invalid_override",
    /// A variant or language override set a field to `null` in the file;
    /// it was ignored (an override only sets values) and is dropped on
    /// the next save (info).
    OverrideNull = "override_null",
    /// Unknown device model (the element is not rendered).
    UnknownDevice = "unknown_device",
    /// `device_map` names an unknown replacement model.
    UnknownMappedDevice = "unknown_mapped_device",
    /// The colour variant does not exist for the (mapped) model.
    UnknownDeviceColor = "unknown_device_color",
    /// `frame: imported:<source>` but the frame is not installed on this
    /// machine; the parametric frame is used.
    ImportedFrameMissing = "imported_frame_missing",
    /// The pinned `bezel` is not installed, or no installed bezel size
    /// matches the target / screenshot (a fallback is used).
    ImportedBezelFallback = "imported_bezel_fallback",
    /// The colour is not installed for the imported frame; another
    /// installed colour is drawn.
    ImportedColorFallback = "imported_color_fallback",
    UnsupportedOrientation = "unsupported_orientation",
    /// Unknown `@colour` (rendered magenta).
    UnknownColor = "unknown_color",
    UnknownTextStyle = "unknown_text_style",
    /// `[name]…[/]` names no span style.
    UnknownSpanStyle = "unknown_span_style",
    UnknownShadow = "unknown_shadow",
    UnknownGallery = "unknown_gallery",
    /// `gaps: as_store` on a banner without gallery.
    NoGallery = "no_gallery",
    /// An element's home page is outside the strip.
    PageOutOfRange = "page_out_of_range",
    /// A banner region exceeds the strip.
    RegionOutOfRange = "region_out_of_range",
    /// A required size is missing; a default is used.
    MissingSize = "missing_size",
    /// Page count is not a multiple of `group`.
    PartialImage = "partial_image",
    /// The target's `screenshots` key is not a catalog screenshot variant.
    UnknownScreenshotVariant = "unknown_screenshot_variant",
    /// The store slot value is not a known store API value.
    UnknownStoreSlot = "unknown_store_slot",
    /// The store slot is known only from unconfirmed sources.
    UnverifiedStoreSlot = "unverified_store_slot",
    // ---- texts
    /// The text key exists in no language of the chain (nothing rendered).
    TextMissing = "text_missing",
    /// The text comes from a fallback language.
    TextFallback = "text_fallback",
    /// The text does not fit its box / `max_lines` even at `shrink_to`.
    TextOverflow = "text_overflow",
    /// A word was broken without hyphen (add a soft hyphen `\-`).
    EmergencyBreak = "emergency_break",
    /// Automatic hyphenation was needed to fit the text (info).
    Hyphenated = "hyphenated",
    /// `hyphenate` is on but no patterns are bundled for the language.
    HyphenationUnavailable = "hyphenation_unavailable",
    /// Unbalanced inline markup, rendered literally.
    InvalidMarkup = "invalid_markup",
    UnknownFont = "unknown_font",
    /// Characters without a glyph in any font of the chain.
    MissingGlyphs = "missing_glyphs",
    /// Text reaches into a gap between store images (cut off in exports).
    TextInGap = "text_in_gap",
    /// Text extends beyond its page and is cut off (`clip: page`).
    TextClipped = "text_clipped",
    /// Text lies (partly) outside the target's `safe_area`.
    TextOutsideSafeArea = "text_outside_safe_area",
    // ---- screenshots
    ScreenshotMissing = "screenshot_missing",
    /// The screenshot comes from a fallback language.
    ScreenshotFallback = "screenshot_fallback",
    /// Screenshot aspect differs from the device display (wrong device?).
    ScreenshotAspect = "screenshot_aspect",
    /// The screenshot is enlarged beyond [`Thresholds::max_upscale`].
    ScreenshotUpscale = "screenshot_upscale",
    /// A page of a `screenshot_only` target has no device (empty image).
    ScreenshotOnlyEmpty = "screenshot_only_empty",
    /// The screenshot's embedded colour profile cannot be used (unreadable,
    /// CMYK, HDR …); it is rendered as if it were sRGB (info).
    ColorProfileUnsupported = "color_profile_unsupported",
    // ---- store rules
    /// Target size not accepted for the store slot.
    StoreSize = "store_size",
    /// Image count outside the store's min/max per set.
    StoreCount = "store_count",
    /// Export format not accepted by the store.
    StoreFormat = "store_format",
    /// The store wants the bare app UI (Wear OS) but the target draws
    /// frames, texts or backgrounds (info; `screenshot_only` avoids it).
    StoreDecorations = "store_decorations",
    // ---- store mapping (`store:` in project.yaml)
    /// An explicit `store.locales` code is not a locale of that store.
    StoreLocaleUnknown = "store_locale_unknown",
    /// A `store.locales` entry names a language the project does not have (info).
    StoreLocaleUnused = "store_locale_unused",
    /// A language to upload has no store locale (explicit or default).
    StoreLocaleMissing = "store_locale_missing",
    /// Several languages map to the same store locale.
    StoreLocaleDuplicate = "store_locale_duplicate",
    /// The project is mapped to a store but no target has a slot for it.
    StoreNoSlot = "store_no_slot",
    /// Two targets fill the same store slot for the same language.
    StoreSlotDuplicate = "store_slot_duplicate",
    // ---- layout
    /// Target page height deviates strongly from the strip: suggest a variant.
    VariantSuggested = "variant_suggested",
    /// An element lies entirely outside every page.
    ElementOutside = "element_outside",
}

impl std::fmt::Display for FindingCode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// One finding.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Finding {
    pub level: Severity,
    pub code: FindingCode,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target: Option<TargetId>,
    /// Set for language-dependent findings only.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lang: Option<Lang>,
    /// Home page of the element (0-based), if the finding is about one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub page: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub element: Option<ElementId>,
    pub message: String,
}

/// Result of [`validate`]: findings in a stable order (targets in project
/// order, languages in target order, elements in painting order).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct ValidationReport {
    pub findings: Vec<Finding>,
}

impl ValidationReport {
    pub fn has_errors(&self) -> bool {
        self.findings.iter().any(|f| f.level == Severity::Error)
    }

    /// Number of findings with this severity.
    pub fn count(&self, level: Severity) -> usize {
        self.findings.iter().filter(|f| f.level == level).count()
    }

    /// Highest severity (`None` = no findings).
    pub fn max_level(&self) -> Option<Severity> {
        self.findings.iter().map(|f| f.level).max()
    }

    pub fn with_code(&self, code: FindingCode) -> impl Iterator<Item = &Finding> {
        self.findings.iter().filter(move |f| f.code == code)
    }

    pub fn is_empty(&self) -> bool {
        self.findings.is_empty()
    }
}

/// What the validation is for.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ValidateMode {
    /// Before export/upload: a missing screenshot is an error.
    #[default]
    Export,
    /// While editing: a missing screenshot is a warning (the preview shows a
    /// placeholder).
    Preview,
}

/// Configurable limits.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Thresholds {
    /// Max relative deviation of screenshot vs display aspect (KONZEPT §4.3).
    pub screenshot_aspect: f64,
    /// Max screenshot enlargement factor ([`DEFAULT_MAX_UPSCALE`]).
    pub max_upscale: f64,
    /// Max relative page height deviation target vs strip without a variant
    /// ([`DEFAULT_VARIANT_ASPECT`]).
    pub variant_aspect: f64,
}

impl Default for Thresholds {
    fn default() -> Self {
        Self {
            screenshot_aspect: ASPECT_TOLERANCE,
            max_upscale: DEFAULT_MAX_UPSCALE,
            variant_aspect: DEFAULT_VARIANT_ASPECT,
        }
    }
}

/// Inputs besides the project.
#[derive(Clone)]
pub struct ValidateOptions<'a> {
    pub catalog: &'a Catalog,
    /// Screenshot existence and dimensions.
    pub assets: &'a dyn AssetLookup,
    /// Fonts for text layout (bundled + project `fonts/`).
    pub fonts: FontLibrary,
    /// Only these targets (`None` = all).
    pub targets: Option<Vec<String>>,
    /// Only these languages (`None` = each target's languages).
    pub langs: Option<Vec<String>>,
    pub mode: ValidateMode,
    pub thresholds: Thresholds,
    /// Installed vendor frames (ADR 0012); `None` = none installed.
    pub frames: Option<&'a crate::frame_import::FrameIndex>,
}

impl<'a> ValidateOptions<'a> {
    /// Built-in catalog, bundled fonts, all targets and languages, export
    /// mode, default thresholds.
    pub fn new(assets: &'a dyn AssetLookup) -> Self {
        Self {
            catalog: crate::catalog::catalog(),
            assets,
            fonts: FontLibrary::bundled(),
            targets: None,
            langs: None,
            mode: ValidateMode::default(),
            thresholds: Thresholds::default(),
            frames: None,
        }
    }
}

impl std::fmt::Debug for ValidateOptions<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ValidateOptions")
            .field("targets", &self.targets)
            .field("langs", &self.langs)
            .field("mode", &self.mode)
            .field("thresholds", &self.thresholds)
            .finish_non_exhaustive()
    }
}

/// Collects findings in order, dropping exact duplicates (e.g. the same
/// unknown style reported for every language).
#[derive(Debug, Default)]
struct Sink {
    seen: HashSet<Finding>,
    findings: Vec<Finding>,
}

impl Sink {
    fn push(&mut self, f: Finding) {
        if self.seen.insert(f.clone()) {
            self.findings.push(f);
        }
    }
}

/// Where findings of the current step apply.
#[derive(Clone, Debug, Default)]
struct Ctx {
    target: Option<TargetId>,
    lang: Option<Lang>,
}

impl Ctx {
    /// A finding; `lang_dependent` decides whether the language is recorded.
    fn finding(
        &self,
        level: Severity,
        code: FindingCode,
        lang_dependent: bool,
        page: Option<u32>,
        element: Option<&ElementId>,
        message: String,
    ) -> Finding {
        Finding {
            level,
            code,
            target: self.target.clone(),
            lang: if lang_dependent {
                self.lang.clone()
            } else {
                None
            },
            page,
            element: element.cloned(),
            message,
        }
    }
}

/// Validate `project` for all targets × languages (or the filters in
/// `options`).
pub fn validate(project: &Project, options: &ValidateOptions<'_>) -> ValidationReport {
    let mut out = Sink::default();
    let layouter = std::cell::RefCell::new(TextLayouter::new(options.fonts.clone()));
    let root = Ctx::default();
    if let Some(filter) = &options.targets {
        for t in filter {
            if project.target(t).is_none() {
                out.push(root.finding(
                    Severity::Error,
                    FindingCode::UnknownTarget,
                    false,
                    None,
                    None,
                    format!("unknown target `{t}`"),
                ));
            }
        }
    }
    if let Some(filter) = &options.langs {
        for l in filter {
            if !project.languages.is_available(l) {
                out.push(root.finding(
                    Severity::Warn,
                    FindingCode::LangNotAvailable,
                    false,
                    None,
                    None,
                    format!("language `{l}` is not listed in `languages.available`"),
                ));
            }
        }
    }
    for place in &project.dropped_override_nulls {
        out.push(root.finding(
            Severity::Info,
            FindingCode::OverrideNull,
            false,
            None,
            None,
            format!(
                "override field set to null ({place}) was ignored: an override only sets \
                 values; remove the field to use the base value (dropped on the next save)"
            ),
        ));
    }
    for (family, f) in &project.styles.fonts {
        for fb in &f.fallback {
            if !options.fonts.has_family(fb) {
                out.push(root.finding(
                    Severity::Warn,
                    FindingCode::UnknownFont,
                    false,
                    None,
                    None,
                    format!(
                        "fallback font `{fb}` of `{family}` is not available (put it into `fonts/`)"
                    ),
                ));
            }
        }
    }
    for s in &project.strips {
        s.walk_elements(&mut |e| {
            for l in e.text_props().into_iter().flat_map(|t| t.langs.keys()) {
                if !project.languages.is_available(l.as_str()) {
                    out.push(root.finding(
                        Severity::Warn,
                        FindingCode::LangNotAvailable,
                        false,
                        Some(e.page),
                        Some(&e.id),
                        format!(
                            "language override `{l}` names a language not in `languages.available`"
                        ),
                    ));
                }
            }
        });
    }
    store::store_checks(project, &mut out);
    for target in &project.targets {
        if options
            .targets
            .as_ref()
            .is_some_and(|f| !f.iter().any(|t| t == target.id.as_str()))
        {
            continue;
        }
        validate_target(project, target, options, &layouter, &mut out);
    }
    ValidationReport {
        findings: out.findings,
    }
}

/// Languages to check for a target: its `langs` (default: all available,
/// at least the default language), restricted by the filter.
fn target_langs(project: &Project, target: &Target, options: &ValidateOptions<'_>) -> Vec<String> {
    let base: Vec<String> = match &target.langs {
        Some(l) => l.iter().map(|l| l.as_str().to_owned()).collect(),
        None if project.languages.available.is_empty() => {
            vec![project.languages.default.as_str().to_owned()]
        }
        None => project
            .languages
            .available
            .iter()
            .map(|l| l.as_str().to_owned())
            .collect(),
    };
    match &options.langs {
        None => base,
        Some(filter) => base
            .into_iter()
            .filter(|l| filter.iter().any(|f| f == l))
            .collect(),
    }
}

fn validate_target(
    project: &Project,
    target: &Target,
    options: &ValidateOptions<'_>,
    layouter: &std::cell::RefCell<TextLayouter>,
    out: &mut Sink,
) {
    let mut ctx = Ctx {
        target: Some(target.id.clone()),
        lang: None,
    };
    let Some(strip) = project.strip(target.strip.as_str()) else {
        out.push(ctx.finding(
            Severity::Error,
            FindingCode::UnknownStrip,
            false,
            None,
            None,
            format!(
                "target `{}` refers to unknown strip `{}`",
                target.id, target.strip
            ),
        ));
        return;
    };
    let scope = checks::Scope {
        project,
        target,
        strip,
        options,
    };
    checks::target_checks(&scope, &ctx, out);
    let resolve_options = ResolveOptions {
        catalog: options.catalog,
        assets: options.assets,
        text: Some(layouter),
        frames: options.frames,
    };
    let mut frame_checked = false;
    for lang in target_langs(project, target, options) {
        ctx.lang = Lang::parse(&lang).ok();
        let scene = match resolve(project, target.id.as_str(), &lang, &resolve_options) {
            Ok(s) => s,
            Err(ResolveError::InvalidLang(l)) => {
                out.push(ctx.finding(
                    Severity::Error,
                    FindingCode::InvalidLang,
                    false,
                    None,
                    None,
                    format!("invalid language code `{l}`"),
                ));
                continue;
            }
            // Target and strip exist (checked above).
            Err(e) => unreachable!("{e}"),
        };
        if !frame_checked {
            checks::frame_checks(&scope, &scene, &ctx, out);
            frame_checked = true;
        }
        checks::scene_checks(&scope, &scene, &mut layouter.borrow_mut(), &ctx, out);
    }
}
