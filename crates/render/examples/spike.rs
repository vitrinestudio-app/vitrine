//! Phase 0 rendering spike.
//!
//! ```sh
//! cargo run -p arrangia-render --release --example spike [-- --regen-assets] [--runs N]
//! ```
//!
//! Writes all images to `docs/spike/` and prints timings. See
//! `docs/adr/0001-render-pipeline.md`. Text goes through `arrangia_core::layout`
//! (the cosmic-text comparison of Phase 0 was removed in Phase 1; its image
//! `docs/spike/text-engines_comparison.png` is kept as a historical record).

#![allow(
    clippy::too_many_lines,
    clippy::too_many_arguments,
    clippy::many_single_char_names,
    reason = "spike driver"
)]

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use arrangia_core::layout::{
    Fit, FontLibrary, ResolvedSpanStyle, ResolvedTextStyle, TextInput, TextLayouter,
};
use arrangia_core::model::{Align, VAlign};
use arrangia_render::frames::{catalog_device, device_geometry, device_size, draw_device};
use arrangia_render::geometry::{AnchorY, PAGE_W, StripLayout, Viewport};
use arrangia_render::paint::{hex, round_rect, solid};
use arrangia_render::raster::{SourceImage, downscale, save_png_rgb};
use arrangia_render::spike::{
    self, PLACEHOLDER_GAP_LU, Placement, PreparedText, RenderParams, Scene, Screenshots, Variant,
    compose, screens,
};
use arrangia_render::spike::{FrameStyle, Orientation};
use arrangia_render::text::{draw_layout, draw_text, to_paths};
use tiny_skia::{FillRule, Pixmap, Stroke, Transform};

const BG_WHITE: [u8; 3] = [255, 255, 255];
/// Placeholder gap for the grouped iPad set (not yet measured).
const PLACEHOLDER_GAP_IPAD_LU: f64 = 60.0;

struct Ctx {
    out: PathBuf,
    layouter: TextLayouter,
    scene: Scene,
    shots: Screenshots,
    written: Vec<(String, usize, u32, u32)>,
}

impl Ctx {
    fn save(&mut self, pm: &Pixmap, rel: &str) {
        let path = self.out.join(rel);
        let n = save_png_rgb(pm, &path, BG_WHITE).expect("write png");
        self.written
            .push((rel.to_owned(), n, pm.width(), pm.height()));
    }
}

fn median(mut v: Vec<Duration>) -> Duration {
    v.sort();
    v[v.len() / 2]
}

fn ms(d: Duration) -> f64 {
    d.as_secs_f64() * 1000.0
}

fn main() -> anyhow::Result<()> {
    let args: Vec<String> = std::env::args().collect();
    let arg = |name: &str| {
        args.iter()
            .position(|a| a == name)
            .and_then(|i| args.get(i + 1).cloned())
    };
    let runs: usize = arg("--runs").and_then(|s| s.parse().ok()).unwrap_or(15);
    let regen = args.iter().any(|a| a == "--regen-assets");

    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()?;
    let out = root.join("docs/spike");
    if out.join("pages").exists() {
        std::fs::remove_dir_all(out.join("pages"))?;
    }
    for e in std::fs::read_dir(&out).into_iter().flatten().flatten() {
        if e.path().extension().is_some_and(|x| x == "png")
            && e.file_name() != "text-engines_comparison.png"
        {
            std::fs::remove_file(e.path())?;
        }
    }
    std::fs::create_dir_all(&out)?;

    // ---- one-time costs -------------------------------------------------
    let t = Instant::now();
    let mut layouter = TextLayouter::new(FontLibrary::bundled());
    let t_layouter_init = t.elapsed();

    let assets = root.join("assets/spike");
    let shot_files = [
        ("phone-home", "phone-home.png"),
        ("phone-harvest", "phone-harvest.png"),
        ("tablet-planner", "tablet-planner.png"),
    ];
    if regen || shot_files.iter().any(|(_, f)| !assets.join(f).exists()) {
        println!("generating test screenshots in {}", assets.display());
        let e = &mut layouter;
        save_png_rgb(
            &screens::phone_home(e),
            &assets.join("phone-home.png"),
            BG_WHITE,
        )?;
        save_png_rgb(
            &screens::phone_harvest(e),
            &assets.join("phone-harvest.png"),
            BG_WHITE,
        )?;
        save_png_rgb(
            &screens::tablet_planner(e),
            &assets.join("tablet-planner.png"),
            BG_WHITE,
        )?;
    }
    let t = Instant::now();
    let mut shots = Screenshots::default();
    for (key, file) in shot_files {
        shots.insert(key, SourceImage::load(&assets.join(file))?);
    }
    let t_decode = t.elapsed();

    let mut ctx = Ctx {
        out,
        layouter,
        scene: spike::demo_scene(),
        shots,
        written: Vec::new(),
    };

    // ---- exports ----------------------------------------------------------
    let texts: HashMap<&str, HashMap<&'static str, PreparedText>> = ["en", "de"]
        .into_iter()
        .map(|lang| {
            (
                lang,
                spike::prepare_texts(&ctx.scene, lang, &mut ctx.layouter),
            )
        })
        .collect();
    for (lang, t) in &texts {
        let mut ids: Vec<_> = t.keys().collect();
        ids.sort();
        for id in ids {
            let p = &t[id];
            println!(
                "  {lang} {id}: size {:.1} lu, {} lines, width {:.0} lu, fits {}",
                p.size, p.lines, p.width, p.fits
            );
        }
    }

    let none = Variant::none();
    let h0 = spike::design_page_height();
    let design = StripLayout::new(ctx.scene.page_count, h0, 0.0);
    let t_resize = Instant::now();
    export_pages(
        &mut ctx,
        &texts,
        &none,
        design.with_gap(PLACEHOLDER_GAP_LU),
        (1320, 2868),
        1,
        "gap40",
        FrameStyle::Detailed,
        (&["en", "de"], &["en", "de"]),
    );
    let t_first_export = t_resize.elapsed();
    export_pages(
        &mut ctx,
        &texts,
        &none,
        design,
        (1320, 2868),
        1,
        "nogap",
        FrameStyle::Detailed,
        (&["en"], &["en"]),
    );
    export_pages(
        &mut ctx,
        &texts,
        &none,
        design.with_gap(PLACEHOLDER_GAP_LU),
        (1080, 2160),
        1,
        "gap40",
        FrameStyle::Detailed,
        (&["en", "de"], &["de"]),
    );

    // Grouped landscape set (tablet variant).
    let mut tablet_variant = Variant {
        content_scale: 0.75,
        overrides: HashMap::new(),
    };
    tablet_variant.overrides.insert(
        "hero-phone",
        Placement {
            page: 0,
            x: 1000.0,
            y: h0 - 380.0,
            anchor: AnchorY::Bottom,
            rotation: -8.0,
        },
    );
    tablet_variant.overrides.insert(
        "harvest-phone",
        Placement {
            page: 2,
            x: 500.0,
            y: h0 - 470.0,
            anchor: AnchorY::Bottom,
            rotation: 6.0,
        },
    );
    export_pages(
        &mut ctx,
        &texts,
        &tablet_variant,
        design.with_gap(PLACEHOLDER_GAP_IPAD_LU),
        (2752, 2064),
        2,
        "group2-gap60",
        FrameStyle::Detailed,
        (&["en", "de"], &["en", "de"]),
    );

    // Stylized style for comparison (page sheet only).
    export_pages(
        &mut ctx,
        &texts,
        &none,
        design.with_gap(PLACEHOLDER_GAP_LU),
        (1320, 2868),
        1,
        "gap40-stylized",
        FrameStyle::Stylized,
        (&["en"], &[]),
    );

    // Banner: whole canvas, gapless, cover.
    for lang in ["en", "de"] {
        let layout = design;
        let view = Viewport::cover(layout.canvas_rect(), 2400, 1000, (0.5, 0.0));
        let params = RenderParams {
            layout,
            variant: &none,
            style: FrameStyle::Detailed,
        };
        let pm = spike::render(&ctx.scene, &texts[lang], &ctx.shots, &params, &view);
        ctx.save(&pm, &format!("banner_{lang}_2400x1000.png"));
    }

    // Canvas debug view.
    {
        let layout = design.with_gap(PLACEHOLDER_GAP_LU);
        let view = Viewport::fit_width(layout.canvas_rect(), 2400);
        let params = RenderParams {
            layout,
            variant: &none,
            style: FrameStyle::Detailed,
        };
        let mut pm = spike::render(&ctx.scene, &texts["en"], &ctx.shots, &params, &view);
        compose::debug_overlay(&mut pm, &layout, &view, &mut ctx.layouter);
        ctx.save(&pm, "canvas-debug_en_gap40.png");
        let layout = design
            .with_gap(PLACEHOLDER_GAP_IPAD_LU)
            .for_target(2752, 2064, 2);
        let view = Viewport::fit_width(layout.canvas_rect(), 2400);
        let params = RenderParams {
            layout,
            variant: &tablet_variant,
            style: FrameStyle::Detailed,
        };
        let mut pm = spike::render(&ctx.scene, &texts["en"], &ctx.shots, &params, &view);
        compose::debug_overlay(&mut pm, &layout, &view, &mut ctx.layouter);
        ctx.save(&pm, "canvas-debug_en_group2-gap60.png");
    }

    frames_sheet(&mut ctx);
    text_layout_sheet(&mut ctx);

    // ---- timings -------------------------------------------------------------
    println!();
    println!("machine: {}", machine());
    println!(
        "one-time: text layouter (fonts) {:.1} ms, screenshot decode (3 PNGs) {:.1} ms",
        ms(t_layouter_init),
        ms(t_decode)
    );
    println!(
        "first gap40 export of 4 pages x 2 langs incl. Lanczos3 resize + PNG encode: {:.0} ms",
        ms(t_first_export)
    );
    let layout = design.with_gap(PLACEHOLDER_GAP_LU);
    {
        let name = "parley";
        let mut prep = Vec::new();
        for _ in 0..runs {
            let t = Instant::now();
            let _ = spike::prepare_texts(&ctx.scene, "de", &mut ctx.layouter);
            prep.push(t.elapsed());
        }
        let tx = spike::prepare_texts(&ctx.scene, "de", &mut ctx.layouter);
        for (label, w, h) in [
            ("export 1320x2868", 1320u32, 2868u32),
            ("preview 400x869", 400, 869),
        ] {
            let params = RenderParams {
                layout,
                variant: &none,
                style: FrameStyle::Detailed,
            };
            // cold: empty screenshot cache (includes Lanczos3 resampling)
            ctx.shots.clear_cache();
            let t = Instant::now();
            for p in 0..4 {
                let view = spike::image_viewport(&layout, p, w, h);
                let _ = spike::render(&ctx.scene, &tx, &ctx.shots, &params, &view);
            }
            let cold = t.elapsed() / 4;
            let mut per_page = Vec::new();
            let mut all = Vec::new();
            for p in 0..4 {
                let view = spike::image_viewport(&layout, p, w, h);
                let mut v = Vec::new();
                for _ in 0..runs {
                    let t = Instant::now();
                    let _ = spike::render(&ctx.scene, &tx, &ctx.shots, &params, &view);
                    v.push(t.elapsed());
                }
                all.extend(v.iter().copied());
                per_page.push(median(v));
            }
            let enc = {
                let view = spike::image_viewport(&layout, 0, w, h);
                let pm = spike::render(&ctx.scene, &tx, &ctx.shots, &params, &view);
                let t = Instant::now();
                let _ = arrangia_render::raster::encode_png_rgb(&pm, BG_WHITE)?;
                t.elapsed()
            };
            println!(
                "{name:12} {label:17}: median {:.1} ms/page (pages: {}) | cold avg {:.1} ms/page | PNG encode {:.0} ms | text layout+autofit (4 headlines, de) {:.2} ms",
                ms(median(all)),
                per_page
                    .iter()
                    .map(|d| format!("{:.1}", ms(*d)))
                    .collect::<Vec<_>>()
                    .join(" / "),
                ms(cold),
                ms(enc),
                ms(median(prep.clone())),
            );
        }
    }

    // ---- summary ---------------------------------------------------------------
    println!();
    let mut total = 0;
    for (rel, n, w, h) in &ctx.written {
        total += n;
        println!("{:>8.1} KB  {w}x{h}  docs/spike/{rel}", *n as f64 / 1024.0);
    }
    println!(
        "{:.1} MB total in docs/spike",
        total as f64 / 1024.0 / 1024.0
    );
    Ok(())
}

fn machine() -> String {
    std::process::Command::new("sysctl")
        .args(["-n", "machdep.cpu.brand_string"])
        .output()
        .ok()
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .map_or_else(|| "unknown".to_owned(), |s| s.trim().to_owned())
}

/// Renders all export images of one target for en and de, writes them, and
/// a store gallery preview of the set.
#[allow(clippy::too_many_arguments)]
fn export_pages(
    ctx: &mut Ctx,
    texts: &HashMap<&str, HashMap<&'static str, PreparedText>>,
    variant: &Variant,
    layout: StripLayout,
    (w, h): (u32, u32),
    group: usize,
    tag: &str,
    style: FrameStyle,
    (langs, page_langs): (&[&str], &[&str]),
) {
    let layout = layout.for_target(w, h, group);
    let params = RenderParams {
        layout,
        variant,
        style,
    };
    for &lang in langs {
        let mut images = Vec::new();
        for i in 0..layout.image_count() {
            let view = spike::image_viewport(&layout, i, w, h);
            let pm = spike::render(&ctx.scene, &texts[lang], &ctx.shots, &params, &view);
            images.push(pm);
        }
        let dir = format!("pages/{w}x{h}_{tag}");
        if page_langs.contains(&lang) {
            for (i, pm) in images.iter().enumerate() {
                ctx.save(pm, &format!("{dir}/{lang}_{:02}.png", i + 1));
            }
        }
        // Gallery preview at reduced size (store-like).
        let gw = if group > 1 { 760 } else { 420 };
        let small: Vec<Pixmap> = images
            .iter()
            .map(|p| downscale(p, gw).expect("downscale"))
            .collect();
        let scale = f64::from(gw) / (PAGE_W * group as f64);
        let g = compose::gallery(&small, &layout, scale, 40.0, 40, hex("#F2F2F7"));
        ctx.save(&g, &format!("gallery_{w}x{h}_{tag}_{lang}.png"));
    }
}

/// Large comparison of both frame styles (phone + tablet).
fn frames_sheet(ctx: &mut Ctx) {
    let (w, h) = (2400u32, 1400u32);
    let mut pm = Pixmap::new(w, h).expect("size");
    let bg = arrangia_render::paint::linear(
        (0.0, 0.0),
        (w as f32, h as f32),
        &[(0.0, hex("#1C3B33")), (1.0, hex("#2C3F5E"))],
    );
    if let Some(r) = tiny_skia::Rect::from_xywh(0.0, 0.0, w as f32, h as f32) {
        pm.fill_rect(
            r,
            &arrangia_render::paint::shader_paint(bg),
            Transform::identity(),
            None,
        );
    }
    let device = |model: &str, o: Orientation, style: FrameStyle, color: &str| {
        catalog_device(model, o, style, Some(color)).expect("catalog model")
    };
    let items = [
        (250.0f32, FrameStyle::Stylized, "porcelain", "phone-home"),
        (660.0, FrameStyle::Stylized, "midnight", "phone-harvest"),
        (
            1110.0,
            FrameStyle::Detailed,
            "natural-titanium",
            "phone-home",
        ),
        (
            1520.0,
            FrameStyle::Detailed,
            "black-titanium",
            "phone-harvest",
        ),
    ];
    for (x, style, color, shot) in items {
        let dev = device("iphone-island", Orientation::Portrait, style, color);
        let size = device_size(&dev, 360.0, None);
        let g = device_geometry(&dev, size);
        let ts = Transform::from_translate(x, 690.0).pre_rotate(-8.0);
        for (sigma, dy, a) in [(26.0, 24.0, 0.42), (6.0, 6.0, 0.3)] {
            arrangia_render::blur::draw_shadow(
                &mut pm,
                &g.silhouette,
                ts,
                arrangia_render::blur::ShadowPx {
                    sigma,
                    dx: 0.0,
                    dy,
                    color: [0, 0, 0, (a * 255.0) as u8],
                },
            );
        }
        let shot = ctx
            .shots
            .get(shot, g.screen.w.round() as u32, g.screen.h.round() as u32);
        draw_device(&mut pm.as_mut(), &dev, size, shot.as_deref(), ts);
    }
    for (x, y, style, color) in [
        (2060.0f32, 420.0f32, FrameStyle::Stylized, "silver"),
        (2060.0, 940.0, FrameStyle::Detailed, "space-gray"),
    ] {
        let dev = device("ipad-modern", Orientation::Landscape, style, color);
        let size = device_size(&dev, 560.0, None);
        let tg = device_geometry(&dev, size);
        let ts = Transform::from_translate(x, y);
        arrangia_render::blur::draw_shadow(
            &mut pm,
            &tg.silhouette,
            ts,
            arrangia_render::blur::ShadowPx {
                sigma: 22.0,
                dx: 0.0,
                dy: 18.0,
                color: [0, 0, 0, 100],
            },
        );
        let shot = ctx.shots.get(
            "tablet-planner",
            tg.screen.w.round() as u32,
            tg.screen.h.round() as u32,
        );
        draw_device(&mut pm.as_mut(), &dev, size, shot.as_deref(), ts);
    }
    // Labels
    for (x, text) in [
        (455.0, "**stylized** · porcelain / midnight"),
        (1315.0, "**detailed** · silver / graphite"),
        (2060.0, "tablet · stylized / detailed"),
    ] {
        let style = small_style();
        let l = ctx.layouter.layout(&TextInput {
            align: Align::Center,
            ..TextInput::new(text, &style, 440.0)
        });
        let ts = Transform::from_translate(x - 220.0, 1290.0);
        draw_layout(&mut pm, &l, ctx.layouter.fonts(), ts, None);
    }
    ctx.save(&pm, "frames_styles.png");

    // Close-up of the island/top of both styles (2x zoom crop).
    let mut zoom = Pixmap::new(1600, 760).expect("size");
    zoom.fill(tiny_skia::Color::from_rgba8(30, 50, 45, 255));
    for (i, (style, color)) in [
        (FrameStyle::Stylized, "midnight"),
        (FrameStyle::Detailed, "natural-titanium"),
    ]
    .into_iter()
    .enumerate()
    {
        let dev = device("iphone-island", Orientation::Portrait, style, color);
        let size = device_size(&dev, 700.0, None);
        let g = device_geometry(&dev, size);
        let ts = Transform::from_translate(400.0 + 800.0 * i as f32, 50.0 - g.outer.y as f32);
        let shot = ctx.shots.get(
            "phone-home",
            g.screen.w.round() as u32,
            g.screen.h.round() as u32,
        );
        draw_device(&mut zoom.as_mut(), &dev, size, shot.as_deref(), ts);
    }
    ctx.save(&zoom, "frames_closeup.png");
}

fn small_style() -> ResolvedTextStyle {
    ResolvedTextStyle {
        size: 30.0,
        color: spike::core_color(hex("#FFFFFF")),
        ..ResolvedTextStyle::default()
    }
}

/// Text layout features of `arrangia_core::layout`: auto-fit, soft hyphens,
/// emergency breaks, inline styles.
fn text_layout_sheet(ctx: &mut Ctx) {
    let fit = |shrink_to: f32, max_lines: u32| Fit {
        shrink_to: Some(shrink_to),
        max_lines: Some(max_lines),
    };
    let cases: [(&str, &str, Fit); 6] = [
        (
            "auto-fit 104 → 56 lu, 2 lines (de)",
            "**Mischkulturempfehlungen** für jedes Beet",
            fit(56.0, 2),
        ),
        (
            "auto-fit (de)",
            "Behalte jede [accent]Ernte[/] auf einen Blick im Griff",
            fit(56.0, 2),
        ),
        (
            "soft hyphens Misch\\-kultur\\-empfehlungen, no shrinking",
            "**Misch\\-kultur\\-empfehlungen** für jedes Beet",
            fit(104.0, 3),
        ),
        (
            "no soft hyphen, no shrinking: emergency break (warning)",
            "**Mischkulturempfehlungen** für jedes Beet",
            fit(104.0, 3),
        ),
        (
            "max_lines 1 at shrink_to: overflow (error)",
            "Plane deine Beete [accent]in Minuten[/] und behalte alles im Blick",
            fit(80.0, 1),
        ),
        (
            "_italic_ (synthetic oblique), [hl]highlight[/], **bold**",
            "Plane _deine_ Beete [hl]in Minuten[/] — **wirklich**",
            fit(56.0, 2),
        ),
    ];
    let row_h = 420.0f32;
    let label_h = 70.0f32;
    let w = 1000u32;
    let h = (cases.len() as f32 * (row_h + label_h) + 30.0) as u32;
    let mut pm = Pixmap::new(w, h).expect("size");
    pm.fill(tiny_skia::Color::from_rgba8(16, 52, 42, 255));
    let mut spans = spike::spike_spans();
    spans.insert(
        "hl".to_owned(),
        ResolvedSpanStyle {
            color: Some(spike::core_color(hex("#10342A"))),
            background: Some(spike::core_color(hex("#FFC65C"))),
            ..ResolvedSpanStyle::default()
        },
    );
    let bw = 840.0f32;
    for (ri, (title, markup, fit)) in cases.iter().enumerate() {
        let y0 = 30.0 + ri as f32 * (row_h + label_h);
        let style = spike::headline_style(104.0);
        let layout = ctx.layouter.layout(&TextInput {
            spans: Some(&spans),
            align: Align::Center,
            valign: VAlign::Center,
            height: Some(row_h - 60.0),
            fit: *fit,
            ..TextInput::new(markup, &style, bw)
        });
        if let Some(p) = round_rect(10.0, y0, w as f32 - 20.0, row_h, 18.0) {
            pm.fill_path(
                &p,
                &solid([255, 255, 255, 18]),
                FillRule::Winding,
                Transform::identity(),
                None,
            );
        }
        if let Some(r) = tiny_skia::Rect::from_xywh(80.0, y0 + 30.0, bw, row_h - 60.0) {
            let p = tiny_skia::PathBuilder::from_rect(r);
            pm.stroke_path(
                &p,
                &solid([255, 198, 92, 150]),
                &Stroke {
                    width: 2.0,
                    dash: tiny_skia::StrokeDash::new(vec![10.0, 8.0], 0.0),
                    ..Stroke::default()
                },
                Transform::identity(),
                None,
            );
        }
        // Clip each panel like a page (`clip: page`).
        let mut clip = tiny_skia::Mask::new(pm.width(), pm.height()).expect("mask");
        if let Some(r) = tiny_skia::Rect::from_xywh(10.0, y0, w as f32 - 20.0, row_h) {
            clip.fill_path(
                &tiny_skia::PathBuilder::from_rect(r),
                FillRule::Winding,
                true,
                Transform::identity(),
            );
        }
        let paths = to_paths(&layout, ctx.layouter.fonts());
        draw_text(
            &mut pm,
            &paths,
            Transform::from_translate(80.0, y0 + 30.0),
            Some(&clip),
        );
        let report = layout.report();
        let status = if report.overflow {
            "OVERFLOW".to_owned()
        } else if report.diagnostics.is_empty() {
            "fits".to_owned()
        } else {
            report
                .diagnostics
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>()
                .join("; ")
        };
        let info = format!(
            "{title}: {:.2} lu · {} line(s) · {status}",
            report.font_size, report.line_count
        );
        let mut st = small_style();
        st.size = 22.0;
        st.color = spike::core_color(hex("#CFE3D8"));
        // Show the label literally: escape markup characters.
        let literal: String = info
            .chars()
            .flat_map(|c| {
                let esc = matches!(c, '\\' | '_' | '*' | '[').then_some('\\');
                esc.into_iter().chain(std::iter::once(c))
            })
            .collect();
        let l = ctx
            .layouter
            .layout(&TextInput::new(&literal, &st, w as f32 - 60.0));
        draw_layout(
            &mut pm,
            &l,
            ctx.layouter.fonts(),
            Transform::from_translate(30.0, y0 + row_h + 12.0),
            None,
        );
    }
    ctx.save(&pm, "text-layout.png");
}
