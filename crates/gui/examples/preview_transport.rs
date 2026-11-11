//! Measures preview payload options (ADR 0008): raw RGBA vs PNG at several
//! compression levels, for output images of the example project.
//!
//! ```sh
//! cargo run -p vitrine-gui --release --example preview_transport
//! ```

use std::time::Instant;

use vitrine_core::Project;
use vitrine_core::resolve::{FsAssets, ResolveOptions, resolve};
use vitrine_gui::preview::straight_rgba;
use vitrine_gui::testutil::example_fixture;
use vitrine_render::scene::{Quality, RenderContext, RenderOptions, Scale};

const RUNS: u32 = 5;

fn ms(t: Instant) -> f64 {
    t.elapsed().as_secs_f64() * 1000.0
}

fn encode(rgba: &[u8], w: u32, h: u32, c: png::Compression, f: png::Filter) -> Vec<u8> {
    let mut out = Vec::new();
    let mut enc = png::Encoder::new(&mut out, w, h);
    enc.set_color(png::ColorType::Rgba);
    enc.set_depth(png::BitDepth::Eight);
    enc.set_compression(c);
    enc.set_filter(f);
    let mut wr = enc.write_header().unwrap();
    wr.write_image_data(rgba).unwrap();
    wr.finish().unwrap();
    out
}

#[allow(clippy::many_single_char_names)]
fn main() {
    let root = example_fixture();
    let project = Project::load(&root).unwrap();
    let (ctx, _) = RenderContext::new(&root);
    let fs = FsAssets::new(&root);
    for (target, image) in [
        ("ios-iphone-69", 0),
        ("ios-ipad-13-landscape", 0),
        ("mac", 0),
    ] {
        let scene = resolve(&project, target, "en", &ResolveOptions::new(&fs)).unwrap();
        let prepared = ctx.prepare(&scene);
        for edge in [400u32, 800] {
            let opts = RenderOptions {
                scale: Scale::MaxEdge(edge),
                quality: Quality::Preview,
            };
            let _ = ctx.rasterize_prepared(&prepared, image, &opts); // warm caches
            let t = Instant::now();
            let mut r = None;
            for _ in 0..RUNS {
                r = Some(ctx.rasterize_prepared(&prepared, image, &opts));
            }
            let render = ms(t) / f64::from(RUNS);
            let pm = r.unwrap().pixmap;
            let (w, h) = (pm.width(), pm.height());
            let t = Instant::now();
            let rgba = straight_rgba(&pm);
            let straight = ms(t);
            println!(
                "{target} image {image} @ {edge}px: {w}x{h}, render {render:.1} ms, raw RGBA {} KiB (straight alpha {straight:.2} ms)",
                rgba.len() / 1024
            );
            for (name, c, f) in [
                ("fastest/up", png::Compression::Fastest, png::Filter::Up),
                ("fast/up", png::Compression::Fast, png::Filter::Up),
                (
                    "fast/adaptive",
                    png::Compression::Fast,
                    png::Filter::Adaptive,
                ),
                (
                    "balanced/adaptive",
                    png::Compression::Balanced,
                    png::Filter::Adaptive,
                ),
            ] {
                let t = Instant::now();
                let mut bytes = Vec::new();
                for _ in 0..RUNS {
                    bytes = encode(&rgba, w, h, c, f);
                }
                let enc = ms(t) / f64::from(RUNS);
                let t = Instant::now();
                for _ in 0..RUNS {
                    let mut d = png::Decoder::new(std::io::Cursor::new(&bytes))
                        .read_info()
                        .unwrap();
                    let mut buf = vec![0; d.output_buffer_size().unwrap()];
                    d.next_frame(&mut buf).unwrap();
                }
                let dec = ms(t) / f64::from(RUNS);
                println!(
                    "  png {name:18} {:>6} KiB  encode {enc:6.2} ms  decode {dec:6.2} ms",
                    bytes.len() / 1024
                );
            }
        }
    }
}
