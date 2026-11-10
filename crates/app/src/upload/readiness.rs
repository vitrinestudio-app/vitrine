//! Upload readiness: what is missing before a plan can upload, in the order
//! the user has to fix it, each with the place to fix it (`fix`).
//!
//! ```text
//! 1. App settings → Stores      no_account, account_missing_for_project
//! 2. Project settings → Stores  project_not_mapped, store_locale_unknown,
//!                               store_locale_conflict, no_store_locale (warn)
//! 3. Targets editor             no_store_slots, store_slot_conflict
//! ```
//!
//! Cheap (no rendering, no keychain, no network): the upload dialog shows
//! it before the plan, `upload_plan` includes it (`readiness`), MCP prints
//! the hints. Validation errors of single images stay in the plan table.

use std::collections::BTreeMap;

use arrangia_core::Project;
use arrangia_core::catalog;
use arrangia_core::model::{Lang, StoreKind, Target};
use serde::{Deserialize, Serialize};

use crate::stores::StoreAccount;

/// Where a readiness step is fixed (the GUI opens that place).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Fix {
    /// App settings → Stores (accounts, keys).
    AppSettingsStores,
    /// Project settings → Stores (account choice, app identity, locales).
    ProjectSettingsStores,
    /// Targets editor (store slots).
    Targets,
}

impl Fix {
    /// Human name of the place (MCP hints, CLI).
    pub fn place(self) -> &'static str {
        match self {
            Self::AppSettingsStores => "App settings → Stores",
            Self::ProjectSettingsStores => "Project settings → Stores",
            Self::Targets => "the targets editor (Targets → store slot)",
        }
    }
}

/// One thing to do before uploading.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReadinessStep {
    /// Stable code (`no_account`, `account_missing_for_project`,
    /// `project_not_mapped`, `no_store_slots`, …).
    pub code: String,
    /// The store it is about (`None` = both / any).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub store: Option<StoreKind>,
    pub message: String,
    pub fix: Fix,
    /// `false` = a warning (e.g. a language without store locale is skipped).
    pub blocking: bool,
}

/// The checklist (`ready` = no blocking step).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Readiness {
    pub ready: bool,
    /// In the order to fix them: app settings, project settings, targets.
    pub steps: Vec<ReadinessStep>,
}

impl Readiness {
    /// One line per step, `n. message` (MCP output).
    pub fn hints(&self) -> Vec<String> {
        self.steps
            .iter()
            .enumerate()
            .map(|(n, s)| format!("{}. {}", n + 1, s.message))
            .collect()
    }
}

fn store_name(store: StoreKind) -> &'static str {
    match store {
        StoreKind::Apple => "App Store",
        StoreKind::Google => "Google Play",
    }
}

fn account_kind_name(store: StoreKind) -> &'static str {
    match store {
        StoreKind::Apple => "an App Store Connect API key",
        StoreKind::Google => "a Google Play service account",
    }
}

fn has_slot(target: &Target, store: StoreKind) -> bool {
    target.store.as_ref().is_some_and(|s| match store {
        StoreKind::Apple => s.apple.is_some(),
        StoreKind::Google => s.google.is_some(),
    })
}

fn slot_of(target: &Target, store: StoreKind) -> Option<&str> {
    let s = target.store.as_ref()?;
    match store {
        StoreKind::Apple => s.apple.as_ref().map(|a| a.display_type.as_str()),
        StoreKind::Google => s.google.as_ref().map(|g| g.image_type.as_str()),
    }
}

/// The checklist for `project` with the accounts of this machine; `only`
/// limits it to one store (the upload dialog's tab).
#[allow(clippy::too_many_lines)]
pub fn readiness(
    project: &Project,
    accounts: &[StoreAccount],
    only: Option<StoreKind>,
) -> Readiness {
    let wanted = |s: &StoreKind| only.is_none_or(|o| o == *s);
    let mapped: Vec<StoreKind> = StoreKind::ALL
        .into_iter()
        .filter(wanted)
        .filter(|s| project.store.has(*s))
        .collect();
    let mut steps = Vec::new();
    let mut step = |code: &str, store: Option<StoreKind>, fix: Fix, blocking: bool, msg: String| {
        steps.push(ReadinessStep {
            code: code.to_owned(),
            store,
            message: msg,
            fix,
            blocking,
        });
    };
    let of_kind = |s: StoreKind| accounts.iter().filter(move |a| a.kind == s);

    // 1. App settings: accounts on this machine.
    let relevant: Vec<StoreKind> = StoreKind::ALL.into_iter().filter(wanted).collect();
    if relevant.iter().all(|s| of_kind(*s).next().is_none()) && mapped.is_empty() {
        step(
            "no_account",
            only,
            Fix::AppSettingsStores,
            true,
            match only {
                Some(s) => format!(
                    "No {} account is set up on this machine: add {} in {}.",
                    store_name(s),
                    account_kind_name(s),
                    Fix::AppSettingsStores.place()
                ),
                None => format!(
                    "No store account is set up on this machine: add an App Store Connect API \
                     key and/or a Google Play service account in {}.",
                    Fix::AppSettingsStores.place()
                ),
            },
        );
    }
    for &s in &mapped {
        match project.store.account(s) {
            Some(id) if !of_kind(s).any(|a| a.id == id) => step(
                "account_missing_for_project",
                Some(s),
                Fix::AppSettingsStores,
                true,
                format!(
                    "The project uses the {} account `{id}`, which is not set up on this \
                     machine: add {} in {} (or choose another account in {}).",
                    store_name(s),
                    account_kind_name(s),
                    Fix::AppSettingsStores.place(),
                    Fix::ProjectSettingsStores.place()
                ),
            ),
            None if of_kind(s).next().is_none() => step(
                "no_account",
                Some(s),
                Fix::AppSettingsStores,
                true,
                format!(
                    "No {} account is set up on this machine: add {} in {}.",
                    store_name(s),
                    account_kind_name(s),
                    Fix::AppSettingsStores.place()
                ),
            ),
            _ => {}
        }
    }

    // 2. Project settings: mapping.
    if mapped.is_empty() {
        step(
            "project_not_mapped",
            only,
            Fix::ProjectSettingsStores,
            true,
            format!(
                "The project is not linked to {} yet: choose the account and the app in {}.",
                only.map_or("a store", store_name),
                Fix::ProjectSettingsStores.place()
            ),
        );
    }
    for &s in &mapped {
        let mut missing = Vec::new();
        if project.store.account(s).is_none() {
            missing.push("the account");
        }
        let app_known = match s {
            StoreKind::Apple => project
                .store
                .apple
                .as_ref()
                .is_some_and(|a| a.app_id.is_some() || a.bundle_id.is_some()),
            StoreKind::Google => project
                .store
                .google
                .as_ref()
                .is_some_and(|g| g.package_name.is_some()),
        };
        if !app_known {
            missing.push(match s {
                StoreKind::Apple => "the app (Apple ID or bundle id)",
                StoreKind::Google => "the package name",
            });
        }
        if !missing.is_empty() {
            step(
                "project_not_mapped",
                Some(s),
                Fix::ProjectSettingsStores,
                true,
                format!(
                    "{}: choose {} in {}.",
                    store_name(s),
                    missing.join(" and "),
                    Fix::ProjectSettingsStores.place()
                ),
            );
        }
        let list = catalog::store_locales().store(s);
        let explicit = project.store.locales.get(s);
        for (lang, code) in explicit {
            if project.languages.is_available(lang) && !list.is_known(code) {
                step(
                    "store_locale_unknown",
                    Some(s),
                    Fix::ProjectSettingsStores,
                    true,
                    format!(
                        "{}: `{code}` (for `{lang}`) is not a store language – pick one in {}.",
                        store_name(s),
                        Fix::ProjectSettingsStores.place()
                    ),
                );
            }
        }
        let mut by_code: BTreeMap<String, Vec<&str>> = BTreeMap::new();
        let mut without = Vec::new();
        for l in &project.languages.available {
            match list.resolve(explicit, l.as_str()) {
                Some((code, _)) => by_code.entry(code.to_owned()).or_default().push(l.as_str()),
                None => without.push(l.as_str()),
            }
        }
        for (code, langs) in by_code.iter().filter(|(_, l)| l.len() > 1) {
            step(
                "store_locale_conflict",
                Some(s),
                Fix::ProjectSettingsStores,
                true,
                format!(
                    "{}: languages {} all go to the store language `{code}` – map them to \
                     different store languages in {}.",
                    store_name(s),
                    langs.join(", "),
                    Fix::ProjectSettingsStores.place()
                ),
            );
        }
        if !without.is_empty() {
            step(
                "no_store_locale",
                Some(s),
                Fix::ProjectSettingsStores,
                false,
                format!(
                    "{}: {} {} no store language and will be skipped – map {} in {} if \
                     you want to upload {}.",
                    store_name(s),
                    without.join(", "),
                    if without.len() == 1 { "has" } else { "have" },
                    if without.len() == 1 { "it" } else { "them" },
                    Fix::ProjectSettingsStores.place(),
                    if without.len() == 1 { "it" } else { "them" },
                ),
            );
        }
    }

    // 3. Targets: store slots.
    for &s in &mapped {
        if !project.targets.iter().any(|t| has_slot(t, s)) {
            step(
                "no_store_slots",
                Some(s),
                Fix::Targets,
                true,
                format!(
                    "{}: no target has a {} slot – choose one per target in {} (or add a \
                     target from a {} preset).",
                    store_name(s),
                    match s {
                        StoreKind::Apple => "display type",
                        StoreKind::Google => "image type",
                    },
                    Fix::Targets.place(),
                    store_name(s)
                ),
            );
            continue;
        }
        // Two targets filling the same slot for the same language.
        let mut seen: BTreeMap<(String, String), &str> = BTreeMap::new();
        let mut reported = std::collections::BTreeSet::new();
        for t in &project.targets {
            let Some(slot) = slot_of(t, s) else { continue };
            let langs: Vec<&Lang> = t.langs.as_ref().map_or_else(
                || project.languages.available.iter().collect(),
                |l| l.iter().collect(),
            );
            for l in langs {
                let key = (slot.to_owned(), l.as_str().to_owned());
                if let Some(other) = seen.insert(key, t.id.as_str())
                    && other != t.id.as_str()
                    && reported.insert((slot.to_owned(), other.to_owned(), t.id.to_string()))
                {
                    step(
                        "store_slot_conflict",
                        Some(s),
                        Fix::Targets,
                        true,
                        format!(
                            "{}: targets `{other}` and `{}` both fill `{slot}` – give one of \
                             them another slot or other languages in {}.",
                            store_name(s),
                            t.id,
                            Fix::Targets.place()
                        ),
                    );
                }
            }
        }
    }

    let ready = !steps.iter().any(|s| s.blocking);
    Readiness { ready, steps }
}
