//! Measures the cost of the sRGB conversion at decode (ADR 0021) on an
//! iPhone 6.9" screenshot (1320×2868 RGBA): per image, the best of
//! `RUNS` for the plain `image` decode and for `SourceImage::decode`
//! (decode + profile sniffing + conversion), interleaved so machine load
//! hits both alike.
//!
//! `cargo run -p vitrine-render --release --example color_bench`

use std::time::Instant;

use vitrine_render::raster::SourceImage;

const W: u32 = 1320;
const H: u32 = 2868;
const RUNS: u32 = 15;

fn png(depth: png::BitDepth, icc: Option<Vec<u8>>) -> Vec<u8> {
    let deep = depth == png::BitDepth::Sixteen;
    let mut data = Vec::new();
    for y in 0..H {
        for x in 0..W {
            for v in [x * 255 / W, y * 255 / H, (x + y) % 256, 255] {
                if deep {
                    data.extend_from_slice(&((v * 257) as u16).to_be_bytes());
                } else {
                    data.push(v as u8);
                }
            }
        }
    }
    let mut info = png::Info::with_size(W, H);
    info.color_type = png::ColorType::Rgba;
    info.bit_depth = depth;
    info.icc_profile = icc.map(std::borrow::Cow::Owned);
    let mut out = Vec::new();
    {
        let mut enc = png::Encoder::with_info(&mut out, info).expect("info");
        enc.set_compression(png::Compression::Fast);
        let mut w = enc.write_header().expect("header");
        w.write_image_data(&data).expect("data");
        w.finish().expect("finish");
    }
    out
}

fn ms(t: Instant) -> f64 {
    t.elapsed().as_secs_f64() * 1000.0
}

fn bench(name: &str, bytes: &[u8]) {
    let (mut plain, mut ours) = (f64::MAX, f64::MAX);
    for _ in 0..RUNS {
        let t = Instant::now();
        std::hint::black_box(image::load_from_memory(bytes).expect("decode").to_rgba8());
        plain = plain.min(ms(t));
        let t = Instant::now();
        std::hint::black_box(SourceImage::decode(bytes).expect("decode"));
        ours = ours.min(ms(t));
    }
    println!(
        "{name:<18} image decode {plain:6.1} ms   SourceImage::decode {ours:6.1} ms   overhead {:+6.1} ms",
        ours - plain
    );
}

fn main() {
    let p3 = moxcms::ColorProfile::new_display_p3()
        .encode()
        .expect("P3 profile");
    println!("{W}x{H} RGBA PNG, best of {RUNS}:");
    bench("8 bit untagged", &png(png::BitDepth::Eight, None));
    bench(
        "8 bit Display P3",
        &png(png::BitDepth::Eight, Some(p3.clone())),
    );
    bench("16 bit untagged", &png(png::BitDepth::Sixteen, None));
    bench("16 bit Display P3", &png(png::BitDepth::Sixteen, Some(p3)));
}
