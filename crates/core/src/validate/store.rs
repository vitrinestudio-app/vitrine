//! Store mapping checks (`store:` in `project.yaml`, Phase 5.1): explicit
//! locale codes, languages without or with a shared store locale, store
//! slots missing or used twice. Project-level findings; only for stores the
//! project is mapped to (`store.apple` / `store.google`), except the
//! explicit-map checks. Plus the sets App Store Connect and Google Play
//! require ([`missing_required_sets`]).

use std::collections::BTreeMap;

use super::{Ctx, FindingCode, Severity, Sink};
use crate::catalog::{Catalog, Platform, Requirement, Store, TargetPreset, store_locales};
use crate::model::{ApplePlatform, Lang, Project, StoreKind, StoreSlot, Target, TargetKind};

/// Slot value of `target` for `store` (`APP_IPHONE_67`, `phoneScreenshots`).
pub(crate) fn slot_value(target: &Target, store: StoreKind) -> Option<&str> {
    let slot = target.store.as_ref()?;
    match store {
        StoreKind::Apple => slot.apple.as_ref().map(|a| a.display_type.as_str()),
        StoreKind::Google => slot.google.as_ref().map(|g| g.image_type.as_str()),
    }
}

/// Languages a target exports (its `langs`, else all available).
fn langs_of<'a>(project: &'a Project, target: &'a Target) -> Vec<&'a str> {
    match &target.langs {
        Some(l) => l.iter().map(Lang::as_str).collect(),
        None => project
            .languages
            .available
            .iter()
            .map(Lang::as_str)
            .collect(),
    }
}

pub(super) fn store_checks(project: &Project, out: &mut Sink) {
    let lists = store_locales();
    let settings = &project.store;
    for store in StoreKind::ALL {
        let list = lists.store(store);
        let explicit = settings.locales.get(store);
        let root = Ctx::default();
        let finding = |level, code, lang: Option<&str>, message: String| {
            let mut f = root.finding(level, code, false, None, None, message);
            f.lang = lang.and_then(|l| Lang::parse(l).ok());
            f
        };
        for (lang, code) in explicit {
            if !list.is_known(code) {
                out.push(finding(
                    Severity::Error,
                    FindingCode::StoreLocaleUnknown,
                    Some(lang),
                    format!("store.locales.{store}: `{code}` is not a {store} store locale"),
                ));
            }
            if !project.languages.is_available(lang) {
                out.push(finding(
                    Severity::Info,
                    FindingCode::StoreLocaleUnused,
                    None,
                    format!(
                        "store.locales.{store}: `{lang}` is not a project language (entry unused)"
                    ),
                ));
            }
        }
        if !settings.has(store) {
            continue;
        }
        let targets: Vec<&Target> = project
            .targets
            .iter()
            .filter(|t| slot_value(t, store).is_some())
            .collect();
        if targets.is_empty() {
            out.push(finding(
                Severity::Warn,
                FindingCode::StoreNoSlot,
                None,
                format!(
                    "the project is mapped to {store} but no target has a {store} store slot \
                     (nothing to upload)"
                ),
            ));
            continue;
        }
        // Languages that get uploaded, in project order.
        let mut langs: Vec<&str> = Vec::new();
        for l in &project.languages.available {
            if targets
                .iter()
                .any(|t| langs_of(project, t).contains(&l.as_str()))
            {
                langs.push(l.as_str());
            }
        }
        let mut by_locale: BTreeMap<&str, Vec<&str>> = BTreeMap::new();
        for lang in langs {
            match list.resolve(explicit, lang) {
                Some((code, _)) if list.is_known(code) => {
                    by_locale.entry(code).or_default().push(lang);
                }
                Some(_) => {} // reported above
                None => out.push(finding(
                    Severity::Warn,
                    FindingCode::StoreLocaleMissing,
                    Some(lang),
                    format!(
                        "language `{lang}` has no {store} store locale; set one in \
                         store.locales.{store} (it is not uploaded otherwise)"
                    ),
                )),
            }
        }
        for (code, langs) in &by_locale {
            if langs.len() > 1 {
                out.push(finding(
                    Severity::Warn,
                    FindingCode::StoreLocaleDuplicate,
                    Some(langs[1]),
                    format!(
                        "languages {} all map to the {store} locale `{code}`; only one can be \
                         uploaded (map the others explicitly in store.locales.{store})",
                        langs
                            .iter()
                            .map(|l| format!("`{l}`"))
                            .collect::<Vec<_>>()
                            .join(", ")
                    ),
                ));
            }
        }
        slot_duplicates(project, store, &targets, out);
    }
}

/// Two targets filling the same slot of `store` for a shared language.
fn slot_duplicates(project: &Project, store: StoreKind, targets: &[&Target], out: &mut Sink) {
    for (i, a) in targets.iter().enumerate() {
        for b in &targets[i + 1..] {
            let slot = slot_value(a, store);
            if slot != slot_value(b, store) {
                continue;
            }
            let la = langs_of(project, a);
            let shared: Vec<&str> = langs_of(project, b)
                .into_iter()
                .filter(|l| la.contains(l))
                .collect();
            if shared.is_empty() {
                continue;
            }
            let mut f = Ctx::default().finding(
                Severity::Warn,
                FindingCode::StoreSlotDuplicate,
                false,
                None,
                None,
                format!(
                    "targets `{}` and `{}` both fill the {store} slot `{}` for {}; a store \
                     set can only be uploaded from one target per language",
                    a.id,
                    b.id,
                    slot.unwrap_or_default(),
                    shared.join(", ")
                ),
            );
            f.target = Some(b.id.clone());
            out.push(f);
        }
    }
}

/// A store set or graphic that `targets.yaml` marks `requirement:
/// required` for a platform the project targets, and that no target fills
/// (App Store: iPhone 6.3″, iPad 13″, Mac; Google Play: phone screenshots,
/// feature graphic).
#[derive(Clone, Copy, Debug)]
pub struct MissingStoreSet<'c> {
    /// The required preset (id = the target to add, label, slot). When
    /// several required presets share a slot (Play phone 9:16 and 1:2),
    /// the first one in the catalog.
    pub preset: &'c TargetPreset,
    /// Store of the slot.
    pub store: StoreKind,
    /// Why the platform counts: the project has targets of it (`true`) or
    /// only the store mapping names it (`false`).
    pub from_targets: bool,
}

impl MissingStoreSet<'_> {
    /// Slot of the missing set: Apple display type (`APP_IPHONE_61`) or
    /// Google Play image type (`featureGraphic`).
    pub fn slot(&self) -> &str {
        self.preset
            .slot
            .as_ref()
            .and_then(|s| preset_slot_value(s, self.store))
            .unwrap_or("")
    }

    /// Who needs the set, in words (`iPhone apps`).
    fn needed_by(&self) -> &'static str {
        match self.preset.platform {
            Platform::Ios => "iPhone apps",
            Platform::Ipados => "apps that run on iPad",
            Platform::Macos => "Mac apps",
            Platform::Watchos => "Apple Watch apps",
            Platform::Android => "Android apps",
            Platform::Wearos => "Wear OS apps",
            Platform::Chromeos => "Chromebook apps",
            Platform::Web => "this platform",
        }
    }

    /// The finding / readiness message: what is missing and how to add it.
    pub fn message(&self) -> String {
        let why = match (self.store, self.preset.platform, self.from_targets) {
            (StoreKind::Apple, _, false) => {
                " (the project is mapped to the App Store: store.apple.platform)"
            }
            (StoreKind::Google, _, false) => {
                " (the project is mapped to Google Play: store.google)"
            }
            (_, Platform::Ipados, true) => " (reported because the project has iPad targets)",
            (_, Platform::Macos, true) => " (reported because the project has Mac targets)",
            _ => "",
        };
        let requirer = match self.store {
            StoreKind::Apple => "App Store Connect",
            StoreKind::Google => "Google Play",
        };
        let slot = self.slot();
        let id = &self.preset.id;
        let label = &self.preset.label;
        let needed_by = self.needed_by();
        match self.preset.kind {
            TargetKind::Pages => format!(
                "no {label} set (`{slot}`), required by {requirer} for {needed_by}{why}: add the \
                 target `{id}` (Canvases & targets → Add target)"
            ),
            TargetKind::Banner => {
                let size = self
                    .preset
                    .default_size()
                    .map(|s| format!(", {}×{}", s.w, s.h))
                    .unwrap_or_default();
                format!(
                    "no {label} (`{slot}`{size}), required by {requirer} for {needed_by}{why}: \
                     add the banner target `{id}` (Canvases & targets → Add target, or a canvas \
                     → Banner from this canvas)"
                )
            }
        }
    }
}

/// Slot value of a preset slot for `store`.
fn preset_slot_value(slot: &StoreSlot, store: StoreKind) -> Option<&str> {
    match store {
        StoreKind::Apple => slot.apple.as_ref().map(|a| a.display_type.as_str()),
        StoreKind::Google => slot.google.as_ref().map(|g| g.image_type.as_str()),
    }
}

/// Catalog store of a model store.
fn catalog_store(store: StoreKind) -> Store {
    match store {
        StoreKind::Apple => Store::Apple,
        StoreKind::Google => Store::Google,
    }
}

/// Required store sets the project lacks, App Store and Google Play.
/// Data-driven: every preset with `requirement: required` and a store slot
/// is needed for its `platform` (iPhone `ios`, iPad `ipados`, Mac `macos`,
/// Android `android`) once the project targets that platform in that
/// store, and is filled by any target with the same slot value.
/// `conditional` and `optional` presets (Apple Watch, Play tablets,
/// Wear OS) are never asked for.
///
/// The project's platforms per store are those of the presets its
/// targets' slots of that store belong to (Play tablets count as
/// `android`). Whether an app runs on iPad (or Mac) cannot be known
/// otherwise, so those sets are only asked for when the project already
/// has such targets. A project mapped to the App Store (`store.apple`)
/// without iPhone or iPad targets also needs the sets of its
/// `store.apple.platform` (`ios` → iPhone, `macos` → Mac); one mapped to
/// Google Play (`store.google`) without Play targets those of `android`.
pub fn missing_required_sets<'c>(
    project: &Project,
    catalog: &'c Catalog,
) -> Vec<MissingStoreSet<'c>> {
    let mut out = Vec::new();
    for store in StoreKind::ALL {
        missing_for_store(project, catalog, store, &mut out);
    }
    out
}

fn missing_for_store<'c>(
    project: &Project,
    catalog: &'c Catalog,
    store: StoreKind,
    out: &mut Vec<MissingStoreSet<'c>>,
) {
    let presets: Vec<&TargetPreset> = catalog
        .targets
        .presets
        .iter()
        .filter(|p| p.store == catalog_store(store))
        .collect();
    let filled: Vec<&str> = project
        .targets
        .iter()
        .filter_map(|t| slot_value(t, store))
        .collect();
    let mut platforms: Vec<(Platform, bool)> = Vec::new();
    for value in &filled {
        for p in &presets {
            let slot = p.slot.as_ref().and_then(|s| preset_slot_value(s, store));
            if slot == Some(*value) && !platforms.iter().any(|(q, _)| *q == p.platform) {
                platforms.push((p.platform, true));
            }
        }
    }
    let named = match store {
        StoreKind::Apple => project
            .store
            .apple
            .as_ref()
            .map(|apple| match apple.platform {
                ApplePlatform::Ios => Platform::Ios,
                ApplePlatform::Macos => Platform::Macos,
            }),
        StoreKind::Google => project.store.google.as_ref().map(|_| Platform::Android),
    };
    if let Some(named) = named {
        let covered = platforms.iter().any(|(q, _)| match named {
            // An iOS app is an iPhone and/or an iPad app.
            Platform::Ios => matches!(q, Platform::Ios | Platform::Ipados),
            // A Play app is a phone/tablet and/or a Wear OS app.
            Platform::Android => matches!(q, Platform::Android | Platform::Wearos),
            other => *q == other,
        });
        if !covered {
            platforms.push((named, false));
        }
    }
    let mut reported: Vec<&str> = Vec::new();
    for p in presets {
        if p.requirement != Some(Requirement::Required) {
            continue;
        }
        let Some(slot) = p.slot.as_ref().and_then(|s| preset_slot_value(s, store)) else {
            continue;
        };
        let Some(&(_, from_targets)) = platforms.iter().find(|(q, _)| *q == p.platform) else {
            continue;
        };
        if filled.contains(&slot) || reported.contains(&slot) {
            continue;
        }
        reported.push(slot);
        out.push(MissingStoreSet {
            preset: p,
            store,
            from_targets,
        });
    }
}

/// [`FindingCode::RequiredStoreSetMissing`] for every missing required set.
pub(super) fn required_set_checks(project: &Project, catalog: &Catalog, out: &mut Sink) {
    for missing in missing_required_sets(project, catalog) {
        out.push(Ctx::default().finding(
            Severity::Warn,
            FindingCode::RequiredStoreSetMissing,
            false,
            None,
            None,
            missing.message(),
        ));
    }
}
