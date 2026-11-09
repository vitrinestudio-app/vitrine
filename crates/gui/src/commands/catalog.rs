//! Catalog data for pickers: devices, target presets, gallery profiles,
//! fonts. (The core catalog types are deserialize-only; these are the UI's
//! views of them.)

use std::fmt::Debug;

use arrangia_core::catalog;
use arrangia_core::layout::FontLibrary;
use arrangia_core::model::{Orientation, PixelSize, StoreSlot, TargetKind};
use serde::Serialize;

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

#[derive(Debug, Clone, Serialize)]
pub struct DeviceInfo {
    pub id: String,
    pub label: String,
    pub class: String,
    pub natural: Orientation,
    pub orientations: Vec<Orientation>,
    pub colors: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default_color: Option<String>,
    /// Display aspect `w:h` (natural orientation); `None` = free.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub display_aspect: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub screenshot_variant: Option<String>,
    pub screenshot_sizes: Vec<PixelSize>,
    pub browser: bool,
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
    pub measured: String,
    pub source: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct FontsInfo {
    /// Bundled families (always available).
    pub bundled: Vec<String>,
    /// Families from the open project's `fonts/` folder.
    pub project: Vec<String>,
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
            default_color: d.default_color.clone(),
            display_aspect: d.screenshot.aspect.map(|a| a.to_string()),
            screenshot_variant: d.screenshot.variant.clone(),
            screenshot_sizes: d.screenshot.sizes.clone(),
            browser: d.browser,
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
            measured: snake(g.measured),
            source: g.source.clone(),
            note: g.note.clone(),
        })
        .collect())
}

pub fn catalog_fonts(app: &AppState, _: NoArgs) -> CmdResult<FontsInfo> {
    let bundled: Vec<String> = FontLibrary::bundled()
        .families()
        .into_iter()
        .map(str::to_owned)
        .collect();
    let project = app.session().map_or_else(
        |_| Vec::new(),
        |s| {
            s.render_context()
                .fonts()
                .families()
                .into_iter()
                .filter(|f| !bundled.iter().any(|b| b == f))
                .map(str::to_owned)
                .collect()
        },
    );
    Ok(FontsInfo { bundled, project })
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
