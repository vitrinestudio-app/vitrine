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
    assert_eq!(pkg.skipped, 1, "License.rtf");
    let plan = import(&store, &pkg, apple(), false).unwrap();
    assert!(!plan.applied);
    assert!(plan.unverified_rules);
    let mut installs: Vec<_> = plan
        .installs()
        .map(|i| {
            (
                i.matched.model.as_str(),
                i.matched.color.as_str(),
                orientation_name(i.matched.orientation),
            )
        })
        .collect();
    installs.sort_unstable();
    assert_eq!(
        installs,
        [
            ("ipad-modern", "space-black", "portrait"),
            ("iphone-island", "black-titanium", "landscape"),
            ("iphone-island", "black-titanium", "portrait"),
            ("iphone-island", "desert-titanium", "portrait"),
            ("macbook", "space-black", "landscape"),
        ]
    );
    let superseded: Vec<_> = plan
        .items
        .iter()
        .filter(|i| matches!(i.action, Action::Superseded { .. }))
        .map(|i| i.name.as_str())
        .collect();
    assert_eq!(superseded, ["iPhone 16 Pro - Black Titanium - Portrait"]);
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
    let bezel = root
        .join("iphone-island")
        .join("black-titanium")
        .join("portrait.png");
    assert!(bezel.is_file());
    assert!(
        root.join("iphone-island")
            .join("black-titanium")
            .join("portrait.mask.png")
            .is_file()
    );
    // The bezel is copied unchanged.
    let original = std::fs::read(
        dir.path()
            .join("iPhone 16 Pro Max")
            .join("iPhone 16 Pro Max - Black Titanium - Portrait.png"),
    )
    .unwrap();
    assert_eq!(std::fs::read(&bezel).unwrap(), original);

    let index = store.index();
    assert_eq!(index.frames("apple").len(), 5);
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

    // Remove one colour, then everything.
    assert_eq!(
        store
            .remove(
                "apple",
                Some("iphone-island"),
                Some("desert-titanium"),
                None
            )
            .unwrap(),
        1
    );
    assert!(!root.join("iphone-island").join("desert-titanium").exists());
    assert_eq!(store.remove("apple", None, None, None).unwrap(), 4);
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
    assert_eq!(pkg.entries.len(), 7, "resource forks are ignored");
    assert_eq!(pkg.skipped, 1);
    let store_dir = tempfile::tempdir().unwrap();
    let store = FrameStore::new(store_dir.path());
    let plan = import(&store, &pkg, apple(), true).unwrap();
    assert_eq!(plan.installs().count(), 5);
    assert_eq!(plan.package, "Bezels.zip");
    assert_eq!(store.index().frames("apple").len(), 5);
}

#[test]
fn other_inputs_are_rejected() {
    let dir = tempfile::tempdir().unwrap();
    let png = dir.path().join("x.png");
    std::fs::write(&png, b"x").unwrap();
    assert!(
        Package::open(&png)
            .unwrap_err()
            .contains("whole bezel package")
    );
    let txt = dir.path().join("x.txt");
    std::fs::write(&txt, b"x").unwrap();
    assert!(Package::open(&txt).is_err());
    #[cfg(not(target_os = "macos"))]
    {
        let dmg = dir.path().join("x.dmg");
        std::fs::write(&dmg, b"x").unwrap();
        assert!(Package::open(&dmg).unwrap_err().contains("extract"));
    }
}

#[test]
fn today_is_a_date() {
    let t = today();
    assert_eq!(t.len(), 10);
    assert!(t.starts_with("20"));
}
