//! Image IO: screenshot decoding (converted to sRGB, ADR 0021) and
//! Lanczos3 resampling, PNG export (24-bit RGB, no alpha, sRGB chunk) and
//! JPEG export (untagged = sRGB).

use std::io::Write;
use std::path::Path as FsPath;

use fast_image_resize::images::Image;
use fast_image_resize::{FilterType, PixelType, ResizeAlg, ResizeOptions, Resizer};
use image::DynamicImage;
use tiny_skia::{IntSize, Pixmap};
use vitrine_core::colorspace::{ToSrgb, sniff_bytes};

/// A decoded raster image: straight RGBA8 in sRGB.
#[derive(Debug, Clone)]
pub struct SourceImage {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
}

impl SourceImage {
    /// Reads and decodes a file (see [`SourceImage::decode`]).
    pub fn load(path: &FsPath) -> anyhow::Result<Self> {
        Self::decode(&std::fs::read(path)?)
    }

    /// Decodes PNG/JPEG bytes to straight RGBA8 and converts embedded
    /// colour profiles (Display P3 …) to sRGB once
    /// (`vitrine_core::colorspace`). Untagged and sRGB images are not
    /// touched; unusable profiles are ignored (validation reports them).
    pub fn decode(bytes: &[u8]) -> anyhow::Result<Self> {
        let img = image::ImageReader::new(std::io::Cursor::new(bytes))
            .with_guessed_format()?
            .decode()?;
        let to_srgb = ToSrgb::new(&sniff_bytes(bytes));
        Ok(Self {
            width: img.width(),
            height: img.height(),
            rgba: rgba_srgb(&img, &to_srgb),
        })
    }

    pub fn from_pixmap(pm: &Pixmap) -> Self {
        Self {
            width: pm.width(),
            height: pm.height(),
            rgba: pm.data().to_vec(),
        }
    }

    /// Resamples to exactly `w × h` with Lanczos3, cropping to the target
    /// aspect first (`fit: cover`, centred), and returns a pixmap ready for
    /// tiny-skia. Alpha is premultiplied by the resizer.
    pub fn resized_cover(&self, w: u32, h: u32) -> anyhow::Result<Pixmap> {
        self.resample(w, h, Some((0.5, 0.5)), Filter::Lanczos3)
    }

    /// Resamples to exactly `w × h` (premultiplied pixmap). With `crop`
    /// (focus fractions) the source is first cropped to the target aspect
    /// (`cover`); without, it is stretched.
    pub fn resample(
        &self,
        w: u32,
        h: u32,
        crop: Option<(f64, f64)>,
        filter: Filter,
    ) -> anyhow::Result<Pixmap> {
        let (w, h) = (w.max(1), h.max(1));
        let src = Image::from_vec_u8(self.width, self.height, self.rgba.clone(), PixelType::U8x4)?;
        let mut dst = Image::new(w, h, PixelType::U8x4);
        let opts = ResizeOptions::new()
            .resize_alg(ResizeAlg::Convolution(match filter {
                Filter::Bilinear => FilterType::Bilinear,
                Filter::Lanczos3 => FilterType::Lanczos3,
            }))
            .fit_into_destination(crop);
        Resizer::new().resize(&src, &mut dst, &opts)?;
        let mut data = dst.into_vec();
        premultiply(&mut data);
        Pixmap::from_vec(
            data,
            IntSize::from_wh(w, h).ok_or_else(|| anyhow::anyhow!("bad size"))?,
        )
        .ok_or_else(|| anyhow::anyhow!("pixmap"))
    }
}

/// The pixels of `img` as straight RGBA8 in sRGB. 16-bit images are
/// converted at 16 bit and rounded afterwards; a profile that does not fit
/// the pixel data (gray profile on colour pixels) leaves them unchanged.
fn rgba_srgb(img: &DynamicImage, t: &ToSrgb) -> Vec<u8> {
    use DynamicImage as D;
    if t.is_identity() {
        return img.to_rgba8().into_raw();
    }
    let gray = matches!(
        img,
        D::ImageLuma8(_) | D::ImageLumaA8(_) | D::ImageLuma16(_) | D::ImageLumaA16(_)
    );
    let deep = matches!(
        img,
        D::ImageLuma16(_) | D::ImageLumaA16(_) | D::ImageRgb16(_) | D::ImageRgba16(_)
    );
    let converted = match (t.is_gray(), gray, deep) {
        (true, true, false) => t.gray_alpha8_to_rgba8(img.to_luma_alpha8().as_raw()),
        (true, true, true) => t.gray_alpha16_to_rgba8(img.to_luma_alpha16().as_raw()),
        (true, false, _) => Err("gray profile on a colour image".to_owned()),
        (false, _, true) => t.rgba16_to_rgba8(img.to_rgba16().as_raw()),
        (false, _, false) => {
            let mut data = img.to_rgba8().into_raw();
            t.rgba8(&mut data).map(|()| data)
        }
    };
    converted.unwrap_or_else(|_| img.to_rgba8().into_raw())
}

/// Resampling filter: `Bilinear` (a proper convolution, cheap; previews)
/// or `Lanczos3` (export).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Filter {
    Bilinear,
    Lanczos3,
}

fn premultiply(data: &mut [u8]) {
    for px in data.as_chunks_mut::<4>().0 {
        let a = u16::from(px[3]);
        if a < 255 {
            for c in &mut px[..3] {
                *c = ((u16::from(*c) * a + 127) / 255) as u8;
            }
        }
    }
}

/// Flattens a premultiplied pixmap onto an opaque background and returns RGB8.
pub fn to_rgb(pm: &Pixmap, background: [u8; 3]) -> Vec<u8> {
    let mut out = Vec::with_capacity(pm.width() as usize * pm.height() as usize * 3);
    for px in pm.data().as_chunks::<4>().0 {
        let inv = 255 - u16::from(px[3]);
        for c in 0..3 {
            let v = u16::from(px[c]) + (u16::from(background[c]) * inv + 127) / 255;
            out.push(v.min(255) as u8);
        }
    }
    out
}

/// Encodes a pixmap as an opaque 24-bit RGB PNG with an sRGB chunk
/// (`Compression::High`).
pub fn encode_png_rgb(pm: &Pixmap, background: [u8; 3]) -> anyhow::Result<Vec<u8>> {
    encode_png_rgb_with(pm, background, png::Compression::High)
}

/// [`encode_png_rgb`] with a chosen compression level.
pub fn encode_png_rgb_with(
    pm: &Pixmap,
    background: [u8; 3],
    compression: png::Compression,
) -> anyhow::Result<Vec<u8>> {
    let rgb = to_rgb(pm, background);
    let mut out = Vec::new();
    {
        let mut enc = png::Encoder::new(&mut out, pm.width(), pm.height());
        enc.set_color(png::ColorType::Rgb);
        enc.set_depth(png::BitDepth::Eight);
        enc.set_source_srgb(png::SrgbRenderingIntent::Perceptual);
        enc.set_compression(compression);
        enc.set_filter(png::Filter::Adaptive);
        let mut w = enc.write_header()?;
        w.write_image_data(&rgb)?;
        w.finish()?;
    }
    Ok(out)
}

/// Encodes a pixmap as a baseline JPEG (RGB, flattened onto `background`)
/// with `quality` 1..=100.
pub fn encode_jpeg(pm: &Pixmap, background: [u8; 3], quality: u8) -> anyhow::Result<Vec<u8>> {
    let rgb = to_rgb(pm, background);
    let mut out = Vec::new();
    image::codecs::jpeg::JpegEncoder::new_with_quality(&mut out, quality.clamp(1, 100)).encode(
        &rgb,
        pm.width(),
        pm.height(),
        image::ExtendedColorType::Rgb8,
    )?;
    Ok(out)
}

pub fn save_png_rgb(pm: &Pixmap, path: &FsPath, background: [u8; 3]) -> anyhow::Result<usize> {
    let bytes = encode_png_rgb(pm, background)?;
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    std::fs::File::create(path)?.write_all(&bytes)?;
    Ok(bytes.len())
}

/// Area-average downscale (for review copies / previews of finished images).
pub fn downscale(pm: &Pixmap, w: u32) -> anyhow::Result<Pixmap> {
    let h = ((u64::from(pm.height()) * u64::from(w)) / u64::from(pm.width())).max(1) as u32;
    let src = Image::from_vec_u8(pm.width(), pm.height(), pm.data().to_vec(), PixelType::U8x4)?;
    let mut dst = Image::new(w, h, PixelType::U8x4);
    let opts = ResizeOptions::new()
        .resize_alg(ResizeAlg::Convolution(FilterType::Lanczos3))
        .use_alpha(false);
    Resizer::new().resize(&src, &mut dst, &opts)?;
    Pixmap::from_vec(
        dst.into_vec(),
        IntSize::from_wh(w, h).ok_or_else(|| anyhow::anyhow!("bad size"))?,
    )
    .ok_or_else(|| anyhow::anyhow!("pixmap"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn png_is_rgb_without_alpha_and_has_srgb() {
        let mut pm = Pixmap::new(4, 3).unwrap();
        pm.fill(tiny_skia::Color::from_rgba8(10, 20, 30, 128));
        let bytes = encode_png_rgb(&pm, [255, 255, 255]).unwrap();
        let dec = png::Decoder::new(std::io::Cursor::new(bytes));
        let reader = dec.read_info().unwrap();
        let info = reader.info();
        assert_eq!(info.color_type, png::ColorType::Rgb);
        assert_eq!(info.bit_depth, png::BitDepth::Eight);
        assert!(info.srgb.is_some());
    }

    #[test]
    fn jpeg_roundtrip_is_rgb() {
        let mut pm = Pixmap::new(16, 8).unwrap();
        pm.fill(tiny_skia::Color::from_rgba8(200, 20, 30, 255));
        let bytes = encode_jpeg(&pm, [255, 255, 255], 90).unwrap();
        let img = image::load_from_memory(&bytes).unwrap();
        assert_eq!(img.color(), image::ColorType::Rgb8);
        assert_eq!((img.width(), img.height()), (16, 8));
    }

    #[test]
    fn resample_modes() {
        let mut pm = Pixmap::new(40, 20).unwrap();
        pm.fill(tiny_skia::Color::from_rgba8(0, 0, 255, 255));
        let src = SourceImage::from_pixmap(&pm);
        let a = src
            .resample(10, 10, Some((0.5, 0.5)), Filter::Lanczos3)
            .unwrap();
        let b = src.resample(10, 3, None, Filter::Bilinear).unwrap();
        assert_eq!(
            (a.width(), a.height(), b.width(), b.height()),
            (10, 10, 10, 3)
        );
        assert_eq!(a.pixel(5, 5).unwrap().blue(), 255);
    }

    fn p3_icc() -> Vec<u8> {
        moxcms::ColorProfile::new_display_p3().encode().unwrap()
    }

    /// A PNG of the given colour type/depth and raw samples, optionally
    /// with an ICC profile.
    fn png_bytes(
        (w, h): (u32, u32),
        color: png::ColorType,
        depth: png::BitDepth,
        data: &[u8],
        icc: Option<Vec<u8>>,
    ) -> Vec<u8> {
        let mut info = png::Info::with_size(w, h);
        info.color_type = color;
        info.bit_depth = depth;
        info.icc_profile = icc.map(std::borrow::Cow::Owned);
        let mut out = Vec::new();
        {
            let mut wr = png::Encoder::with_info(&mut out, info)
                .unwrap()
                .write_header()
                .unwrap();
            wr.write_image_data(data).unwrap();
            wr.finish().unwrap();
        }
        out
    }

    fn near(a: &[u8], b: &[u8], tol: u8) -> bool {
        a.iter().zip(b).all(|(x, y)| x.abs_diff(*y) <= tol)
    }

    #[test]
    fn decode_converts_p3_png() {
        let px = [255, 0, 0, 255, 128, 128, 128, 255, 200, 100, 50, 77];
        let tagged = png_bytes(
            (3, 1),
            png::ColorType::Rgba,
            png::BitDepth::Eight,
            &px,
            Some(p3_icc()),
        );
        let img = SourceImage::decode(&tagged).unwrap();
        assert!(
            near(&img.rgba[0..4], &[255, 0, 0, 255], 1),
            "{:?}",
            img.rgba
        );
        assert!(near(&img.rgba[4..8], &[128, 128, 128, 255], 1));
        // Reference from the P3 → sRGB matrix: (215, 93, 31).
        assert!(near(&img.rgba[8..11], &[215, 93, 31], 2), "{:?}", img.rgba);
        assert_eq!(img.rgba[11], 77);
        // Untagged: unchanged.
        let plain = png_bytes(
            (3, 1),
            png::ColorType::Rgba,
            png::BitDepth::Eight,
            &px,
            None,
        );
        assert_eq!(SourceImage::decode(&plain).unwrap().rgba, px);
    }

    #[test]
    fn decode_converts_p3_sixteen_bit_png() {
        // 200·257, 100·257, 50·257 as big-endian RGB16.
        let data: Vec<u8> = [51400u16, 25700, 12850]
            .iter()
            .flat_map(|v| v.to_be_bytes())
            .collect();
        let tagged = png_bytes(
            (1, 1),
            png::ColorType::Rgb,
            png::BitDepth::Sixteen,
            &data,
            Some(p3_icc()),
        );
        let img = SourceImage::decode(&tagged).unwrap();
        assert!(near(&img.rgba, &[215, 93, 31, 255], 2), "{:?}", img.rgba);
    }

    #[test]
    fn decode_converts_p3_jpeg() {
        use image::ImageEncoder;
        let rgb: Vec<u8> = [200u8, 100, 50].repeat(64);
        let encode = |icc: Option<Vec<u8>>| {
            let mut out = Vec::new();
            let mut enc = image::codecs::jpeg::JpegEncoder::new_with_quality(&mut out, 100);
            if let Some(icc) = icc {
                enc.set_icc_profile(icc).unwrap();
            }
            enc.write_image(&rgb, 8, 8, image::ExtendedColorType::Rgb8)
                .unwrap();
            out
        };
        let plain = SourceImage::decode(&encode(None)).unwrap();
        let tagged = SourceImage::decode(&encode(Some(p3_icc()))).unwrap();
        // JPEG is lossy: compare the tagged result with the matrix
        // reference shifted by the codec error of the untagged one.
        let err: Vec<i16> = plain.rgba[0..3]
            .iter()
            .zip([200i16, 100, 50])
            .map(|(&a, b)| i16::from(a) - b)
            .collect();
        for ((&got, want), e) in tagged.rgba[0..3].iter().zip([215i16, 93, 31]).zip(err) {
            assert!(
                (i16::from(got) - want - e).abs() <= 4,
                "{:?}",
                &tagged.rgba[0..4]
            );
        }
    }

    #[test]
    fn decode_converts_gray_profile() {
        let icc = moxcms::ColorProfile::new_gray_with_gamma(1.0)
            .encode()
            .unwrap();
        // Linear gray 0.5 → sRGB ≈ 188.
        let tagged = png_bytes(
            (1, 1),
            png::ColorType::Grayscale,
            png::BitDepth::Eight,
            &[128],
            Some(icc),
        );
        let img = SourceImage::decode(&tagged).unwrap();
        assert!(near(&img.rgba, &[188, 188, 188, 255], 2), "{:?}", img.rgba);
    }

    #[test]
    fn decode_ignores_unusable_profiles() {
        let tagged = png_bytes(
            (1, 1),
            png::ColorType::Rgb,
            png::BitDepth::Eight,
            &[200, 100, 50],
            Some(b"not a profile".to_vec()),
        );
        let img = SourceImage::decode(&tagged).unwrap();
        assert_eq!(img.rgba, [200, 100, 50, 255]);
    }

    #[test]
    fn flatten_onto_background() {
        let mut pm = Pixmap::new(1, 1).unwrap();
        pm.fill(tiny_skia::Color::TRANSPARENT);
        assert_eq!(to_rgb(&pm, [1, 2, 3]), vec![1, 2, 3]);
    }
}
