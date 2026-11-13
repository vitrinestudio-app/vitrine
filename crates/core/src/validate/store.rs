//! Store mapping checks (`store:` in `project.yaml`, Phase 5.1): explicit
//! locale codes, languages without or with a shared store locale, store
//! slots missing or used twice. Project-level findings; only for stores the
//! project is mapped to (`store.apple` / `store.google`), except the
//! explicit-map checks. Plus the sets App Store Connect requires
//! ([`missing_required_sets`]).

use std::collections::BTreeMap;

use super::{Ctx, FindingCode, Severity, Sink};
use crate::catalog::{Catalog, Platform, Requirement, Store, TargetPreset, store_locales};
use crate::model::{ApplePlatform, Lang, Project, StoreKind, StoreSlot, Target};

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

/// An App Store set that `targets.yaml` marks `requirement: required` for
/// a platform the project targets, and that no target fills.
#[derive(Clone, Copy, Debug)]
pub struct MissingStoreSet<'c> {
    /// The required preset (id = the target to add, label, slot).
    pub preset: &'c TargetPreset,
    /// Why the platform counts: the project has targets of it (`true`) or
    /// only `store.apple.platform` names it (`false`).
    pub from_targets: bool,
}

impl MissingStoreSet<'_> {
    /// Display type of the missing set (`APP_IPHONE_61`).
    pub fn display_type(&self) -> &str {
        self.preset
            .slot
            .as_ref()
            .and_then(|s| s.apple.as_ref())
            .map_or("", |a| a.display_type.as_str())
    }

    /// Who needs the set, in words (`iPhone apps`).
    fn needed_by(&self) -> &'static str {
        match self.preset.platform {
            Platform::Ios => "iPhone apps",
            Platform::Ipados => "apps that run on iPad",
            Platform::Macos => "Mac apps",
            Platform::Watchos => "Apple Watch apps",
            _ => "this platform",
        }
    }

    /// The finding / readiness message: what is missing and how to add it.
    pub fn message(&self) -> String {
        let why = match (self.preset.platform, self.from_targets) {
            (_, false) => " (the project is mapped to the App Store: store.apple.platform)",
            (Platform::Ipados, true) => " (reported because the project has iPad targets)",
            (Platform::Macos, true) => " (reported because the project has Mac targets)",
            _ => "",
        };
        format!(
            "no {} set (`{}`), required by App Store Connect for {}{why}: add the target \
             `{}` (Canvases & targets → Add target)",
            self.preset.label,
            self.display_type(),
            self.needed_by(),
            self.preset.id,
        )
    }
}

/// Required App Store sets the project lacks. Data-driven: every Apple
/// preset with `requirement: required` and a store slot is needed for its
/// `platform` (iPhone `ios`, iPad `ipados`, Mac `macos`) once the project
/// targets that platform, and is filled by any target with the same slot.
///
/// The project's platforms are those of the presets its Apple targets'
/// slots belong to. Whether an app runs on iPad (or Mac) cannot be known
/// otherwise, so those sets are only asked for when the project already
/// has such targets. A project mapped to the App Store (`store.apple`)
/// without iPhone or iPad targets also needs the sets of its
/// `store.apple.platform` (`ios` → iPhone, `macos` → Mac).
pub fn missing_required_sets<'c>(
    project: &Project,
    catalog: &'c Catalog,
) -> Vec<MissingStoreSet<'c>> {
    let presets = &catalog.targets.presets;
    let apple_slots: Vec<&StoreSlot> = project
        .targets
        .iter()
        .filter_map(|t| t.store.as_ref().filter(|s| s.apple.is_some()))
        .collect();
    let mut platforms: Vec<(Platform, bool)> = Vec::new();
    for slot in &apple_slots {
        for p in presets.iter().filter(|p| p.slot.as_ref() == Some(*slot)) {
            if !platforms.iter().any(|(q, _)| *q == p.platform) {
                platforms.push((p.platform, true));
            }
        }
    }
    if let Some(apple) = &project.store.apple {
        let named = match apple.platform {
            ApplePlatform::Ios => Platform::Ios,
            ApplePlatform::Macos => Platform::Macos,
        };
        let covered = platforms.iter().any(|(q, _)| match named {
            // An iOS app is an iPhone and/or an iPad app.
            Platform::Ios => matches!(q, Platform::Ios | Platform::Ipados),
            other => *q == other,
        });
        if !covered {
            platforms.push((named, false));
        }
    }
    presets
        .iter()
        .filter(|p| p.store == Store::Apple && p.requirement == Some(Requirement::Required))
        .filter_map(|p| {
            let slot = p.slot.as_ref().filter(|s| s.apple.is_some())?;
            let (_, from_targets) = *platforms.iter().find(|(q, _)| *q == p.platform)?;
            if apple_slots.contains(&slot) {
                return None;
            }
            Some(MissingStoreSet {
                preset: p,
                from_targets,
            })
        })
        .collect()
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
