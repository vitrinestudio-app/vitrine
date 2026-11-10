//! Store mapping (`store:` in project.yaml, Phase 5.1): YAML round trip,
//! `set_store` op, validation.

use std::path::Path;

use arrangia_core::Project;
use arrangia_core::io;
use arrangia_core::model::{ApplePlatform, AppleStore, Lang, StoreKind, StoreSettings};
use arrangia_core::ops::Op;
use arrangia_core::resolve::NoAssets;
use arrangia_core::validate::{FindingCode, ValidateOptions, ValidationReport, validate};

fn sample() -> Project {
    Project::load(Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data/sample")).unwrap()
}

fn store(yaml: &str) -> StoreSettings {
    io::from_yaml(yaml).unwrap()
}

fn report(p: &Project) -> ValidationReport {
    validate(p, &ValidateOptions::new(&NoAssets))
}

fn codes(r: &ValidationReport, code: FindingCode) -> Vec<String> {
    r.with_code(code).map(|f| f.message.clone()).collect()
}

#[test]
fn yaml_round_trip() {
    let yaml = "apple: {account: team-a, app_id: \"1234567890\", bundle_id: com.example.app, platform: macos}\n\
                google: {account: play, package_name: com.example.app}\n\
                locales:\n  apple: {de: de-DE, en: en-GB}\n  google: {he: iw-IL}\n";
    let s = store(yaml);
    let a = s.apple.as_ref().unwrap();
    assert_eq!(a.app_id.as_deref(), Some("1234567890"));
    assert_eq!(a.platform, ApplePlatform::Macos);
    assert_eq!(s.account(StoreKind::Google), Some("play"));
    let out = io::to_yaml(&s).unwrap();
    assert_eq!(store(&out), s, "{out}");
    assert!(
        out.contains("app_id: '1234567890'") || out.contains("app_id: \"1234567890\""),
        "{out}"
    );
    assert!(out.contains("apple: {"), "flow style: {out}");

    // A hand-written numeric app id is accepted; iOS is the default platform.
    let s = store("apple: {app_id: 987654321}");
    let a = s.apple.as_ref().unwrap();
    assert_eq!(a.app_id.as_deref(), Some("987654321"));
    assert_eq!(a.platform, ApplePlatform::Ios);
    assert!(!io::to_yaml(&s).unwrap().contains("platform"));

    // Inside a project, missing store = empty and not written.
    let mut p = sample();
    assert!(p.store.is_empty());
    assert!(!io::to_yaml(&p).unwrap().contains("\nstore:"));
    p.store = store(yaml);
    let text = io::to_yaml(&p).unwrap();
    let back = io::project_from_yaml(&text).unwrap();
    assert_eq!(back.store, p.store);
}

#[test]
fn bad_values_are_rejected() {
    assert!(io::from_yaml::<StoreSettings>("apple: {nope: 1}").is_err());
    assert!(io::from_yaml::<StoreSettings>("amazon: {}").is_err());
    let mut p = sample();
    for bad in [
        "apple: {app_id: 12ab}",
        "apple: {bundle_id: nodots}",
        "apple: {account: 'a b'}",
        "google: {package_name: com.1x.app}",
        "locales: {apple: {de: de-XX}}",
        "locales: {google: {de: de}}",
        "locales: {apple: {'bad lang': de-DE}}",
    ] {
        let before = p.clone();
        assert!(
            p.apply(Op::SetStore { store: store(bad) }).is_err(),
            "{bad}"
        );
        assert_eq!(p, before);
    }
}

#[test]
fn set_store_op_and_undo() {
    let mut p = sample();
    let s = StoreSettings {
        apple: Some(AppleStore {
            bundle_id: Some("com.example.app".into()),
            ..AppleStore::default()
        }),
        ..StoreSettings::default()
    };
    let inverse = p.apply(Op::SetStore { store: s.clone() }).unwrap();
    assert_eq!(p.store, s);
    p.apply(inverse).unwrap();
    assert!(p.store.is_empty());
    // JSON form of the op.
    let op: Op = serde_json::from_value(serde_json::json!({
        "op": "set_store", "store": {"google": {"package_name": "com.example.app"}}
    }))
    .unwrap();
    p.apply(op).unwrap();
    assert!(p.store.has(StoreKind::Google));
}

#[test]
fn validation_only_for_mapped_stores() {
    let p = sample();
    let r = report(&p);
    for c in [
        FindingCode::StoreNoSlot,
        FindingCode::StoreLocaleMissing,
        FindingCode::StoreLocaleDuplicate,
        FindingCode::StoreSlotDuplicate,
    ] {
        assert_eq!(codes(&r, c), Vec::<String>::new(), "{c}");
    }
}

#[test]
fn validation_locales_and_slots() {
    let mut p = sample();
    // en, de, fr, es + a language without any store locale and one that
    // collides with `de` on Apple.
    p.languages.available.push(Lang::new("de-AT"));
    p.languages.available.push(Lang::new("tlh"));
    p.store = store("apple: {}\ngoogle: {}\nlocales: {google: {fr: fr-CA, xx: de-DE}}");
    let r = report(&p);
    let missing = codes(&r, FindingCode::StoreLocaleMissing);
    assert_eq!(missing.len(), 2, "tlh for both stores: {missing:?}");
    let dup = codes(&r, FindingCode::StoreLocaleDuplicate);
    assert_eq!(dup.len(), 2, "de/de-AT on both stores: {dup:?}");
    assert!(dup[0].contains("`de`, `de-AT`"), "{dup:?}");
    assert_eq!(codes(&r, FindingCode::StoreLocaleUnused).len(), 1);
    assert_eq!(codes(&r, FindingCode::StoreNoSlot), Vec::<String>::new());
    assert_eq!(
        codes(&r, FindingCode::StoreSlotDuplicate),
        Vec::<String>::new()
    );

    // Explicit mapping resolves the collision.
    p.store.locales.apple.insert("de-AT".into(), "en-GB".into());
    p.store
        .locales
        .google
        .insert("de-AT".into(), "en-GB".into());
    let r = report(&p);
    assert_eq!(
        codes(&r, FindingCode::StoreLocaleDuplicate),
        Vec::<String>::new()
    );

    // A hand-written unknown code is an error.
    p.store.locales.apple.insert("fr".into(), "fr-XX".into());
    let r = report(&p);
    assert_eq!(codes(&r, FindingCode::StoreLocaleUnknown).len(), 1);
    assert!(r.has_errors());

    // A second target on the same display type for the same languages.
    let mut p = sample();
    p.store = store("apple: {}");
    let mut t = p.target("ios-phone-69").unwrap().clone();
    t.id = arrangia_core::model::TargetId::new("ios-phone-69-b");
    t.langs = Some(vec![Lang::new("de")]);
    p.targets.push(t);
    let r = report(&p);
    let dup = codes(&r, FindingCode::StoreSlotDuplicate);
    assert_eq!(dup.len(), 1, "{dup:?}");
    assert!(
        dup[0].contains("APP_IPHONE_67") && dup[0].contains("for de"),
        "{dup:?}"
    );

    // Mapped to a store without any slot for it.
    let mut p = sample();
    p.targets
        .retain(|t| t.store.as_ref().is_none_or(|s| s.google.is_none()));
    p.store = store("google: {package_name: com.example.app}");
    assert_eq!(codes(&report(&p), FindingCode::StoreNoSlot).len(), 1);
}
