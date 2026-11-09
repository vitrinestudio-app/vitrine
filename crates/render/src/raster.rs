//! Image IO: screenshot decoding and Lanczos3 resampling, PNG export
//! (24-bit RGB, no alpha, sRGB chunk).

use std::io::Write;
use std::path::Path as FsPath;

use fast_image_resize::images::Image;
use fast_image_resize::{FilterType, PixelType, ResizeAlg, ResizeOptions, Resizer};
use tiny_skia::{IntSize, Pixmap};

/// A decoded screenshot (straight RGBA8; screenshots are opaque).
#[derive(Debug, Clone)]
pub struct SourceImage {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
}

impl SourceImage {
    pub fn load(path: &FsPath) -> anyhow::Result<Self> {
        let img = image::open(path)?.to_rgba8();
        Ok(Self {
            width: img.width(),
            height: img.height(),
            rgba: img.into_raw(),
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
        let (w, h) = (w.max(1), h.max(1));
        let src = Image::from_vec_u8(self.width, self.height, self.rgba.clone(), PixelType::U8x4)?;
        let mut dst = Image::new(w, h, PixelType::U8x4);
        let opts = ResizeOptions::new()
            .resize_alg(ResizeAlg::Convolution(FilterType::Lanczos3))
            .fit_into_destination(Some((0.5, 0.5)));
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

/// Encodes a pixmap as an opaque 24-bit RGB PNG with an sRGB chunk.
pub fn encode_png_rgb(pm: &Pixmap, background: [u8; 3]) -> anyhow::Result<Vec<u8>> {
    let rgb = to_rgb(pm, background);
    let mut out = Vec::new();
    {
        let mut enc = png::Encoder::new(&mut out, pm.width(), pm.height());
        enc.set_color(png::ColorType::Rgb);
        enc.set_depth(png::BitDepth::Eight);
        enc.set_source_srgb(png::SrgbRenderingIntent::Perceptual);
        enc.set_compression(png::Compression::High);
        enc.set_filter(png::Filter::Adaptive);
        let mut w = enc.write_header()?;
        w.write_image_data(&rgb)?;
        w.finish()?;
    }
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
    fn flatten_onto_background() {
        let mut pm = Pixmap::new(1, 1).unwrap();
        pm.fill(tiny_skia::Color::TRANSPARENT);
        assert_eq!(to_rgb(&pm, [1, 2, 3]), vec![1, 2, 3]);
    }
}
