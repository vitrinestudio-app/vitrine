//! Store mapping checks (`store:` in `project.yaml`, Phase 5.1): explicit
//! locale codes, languages without or with a shared store locale, store
//! slots missing or used twice. Project-level findings; only for stores the
//! project is mapped to (`store.apple` / `store.google`), except the
//! explicit-map checks.

use std::collections::BTreeMap;

use super::{Ctx, FindingCode, Severity, Sink};
use crate::catalog::store_locales;
use crate::model::{Lang, Project, StoreKind, Target};

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
