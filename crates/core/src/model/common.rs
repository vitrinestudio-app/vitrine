//! Small value types shared by elements, strips and targets.
//!
//! Units are lu (a page is [`crate::PAGE_WIDTH_LU`] lu wide), y points down,
//! rotations are degrees with positive = clockwise (ADR 0001).

use std::fmt;

use serde::{Deserialize, Serialize};

use super::fmt::{flow_style, num, string_serde};

/// A point in lu, relative to the home page's top-left corner (or, for
/// `pivot`, relative to the element centre).
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(remote = "Self", deny_unknown_fields)]
pub struct Point {
    #[serde(serialize_with = "num")]
    pub x: f64,
    #[serde(serialize_with = "num")]
    pub y: f64,
}
flow_style!(Point);

impl Point {
    pub const ZERO: Self = Self { x: 0.0, y: 0.0 };

    pub const fn new(x: f64, y: f64) -> Self {
        Self { x, y }
    }

    pub fn is_zero(&self) -> bool {
        self.x == 0.0 && self.y == 0.0
    }
}

/// Page aspect ratio `width:height`, written as `"1320:2868"`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Aspect {
    pub w: u32,
    pub h: u32,
}

impl Aspect {
    pub const fn new(w: u32, h: u32) -> Self {
        Self { w, h }
    }

    pub fn parse(s: &str) -> Result<Self, String> {
        let err = || format!("invalid aspect `{s}`: expected `width:height`, e.g. `1320:2868`");
        let (w, h) = s.split_once(':').ok_or_else(err)?;
        let w: u32 = w.trim().parse().map_err(|_| err())?;
        let h: u32 = h.trim().parse().map_err(|_| err())?;
        if w == 0 || h == 0 {
            return Err(err());
        }
        Ok(Self { w, h })
    }

    /// Page height in lu for this aspect (page width is 1000 lu).
    pub fn page_height(&self) -> f64 {
        crate::page_height_lu(f64::from(self.w), f64::from(self.h))
    }
}

impl fmt::Display for Aspect {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}", self.w, self.h)
    }
}

string_serde!(Aspect, "an aspect ratio `width:height`");

/// Pixel size `[width, height]` of an export target.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct PixelSize {
    pub w: u32,
    pub h: u32,
}

impl PixelSize {
    pub const fn new(w: u32, h: u32) -> Self {
        Self { w, h }
    }
}

impl Serialize for PixelSize {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        super::fmt::flow_seq(&[self.w, self.h], s)
    }
}

impl<'de> Deserialize<'de> for PixelSize {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let [w, h] = <[u32; 2]>::deserialize(d)?;
        Ok(Self { w, h })
    }
}

/// Vertical anchor: how an element follows a change of page height (§3.4).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AnchorY {
    /// Distance to the top edge is kept (headlines).
    Top,
    /// Distance to the bottom edge is kept (devices running out of the image).
    Bottom,
    /// Offset from the vertical centre is kept.
    Center,
    /// Position and height scale with the page height (backgrounds).
    Stretch,
}

/// Clipping of an element.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Clip {
    /// Free on the canvas, may run into neighbouring pages.
    None,
    /// Clipped to the home page (default for text).
    Page,
}

/// How an image fills a rectangle.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FitMode {
    Cover,
    Contain,
    Stretch,
}

/// Restrict an element to some targets and/or languages. An empty list
/// means "no restriction" for that dimension.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(remote = "Self", deny_unknown_fields)]
pub struct VisibleIn {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub targets: Vec<super::TargetId>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub langs: Vec<super::Lang>,
}
flow_style!(VisibleIn);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn aspect() {
        let a = Aspect::parse("1320:2868").unwrap();
        assert_eq!(a, Aspect::new(1320, 2868));
        assert_eq!(a.to_string(), "1320:2868");
        assert!((a.page_height() - 2172.727).abs() < 0.001);
        assert!(Aspect::parse("1320x2868").is_err());
        assert!(Aspect::parse("0:1").is_err());
    }
}
