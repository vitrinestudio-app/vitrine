use super::synthetic::{self, BezelSpec};
use super::*;

fn package_dir() -> (tempfile::TempDir, Vec<BezelSpec>) {
    let dir = tempfile::tempdir().unwrap();
    let specs = synthetic::default_specs();
    synthetic::write_folder(dir.path(), &specs).unwrap();
    (dir, specs)
}

fn apple() -> &'static Rules {
    Rules::builtin("apple").unwrap()
}

#[test]
fn screen_detection_is_accurate() {
    for spec in synthetic::default_specs() {
        let img = synthetic::render(&spec);
        let a = analyze(&img).unwrap();
        let s = spec.screen;
        for (found, want) in [
            (a.screen.x, s.x),
            (a.screen.y, s.y),
            (a.screen.w, s.w),
            (a.screen.h, s.h),
        ] {
            assert!(
                found.abs_diff(want) <= 1,
                "{}: {:?} vs {:?}",
                spec.file,
                a.screen,
                s
            );
        }
        assert!(
            (a.radius - spec.screen_radius).abs() <= spec.screen_radius * 0.15 + 2.0,
            "{}: radius {} vs {}",
            spec.file,
            a.radius,
            spec.screen_radius
        );
        // Mask: screen centre inside, island and bezel outside.
        let at = |x: u32, y: u32| a.mask[(y * img.width + x) as usize];
        assert_eq!(at(img.width / 2, img.height / 2), 255);
        assert_eq!(at(2, img.height / 2), 0);
        if spec.island && s.h > s.w {
            let island_y = s.y + 12 + (f64::from(s.w) * 0.05) as u32;
            assert_eq!(at(img.width / 2, island_y), 0, "island is not screen");
        }
    }
}

#[test]
fn opaque_or_open_images_are_rejected() {
    let mut spec = synthetic::default_specs().remove(0);
    let mut img = synthetic::render(&spec);
    for p in img.data.as_chunks_mut::<4>().0 {
        p[3] = 255;
    }
    assert!(analyze(&img).unwrap_err().contains("centre"));
    // A screen running into the image edge.
    spec.screen.y = 0;
    spec.screen.h += 25;
    let img = synthetic::render(&spec);
    assert!(analyze(&img).unwrap_err().contains("edge"));
}

#[test]
fn plan_matches_dedupes_and_lists_unmatched() {
    let (dir, _) = package_dir();
    let store_dir = tempfile::tempdir().unwrap();
    let store = FrameStore::new(store_dir.path());
    let pkg = Package::open(dir.path()).unwrap();
    assert_eq!(pkg.kind, PackageKind::Folder);
    assert_eq!(pkg.skipped, 1, "Read Me.txt");
    assert_eq!(pkg.psd, 2);
    assert_eq!(pkg.licenses, ["License.rtf"]);
    let plan = import(&store, &pkg, apple(), false).unwrap();
    assert_eq!(plan.psd, 2);
    assert_eq!(plan.skipped, 1);
    assert_eq!(plan.licenses.len(), 1);
    assert_eq!(
        std::path::Path::new(&plan.licenses[0]),
        dir.path().join("License.rtf")
    );
    assert!(!plan.applied);
    assert!(plan.unverified_rules);
    let mut installs: Vec<_> = plan
        .installs()
        .map(|i| {
            (
                i.matched.model.as_str(),
                i.matched.color.as_str(),
                orientation_name(i.matched.orientation),
                i.matched.apple_model.as_str(),
            )
        })
        .collect();
    installs.sort_unstable();
    // Both iPhone sizes are kept (ADR 0012): Pro and Pro Max.
    assert_eq!(
        installs,
        [
            ("ipad-modern", "space-black", "portrait", "iPad Pro 13 - M4"),
            (
                "iphone-island",
                "black-titanium",
                "landscape",
                "iPhone 16 Pro"
            ),
            (
                "iphone-island",
                "black-titanium",
                "landscape",
                "iPhone 16 Pro Max"
            ),
            (
                "iphone-island",
                "black-titanium",
                "portrait",
                "iPhone 16 Pro"
            ),
            (
                "iphone-island",
                "black-titanium",
                "portrait",
                "iPhone 16 Pro Max"
            ),
            (
                "iphone-island",
                "desert-titanium",
                "portrait",
                "iPhone 16 Pro Max"
            ),
            ("macbook", "space-black", "landscape", "MacBook Pro 14 - M4"),
        ]
    );
    let superseded: Vec<_> = plan
        .items
        .iter()
        .filter(|i| matches!(i.action, Action::Superseded { .. }))
        .map(|i| i.file.as_str())
        .collect();
    // Only a true duplicate (same vendor model) is superseded.
    assert_eq!(
        superseded,
        ["Small/iPhone 16 Pro - Black Titanium - Portrait.png"]
    );
    assert_eq!(plan.unmatched.len(), 1);
    assert_eq!(
        plan.unmatched[0].name,
        "Apple Vision Pro - Space Gray - Portrait"
    );
    for i in plan.installs() {
        assert!(i.warnings.is_empty(), "{}: {:?}", i.name, i.warnings);
    }
    // A dry run writes nothing.
    assert!(!store_dir.path().join("apple").exists());
}

#[test]
fn apply_writes_the_store_and_index() {
    let (dir, specs) = package_dir();
    let store_dir = tempfile::tempdir().unwrap();
    let store = FrameStore::new(store_dir.path().join("frames"));
    let pkg = Package::open(dir.path()).unwrap();
    let plan = import(&store, &pkg, apple(), true).unwrap();
    assert!(plan.applied);
    let root = store_dir.path().join("frames").join("apple");
    let slot = root
        .join("iphone-island")
        .join("black-titanium")
        .join("portrait");
    let bezel = slot.join("iphone-16-pro-max.png");
    assert!(bezel.is_file());
    assert!(slot.join("iphone-16-pro-max.mask.png").is_file());
    assert!(slot.join("iphone-16-pro.png").is_file());
    assert!(slot.join("iphone-16-pro.mask.png").is_file());
    // The bezel is copied unchanged.
    let original = std::fs::read(
        dir.path()
            .join("iPhone 16 Pro Max")
            .join("iPhone 16 Pro Max - Black Titanium - Portrait.png"),
    )
    .unwrap();
    assert_eq!(std::fs::read(&bezel).unwrap(), original);

    let index = store.index();
    assert_eq!(index.frames("apple").len(), 7);
    let pro = index
        .frames("apple")
        .iter()
        .find(|f| f.apple_model == "iPhone 16 Pro" && f.orientation == Orientation::Portrait)
        .unwrap();
    assert_eq!(pro.screen, specs[3].screen, "the full-size file won");
    assert_eq!(pro.license.as_deref(), Some("License.rtf"));
    let yaml = std::fs::read_to_string(root.join(INDEX_FILE)).unwrap();
    assert!(yaml.contains("apple_model: iPhone 16 Pro Max"), "{yaml}");
    assert!(yaml.contains("license: License.rtf"), "{yaml}");
    let f = index
        .find(
            "apple",
            "iphone-island",
            Some("black-titanium"),
            None,
            Orientation::Portrait,
        )
        .unwrap();
    assert_eq!(f.screen, specs[0].screen);
    assert_eq!(f.size, PixelSize::new(specs[0].width, specs[0].height));
    assert_eq!(f.color_label, "Black Titanium");
    assert_eq!(f.package, dir.path().file_name().unwrap().to_string_lossy());
    assert_eq!(f.apple_model, "iPhone 16 Pro Max", "find = the largest");
    // Without a colour: the default if installed, else the first.
    let first = index
        .find(
            "apple",
            "iphone-island",
            None,
            Some("nope"),
            Orientation::Portrait,
        )
        .unwrap();
    assert_eq!(first.color, "black-titanium");
    let d = index
        .find(
            "apple",
            "iphone-island",
            None,
            Some("desert-titanium"),
            Orientation::Portrait,
        )
        .unwrap();
    assert_eq!(d.color, "desert-titanium");
    assert_eq!(
        index.colors("apple", "iphone-island"),
        [
            ("black-titanium".to_owned(), "Black Titanium".to_owned()),
            ("desert-titanium".to_owned(), "Desert Titanium".to_owned())
        ]
    );
    // A second import replaces.
    let again = import(&store, &pkg, apple(), false).unwrap();
    assert!(again.installs().all(|i| i.action == Action::Replace));

    // Remove one vendor model of one slot, one colour, then everything.
    let filter = RemoveFilter {
        model: Some("iphone-island"),
        color: Some("black-titanium"),
        orientation: Some(Orientation::Portrait),
        bezel: Some("iphone-16-pro"),
    };
    assert_eq!(store.remove("apple", &filter).unwrap(), 1);
    assert!(!slot.join("iphone-16-pro.png").exists());
    assert!(bezel.is_file());
    let filter = RemoveFilter {
        model: Some("iphone-island"),
        color: Some("desert-titanium"),
        ..RemoveFilter::default()
    };
    assert_eq!(store.remove("apple", &filter).unwrap(), 1);
    assert!(!root.join("iphone-island").join("desert-titanium").exists());
    assert_eq!(store.remove("apple", &RemoveFilter::default()).unwrap(), 5);
    assert!(store.index().is_empty());
    assert!(!root.join("iphone-island").exists());
}

#[test]
fn user_mapping_assigns_and_ignores() {
    let (dir, _) = package_dir();
    let store_dir = tempfile::tempdir().unwrap();
    let store = FrameStore::new(store_dir.path());
    let mut mapping = UserMapping::default();
    mapping.files.insert(
        "Apple Vision Pro - Space Gray - Portrait".into(),
        Some(Assignment {
            model: "iphone-island".into(),
            color: "Space Gray".into(),
            orientation: Orientation::Portrait,
        }),
    );
    mapping
        .files
        .insert("iPhone 16 Pro - Black Titanium - Portrait".into(), None);
    store.save_user_mapping("apple", &mapping).unwrap();
    let pkg = Package::open(dir.path()).unwrap();
    let plan = import(&store, &pkg, apple(), false).unwrap();
    let user = plan
        .items
        .iter()
        .find(|i| i.matched.rule.is_none())
        .unwrap();
    assert_eq!(user.matched.color, "space-gray");
    assert_eq!(user.action, Action::Install);
    assert!(plan.unmatched.iter().all(|u| u.ignored));
}

#[test]
fn zip_packages_are_read() {
    let dir = tempfile::tempdir().unwrap();
    let zip =
        synthetic::write_zip(&dir.path().join("Bezels.zip"), &synthetic::default_specs()).unwrap();
    let pkg = Package::open(&zip).unwrap();
    assert_eq!(pkg.kind, PackageKind::Zip);
    assert_eq!(pkg.entries.len(), 9, "resource forks are ignored");
    assert_eq!(pkg.skipped, 1);
    assert_eq!(pkg.psd, 2);
    let store_dir = tempfile::tempdir().unwrap();
    let store = FrameStore::new(store_dir.path());
    let plan = import(&store, &pkg, apple(), true).unwrap();
    assert_eq!(plan.installs().count(), 7);
    assert_eq!(plan.package, "Bezels.zip");
    assert_eq!(plan.licenses, ["Bezels.zip/Bezels/License.rtf"]);
    assert_eq!(store.index().frames("apple").len(), 7);
}

#[test]
fn other_inputs_are_rejected() {
    let dir = tempfile::tempdir().unwrap();
    let png = dir.path().join("x.png");
    std::fs::write(&png, b"x").unwrap();
    assert!(
        Package::open(&png)
            .unwrap_err()
            .to_string()
            .contains("whole bezel package")
    );
    let txt = dir.path().join("x.txt");
    std::fs::write(&txt, b"x").unwrap();
    assert!(Package::open(&txt).is_err());
    #[cfg(not(target_os = "macos"))]
    {
        let dmg = dir.path().join("x.dmg");
        std::fs::write(&dmg, b"x").unwrap();
        assert!(
            Package::open(&dmg)
                .unwrap_err()
                .to_string()
                .contains("extract")
        );
    }
}

#[test]
fn today_is_a_date() {
    let t = today();
    assert_eq!(t.len(), 10);
    assert!(t.starts_with("20"));
}

fn frame(apple_model: &str, screen: (u32, u32), priority: i32) -> InstalledFrame {
    InstalledFrame {
        model: "iphone-island".into(),
        color: "black".into(),
        color_label: "Black".into(),
        orientation: Orientation::Portrait,
        apple_model: apple_model.into(),
        file: String::new(),
        mask: String::new(),
        size: PixelSize::new(screen.0 + 150, screen.1 + 150),
        screen: PxRect {
            x: 75,
            y: 75,
            w: screen.0,
            h: screen.1,
        },
        radius: 0.0,
        priority,
        source_file: String::new(),
        package: String::new(),
        license: None,
        imported: String::new(),
        swatch: None,
    }
}

#[test]
fn bezel_selection() {
    let max = frame("iPhone 18 Pro Max", (1320, 2868), 30);
    let pro = frame("iPhone 18 Pro", (1206, 2622), 20);
    let old = frame("iPhone 15", (1179, 2556), 20);
    let c = [&max, &pro, &old];
    let px = |w, h| Some(PixelSize::new(w, h));
    let pick = |pin, target, shot| {
        let (f, why) = select_bezel(&c, pin, target, shot).unwrap();
        (f.apple_model.as_str(), why)
    };
    // By the target's page size first (6.3" slot → Pro), either orientation.
    assert_eq!(
        pick(None, px(1206, 2622), px(1320, 2868)),
        ("iPhone 18 Pro", BezelChoice::Target)
    );
    assert_eq!(
        pick(None, px(2868, 1320), None),
        ("iPhone 18 Pro Max", BezelChoice::Target)
    );
    // Then by the screenshot's pixel size.
    assert_eq!(
        pick(None, px(1080, 1920), px(1179, 2556)),
        ("iPhone 15", BezelChoice::Screenshot)
    );
    assert_eq!(
        pick(None, None, px(1206, 2622)),
        ("iPhone 18 Pro", BezelChoice::Screenshot)
    );
    // Aspect: 1206:2622 and 1320:2868 are within 0.1 % → priority wins;
    // 1179:2556 (0.4613) is further away.
    assert_eq!(
        pick(None, px(603, 1311), None),
        ("iPhone 18 Pro Max", BezelChoice::Aspect)
    );
    // Nothing close (iPad aspect): the largest, a fallback.
    assert_eq!(
        pick(None, px(2064, 2752), None),
        ("iPhone 18 Pro Max", BezelChoice::Largest)
    );
    assert_eq!(
        pick(None, None, None),
        ("iPhone 18 Pro Max", BezelChoice::Largest)
    );
    // Pinned, case-insensitive or as slug; unknown pins fall through.
    assert_eq!(
        pick(Some("iphone 15"), px(1320, 2868), None),
        ("iPhone 15", BezelChoice::Pinned)
    );
    assert_eq!(
        pick(Some("iphone-18-pro"), None, None),
        ("iPhone 18 Pro", BezelChoice::Pinned)
    );
    assert_eq!(
        pick(Some("iPhone 99"), px(1206, 2622), None),
        ("iPhone 18 Pro", BezelChoice::Target)
    );
    // Deterministic ties: same size and priority → name order.
    let a = frame("B model", (100, 200), 0);
    let b = frame("A model", (100, 200), 0);
    let (f, _) = select_bezel(&[&a, &b], None, None, None).unwrap();
    assert_eq!(f.apple_model, "A model");
    let (f, _) = select_bezel(&[&b, &a], None, None, None).unwrap();
    assert_eq!(f.apple_model, "A model");
    assert!(select_bezel(&[], None, None, None).is_none());
}

#[test]
fn legacy_store_is_migrated() {
    // A store written before vendor models existed:
    // `<model>/<colour>/<orientation>.png`, no `apple_model`.
    let store_dir = tempfile::tempdir().unwrap();
    let store = FrameStore::new(store_dir.path());
    let src = store_dir.path().join("apple");
    let old_dir = src.join("iphone-island").join("black");
    std::fs::create_dir_all(&old_dir).unwrap();
    std::fs::write(old_dir.join("portrait.png"), b"bezel").unwrap();
    std::fs::write(old_dir.join("portrait.mask.png"), b"mask").unwrap();
    std::fs::write(
        src.join(INDEX_FILE),
        "frames:\n- model: iphone-island\n  color: black\n  color_label: Black\n  \
         orientation: portrait\n  file: iphone-island/black/portrait.png\n  \
         mask: iphone-island/black/portrait.mask.png\n  size: [1520, 3068]\n  \
         screen: {x: 100, y: 100, w: 1320, h: 2868}\n  radius: 160\n  \
         source_file: iPhone 18 Pro Max - Black - Portrait.png\n  \
         package: Bezel-iPhone-18\n  imported: 2026-11-10\n\
         - model: iphone-island\n  color: gone\n  color_label: Gone\n  \
         orientation: portrait\n  file: iphone-island/gone/portrait.png\n  \
         mask: iphone-island/gone/portrait.mask.png\n  size: [10, 10]\n  \
         screen: {x: 1, y: 1, w: 8, h: 8}\n  radius: 1\n  \
         source_file: iPhone 18 Pro Max - Gone - Portrait.png\n  \
         package: x\n  imported: 2026-11-10\n",
    )
    .unwrap();
    // Read-only users see the vendor model at once (files stay).
    let index = FrameIndex::load(store_dir.path());
    let f = &index.frames("apple")[0];
    assert_eq!(f.apple_model, "iPhone 18 Pro Max");
    assert_eq!(f.file, "iphone-island/black/portrait.png");
    // Migration moves the files and drops entries without files.
    let migrated = store.migrate("apple").unwrap();
    assert_eq!(migrated.frames.len(), 1);
    let new = old_dir.join("portrait").join("iphone-18-pro-max.png");
    assert_eq!(std::fs::read(&new).unwrap(), b"bezel");
    assert!(
        old_dir
            .join("portrait")
            .join("iphone-18-pro-max.mask.png")
            .is_file()
    );
    assert!(!old_dir.join("portrait.png").exists());
    let index = store.index();
    assert_eq!(
        index.frames("apple")[0].file,
        "iphone-island/black/portrait/iphone-18-pro-max.png"
    );
    // Idempotent.
    assert_eq!(
        store.migrate("apple").unwrap(),
        store.source_index("apple").unwrap()
    );
    // A later install of the other size keeps the migrated one.
    let mut pro = frame("iPhone 18 Pro", (1206, 2622), 0);
    pro.color = "black".into();
    store.install("apple", pro, b"pro", b"pro-mask").unwrap();
    assert_eq!(store.index().frames("apple").len(), 2);
}

#[test]
fn dmg_licence_agreement_is_explained() {
    let dmg = std::path::Path::new("/Users/me/Downloads/Bezel-iPhone-18.dmg");
    let e = attach_error(dmg, "hdiutil: attach canceled\n");
    assert_eq!(
        e,
        PackageError::LicenseAgreement {
            path: dmg.to_owned(),
            volume: std::path::Path::new("/Volumes").join("Bezel-iPhone-18"),
        }
    );
    let msg = e.to_string();
    assert!(msg.contains("licence agreement"), "{msg}");
    assert!(msg.contains("accept the licence"), "{msg}");
    assert!(msg.contains("Bezel-iPhone-18"), "{msg}");
    // Other failures keep hdiutil's message.
    let other = attach_error(dmg, "hdiutil: attach failed - image not recognized");
    assert!(matches!(other, PackageError::Other(ref m) if m.contains("not recognized")));
    // `hdiutil imageinfo -plist`.
    let with = "<dict><key>Properties</key><dict>\n\t<key>Software License Agreement</key>\n\t<true/>\n</dict></dict>";
    let without = with.replace("<true/>", "<false/>");
    assert!(imageinfo_has_sla(with));
    assert!(!imageinfo_has_sla(&without));
    assert!(!imageinfo_has_sla("<dict></dict>"));
}

#[test]
fn verified_rules_need_no_warning() {
    // Only iPhone 15+ files (verified rules): no "unverified" note.
    let dir = tempfile::tempdir().unwrap();
    let specs: Vec<_> = synthetic::default_specs()
        .into_iter()
        .filter(|s| s.file.starts_with("iPhone"))
        .collect();
    synthetic::write_folder(dir.path(), &specs).unwrap();
    let store_dir = tempfile::tempdir().unwrap();
    let store = FrameStore::new(store_dir.path());
    let plan = import(&store, &Package::open(dir.path()).unwrap(), apple(), false).unwrap();
    assert_eq!(plan.installs().count(), 5);
    assert!(!plan.unverified_rules);
    assert!(plan.items.iter().all(|i| i.matched.verified));
}

/// A bezel like Apple's: coloured metal rim (`rim` px, with a lighter
/// highlight line and a darker shadow line), a near-black glass border, a
/// transparent screen.
fn rimmed_bezel(rim_color: [u8; 3], rim: u32) -> (RgbaImage, PxRect) {
    let (w, h, m) = (300u32, 600u32, 60u32);
    let screen = PxRect {
        x: m,
        y: m,
        w: w - 2 * m,
        h: h - 2 * m,
    };
    let mut data = Vec::with_capacity((w * h * 4) as usize);
    for y in 0..h {
        for x in 0..w {
            let edge = x.min(y).min(w - 1 - x).min(h - 1 - y);
            let px = if edge >= m {
                [0, 0, 0, 0]
            } else if edge == 0 {
                [rim_color[0] / 2, rim_color[1] / 2, rim_color[2] / 2, 140]
            } else if edge == 1 {
                [255, 255, 255, 255]
            } else if edge == rim - 1 {
                [0, 0, 0, 255]
            } else if edge < rim {
                [rim_color[0], rim_color[1], rim_color[2], 255]
            } else {
                [6, 6, 8, 255]
            };
            data.extend_from_slice(&px);
        }
    }
    (
        RgbaImage {
            width: w,
            height: h,
            data,
        },
        screen,
    )
}

#[test]
fn swatch_is_the_rim_colour_not_the_glass() {
    for (rgb, rim) in [
        ([0x2E, 0x2E, 0x30], 14),
        ([0x5C, 0x1E, 0x2B], 10),
        ([0xC4, 0xD5, 0xE2], 18),
        ([0xDA, 0xDB, 0xDD], 6),
    ] {
        let (img, screen) = rimmed_bezel(rgb, rim);
        let got = body_swatch(&img, screen);
        let off = (0..3).map(|i| got[i].abs_diff(rgb[i])).max().unwrap();
        assert!(off <= 3, "{rgb:?} rim {rim}: got {got:?}");
        assert_eq!(analyze(&img).unwrap().swatch, got);
    }
    // An all-dark bezel (rim as dark as the glass) still gives a dark colour.
    let (img, screen) = rimmed_bezel([12, 12, 14], 10);
    let got = body_swatch(&img, screen);
    assert!(got.iter().all(|&c| c < 20), "{got:?}");
}

#[test]
fn swatches_are_measured_and_filled_in() {
    let (dir, _) = package_dir();
    let store_dir = tempfile::tempdir().unwrap();
    let store = FrameStore::new(store_dir.path().join("frames"));
    let pkg = Package::open(dir.path()).unwrap();
    import(&store, &pkg, apple(), true).unwrap();
    let root = store_dir.path().join("frames").join("apple");
    let index = store.index();
    let yaml = std::fs::read_to_string(root.join(INDEX_FILE)).unwrap();
    // Swatches measured at import (uniform synthetic bezels: their colour).
    let colors = index.installed_colors("apple", "iphone-island");
    assert_eq!(colors[0].swatch.as_deref(), Some("#2C2C30"));
    assert_eq!(colors[1].swatch.as_deref(), Some("#BFA48F"));
    assert!(
        yaml.contains("swatch: '#2C2C30'") || yaml.contains("swatch: \"#2C2C30\""),
        "{yaml}"
    );
    // An unknown colour falls back like no colour.
    let fb = index.candidates(
        "apple",
        "iphone-island",
        Some("cosmic-orange"),
        Some("desert-titanium"),
        Orientation::Portrait,
    );
    assert!(fb.iter().all(|f| f.color == "desert-titanium") && !fb.is_empty());

    // Stores without swatches get them filled in.
    let path = root.join(INDEX_FILE);
    let stripped: String = std::fs::read_to_string(&path)
        .unwrap()
        .lines()
        .filter(|l| !l.trim_start().starts_with("swatch:"))
        .fold(String::new(), |mut out, l| {
            out.push_str(l);
            out.push('\n');
            out
        });
    std::fs::write(&path, stripped).unwrap();
    assert!(
        store.index().installed_colors("apple", "iphone-island")[0]
            .swatch
            .is_none()
    );
    assert_eq!(store.fill_swatches("apple").unwrap(), 7);
    assert_eq!(store.fill_swatches("apple").unwrap(), 0);
    assert_eq!(
        store.index().installed_colors("apple", "iphone-island")[0]
            .swatch
            .as_deref(),
        Some("#2C2C30")
    );
}
