//! Output of `core::resolve`: a flat, concrete scene in canvas lu, free of
//! YAML concepts (no `@colour` refs, no style names, no variants, no
//! anchors, no home pages).

use std::collections::BTreeMap;

use serde::Serialize;

use crate::catalog::{ColorVariant, Device};
use crate::geometry::{CanvasLayout, Rect, Vec2, Viewport};
use crate::model::{
    Align, Appearance, CanvasId, Chrome, ElementId, FitMode, FrameStyle, ImageFillMode, Lang,
    Orientation, PixelSize, Rgba, ShapeKind, StringKey, StrokeJoin, StrokePosition, TargetId,
    TargetKind, TextTransform, VAlign, VariantId,
};

/// A target × language, resolved.
#[derive(Clone, Debug, Serialize)]
pub struct ResolvedScene {
    pub target: TargetId,
    pub canvas: CanvasId,
    /// Requested language.
    pub lang: Lang,
    /// Variant applied (`None` if the target names none or it is unknown).
    pub variant: Option<VariantId>,
    pub frame: Frame,
    /// Canvas-wide backgrounds, bottom to top, covering [`Frame::canvas`]
    /// (pages and gaps; stretched to the target page height).
    pub backgrounds: Vec<ResolvedFill>,
    /// Elements in painting order (bottom to top), groups flattened.
    pub elements: Vec<ResolvedElement>,
    /// All span styles of the project for inline markup `[name]…[/]`.
    pub spans: BTreeMap<String, ResolvedSpanStyle>,
    /// Infos and warnings collected while resolving.
    pub diagnostics: Vec<Diagnostic>,
}

impl ResolvedScene {
    pub fn element(&self, id: &str) -> Option<&ResolvedElement> {
        self.elements.iter().find(|e| e.id.as_str() == id)
    }
}

/// Canvas and output geometry of a target.
#[derive(Clone, Debug, Serialize)]
pub struct Frame {
    pub kind: TargetKind,
    /// Target size in px.
    pub size: PixelSize,
    /// Page placement (page height, gap, grouping); see `core::geometry`.
    pub layout: CanvasLayout,
    /// The whole canvas (pages and gaps), lu.
    pub canvas: Rect,
    pub pages: Vec<PageFrame>,
    /// Output images: one per page or page group, or the single banner.
    pub images: Vec<OutputImage>,
    /// Gaps between export images (content there is in no page export).
    pub gaps: Vec<Rect>,
    /// Gallery profile used for the gaps (preview corner radius etc.).
    pub gallery: Option<GalleryInfo>,
}

/// One page of the canvas on the canvas.
#[derive(Clone, Debug, Serialize)]
pub struct PageFrame {
    pub index: u32,
    /// Page id for export names (`hero`, `page-02`).
    pub id: String,
    /// Canvas rect of the page (= its column when grouped).
    pub rect: Rect,
    /// Output image holding the page (pages targets).
    pub image: Option<u32>,
    /// Column within that image.
    pub column: u32,
}

/// One output image.
#[derive(Clone, Debug, Serialize)]
pub struct OutputImage {
    pub index: u32,
    /// Pages shown (columns of a grouped image; pages touched by a banner).
    pub pages: Vec<u32>,
    /// Canvas rect covered by the image, lu (= `viewport.visible()`).
    pub source: Rect,
    /// Column rects (grouped page images); one entry per page.
    pub columns: Vec<Rect>,
    /// Canvas lu → pixel mapping of this image.
    pub viewport: Viewport,
}

/// Gallery profile of the target in canvas lu.
#[derive(Clone, Debug, Serialize)]
pub struct GalleryInfo {
    /// Preset id (`None` for a pure override without preset).
    pub preset: Option<String>,
    /// Gap in canvas lu.
    pub gap: f64,
    /// Corner radius of the images in canvas lu (`x` along the width).
    pub radius: Vec2,
    pub background: Option<Rgba>,
    /// The store's page colour in dark mode (gallery preview on `dark`,
    /// design check `page_blends_with_store`), if known.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub background_dark: Option<Rgba>,
    /// How the preset's values were obtained (`no` = not yet measured,
    /// `web_css`, `fixed`); `None` without preset.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub measured: Option<&'static str>,
}

/// A concrete element on the canvas.
#[derive(Clone, Debug, Serialize)]
pub struct ResolvedElement {
    pub id: ElementId,
    /// Enclosing groups, outermost first.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub groups: Vec<ElementId>,
    /// Home page (0-based).
    pub page: u32,
    /// Effective layer (own `z` plus the `z` of enclosing groups).
    pub z: i32,
    /// Centre on the canvas, lu. For text boxes: centre of [`TextFrame::rect`].
    pub center: Vec2,
    /// Width and height, lu (content scale applied).
    pub size: Vec2,
    /// Degrees, positive = clockwise (group rotation included).
    pub rotation: f64,
    /// Rotation pivot as offset from `center`, lu.
    pub pivot: Vec2,
    /// Product of the `scale` of the enclosing groups (ADR 0030), already
    /// applied to `center`, `size`, `pivot` and the kind-specific lengths
    /// (font sizes, strokes, radii, shadows); 1 outside scaled groups.
    #[serde(skip_serializing_if = "is_one")]
    pub scale: f64,
    /// `0..=1`, group opacity multiplied in.
    pub opacity: f64,
    /// Clip rectangle on the canvas (`clip: page` → home page / column).
    pub clip: Option<Rect>,
    /// Finding codes allowed for this element (own `allow` plus its
    /// groups'; validation skips them).
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub allow: Vec<String>,
    pub props: ResolvedKind,
}

/// Kind-specific resolved properties.
#[derive(Clone, Debug, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ResolvedKind {
    /// Page background filling its home page (column).
    Background {
        fill: ResolvedFill,
    },
    Device(Box<ResolvedDevice>),
    Text(Box<ResolvedText>),
    Image(ResolvedImage),
    Shape(ResolvedShape),
}

#[derive(Clone, Debug, Serialize)]
pub struct ResolvedDevice {
    /// Catalog model id after `device_map`.
    pub model: String,
    /// Model before `device_map`, if it was swapped.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mapped_from: Option<String>,
    /// The catalog entry (not serialized; look it up by `model`). For
    /// `free_aspect` models whose screenshot size is known, a copy whose
    /// display (and body) height follows the screenshot aspect
    /// ([`Device::with_display_aspect`]); draw this, not the catalog entry.
    #[serde(skip)]
    pub device: Device,
    pub orientation: Orientation,
    pub frame: FrameStyle,
    pub color: DeviceColor,
    /// The catalog palette entry for a named colour (not serialized).
    #[serde(skip)]
    pub palette: Option<ColorVariant>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub chrome: Option<Chrome>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub appearance: Option<Appearance>,
    /// The display adapts to the screenshot (`aspect: free` models). When
    /// the screenshot's pixel size is known, `device` and the element size
    /// follow its aspect; otherwise the catalog aspect is used.
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub free_aspect: bool,
    pub screenshot: Option<ResolvedScreenshot>,
    pub fit: FitMode,
    /// Content scale applied to size, pivot and shadow.
    pub content_scale: f64,
    pub shadow: Vec<ResolvedShadowLayer>,
    /// The installed vendor frame drawn instead of the parametric one
    /// (`frame: imported:<source>` and the frame is installed; ADR 0012).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub imported: Option<ImportedFrame>,
}

/// An installed vendor frame (bezel PNG + screen mask) chosen by resolve.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct ImportedFrame {
    /// Frame source (`apple`, or `pack:<id>` for a frame pack).
    pub source: String,
    /// Bezel PNG relative to the source folder
    /// ([`crate::frame_import::FrameIndex::source_dir`]).
    pub file: String,
    /// Screen mask PNG relative to the source folder.
    pub mask: String,
    /// Bezel pixel size; the element's aspect follows it.
    pub size: PixelSize,
    /// Screen rectangle in bezel pixels.
    pub screen: crate::frame_import::PxRect,
    /// Colour id of the installed frame.
    pub color: String,
    /// The vendor model drawn (`iPhone 18 Pro Max`).
    pub bezel: String,
    /// Why it was chosen.
    pub choice: crate::frame_import::BezelChoice,
    /// What the automatic choice would be (differs from `bezel` when the
    /// element pins another one).
    pub automatic: String,
    /// All installed vendor models for this model × colour × orientation.
    pub bezels: Vec<String>,
    /// Measured housing colour of the drawn bezel (`#RRGGBB`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub swatch: Option<String>,
    /// Installed colours of this model (the inspector's swatches).
    pub colors: Vec<crate::frame_import::InstalledColor>,
    /// Where the drawn bezel comes from: this computer's frame store
    /// (`app`) or the project's copy (`project`, ADR 0027).
    pub origin: crate::frame_import::FrameOrigin,
}

impl ImportedFrame {
    /// Frame-index key of the folder `file` / `mask` are relative to
    /// (`apple`, or `project:apple` for a project copy).
    pub fn files_key(&self) -> String {
        match self.origin {
            crate::frame_import::FrameOrigin::App => self.source.clone(),
            crate::frame_import::FrameOrigin::Project => {
                crate::frame_import::project_source_key(&self.source)
            }
        }
    }
}

/// Colour of a device frame.
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DeviceColor {
    /// A palette entry of `devices.yaml`.
    Named(String),
    /// A custom `#RRGGBB`.
    Custom(Rgba),
    /// The model has no colour variants.
    None,
}

/// A screenshot asset after the lookup chain (§5.2).
#[derive(Clone, Debug, Serialize)]
pub struct ResolvedScreenshot {
    pub key: String,
    /// Path relative to the project folder; `None` = missing → placeholder
    /// showing the key.
    pub path: Option<String>,
    /// Language folder the file was found in.
    pub lang: Option<Lang>,
    /// Screenshot variant folder that was asked for.
    pub variant: Option<String>,
    pub source: ScreenshotSource,
    /// Pixel size of the file, if the asset lookup knows it
    /// ([`super::AssetLookup::dimensions`]).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub size: Option<PixelSize>,
}

/// Which step of the lookup chain found the screenshot.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ScreenshotSource {
    /// `screenshots/<lang>/<variant>/<key>`
    LangVariant,
    /// `screenshots/<lang>/<key>`
    Lang,
    /// A fallback language (`de` for `de-AT`, explicit fallback, default)
    /// with the variant folder.
    FallbackVariant,
    /// A fallback language without the variant folder.
    Fallback,
    /// Not found.
    Missing,
}

#[derive(Clone, Debug, Serialize)]
pub struct ResolvedText {
    pub key: StringKey,
    /// Raw string with inline markup (parsed by `core::markup`); `None` if
    /// the key exists in no language of the chain.
    pub text: Option<String>,
    /// Language the string came from.
    pub text_lang: Option<Lang>,
    pub style: ResolvedTextStyle,
    pub frame: TextFrame,
    pub align: Align,
    pub valign: VAlign,
    /// Auto-fit lower bound (lu); `None` = no shrinking.
    pub shrink_to: Option<f64>,
    pub max_lines: Option<u32>,
    /// Automatic hyphenation when the text does not fit at `shrink_to`
    /// (style ← element ← language override; ADR 0009).
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub hyphenate: bool,
    /// Project font fallbacks (`styles.fonts`): family → fallback families.
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub font_fallbacks: BTreeMap<String, Vec<String>>,
    /// Default rotation pivot as offset from the element centre (the frame
    /// centre): for auto-height boxes the centre of the laid-out content
    /// (ADR 0004, *Texts*), else zero. [`ResolvedElement::pivot`] is this
    /// plus the element's own `pivot` offset.
    pub default_pivot: Vec2,
    /// Group scale (ADR 0030), already applied to `frame`, `style`,
    /// `shrink_to` and `default_pivot`. Span styles are project-wide, so
    /// their effect lengths are multiplied by it when laying out. 1 =
    /// unscaled.
    #[serde(skip_serializing_if = "is_one")]
    pub scale: f64,
}

#[allow(clippy::trivially_copy_pass_by_ref, reason = "serde signature")]
fn is_one(v: &f64) -> bool {
    (*v - 1.0).abs() < 1e-12
}

impl ResolvedText {
    /// Scale every length of the text by `k` (group scale, ADR 0030):
    /// frame size, font size, auto-fit bound, outline, glow, shadow and
    /// default pivot. The frame keeps its centre; the caller moves it.
    pub fn scale_by(&mut self, k: f64) {
        let c = self.frame.rect.center();
        self.frame.rect = Rect::centered(c, self.frame.rect.w * k, self.frame.rect.h * k);
        self.style.size *= k;
        self.style.stroke = self.style.stroke.map(|s| s.scaled(k));
        self.style.glow = self.style.glow.map(|g| g.scaled(k));
        for l in &mut self.style.shadow {
            *l = l.scaled(k);
        }
        self.shrink_to = self.shrink_to.map(|v| v * k);
        self.default_pivot = self.default_pivot * k;
        self.scale *= k;
    }
}

/// Text box on the canvas.
#[derive(Clone, Copy, Debug, Serialize)]
pub struct TextFrame {
    /// Box in canvas lu. With `auto_height` its height is the space
    /// available below the top (margin boxes without `bottom`) or 0 (free
    /// boxes without `height`, centred at `rect.center()`).
    pub rect: Rect,
    /// The text block's height follows the laid-out text.
    pub auto_height: bool,
    /// Box from page margins (`box`) rather than a free box.
    pub margins: bool,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct ResolvedTextStyle {
    pub font: String,
    pub weight: u16,
    pub italic: bool,
    /// Font size, lu.
    pub size: f64,
    /// Factor of the font size.
    pub line_height: f64,
    /// Letter spacing, em.
    pub tracking: f64,
    pub color: Rgba,
    pub transform: TextTransform,
    pub shadow: Vec<ResolvedShadowLayer>,
    /// Glyph outline (style ← element override).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stroke: Option<ResolvedTextStroke>,
    /// Glow around the glyphs (style ← element override).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub glow: Option<ResolvedGlow>,
}

impl ResolvedTextStyle {
    /// Outline, glow and shadow of the text as one value.
    pub fn effects(&self) -> TextEffects {
        TextEffects {
            stroke: self.stroke,
            glow: self.glow,
            shadow: self.shadow.clone(),
        }
    }
}

/// A text outline in lu. `width <= 0` draws nothing.
#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
pub struct ResolvedTextStroke {
    pub color: Rgba,
    pub width: f64,
    pub position: StrokePosition,
    pub join: StrokeJoin,
}

impl ResolvedTextStroke {
    /// The outline with its width multiplied by `k`.
    #[must_use]
    pub fn scaled(self, k: f64) -> Self {
        Self {
            width: self.width * k,
            ..self
        }
    }

    pub fn is_visible(&self) -> bool {
        self.width > 0.0 && self.color.a > 0
    }

    /// How far the outline reaches beyond the glyph edge, lu.
    pub fn outset(&self) -> f64 {
        if !self.is_visible() {
            return 0.0;
        }
        match self.position {
            StrokePosition::Outside => self.width,
            StrokePosition::Center => self.width / 2.0,
        }
    }
}

/// A text glow in lu: glyph shapes dilated by `spread`, blurred with sigma
/// `radius`, tinted with `color` at `opacity`. `opacity <= 0` draws nothing.
#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
pub struct ResolvedGlow {
    pub color: Rgba,
    pub radius: f64,
    pub opacity: f64,
    pub spread: f64,
}

impl ResolvedGlow {
    /// The glow with radius and spread multiplied by `k`.
    #[must_use]
    pub fn scaled(self, k: f64) -> Self {
        Self {
            radius: self.radius * k,
            spread: self.spread * k,
            ..self
        }
    }

    pub fn is_visible(&self) -> bool {
        self.opacity > 0.0 && self.color.a > 0
    }

    /// How far the glow visibly reaches beyond the glyph edge, lu.
    pub fn outset(&self) -> f64 {
        if self.is_visible() {
            self.spread + 3.0 * self.radius
        } else {
            0.0
        }
    }
}

impl ResolvedShadowLayer {
    /// The layer with blur and offset multiplied by `k`.
    #[must_use]
    pub fn scaled(self, k: f64) -> Self {
        Self {
            blur: self.blur * k,
            x: self.x * k,
            y: self.y * k,
            ..self
        }
    }

    /// How far the blurred, offset shadow reaches beyond the silhouette, lu.
    pub fn outset(&self) -> f64 {
        3.0 * self.blur + self.x.abs().max(self.y.abs())
    }
}

/// Effects drawn with a run of glyphs, behind/around the fill: shadow →
/// glow → outline → fill (ADR 0003).
#[derive(Clone, Debug, Default, PartialEq, Serialize)]
pub struct TextEffects {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stroke: Option<ResolvedTextStroke>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub glow: Option<ResolvedGlow>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub shadow: Vec<ResolvedShadowLayer>,
}

impl TextEffects {
    /// Does any effect draw something?
    pub fn is_empty(&self) -> bool {
        !self.stroke.is_some_and(|s| s.is_visible())
            && !self.glow.is_some_and(|g| g.is_visible())
            && self.shadow.is_empty()
    }

    /// Hard-edged extent beyond the glyphs (the outline), lu.
    pub fn ink_outset(&self) -> f64 {
        self.stroke.map_or(0.0, |s| s.outset())
    }

    /// Visible extent beyond the glyphs including glow and shadow, lu.
    pub fn visual_outset(&self) -> f64 {
        let glow = self.glow.map_or(0.0, |g| g.outset());
        let shadow = self
            .shadow
            .iter()
            .map(ResolvedShadowLayer::outset)
            .fold(0.0, f64::max);
        self.ink_outset().max(glow).max(shadow)
    }
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct ResolvedSpanStyle {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub font: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub weight: Option<u16>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub italic: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub color: Option<Rgba>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub background: Option<Rgba>,
    /// Replaces the text's outline (`width: 0` = none).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stroke: Option<ResolvedTextStroke>,
    /// Replaces the text's glow (`opacity: 0` = none).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub glow: Option<ResolvedGlow>,
    /// Replaces the text's shadow (empty = preset `none`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub shadow: Option<Vec<ResolvedShadowLayer>>,
}

#[derive(Clone, Debug, Serialize)]
pub struct ResolvedImage {
    pub asset: String,
    pub fit: FitMode,
    /// No `height` was given: the renderer takes it from the asset's
    /// aspect (`size.y` holds the width as a placeholder).
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub auto_height: bool,
    pub shadow: Vec<ResolvedShadowLayer>,
}

#[derive(Clone, Debug, Serialize)]
pub struct ResolvedShape {
    pub shape: ShapeKind,
    pub radius: f64,
    pub fill: Option<ResolvedFill>,
    pub stroke: Option<ResolvedStroke>,
    pub shadow: Vec<ResolvedShadowLayer>,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct ResolvedStroke {
    pub color: Rgba,
    pub width: f64,
    pub join: StrokeJoin,
}

/// A fill with concrete colours.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum ResolvedFill {
    Color {
        color: Rgba,
    },
    /// CSS angle convention (0 = to top, 90 = to right).
    LinearGradient {
        angle: f64,
        stops: Vec<ResolvedStop>,
    },
    /// `center` in fractions of the area, `radius` as fraction of its
    /// larger side.
    RadialGradient {
        center: Vec2,
        radius: f64,
        stops: Vec<ResolvedStop>,
    },
    Image {
        asset: String,
        fit: ImageFillMode,
        focus: Vec2,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
pub struct ResolvedStop {
    pub color: Rgba,
    pub pos: f64,
}

/// One shadow layer in lu.
#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
pub struct ResolvedShadowLayer {
    pub color: Rgba,
    pub blur: f64,
    pub x: f64,
    pub y: f64,
}

/// Severity of a diagnostic.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Level {
    Info,
    Warn,
}

/// What a diagnostic is about (stable codes for validation, GUI, MCP).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DiagnosticCode {
    /// The requested language is not in `languages.available`.
    LangNotAvailable,
    /// A text was taken from a fallback language.
    TextFallback,
    /// A text key exists in no language of the chain.
    TextMissing,
    /// A screenshot was taken from a fallback language / without variant.
    ScreenshotFallback,
    ScreenshotMissing,
    UnknownVariant,
    /// A variant override names an element that does not exist.
    UnknownOverrideElement,
    /// A variant override does not apply (e.g. a field of another kind).
    InvalidOverride,
    UnknownDevice,
    /// `device_map` names an unknown replacement model.
    UnknownMappedDevice,
    /// The colour variant does not exist for the (mapped) model.
    DeviceColorFallback,
    UnsupportedOrientation,
    UnknownColor,
    UnknownTextStyle,
    UnknownShadow,
    UnknownGallery,
    /// `gaps: as_store` on a banner without gallery.
    NoGallery,
    /// An element's home page is outside the canvas.
    PageOutOfRange,
    /// A required size is missing; a default is used.
    MissingSize,
    /// Page count is not a multiple of `group`.
    PartialImage,
    /// A banner region exceeds the canvas.
    RegionOutOfRange,
    /// `frame: imported:<source>` but no such frame is installed; the
    /// parametric frame is drawn.
    ImportedFrameMissing,
    /// An imported bezel was chosen as a fallback: the pinned `bezel` is
    /// not installed, or no installed size matches (largest used).
    ImportedBezelFallback,
    /// The device's colour is not installed for its imported frame; the
    /// default (or first) installed colour is drawn.
    ImportedColorFallback,
    /// A page of a `screenshot_only` target has no device: the image
    /// stays empty.
    ScreenshotOnlyEmpty,
    /// A canvas background layer lies under an opaque layer and is never
    /// visible (canvas backgrounds are layers, not one fill per page).
    BackgroundHidden,
}

/// An info or warning collected during resolve.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Diagnostic {
    pub level: Level,
    pub code: DiagnosticCode,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub element: Option<ElementId>,
    pub message: String,
}
