//! Catalog data for pickers: devices, target presets, gallery profiles,
//! fonts. (The core catalog types are deserialize-only; these are the UI's
//! views of them.)

use std::fmt::Debug;

use serde::Serialize;
use vitrine_core::catalog;
use vitrine_core::layout::bundled::{self, FontCategory};
use vitrine_core::model::{
    BannerFit, Fold, Orientation, PixelSize, Region, SafeArea, StoreSlot, TargetKind,
};

use super::{AppState, NoArgs};
use crate::error::CmdResult;

/// `snake_case` name of a unit enum variant (as in the catalog YAML).
fn snake(v: impl Debug) -> String {
    let s = format!("{v:?}");
    let mut out = String::with_capacity(s.len() + 4);
    for (i, c) in s.chars().enumerate() {
        if c.is_ascii_uppercase() {
            if i > 0 {
                out.push('_');
            }
            out.push(c.to_ascii_lowercase());
        } else {
            out.push(c);
        }
    }
    out
}

/// A named frame colour of the catalog.
#[derive(Debug, Clone, Serialize)]
pub struct ColorSwatch {
    pub name: String,
    pub label: String,
    /// Housing colour.
    pub hex: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct DeviceInfo {
    pub id: String,
    pub label: String,
    pub class: String,
    pub natural: Orientation,
    pub orientations: Vec<Orientation>,
    pub colors: Vec<String>,
    /// Housing colour `#RRGGBB` and label of each colour variant in `colors`.
    pub swatches: Vec<ColorSwatch>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default_color: Option<String>,
    /// Display aspect `w:h` (natural orientation); `None` = free.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub display_aspect: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub screenshot_variant: Option<String>,
    pub screenshot_sizes: Vec<PixelSize>,
    pub browser: bool,
    /// Foldables: this model's fold state and the model of the other one.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fold: Option<FoldInfo>,
}

/// Fold link of a foldable model (`fold:` in `devices.yaml`).
#[derive(Debug, Clone, Serialize)]
pub struct FoldInfo {
    pub state: Fold,
    pub other: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct TargetPresetInfo {
    pub id: String,
    pub label: String,
    pub store: String,
    pub platform: String,
    pub kind: TargetKind,
    pub portrait: Vec<PixelSize>,
    pub landscape: Vec<PixelSize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default_size: Option<PixelSize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub slot: Option<StoreSlot>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub gallery: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub device: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub screenshots: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub requirement: Option<String>,
    /// New targets export the bare screenshots (watch, Wear OS).
    pub screenshot_only: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub safe_area: Option<SafeArea>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub region: Option<Region>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fit: Option<BannerFit>,
    pub status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct GalleryInfo {
    pub id: String,
    pub label: String,
    pub basis: String,
    pub gap: f64,
    pub radius_x: f64,
    pub radius_y: f64,
    pub background: String,
    /// Dark-mode page colour (estimate, design check only).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub background_dark: Option<String>,
    pub measured: String,
    pub source: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct FontsInfo {
    /// Bundled families (always available), Inter first, then grouped by
    /// category (sans serif, serif, display, handwriting, monospace).
    pub bundled: Vec<String>,
    /// Details of every bundled family, in the order of `bundled`.
    pub bundled_info: Vec<BundledFontInfo>,
    /// Families from the open project's `fonts/` folder.
    pub project: Vec<String>,
    /// System families loaded as last-resort fallback (app setting
    /// `system_font_fallback`). Only families needed for characters no
    /// bundled or project font covers, so it is often empty while
    /// `system_enabled` is true.
    pub system: Vec<String>,
    /// The app setting `system_font_fallback` as applied to the open
    /// project (the setting itself without a project).
    pub system_enabled: bool,
}

/// A bundled font family (`vitrine_core::layout::bundled`).
#[derive(Debug, Clone, Serialize)]
pub struct BundledFontInfo {
    pub family: String,
    pub category: FontCategory,
    /// Lightest and heaviest weight (variable fonts: any weight between).
    pub weights: [u16; 2],
    /// Has a real italic (otherwise italic is slanted synthetically).
    pub italic: bool,
    /// Licence file below `assets/fonts/` (SIL OFL 1.1).
    pub license: String,
}

/// The bundled families with their weights and styles.
pub fn bundled_fonts() -> Vec<BundledFontInfo> {
    bundled::FAMILIES
        .iter()
        .map(|fam| {
            let files: Vec<_> = bundled::files_of(fam.family).collect();
            BundledFontInfo {
                family: fam.family.to_owned(),
                category: fam.category,
                weights: [
                    files.iter().map(|f| f.weights.0).min().unwrap_or(400),
                    files.iter().map(|f| f.weights.1).max().unwrap_or(400),
                ],
                italic: files.iter().any(|f| f.italic),
                license: fam.license.to_owned(),
            }
        })
        .collect()
}

pub fn catalog_devices(_: &AppState, _: NoArgs) -> CmdResult<Vec<DeviceInfo>> {
    Ok(catalog::devices()
        .iter()
        .map(|d| DeviceInfo {
            id: d.id.clone(),
            label: d.label.clone(),
            class: snake(d.class),
            natural: d.natural,
            orientations: d.orientations.clone(),
            colors: d.colors.clone(),
            swatches: d
                .colors
                .iter()
                .filter_map(|name| {
                    let v = catalog::catalog().devices.colors.get(name)?;
                    Some(ColorSwatch {
                        name: name.clone(),
                        label: v.label.clone(),
                        hex: format!("#{:02X}{:02X}{:02X}", v.body.r, v.body.g, v.body.b),
                    })
                })
                .collect(),
            default_color: d.default_color.clone(),
            display_aspect: d.screenshot.aspect.map(|a| a.to_string()),
            screenshot_variant: d.screenshot.variant.clone(),
            screenshot_sizes: d.screenshot.sizes.clone(),
            browser: d.browser,
            fold: d.fold.as_ref().map(|f| FoldInfo {
                state: f.state,
                other: f.other.clone(),
            }),
        })
        .collect())
}

pub fn catalog_targets(_: &AppState, _: NoArgs) -> CmdResult<Vec<TargetPresetInfo>> {
    Ok(catalog::target_presets()
        .iter()
        .map(|t| TargetPresetInfo {
            id: t.id.clone(),
            label: t.label.clone(),
            store: snake(t.store),
            platform: snake(t.platform),
            kind: t.kind,
            portrait: t.sizes.portrait.clone(),
            landscape: t.sizes.landscape.clone(),
            default_size: t.default_size(),
            slot: t.slot.clone(),
            gallery: t.gallery.clone(),
            device: t.device.clone(),
            screenshots: t.screenshots.clone(),
            requirement: t.requirement.map(snake),
            screenshot_only: t.screenshot_only,
            safe_area: t.safe_area,
            region: t.region,
            fit: t.fit,
            status: snake(t.status),
            note: t.note.clone(),
        })
        .collect())
}

pub fn catalog_galleries(_: &AppState, _: NoArgs) -> CmdResult<Vec<GalleryInfo>> {
    Ok(catalog::galleries()
        .iter()
        .map(|g| GalleryInfo {
            id: g.id.clone(),
            label: g.label.clone(),
            basis: snake(g.basis),
            gap: g.gap,
            radius_x: g.radius.x,
            radius_y: g.radius.y,
            background: g.background.to_string(),
            background_dark: g.background_dark.map(|c| c.to_string()),
            measured: snake(g.measured),
            source: g.source.clone(),
            note: g.note.clone(),
        })
        .collect())
}

pub fn catalog_fonts(app: &AppState, _: NoArgs) -> CmdResult<FontsInfo> {
    let bundled_info = bundled_fonts();
    let bundled: Vec<String> = bundled_info.iter().map(|f| f.family.clone()).collect();
    let (project, system, system_enabled) = app.session().map_or_else(
        |_| {
            let on = app
                .settings_store()
                .load()
                .is_ok_and(|s| s.system_font_fallback);
            (Vec::new(), Vec::new(), on)
        },
        |s| {
            let ctx = s.render_context();
            let fonts = ctx.fonts();
            let system = fonts.system_fallbacks().to_vec();
            let project = fonts
                .families()
                .into_iter()
                .filter(|f| !bundled.iter().any(|b| b == f) && !system.iter().any(|x| x == f))
                .map(str::to_owned)
                .collect();
            (project, system, s.system_font_fallback())
        },
    );
    Ok(FontsInfo {
        bundled,
        bundled_info,
        project,
        system,
        system_enabled,
    })
}

#[cfg(test)]
mod tests {
    #[test]
    fn snake_names() {
        #[derive(Debug)]
        enum E {
            WebCss,
            No,
        }
        assert_eq!(super::snake(E::WebCss), "web_css");
        assert_eq!(super::snake(E::No), "no");
    }
}
