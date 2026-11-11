//! Colour management of imported images (ADR 0021): everything Vitrine Studio
//! draws is sRGB, so screenshots, image assets and imported bezels are
//! converted to sRGB once, right after decoding.
//!
//! - [`sniff`] reads the colour information of a PNG (`cICP`, `iCCP`,
//!   `sRGB`; `gAMA`/`cHRM` are ignored) or JPEG (APP2 ICC, CMYK) header
//!   without decoding pixels.
//! - [`ToSrgb::new`] turns that into a conversion (pure-Rust CMS
//!   `moxcms`): untagged, `sRGB` chunk and sRGB-equivalent profiles are
//!   the identity; Display P3 and other RGB/gray profiles are converted
//!   (relative colorimetric, out-of-gamut colours clip); anything that
//!   cannot be used falls back to "treat as sRGB" with a [`ToSrgb::note`].
//! - [`inspect_file`] is the validation side (cached per file): the note
//!   of a file whose colour information cannot be used.
//!
//! Colours in the model (`#RRGGBB`) are sRGB; exports are sRGB.

use std::collections::HashMap;
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock, PoisonError};
use std::time::SystemTime;

use moxcms::{
    CicpColorPrimaries, CicpProfile, ColorProfile, DataColorSpace, Layout, MatrixCoefficients,
    ProfileText, TransferCharacteristics, TransformOptions,
};

/// Largest embedded ICC profile accepted (bytes, decompressed).
pub const MAX_ICC_BYTES: usize = 4 << 20;

/// Container of a sniffed image.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ImageKind {
    Png,
    Jpeg,
    /// Not PNG/JPEG (or unreadable): treated as sRGB.
    #[default]
    Other,
}

/// PNG `cICP` code points (ITU-T H.273).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Cicp {
    pub primaries: u8,
    pub transfer: u8,
    pub matrix: u8,
    pub full_range: bool,
}

/// Colour information found in an image header.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ColorTag {
    pub kind: ImageKind,
    /// Embedded ICC profile (PNG `iCCP` decompressed, JPEG APP2 joined).
    pub icc: Option<Vec<u8>>,
    /// PNG `cICP` chunk (takes precedence over `iCCP`, PNG 3rd edition).
    pub cicp: Option<Cicp>,
    /// PNG `sRGB` chunk.
    pub srgb: bool,
    /// JPEG with four components (CMYK / YCCK).
    pub cmyk: bool,
    /// A colour chunk/segment was present but broken (e.g. bad zlib data,
    /// missing ICC segment).
    pub error: Option<String>,
}

impl ColorTag {
    /// No colour information at all (treated as sRGB).
    pub fn is_untagged(&self) -> bool {
        self.icc.is_none()
            && self.cicp.is_none()
            && !self.srgb
            && !self.cmyk
            && self.error.is_none()
    }
}

/// Reads the colour information of a PNG or JPEG from its header only
/// (stops at the first image data). Other formats give an empty tag.
pub fn sniff<R: Read + Seek>(mut r: R) -> std::io::Result<ColorTag> {
    let mut magic = [0u8; 8];
    let n = read_up_to(&mut r, &mut magic)?;
    if n == 8 && magic == *b"\x89PNG\r\n\x1a\n" {
        sniff_png(&mut r)
    } else if n >= 3 && magic[..3] == [0xFF, 0xD8, 0xFF] {
        r.seek(SeekFrom::Start(2))?;
        sniff_jpeg(&mut r)
    } else {
        Ok(ColorTag::default())
    }
}

/// [`sniff`] on a byte slice (truncated data gives what was found).
pub fn sniff_bytes(bytes: &[u8]) -> ColorTag {
    sniff(std::io::Cursor::new(bytes)).unwrap_or_default()
}

fn read_up_to<R: Read>(r: &mut R, buf: &mut [u8]) -> std::io::Result<usize> {
    let mut n = 0;
    while n < buf.len() {
        match r.read(&mut buf[n..])? {
            0 => break,
            k => n += k,
        }
    }
    Ok(n)
}

fn skip<R: Seek>(r: &mut R, n: usize) -> std::io::Result<()> {
    r.seek(SeekFrom::Current(i64::try_from(n).unwrap_or(i64::MAX)))
        .map(|_| ())
}

fn sniff_png<R: Read + Seek>(r: &mut R) -> std::io::Result<ColorTag> {
    let mut tag = ColorTag {
        kind: ImageKind::Png,
        ..ColorTag::default()
    };
    let mut head = [0u8; 8];
    // Header chunks are few; the bound guards against garbage files.
    for _ in 0..10_000 {
        if read_up_to(r, &mut head)? < 8 {
            break;
        }
        let len = u32::from_be_bytes([head[0], head[1], head[2], head[3]]) as usize;
        match &head[4..8] {
            b"IDAT" | b"IEND" => break,
            b"sRGB" => {
                tag.srgb = true;
                skip(r, len + 4)?;
            }
            b"cICP" if len == 4 => {
                let mut b = [0u8; 4];
                r.read_exact(&mut b)?;
                tag.cicp = Some(Cicp {
                    primaries: b[0],
                    transfer: b[1],
                    matrix: b[2],
                    full_range: b[3] != 0,
                });
                skip(r, 4)?;
            }
            b"iCCP" => {
                if len > MAX_ICC_BYTES {
                    tag.error = Some("embedded ICC profile is too large".into());
                    skip(r, len + 4)?;
                    continue;
                }
                let mut body = vec![0; len];
                r.read_exact(&mut body)?;
                skip(r, 4)?;
                match inflate_iccp(&body) {
                    Ok(icc) => tag.icc = Some(icc),
                    Err(e) => tag.error = Some(e),
                }
            }
            _ => skip(r, len + 4)?,
        }
    }
    Ok(tag)
}

/// `iCCP` body: profile name (1–79 bytes), NUL, method 0, zlib data.
fn inflate_iccp(body: &[u8]) -> Result<Vec<u8>, String> {
    let nul = body
        .iter()
        .take(80)
        .position(|&b| b == 0)
        .ok_or("iCCP chunk without profile name")?;
    if body.get(nul + 1) != Some(&0) {
        return Err("iCCP chunk with unknown compression method".into());
    }
    let mut out = Vec::new();
    flate2::read::ZlibDecoder::new(&body[nul + 2..])
        .take(MAX_ICC_BYTES as u64 + 1)
        .read_to_end(&mut out)
        .map_err(|e| format!("iCCP chunk cannot be decompressed: {e}"))?;
    if out.len() > MAX_ICC_BYTES {
        return Err("embedded ICC profile is too large".into());
    }
    Ok(out)
}

/// One APP2 `ICC_PROFILE` segment: (sequence number, count, data).
type IccPart = (u8, u8, Vec<u8>);

fn sniff_jpeg<R: Read + Seek>(r: &mut R) -> std::io::Result<ColorTag> {
    let mut tag = ColorTag {
        kind: ImageKind::Jpeg,
        ..ColorTag::default()
    };
    let mut parts: Vec<IccPart> = Vec::new();
    let mut total = 0usize;
    let mut b = [0u8; 1];
    for _ in 0..100_000 {
        // Next marker: 0xFF, optional fill bytes, code.
        if read_up_to(r, &mut b)? == 0 {
            break;
        }
        if b[0] != 0xFF {
            continue;
        }
        let mut code = 0xFF;
        while code == 0xFF {
            if read_up_to(r, &mut b)? == 0 {
                return Ok(finish_jpeg(tag, parts));
            }
            code = b[0];
        }
        match code {
            0x00 | 0x01 | 0xD0..=0xD8 => continue,
            0xD9 | 0xDA => break,
            _ => {}
        }
        let mut lb = [0u8; 2];
        if read_up_to(r, &mut lb)? < 2 {
            break;
        }
        let len = usize::from(u16::from_be_bytes(lb)).saturating_sub(2);
        let is_sof = matches!(code, 0xC0..=0xCF) && !matches!(code, 0xC4 | 0xC8 | 0xCC);
        if code == 0xE2 && len >= 14 {
            let mut body = vec![0; len];
            r.read_exact(&mut body)?;
            if body.starts_with(b"ICC_PROFILE\0") {
                total += len - 14;
                if total > MAX_ICC_BYTES {
                    tag.error = Some("embedded ICC profile is too large".into());
                    parts.clear();
                } else if tag.error.is_none() {
                    parts.push((body[12], body[13], body[14..].to_vec()));
                }
            }
        } else if is_sof && len >= 6 {
            let mut body = vec![0; len];
            r.read_exact(&mut body)?;
            tag.cmyk = body[5] == 4;
        } else {
            skip(r, len)?;
        }
    }
    Ok(finish_jpeg(tag, parts))
}

fn finish_jpeg(mut tag: ColorTag, mut parts: Vec<IccPart>) -> ColorTag {
    if parts.is_empty() || tag.error.is_some() {
        return tag;
    }
    parts.sort_by_key(|p| p.0);
    let count = parts[0].1;
    let complete = usize::from(count) == parts.len()
        && parts
            .iter()
            .enumerate()
            .all(|(i, p)| usize::from(p.0) == i + 1 && p.1 == count);
    if complete {
        tag.icc = Some(parts.into_iter().flat_map(|p| p.2).collect());
    } else {
        tag.error = Some("embedded ICC profile is incomplete (APP2 segments missing)".into());
    }
    tag
}

/// The conversion of one image to sRGB.
pub struct ToSrgb {
    /// `None` = the pixels already are sRGB.
    profile: Option<ColorProfile>,
    label: String,
    note: Option<String>,
}

impl std::fmt::Debug for ToSrgb {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ToSrgb")
            .field("label", &self.label)
            .field("identity", &self.profile.is_none())
            .field("note", &self.note)
            .finish()
    }
}

impl ToSrgb {
    /// Picks the conversion for `tag`: `cICP` first, then the ICC profile,
    /// else sRGB. Unusable information falls back to sRGB with a note.
    pub fn new(tag: &ColorTag) -> Self {
        let mut notes = Vec::new();
        if let Some(e) = &tag.error {
            notes.push(e.clone());
        }
        if tag.cmyk {
            notes.push("CMYK JPEG: colours are approximated (no CMYK colour management)".into());
            return Self::fallback(&notes);
        }
        let mut chosen = None;
        if let Some(c) = tag.cicp {
            match cicp_profile(c) {
                Ok(p) => chosen = Some(p),
                Err(e) => notes.push(e),
            }
        }
        if chosen.is_none()
            && let Some(icc) = &tag.icc
        {
            match icc_profile(icc) {
                Ok(p) => chosen = Some(p),
                Err(e) => notes.push(e),
            }
        }
        let Some((profile, label)) = chosen else {
            if notes.is_empty() {
                return Self {
                    profile: None,
                    label: if tag.srgb { "sRGB" } else { "sRGB (untagged)" }.to_owned(),
                    note: None,
                };
            }
            return Self::fallback(&notes);
        };
        let note = (!notes.is_empty()).then(|| notes.join("; "));
        let profile = (!is_srgb_equivalent(&profile)).then_some(profile);
        Self {
            profile,
            label,
            note,
        }
    }

    fn fallback(notes: &[String]) -> Self {
        Self {
            profile: None,
            label: "sRGB (assumed)".into(),
            note: Some(format!("{}; treated as sRGB", notes.join("; "))),
        }
    }

    /// No conversion needed.
    pub fn is_identity(&self) -> bool {
        self.profile.is_none()
    }

    /// The source colour space, e.g. "Display P3" or "sRGB (untagged)".
    pub fn label(&self) -> &str {
        &self.label
    }

    /// Why colour information was ignored (unreadable or unsupported
    /// profile, CMYK); `None` when everything was understood.
    pub fn note(&self) -> Option<&str> {
        self.note.as_deref()
    }

    /// Is the source profile a gray one (needs gray pixel data)?
    pub fn is_gray(&self) -> bool {
        self.profile
            .as_ref()
            .is_some_and(|p| p.color_space == DataColorSpace::Gray)
    }

    /// Converts straight-alpha RGBA8 in place (alpha is kept).
    pub fn rgba8(&self, data: &mut [u8]) -> Result<(), String> {
        let Some(p) = &self.profile else {
            return Ok(());
        };
        let src = data.to_vec();
        p.create_transform_8bit(Layout::Rgba, &srgb(), Layout::Rgba, options())
            .and_then(|t| t.transform(&src, data))
            .map_err(|e| cms_err(&e))
    }

    /// Converts straight-alpha RGBA16 to RGBA8 (alpha is kept).
    pub fn rgba16_to_rgba8(&self, data: &[u16]) -> Result<Vec<u8>, String> {
        let mut out = vec![0u16; data.len()];
        match &self.profile {
            None => out.copy_from_slice(data),
            Some(p) => p
                .create_transform_16bit(Layout::Rgba, &srgb(), Layout::Rgba, options())
                .and_then(|t| t.transform(data, &mut out))
                .map_err(|e| cms_err(&e))?,
        }
        Ok(out.into_iter().map(u16_to_u8).collect())
    }

    /// Converts gray + alpha (8 bit) with a gray profile to RGBA8.
    pub fn gray_alpha8_to_rgba8(&self, data: &[u8]) -> Result<Vec<u8>, String> {
        let mut out = vec![0u8; data.len() * 2];
        match &self.profile {
            None => {
                for (o, g) in out
                    .as_chunks_mut::<4>()
                    .0
                    .iter_mut()
                    .zip(data.as_chunks::<2>().0)
                {
                    *o = [g[0], g[0], g[0], g[1]];
                }
            }
            Some(p) => p
                .create_transform_8bit(Layout::GrayAlpha, &srgb(), Layout::Rgba, options())
                .and_then(|t| t.transform(data, &mut out))
                .map_err(|e| cms_err(&e))?,
        }
        Ok(out)
    }

    /// Converts gray + alpha (16 bit) with a gray profile to RGBA8.
    pub fn gray_alpha16_to_rgba8(&self, data: &[u16]) -> Result<Vec<u8>, String> {
        let mut out = vec![0u16; data.len() * 2];
        match &self.profile {
            None => {
                for (o, g) in out
                    .as_chunks_mut::<4>()
                    .0
                    .iter_mut()
                    .zip(data.as_chunks::<2>().0)
                {
                    *o = [g[0], g[0], g[0], g[1]];
                }
            }
            Some(p) => p
                .create_transform_16bit(Layout::GrayAlpha, &srgb(), Layout::Rgba, options())
                .and_then(|t| t.transform(data, &mut out))
                .map_err(|e| cms_err(&e))?,
        }
        Ok(out.into_iter().map(u16_to_u8).collect())
    }
}

fn u16_to_u8(v: u16) -> u8 {
    ((u32::from(v) * 255 + 32_767) / 65_535) as u8
}

fn cms_err(e: &moxcms::CmsError) -> String {
    format!("colour conversion failed: {e}")
}

fn srgb() -> ColorProfile {
    ColorProfile::new_srgb()
}

fn options() -> TransformOptions {
    TransformOptions {
        rendering_intent: moxcms::RenderingIntent::RelativeColorimetric,
        ..TransformOptions::default()
    }
}

/// A profile from `cICP` code points. Only RGB, full range and SDR
/// transfer curves are used; HDR (PQ/HLG) is not tone-mapped.
fn cicp_profile(c: Cicp) -> Result<(ColorProfile, String), String> {
    if c.matrix != 0 || !c.full_range {
        return Err(format!(
            "cICP with matrix {} / {} range is not supported",
            c.matrix,
            if c.full_range { "full" } else { "narrow" }
        ));
    }
    if matches!(c.transfer, 16 | 18) {
        return Err(format!(
            "cICP transfer {} (HDR) is not supported",
            c.transfer
        ));
    }
    let label = match (c.primaries, c.transfer) {
        (1, 13) => "sRGB".to_owned(),
        (12, 13) => "Display P3".to_owned(),
        (p, t) => format!("cICP {p}/{t}"),
    };
    let unsupported = || format!("cICP {}/{} is not supported", c.primaries, c.transfer);
    // RGB primaries (BT.709, BT.2020, DCI-P3, Display P3) and SDR curves
    // (BT.709 family, gamma 2.2/2.8, linear, sRGB).
    if !matches!(c.primaries, 1 | 9 | 11 | 12) || !matches!(c.transfer, 1 | 4..=8 | 13..=15) {
        return Err(unsupported());
    }
    let primaries = CicpColorPrimaries::try_from(c.primaries).map_err(|_| unsupported())?;
    let transfer = TransferCharacteristics::try_from(c.transfer).map_err(|_| unsupported())?;
    let profile = ColorProfile::new_from_cicp(CicpProfile {
        color_primaries: primaries,
        transfer_characteristics: transfer,
        matrix_coefficients: MatrixCoefficients::Identity,
        full_range: true,
    });
    usable(&profile, Layout::Rgba).map_err(|e| format!("cICP {label}: {e}"))?;
    Ok((profile, label))
}

fn icc_profile(icc: &[u8]) -> Result<(ColorProfile, String), String> {
    let profile = ColorProfile::new_from_slice(icc)
        .map_err(|e| format!("embedded ICC profile cannot be read ({e})"))?;
    let label = profile
        .description
        .as_ref()
        .and_then(profile_text)
        .unwrap_or_else(|| "ICC profile".to_owned());
    let layout = match profile.color_space {
        DataColorSpace::Rgb => Layout::Rgba,
        DataColorSpace::Gray => Layout::GrayAlpha,
        other => {
            return Err(format!(
                "embedded ICC profile \"{label}\" has colour space {other:?} (only RGB and gray are supported)"
            ));
        }
    };
    usable(&profile, layout).map_err(|e| format!("embedded ICC profile \"{label}\": {e}"))?;
    Ok((profile, label))
}

/// Can a transform to sRGB be built?
fn usable(profile: &ColorProfile, layout: Layout) -> Result<(), String> {
    profile
        .create_transform_8bit(layout, &srgb(), Layout::Rgba, options())
        .map(|_| ())
        .map_err(|e| format!("no conversion to sRGB ({e})"))
}

fn profile_text(t: &ProfileText) -> Option<String> {
    let s = match t {
        ProfileText::PlainString(s) => s.clone(),
        ProfileText::Localizable(v) => v
            .iter()
            .find(|l| l.language == "en")
            .or_else(|| v.first())?
            .value
            .clone(),
        ProfileText::Description(d) => d.ascii_string.clone(),
    };
    let s = s.trim_matches(|c: char| c == '\0' || c.is_whitespace());
    (!s.is_empty()).then(|| s.to_owned())
}

/// Does the profile map every probe colour to itself within ±1 (sRGB
/// profiles of all flavours)? Then the pixels are left untouched, so
/// sRGB-tagged images render bit-identical to untagged ones.
fn is_srgb_equivalent(profile: &ColorProfile) -> bool {
    if profile.color_space != DataColorSpace::Rgb {
        return false;
    }
    let steps = [0u8, 17, 51, 102, 128, 153, 204, 238, 255];
    let mut probe = Vec::with_capacity(steps.len().pow(3) * 4);
    for &r in &steps {
        for &g in &steps {
            for &b in &steps {
                probe.extend_from_slice(&[r, g, b, 255]);
            }
        }
    }
    let mut out = vec![0; probe.len()];
    let Ok(t) = profile.create_transform_8bit(Layout::Rgba, &srgb(), Layout::Rgba, options())
    else {
        return false;
    };
    t.transform(&probe, &mut out).is_ok()
        && probe.iter().zip(&out).all(|(a, b)| a.abs_diff(*b) <= 1)
}

type InspectKey = (PathBuf, Option<SystemTime>, u64);

/// Validation side: the note of an image file whose colour information
/// cannot be used (it renders as if it were sRGB); `None` for untagged,
/// sRGB and convertible files and for files that cannot be read. Cached by
/// path, modification time and length.
pub fn inspect_file(path: &Path) -> Option<String> {
    static CACHE: OnceLock<Mutex<HashMap<InspectKey, Option<String>>>> = OnceLock::new();
    let meta = std::fs::metadata(path).ok()?;
    let key = (path.to_path_buf(), meta.modified().ok(), meta.len());
    let cache = CACHE.get_or_init(|| Mutex::new(HashMap::new()));
    if let Some(hit) = cache
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .get(&key)
    {
        return hit.clone();
    }
    let file = std::fs::File::open(path).ok()?;
    let tag = sniff(std::io::BufReader::new(file)).ok()?;
    let note = if tag.is_untagged() {
        None
    } else {
        ToSrgb::new(&tag).note().map(str::to_owned)
    };
    let mut map = cache.lock().unwrap_or_else(PoisonError::into_inner);
    if map.len() > 4096 {
        map.clear();
    }
    map.insert(key, note.clone());
    note
}

#[cfg(test)]
mod tests;
