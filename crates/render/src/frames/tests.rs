use super::*;
use arrangia_core::catalog::{DeviceClass, catalog};
use arrangia_core::model::{Appearance, Chrome};

const STYLES: [FrameStyle; 2] = [FrameStyle::Stylized, FrameStyle::Detailed];

fn all_devices() -> Vec<ResolvedDevice> {
    let mut out = Vec::new();
    for d in &catalog().devices.devices {
        for &o in &d.orientations {
            for style in STYLES {
                out.push(catalog_device(&d.id, o, style, None).expect("catalog model"));
            }
        }
    }
    out
}

#[test]
fn geometry_invariants() {
    for dev in all_devices() {
        let size = device_size(&dev, 400.0);
        let g = device_geometry(&dev, size);
        let id = format!("{} {:?}", dev.model, dev.orientation);
        let eps = 1e-3;
        // Centred, as wide as asked.
        assert!((g.outer.w - 400.0).abs() < eps, "{id}: width");
        assert!(
            g.outer.center().x.abs() < eps && g.outer.center().y.abs() < eps,
            "{id}: not centred"
        );
        // Screen inside outer.
        let (s, o) = (g.screen, g.outer);
        assert!(
            s.x >= o.x - eps
                && s.y >= o.y - eps
                && s.right() <= o.right() + eps
                && s.bottom() <= o.bottom() + eps,
            "{id}: screen outside outer"
        );
        // Silhouette bounds ≈ outer.
        let b = g.silhouette.bounds();
        let tol = 0.5;
        assert!(
            (f64::from(b.left()) - o.x).abs() < tol
                && (f64::from(b.top()) - o.y).abs() < tol
                && (f64::from(b.right()) - o.right()).abs() < tol
                && (f64::from(b.bottom()) - o.bottom()).abs() < tol,
            "{id}: silhouette {b:?} vs outer {o:?}"
        );
        // Screen aspect matches the catalog display (oriented).
        if let Some(a) = dev.device.screenshot.aspect {
            let natural = f64::from(a.w) / f64::from(a.h);
            let want = if dev.orientation == dev.device.natural {
                natural
            } else {
                1.0 / natural
            };
            let got = s.w / s.h;
            assert!(
                (got / want - 1.0).abs() < 0.02,
                "{id}: screen aspect {got} vs {want}"
            );
        }
        // A landscape screen is landscape-shaped.
        if dev.orientation == Orientation::Landscape && dev.device.class != DeviceClass::None {
            assert!(s.w > s.h, "{id}: landscape screen must be wide");
        }
        // Outer aspect matches the catalog outer size.
        if !dev.free_aspect {
            let os = dev.device.outer_size();
            let want = if layout::rotated(&dev) {
                os.w / os.h
            } else {
                os.h / os.w
            };
            assert!((o.h / o.w - want).abs() < 1e-6, "{id}: outer aspect");
        }
    }
}

#[test]
fn free_aspect_follows_screenshot() {
    for model in ["browser", "none"] {
        let dev = catalog_device(model, Orientation::Landscape, FrameStyle::Detailed, None)
            .expect("model");
        let dev = with_screenshot_aspect(dev, 1440.0 / 2000.0);
        let size = device_size(&dev, 600.0);
        let g = device_geometry(&dev, size);
        assert!((g.screen.w / g.screen.h - 0.72).abs() < 1e-6, "{model}");
    }
}

#[test]
fn chrome_on_laptop_shrinks_screen() {
    let mut dev = catalog_device(
        "macbook",
        Orientation::Landscape,
        FrameStyle::Detailed,
        None,
    )
    .expect("m");
    let size = device_size(&dev, 600.0);
    let plain = device_geometry(&dev, size).screen;
    dev.chrome = Some(Chrome::Browser);
    let with = device_geometry(&dev, size).screen;
    assert!((plain.bottom() - with.bottom()).abs() < 1e-6);
    assert!(with.h < plain.h && with.y > plain.y);
}

#[test]
fn landscape_rotates_counter_clockwise() {
    // The top edge of the portrait frame (island) goes to the left, the
    // right edge (power button) to the top.
    let dev = catalog_device(
        "iphone-island",
        Orientation::Landscape,
        FrameStyle::Detailed,
        None,
    )
    .expect("m");
    let l = Layout::new(&dev, device_size(&dev, 400.0));
    let (x, _) = l.pt(dev.device.body.w / 2.0, 0.0);
    assert!(x < -150.0, "top edge maps to the left, got {x}");
    let (_, y) = l.pt(dev.device.body.w, dev.device.body.h / 2.0);
    assert!(y < -80.0, "right edge maps to the top, got {y}");
}

#[test]
fn draws_every_model_in_both_styles() {
    let mut shot = Pixmap::new(40, 60).expect("pixmap");
    shot.fill(tiny_skia::Color::from_rgba8(40, 120, 200, 255));
    for mut dev in all_devices() {
        for (i, img) in [None, Some(&shot)].into_iter().enumerate() {
            dev.url = Some("https://example.com/path".into());
            dev.appearance = (i == 1).then_some(Appearance::Dark);
            if matches!(dev.device.class, DeviceClass::Laptop | DeviceClass::Desktop) {
                dev.chrome = (i == 1).then_some(Chrome::Browser);
            }
            let mut pm = Pixmap::new(300, 300).expect("pixmap");
            let size = device_size(&dev, 250.0);
            draw_device(
                &mut pm.as_mut(),
                &dev,
                size,
                img,
                Transform::from_translate(150.0, 150.0).pre_rotate(7.0),
            );
            assert!(
                pm.data().iter().any(|&b| b != 0),
                "{} drew nothing",
                dev.model
            );
        }
    }
}

#[test]
fn custom_colors_and_degenerate_sizes_do_not_panic() {
    let dev = catalog_device(
        "iphone-notch",
        Orientation::Portrait,
        FrameStyle::Detailed,
        Some("#3366CC"),
    )
    .expect("m");
    assert!(matches!(dev.color, DeviceColor::Custom(_)));
    let mut pm = Pixmap::new(50, 50).expect("pixmap");
    for w in [0.0, 1e-4, 3.0] {
        let size = device_size(&dev, w);
        let _ = device_geometry(&dev, size);
        draw_device(&mut pm.as_mut(), &dev, size, None, Transform::identity());
    }
}
