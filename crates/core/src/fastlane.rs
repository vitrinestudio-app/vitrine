//! fastlane export layout: where an export image goes so `fastlane deliver`
//! (iOS) and `fastlane supply` (Android) upload it as it is, and which
//! combinations would clash (conventions in `data/fastlane.yaml`, see
//! `catalog::fastlane`).
//!
//! | store slot | path below the export folder |
//! |---|---|
//! | Apple screenshot set | `screenshots/<asc-locale>/<nn>_<target>.<ext>` |
//! | Apple iMessage set (`IMESSAGE_…`) | `screenshots/iMessage/<asc-locale>/<nn>_<target>.<ext>` |
//! | Google screenshots (`phoneScreenshots` …) | `metadata/android/<play-locale>/images/<type>/<nn>_<play-locale>.<ext>` |
//! | Google single image (`featureGraphic` …) | `metadata/android/<play-locale>/images/<type>.<ext>` |
//!
//! `nn` is the 1-based image number with two digits, so deliver's natural
//! sort and supply's byte sort both keep the page order. Store locales come
//! from `store.locales` in `project.yaml`, else the built-in defaults
//! (`data/store-locales.yaml`). For the shared 12.9" iPad size the file
//! name carries deliver's marker when the slot is `APP_IPAD_PRO_129`.
//!
//! Not placed (with a note): targets without a store slot, App Store
//! creative assets (header / search results; deliver uploads screenshot
//! sets only), Google image types supply does not upload, sizes deliver
//! rejects, languages without a store locale.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::catalog::{self, fastlane};
use crate::model::{Project, StoreKind, Target, TargetKind};

/// Severity of a [`LayoutNote`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NoteLevel {
    Info,
    Warn,
    Error,
}

/// Something the layout left out or that deliver/supply would trip over.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct LayoutNote {
    pub level: NoteLevel,
    pub target: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lang: Option<String>,
    pub message: String,
}

impl LayoutNote {
    fn new(level: NoteLevel, target: &str, lang: Option<&str>, message: String) -> Self {
        Self {
            level,
            target: target.to_owned(),
            lang: lang.map(str::to_owned),
            message,
        }
    }
}

/// How the files of a placement are named.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Naming {
    /// `<nn>_<target>[_<marker>].<ext>`.
    Deliver {
        target: String,
        marker: Option<String>,
    },
    /// `<nn>_<locale>.<ext>`.
    SupplyScreenshot { locale: String },
    /// `<image_type>.<ext>` (one image only).
    SupplyImage { image_type: String },
}

/// Where the images of one target × language go.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Placement {
    pub store: StoreKind,
    /// Folder below the export folder (`/`-separated).
    pub dir: String,
    /// Store locale code of the folder.
    pub locale: String,
    /// Display type / image type.
    pub slot: String,
    pub naming: Naming,
}

impl Placement {
    /// Path of output image `image` (0-based) with extension `ext`.
    pub fn path(&self, image: u32, ext: &str) -> String {
        let name = match &self.naming {
            Naming::Deliver { target, marker } => match marker {
                Some(m) => format!("{:02}_{target}_{m}.{ext}", image + 1),
                None => format!("{:02}_{target}.{ext}", image + 1),
            },
            Naming::SupplyScreenshot { locale } => format!("{:02}_{locale}.{ext}", image + 1),
            Naming::SupplyImage { image_type } => format!("{image_type}.{ext}"),
        };
        format!("{}/{name}", self.dir)
    }

    /// Only the first output image is placed (single store image).
    pub fn single_image(&self) -> bool {
        matches!(self.naming, Naming::SupplyImage { .. })
    }
}

/// Placements of `target` in language `lang` (one per store slot) and the
/// notes about what is left out.
pub fn placements(
    project: &Project,
    target: &Target,
    lang: &str,
) -> (Vec<Placement>, Vec<LayoutNote>) {
    let mut out = Vec::new();
    let mut notes = Vec::new();
    let Some(slot) = &target.store else {
        notes.push(LayoutNote::new(
            NoteLevel::Info,
            target.id.as_str(),
            None,
            "no store slot: not part of the fastlane layout".to_owned(),
        ));
        return (out, notes);
    };
    if let Some(apple) = &slot.apple {
        match deliver_placement(project, target, lang, apple) {
            Ok((p, n)) => {
                out.push(p);
                notes.extend(n);
            }
            Err(n) => notes.push(n),
        }
    }
    if let Some(google) = &slot.google {
        match supply_placement(project, target, lang, &google.image_type) {
            Ok(p) => out.push(p),
            Err(n) => notes.push(n),
        }
    }
    (out, notes)
}

/// deliver placement of an App Store slot (with warnings), or why none.
fn deliver_placement(
    project: &Project,
    target: &Target,
    lang: &str,
    apple: &crate::model::AppleSlot,
) -> Result<(Placement, Option<LayoutNote>), LayoutNote> {
    let fl = fastlane();
    let tid = target.id.as_str();
    let dt = apple.display_type.as_str();
    let general = |level, msg: String| LayoutNote::new(level, tid, None, msg);
    let note = |level, msg: String| LayoutNote::new(level, tid, Some(lang), msg);
    if apple.placement.is_some() {
        return Err(general(
            NoteLevel::Info,
            format!("App Store creative asset ({dt}): deliver uploads screenshot sets only"),
        ));
    }
    if target.kind == TargetKind::Banner {
        return Err(general(
            NoteLevel::Info,
            "banner: deliver uploads screenshot sets only".to_owned(),
        ));
    }
    if !fl.deliver.knows(dt) {
        return Err(general(
            NoteLevel::Warn,
            format!("deliver has no screenshot set for display type {dt}"),
        ));
    }
    let (code, _) = catalog::store_locales()
        .apple
        .resolve(project.store.locales.get(StoreKind::Apple), lang)
        .ok_or_else(|| {
            note(
                NoteLevel::Warn,
                format!("no App Store locale for `{lang}` (set store.locales.apple)"),
            )
        })?;
    let code = fl.deliver.locale(code).ok_or_else(|| {
        note(
            NoteLevel::Warn,
            format!("deliver does not accept the locale folder `{code}`"),
        )
    })?;
    let imessage = dt.starts_with("IMESSAGE_");
    let dir = if imessage {
        format!(
            "{}/{}/{code}",
            fl.deliver.screenshots_dir, fl.deliver.imessage_dir
        )
    } else {
        format!("{}/{code}", fl.deliver.screenshots_dir)
    };
    let (w, h) = (target.size.w, target.size.h);
    let marker = fl
        .deliver
        .marker_for(w, h, dt)
        .map(|words| words.join("_"))
        .filter(|m| !tid.to_lowercase().contains(&m.to_lowercase()));
    let p = Placement {
        store: StoreKind::Apple,
        dir,
        locale: code.to_owned(),
        slot: dt.to_owned(),
        naming: Naming::Deliver {
            target: tid.to_owned(),
            marker,
        },
    };
    let found = fl
        .deliver
        .display_type(w, h, &p.path(0, "png"), imessage)
        .ok_or_else(|| {
            note(
                NoteLevel::Warn,
                format!(
                    "deliver rejects {w}×{h} (no display type has this size) and would stop the whole upload: not written"
                ),
            )
        })?;
    let warn = (found != dt).then(|| {
        note(
            NoteLevel::Warn,
            format!("deliver detects {w}×{h} as {found}, not the target's {dt}"),
        )
    });
    Ok((p, warn))
}

/// supply placement of a Google Play image type, or why none.
fn supply_placement(
    project: &Project,
    target: &Target,
    lang: &str,
    ty: &str,
) -> Result<Placement, LayoutNote> {
    let fl = fastlane();
    let tid = target.id.as_str();
    let screenshot = fl.supply.screenshot_type(ty);
    if screenshot.is_none() && !fl.supply.image_types.iter().any(|t| t == ty) {
        return Err(LayoutNote::new(
            NoteLevel::Info,
            tid,
            None,
            format!("supply does not upload the image type {ty}"),
        ));
    }
    let (code, _) = catalog::store_locales()
        .google
        .resolve(project.store.locales.get(StoreKind::Google), lang)
        .ok_or_else(|| {
            LayoutNote::new(
                NoteLevel::Warn,
                tid,
                Some(lang),
                format!("no Google Play locale for `{lang}` (set store.locales.google)"),
            )
        })?;
    let base = format!("{}/{code}/{}", fl.supply.metadata_dir, fl.supply.images_dir);
    let (dir, naming) = match screenshot {
        Some(t) => (
            format!("{base}/{t}"),
            Naming::SupplyScreenshot {
                locale: code.to_owned(),
            },
        ),
        None => (
            base,
            Naming::SupplyImage {
                image_type: ty.to_owned(),
            },
        ),
    };
    Ok(Placement {
        store: StoreKind::Google,
        dir,
        locale: code.to_owned(),
        slot: ty.to_owned(),
        naming,
    })
}

/// A file (written now or earlier) claiming a place in the layout.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Claim {
    pub path: String,
    pub target: String,
    pub lang: String,
    pub width: u32,
    pub height: u32,
}

/// The upload set a file of the layout lands in: deliver's display type
/// per locale folder, supply's image folder (or single image).
pub fn set_of(path: &str, width: u32, height: u32) -> Option<String> {
    let fl = fastlane();
    let (dir, _) = path.rsplit_once('/')?;
    let shots = format!("{}/", fl.deliver.screenshots_dir);
    if let Some(rest) = dir.strip_prefix(&shots) {
        let imessage = rest
            .split('/')
            .next()
            .is_some_and(|d| d.eq_ignore_ascii_case(&fl.deliver.imessage_dir));
        let dt = fl
            .deliver
            .display_type(width, height, path, imessage)
            .unwrap_or_else(|| format!("{width}x{height}"));
        return Some(format!("{dir} {dt}"));
    }
    if path.starts_with(&format!("{}/", fl.supply.metadata_dir)) {
        let images = format!("/{}", fl.supply.images_dir);
        return Some(if dir.ends_with(&images) {
            path.to_owned()
        } else {
            dir.to_owned()
        });
    }
    None
}

/// Checks the claims of an export: `existing` (files kept from earlier
/// exports, outside this export's targets × languages) come first, then
/// `new` in order. A set belongs to the first target × language claiming
/// it; later claims of another target or language are rejected (their
/// indices into `new` are returned) with an error note: deliver/supply
/// would mix them into one set. Sets over the store's count limit get a
/// warning.
pub fn check_claims(existing: &[Claim], new: &[Claim]) -> (Vec<usize>, Vec<LayoutNote>) {
    let fl = fastlane();
    let mut owner: BTreeMap<String, (String, String)> = BTreeMap::new();
    let mut count: BTreeMap<String, usize> = BTreeMap::new();
    let mut rejected = Vec::new();
    let mut notes: Vec<LayoutNote> = Vec::new();
    let all = existing
        .iter()
        .map(|c| (None, c))
        .chain(new.iter().enumerate().map(|(i, c)| (Some(i), c)));
    for (idx, c) in all {
        let Some(set) = set_of(&c.path, c.width, c.height) else {
            continue;
        };
        let me = (c.target.clone(), c.lang.clone());
        match owner.get(&set) {
            Some(o) if *o != me => {
                if let Some(i) = idx {
                    rejected.push(i);
                    let msg = format!(
                        "conflict: `{}` ({}) would land in the same upload set as `{}` ({}): {set}; not written",
                        c.target, c.lang, o.0, o.1
                    );
                    if !notes.iter().any(|n| n.message == msg) {
                        notes.push(LayoutNote::new(
                            NoteLevel::Error,
                            &c.target,
                            Some(&c.lang),
                            msg,
                        ));
                    }
                }
                continue;
            }
            Some(_) => {}
            None => {
                owner.insert(set.clone(), me);
            }
        }
        *count.entry(set).or_default() += 1;
    }
    for (set, n) in count {
        let max = if set.starts_with(&fl.deliver.screenshots_dir) {
            fl.deliver.max_per_set
        } else {
            fl.supply.max_per_type
        };
        if n > max {
            let (t, l) = owner.get(&set).cloned().unwrap_or_default();
            notes.push(LayoutNote::new(
                NoteLevel::Warn,
                &t,
                Some(&l),
                format!("{n} images in {set}: the store takes at most {max}"),
            ));
        }
    }
    (rejected, notes)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn project() -> Project {
        let yaml = "format: vitrine/1
revision: 1
name: t
languages:
  default: en
  available: [en, de, fr-CA, xx]
canvases:
- id: s
  page: {aspect: 1000:2000, count: 2}
targets:
- {id: phone, canvas: s, size: [1320, 2868], store: {apple: {display_type: APP_IPHONE_67}}}
- {id: phone-old, canvas: s, size: [1290, 2796], store: {apple: {display_type: APP_IPHONE_67}}}
- {id: pad-old, canvas: s, size: [2048, 2732], store: {apple: {display_type: APP_IPAD_PRO_129}}}
- {id: play, canvas: s, size: [1080, 2160], store: {google: {image_type: phoneScreenshots}}}
- {id: fg, kind: banner, canvas: s, size: [1024, 500], store: {google: {image_type: featureGraphic}}}
- {id: odd, canvas: s, size: [1000, 1000], store: {apple: {display_type: APP_IPHONE_67}}}
- {id: web, canvas: s, size: [1920, 1080]}
store:
  locales:
    google: {fr-CA: fr-CA}
";
        crate::io::project_from_yaml(yaml).unwrap_or_else(|e| panic!("{e}"))
    }

    fn place(p: &Project, t: &str, lang: &str) -> (Vec<Placement>, Vec<LayoutNote>) {
        placements(p, p.target(t).unwrap(), lang)
    }

    #[test]
    fn paths_per_store() {
        let p = project();
        let (pl, notes) = place(&p, "phone", "de");
        assert_eq!(notes, []);
        assert_eq!(pl[0].path(0, "png"), "screenshots/de-DE/01_phone.png");
        assert_eq!(pl[0].path(9, "jpg"), "screenshots/de-DE/10_phone.jpg");
        let (pl, _) = place(&p, "phone", "fr-CA");
        assert_eq!(pl[0].path(1, "png"), "screenshots/fr-CA/02_phone.png");
        let (pl, _) = place(&p, "play", "de");
        assert_eq!(
            pl[0].path(2, "png"),
            "metadata/android/de-DE/images/phoneScreenshots/03_de-DE.png"
        );
        let (pl, _) = place(&p, "fg", "en");
        assert_eq!(
            pl[0].path(0, "jpg"),
            "metadata/android/en-US/images/featureGraphic.jpg"
        );
        assert!(pl[0].single_image());
        // The shared 12.9" size needs deliver's marker for the 2nd gen slot.
        let (pl, notes) = place(&p, "pad-old", "en");
        assert_eq!(notes, []);
        assert_eq!(
            pl[0].path(0, "png"),
            "screenshots/en-US/01_pad-old_app_ipad_pro_129.png"
        );
    }

    #[test]
    fn notes_for_what_is_left_out() {
        let p = project();
        let (pl, notes) = place(&p, "web", "en");
        assert_eq!(pl.len(), 0);
        assert!(notes[0].message.contains("no store slot"), "{notes:?}");
        let (pl, notes) = place(&p, "odd", "en");
        assert_eq!(pl.len(), 0);
        assert!(notes[0].message.contains("deliver rejects"), "{notes:?}");
        let (pl, notes) = place(&p, "phone", "xx");
        assert_eq!(pl.len(), 0);
        assert!(
            notes[0].message.contains("no App Store locale"),
            "{notes:?}"
        );
    }

    #[test]
    fn conflicts_and_limits() {
        let p = project();
        let claim = |t: &str, lang: &str, i: u32| {
            let (pl, _) = place(&p, t, lang);
            let tg = p.target(t).unwrap();
            Claim {
                path: pl[0].path(i, "png"),
                target: t.to_owned(),
                lang: lang.to_owned(),
                width: tg.size.w,
                height: tg.size.h,
            }
        };
        // Two 6.9" sizes in one locale folder: one set APP_IPHONE_67.
        let new = vec![
            claim("phone", "en", 0),
            claim("phone", "en", 1),
            claim("phone-old", "en", 0),
            claim("phone-old", "de", 0),
        ];
        let (rejected, notes) = check_claims(&[], &new);
        assert_eq!(rejected, vec![2]);
        assert_eq!(notes.len(), 1, "{notes:?}");
        assert_eq!(notes[0].level, NoteLevel::Error);
        assert!(notes[0].message.contains("APP_IPHONE_67"), "{notes:?}");
        // A file kept from an earlier export owns its set.
        let (rejected, _) = check_claims(&[claim("phone-old", "en", 0)], &new[..1]);
        assert_eq!(rejected, vec![0]);
        // Over the limit.
        let many: Vec<Claim> = (0..9).map(|i| claim("play", "en", i)).collect();
        let (rejected, notes) = check_claims(&[], &many);
        assert_eq!(rejected.len(), 0);
        assert!(notes[0].message.contains("at most 8"), "{notes:?}");
    }
}
