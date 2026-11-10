use std::path::{Path, PathBuf};

use super::*;
use crate::model::{Lang, StringKey};
use crate::ops::Op;
use crate::resolve::FsAssets;
use crate::validate::{FindingCode, ValidateOptions};

fn example_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/example")
}

fn example() -> Project {
    Project::load(example_dir()).unwrap()
}

fn input(name: &str, data: impl Into<Vec<u8>>) -> ImportInput {
    ImportInput {
        name: name.to_owned(),
        data: data.into(),
    }
}

fn text(f: &ExportFile) -> String {
    String::from_utf8(f.content.clone()).unwrap()
}

fn set(p: &mut Project, lang: &str, key: &str, value: Option<&str>) {
    p.apply(Op::SetString {
        lang: Lang::new(lang),
        key: StringKey::new(key),
        value: value.map(ToOwned::to_owned),
    })
    .unwrap();
}

fn assert_no_changes(p: &Project, files: &[ExportFile]) {
    let inputs: Vec<_> = files
        .iter()
        .map(|f| input(&f.name, f.content.clone()))
        .collect();
    let plan = plan_import(p, &inputs, &ImportOptions::default());
    for f in &plan.files {
        assert_eq!(f.error, None, "{}", f.name);
        assert!(f.unknown_columns.is_empty(), "{:?}", f.unknown_columns);
    }
    assert_eq!(plan.changes, Vec::new(), "round trip changed strings");
    assert_eq!(plan.unknown_keys, Vec::new());
    assert!(
        plan.issues
            .iter()
            .all(|i| i.kind == IssueKind::DiffersFromSource),
        "{:?}",
        plan.issues
    );
}

#[test]
fn csv_round_trip_has_no_changes() {
    let p = example();
    for (delimiter, bom, context) in [
        (Delimiter::Comma, false, false),
        (Delimiter::Semicolon, true, true),
        (Delimiter::Tab, false, true),
    ] {
        let opts = ExportOptions {
            context,
            delimiter,
            bom,
            ..ExportOptions::default()
        };
        let files = export(&p, Format::Csv, &opts).unwrap();
        assert_eq!(files.len(), 1);
        assert_eq!(files[0].name, "Fernwood-translations.csv");
        assert_eq!(files[0].content.starts_with(b"\xEF\xBB\xBF"), bom);
        assert_no_changes(&p, &files);
    }
}

#[test]
fn csv_layout_and_context() {
    let p = example();
    let opts = ExportOptions {
        context: true,
        ..ExportOptions::default()
    };
    let csv = text(&export(&p, Format::Csv, &opts).unwrap()[0]);
    let mut lines = csv.split("\r\n");
    assert_eq!(lines.next(), Some("key,en,de,max_chars,context,notes"));
    // Newlines inside a cell are quoted (RFC 4180).
    assert!(csv.contains(
        "callout.bad,\"**Fennel**\nbetter keep apart\",\"**Fenchel**\nlieber getrennt\""
    ));
    let plan_title = csv.lines().find(|l| l.starts_with("plan.title,")).unwrap();
    assert!(
        plan_title
            .contains("title-plan (strip phone, page 1, style headline): box 860 lu, max 2 lines"),
        "{plan_title}"
    );
}

#[test]
fn xliff_round_trips_have_no_changes() {
    let p = example();
    for format in [Format::Xliff12, Format::Xliff20] {
        for markup in [MarkupMode::Inline, MarkupMode::Plain] {
            let opts = ExportOptions {
                markup,
                context: true,
                ..ExportOptions::default()
            };
            let files = export(&p, format, &opts).unwrap();
            assert_eq!(files.len(), 1);
            assert_eq!(files[0].name, "Fernwood-de.xlf");
            assert_eq!(files[0].lang.as_deref(), Some("de"));
            assert_eq!(detect_format(&files[0].name, &text(&files[0])), format);
            xliff::check_structure(&text(&files[0]))
                .unwrap_or_else(|e| panic!("{format:?} {markup:?}: {e}"));
            assert_no_changes(&p, &files);
        }
    }
}

#[test]
fn xliff_inline_markup_shapes() {
    let mut p = example();
    set(
        &mut p,
        "en",
        "plan.title",
        Some("[accent]Plan **in [green]Mi\\-nuten[/]**[/]\nnow _really_"),
    );
    set(&mut p, "de", "plan.title", None);
    let x20 = text(&export(&p, Format::Xliff20, &ExportOptions::default()).unwrap()[0]);
    let unit = x20
        .split("<unit ")
        .find(|u| u.starts_with("id=\"plan.title\""))
        .unwrap();
    assert!(unit.contains("<data id=\"d1\">[accent]</data>"), "{unit}");
    assert!(unit.contains("<segment state=\"initial\">"));
    assert!(!unit.contains("<target"));
    assert!(
        unit.contains(
            r#"<pc id="2" dataRefStart="d3" dataRefEnd="d3" type="fmt" subType="xlf:b">in "#
        ),
        "{unit}"
    );
    assert!(
        unit.contains(r#"type="fmt" subType="xlf:lb"/>now "#),
        "{unit}"
    );
    let x12 = text(&export(&p, Format::Xliff12, &ExportOptions::default()).unwrap()[0]);
    assert!(x12.contains(
        r#"<source><g id="1" ctype="x-span-accent">Plan <g id="2" ctype="bold">in <g id="3" ctype="x-span-green">Mi<x id="4" ctype="x-soft-hyphen"/>nuten</g></g></g><x id="5" ctype="lb"/>now <g id="6" ctype="italic">really</g></source>"#
    ), "{x12}");
    assert!(x12.contains("<target state=\"needs-translation\"/>"));
}

/// A translator's tool fills the targets (as copies of the source with
/// tags moved around); import maps the tags back to markup.
#[test]
fn xliff_import_maps_inline_elements_back() {
    let p = example();
    let x20 = r#"<?xml version="1.0" encoding="UTF-8"?>
<xliff xmlns="urn:oasis:names:tc:xliff:document:2.0" version="2.0" srcLang="en" trgLang="de">
  <file id="f1">
    <unit id="companions.title" name="companions.title">
      <originalData><data id="d1">[accent]</data><data id="d2">[/]</data><data id="d3">\-</data><data id="d4">**</data><data id="d5">\n</data></originalData>
      <segment state="translated">
        <source>Know your <pc id="1" dataRefStart="d1" dataRefEnd="d2">plant partners</pc></source>
        <target>Die <pc id="1" dataRefStart="d1" dataRefEnd="d2">Pflanzen<ph id="2" dataRef="d3"/>partner <pc id="3" dataRefStart="d4" dataRefEnd="d4">neu</pc></pc><ph id="4" dataRef="d5"/>finden &amp; mehr</target>
      </segment>
    </unit>
    <unit id="u2" name="harvest.title">
      <segment><source>x</source><target><sc id="1" subType="xlf:i" type="fmt"/>So<ec startRef="1"/> wächst <pc id="9" type="other">es</pc></target></segment>
    </unit>
    <unit id="plan.title">
      <segment state="initial"><source>Your garden</source><target>Your garden</target></segment>
    </unit>
  </file>
</xliff>"#;
    let plan = plan_import(&p, &[input("de.xlf", x20)], &ImportOptions::default());
    assert_eq!(plan.files[0].error, None);
    let get = |k: &str| plan.changes.iter().find(|c| c.key == k).unwrap();
    assert_eq!(
        get("companions.title").new.as_deref(),
        Some("Die [accent]Pflanzen\\-partner **neu**[/]\nfinden & mehr")
    );
    assert_eq!(get("harvest.title").new.as_deref(), Some("_So_ wächst es"));
    assert!(
        plan.issues
            .iter()
            .any(|i| i.key == "harvest.title" && i.kind == IssueKind::Inline)
    );
    // Copied source in an untranslated segment is skipped.
    assert!(plan.langs[0].skipped.contains(&"plan.title".to_owned()));
    assert_eq!(plan.langs[0].lang, "de");
    assert_eq!(plan.langs[0].changed.len(), 2);
}

#[test]
fn xliff12_import_with_g_x_and_bx_ex() {
    let p = example();
    let x12 = r#"<xliff version="1.2" xmlns="urn:oasis:names:tc:xliff:document:1.2">
  <file original="x" source-language="en" target-language="de" datatype="plaintext"><body>
    <trans-unit id="1" resname="beds.title" xml:space="preserve">
      <source>Plan every bed <g id="1" ctype="bold">by drag &amp; drop</g></source>
      <target state="translated">Jedes <bx id="1" ctype="bold"/>Beet<ex id="2" rid="1"/> <g id="3" ctype="x-span-nope">planen</g><x id="4" ctype="lb"/>jetzt<x id="5" ctype="x-soft-hyphen"/>!</target>
    </trans-unit>
    <trans-unit id="calendar.title"><source>a</source><target state="needs-translation"/></trans-unit>
  </body></file>
</xliff>"#;
    let plan = plan_import(&p, &[input("de.xliff", x12)], &ImportOptions::default());
    assert_eq!(plan.files[0].format, Some(Format::Xliff12));
    assert_eq!(
        plan.changes[0].new.as_deref(),
        Some("Jedes **Beet** [nope]planen[/]\njetzt\\-!")
    );
    assert!(plan.issues.iter().any(|i| i.kind == IssueKind::UnknownSpan));
    assert_eq!(plan.langs[0].skipped, ["calendar.title"]);
}

#[test]
fn excel_style_csv() {
    let p = example();
    // BOM, semicolons, CRLF, quoted newline, unknown column, unknown key,
    // an empty cell, a language column in upper case.
    let csv = "\u{FEFF}Key;EN;DE;Comment;Status\r\n\
               plan.title;Your garden, [accent]planned in minutes[/];\"Dein Garten,\r\n[accent]schnell geplant[/]\";bitte prüfen;ok\r\n\
               plan.sub;Beds, plants and tasks in one calm place.;;;\r\n\
               new.key;New;Neu;;\r\n\
               ;;;;\r\n";
    let plan = plan_import(
        &p,
        &[input("Translations.csv", csv)],
        &ImportOptions::default(),
    );
    let f = &plan.files[0];
    assert_eq!(f.error, None);
    assert_eq!(f.langs, ["en", "de"]);
    assert_eq!(f.unknown_columns, ["Status"]);
    assert_eq!(
        plan.unknown_keys,
        [UnknownKey {
            key: "new.key".into(),
            file: "Translations.csv".into()
        }]
    );
    assert_eq!(plan.changes.len(), 1);
    let c = &plan.changes[0];
    assert_eq!(
        (c.lang.as_str(), c.key.as_str(), c.kind),
        ("de", "plan.title", ChangeKind::Changed)
    );
    assert_eq!(
        c.new.as_deref(),
        Some("Dein Garten,\r\n[accent]schnell geplant[/]")
    );
    let de = plan.langs.iter().find(|l| l.lang == "de").unwrap();
    assert_eq!(de.skipped, ["plan.sub"]);
    let en = plan.langs.iter().find(|l| l.lang == "en").unwrap();
    assert_eq!(en.unchanged, 2);

    // Remove empty cells; create unknown keys.
    let opts = ImportOptions {
        create_keys: true,
        empty: EmptyCells::Remove,
    };
    let plan = plan_import(&p, &[input("t.csv", csv)], &opts);
    assert!(
        plan.changes
            .iter()
            .any(|c| c.key == "plan.sub" && c.lang == "de" && c.kind == ChangeKind::Removed)
    );
    let created: Vec<_> = plan
        .changes
        .iter()
        .filter(|c| c.created_key)
        .map(|c| c.lang.as_str())
        .collect();
    assert_eq!(created, ["de", "en"]);
    let mut q = p.clone();
    q.apply_ops(plan.ops().unwrap()).unwrap();
    assert_eq!(q.string("de", "new.key"), Some("Neu"));
    assert_eq!(q.string("de", "plan.sub"), None);
}

#[test]
fn malformed_inputs_are_reported_per_file() {
    let p = example();
    let cases: [(&str, &[u8], &str); 7] = [
        ("a.csv", b"", "empty"),
        ("b.csv", b"id,en\nx,y\n", "no `key` column"),
        (
            "c.csv",
            b"key,fr\nx,y\n",
            "no column named after a project language",
        ),
        ("d.csv", b"key,de\n\xff\xfe\xfa,x\n", "UTF-8"),
        (
            "e.xlf",
            b"<xliff version=\"2.0\" srcLang=\"en\" trgLang=\"de\"><file id=\"f\"><unit id=\"x\">",
            "unclosed",
        ),
        (
            "f.xlf",
            b"<xliff version=\"2.0\" srcLang=\"en\" trgLang=\"fr\"><file id=\"f\"/></xliff>",
            "`fr` is not a project language",
        ),
        ("g.xlf", b"<html/>", "not <xliff>"),
    ];
    let inputs: Vec<_> = cases.iter().map(|(n, d, _)| input(n, *d)).collect();
    let ok = input("ok.csv", "key,de\nplan.sub,Neu\n");
    let mut all = inputs.clone();
    all.push(ok);
    let plan = plan_import(&p, &all, &ImportOptions::default());
    for ((name, _, msg), report) in cases.iter().zip(&plan.files) {
        let e = report
            .error
            .as_deref()
            .unwrap_or_else(|| panic!("{name}: no error"));
        assert!(e.contains(msg), "{name}: {e}");
    }
    assert_eq!(plan.files.last().unwrap().error, None);
    assert_eq!(plan.changes.len(), 1);
}

#[test]
fn markup_diagnostics_on_import() {
    let p = example();
    let csv = "key,de\nplan.title,\"Dein **Garten\"\nbeds.title,Jedes Beet planen\n";
    let plan = plan_import(&p, &[input("x.csv", csv)], &ImportOptions::default());
    let kinds: Vec<_> = plan
        .issues
        .iter()
        .map(|i| (i.key.as_str(), i.kind))
        .collect();
    assert!(kinds.contains(&("plan.title", IssueKind::Unbalanced)));
    assert!(kinds.contains(&("beds.title", IssueKind::DiffersFromSource)));
}

#[test]
fn overflow_report_after_importing_a_long_german_string() {
    let p = example();
    let long = "Plane deinen ganzen Garten mit Hochbeeten, Gewächshäusern, Kräuterspiralen und Obstbaumwiesen in wenigen Minuten";
    let csv = format!("key,de\nplan.title,\"{long}\"\nplan.sub,Beete\n");
    let mut plan = plan_import(&p, &[input("x.csv", csv)], &ImportOptions::default());
    let fs = FsAssets::new(example_dir());
    let opts = ValidateOptions::new(&fs);
    check_layout(&mut plan, &p, &opts);
    let overflow: Vec<_> = plan
        .layout
        .iter()
        .filter(|f| f.finding.code == FindingCode::TextOverflow)
        .collect();
    assert!(!overflow.is_empty(), "{:?}", plan.layout);
    assert!(
        overflow
            .iter()
            .all(|f| f.key.as_deref() == Some("plan.title") && f.new)
    );
    assert!(
        plan.layout
            .iter()
            .all(|f| f.finding.lang.as_ref().map(Lang::as_str) == Some("de"))
    );
    // The plan survives a JSON round trip (GUI sends it back to apply).
    let json = serde_json::to_string(&plan).unwrap();
    let back: ImportPlan = serde_json::from_str(&json).unwrap();
    assert_eq!(back, plan);
    assert_eq!(back.conflicts(&p), Vec::<String>::new());
    let mut q = p.clone();
    set(&mut q, "de", "plan.title", Some("edited meanwhile"));
    assert_eq!(back.conflicts(&q), ["plan.title (de)"]);
}

#[test]
fn missing_only_and_language_selection() {
    let mut p = example();
    p.apply(Op::AddLanguage {
        lang: Lang::new("fr"),
        index: None,
        strings: std::collections::BTreeMap::default(),
    })
    .unwrap();
    set(&mut p, "fr", "plan.title", Some("Votre jardin"));
    let opts = ExportOptions {
        langs: vec!["fr".into()],
        missing_only: true,
        ..ExportOptions::default()
    };
    let files = export(&p, Format::Xliff20, &opts).unwrap();
    assert_eq!(files.len(), 1);
    let x = text(&files[0]);
    assert!(x.contains("trgLang=\"fr\""));
    assert!(!x.contains("name=\"plan.title\""));
    assert!(x.contains("name=\"plan.sub\""));
    let csv = text(&export(&p, Format::Csv, &opts).unwrap()[0]);
    assert!(csv.starts_with("key,en,fr\r\n"));
    assert!(!csv.contains("\r\nplan.title,"));
    assert_eq!(
        export(&p, Format::Xliff12, &ExportOptions::default())
            .unwrap()
            .len(),
        2
    );
    assert_eq!(
        export(
            &p,
            Format::Csv,
            &ExportOptions {
                langs: vec!["xx".into()],
                ..ExportOptions::default()
            }
        ),
        Err(ExchangeError::UnknownLanguage("xx".into()))
    );
}

#[test]
fn output_paths_for_files_and_folders() {
    let files = |langs: &[&str]| -> Vec<ExportFile> {
        langs
            .iter()
            .map(|l| ExportFile {
                name: format!("P-{l}.xlf"),
                lang: Some((*l).to_owned()),
                keys: 0,
                skipped: Vec::new(),
                content: Vec::new(),
            })
            .collect()
    };
    let out = Path::new("/tmp/out");
    assert_eq!(
        output_paths(&files(&["de"]), out, true),
        [out.join("P-de.xlf")]
    );
    assert_eq!(
        output_paths(&files(&["de"]), &out.join("x.xlf"), false),
        [out.join("x.xlf")]
    );
    assert_eq!(
        output_paths(&files(&["de", "fr"]), &out.join("x.xlf"), false),
        [out.join("x-de.xlf"), out.join("x-fr.xlf")]
    );
}

#[test]
fn utf16_xliff_is_decoded() {
    let p = example();
    let mut x = String::from_utf8(
        export(&p, Format::Xliff12, &ExportOptions::default()).unwrap()[0]
            .content
            .clone(),
    )
    .unwrap();
    x = x.replace("encoding=\"UTF-8\"", "encoding=\"UTF-16\"");
    let mut data = vec![0xFF, 0xFE];
    for u in x.encode_utf16() {
        data.extend(u.to_le_bytes());
    }
    let plan = plan_import(&p, &[input("de.xlf", data)], &ImportOptions::default());
    assert_eq!(plan.files[0].error, None);
    assert_eq!(plan.changes, Vec::new());
    assert!(plan.langs[0].unchanged > 20);
}
