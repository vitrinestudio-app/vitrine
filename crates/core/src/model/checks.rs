//! Design-check settings stored in `project.yaml` (`checks:`): thresholds
//! of the layout and pixel checks of `core::validate` and finding codes
//! switched off for the whole project (ADR 0028).
//!
//! ```yaml
//! checks:
//!   device_distance: 16      # lu between glyphs and a device frame
//!   min_contrast: 4.5        # WCAG ratio for normal text
//!   min_contrast_large: 3    # … for large text
//!   off: [page_blends_with_store]
//! ```

use serde::{Deserialize, Serialize};

use super::fmt::flow_seq;

/// The `checks:` section of `project.yaml`. Missing values use the
/// built-in defaults (`validate::Thresholds`).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Checks {
    /// Minimum distance in lu between the glyphs of a text and a device
    /// frame (`text_near_device`; `0` = report only touching text).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub device_distance: Option<f64>,
    /// Minimum contrast ratio of normal text (`low_contrast`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min_contrast: Option<f64>,
    /// Minimum contrast ratio of large text (`low_contrast`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min_contrast_large: Option<f64>,
    /// Finding codes not reported for this project.
    #[serde(
        default,
        skip_serializing_if = "Vec::is_empty",
        serialize_with = "flow_seq"
    )]
    pub off: Vec<String>,
}

impl Checks {
    pub fn is_empty(&self) -> bool {
        *self == Self::default()
    }

    /// Checks the values (distance 0..=1000 lu, ratios 1..=21).
    pub fn check(&self) -> Result<(), String> {
        if let Some(d) = self.device_distance
            && !(d.is_finite() && (0.0..=1000.0).contains(&d))
        {
            return Err(format!("checks.device_distance {d} must be 0..1000 lu"));
        }
        for (name, v) in [
            ("min_contrast", self.min_contrast),
            ("min_contrast_large", self.min_contrast_large),
        ] {
            if let Some(v) = v
                && !(v.is_finite() && (1.0..=21.0).contains(&v))
            {
                return Err(format!("checks.{name} {v} must be 1..21"));
            }
        }
        Ok(())
    }
}
