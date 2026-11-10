//! Import: compare exchange files with the project → [`ImportPlan`].

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

use super::context::all_keys;
use super::inline::{markup_signature, parse_tree, same_markup};
use super::{ExchangeError, Format, decode_text, detect_format, table, xliff};
use crate::markup;
use crate::model::{ElementKind, Lang, Project, StringKey};
use crate::ops::Op;
use crate::validate::{Finding, FindingCode, ValidateOptions, validate};

/// One value read from a file.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Entry {
    pub lang: String,
    pub key: String,
    /// `None` = empty cell / untranslated unit.
    pub value: Option<String>,
    /// Inline elements that could not be mapped (XLIFF).
    pub problems: Vec<String>,
}

/// A parsed file.
#[derive(Clone, Debug, Default)]
pub(crate) struct Parsed {
    pub entries: Vec<Entry>,
    pub langs: Vec<String>,
    pub unknown_columns: Vec<String>,
    pub warnings: Vec<String>,
}

/// A file to import.
#[derive(Clone, Debug)]
pub struct ImportInput {
    /// File name (format detection, reports).
    pub name: String,
    pub data: Vec<u8>,
}

/// Import options.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct ImportOptions {
    /// Create keys that are not in the project (else they are reported).
    pub create_keys: bool,
    /// What an empty CSV cell means.
    pub empty: EmptyCells,
}

/// Meaning of an empty CSV cell.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EmptyCells {
    /// No change.
    #[default]
    Keep,
    /// Remove the translation (not for the default language).
    Remove,
}

/// What a file contributed.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct FileReport {
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub format: Option<Format>,
    pub langs: Vec<String>,
    /// Values read (cells / units).
    pub entries: usize,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub unknown_columns: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub warnings: Vec<String>,
    /// The file could not be read (nothing imported from it).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ChangeKind {
    /// No translation before.
    New,
    Changed,
    Removed,
}

/// One string that the import changes.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct StringChange {
    pub lang: String,
    pub key: String,
    pub kind: ChangeKind,
    pub old: Option<String>,
    pub new: Option<String>,
    /// The key did not exist in the project (`create_keys`).
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub created_key: bool,
}

/// Summary per language.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct LangReport {
    pub lang: String,
    pub new: Vec<String>,
    pub changed: Vec<String>,
    pub removed: Vec<String>,
    pub unchanged: usize,
    /// Empty cells / untranslated units (no change).
    pub skipped: Vec<String>,
}

/// A key in a file that the project does not know.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct UnknownKey {
    pub key: String,
    pub file: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum IssueKind {
    /// Unbalanced marker (rendered literally).
    Unbalanced,
    /// `[name]` names no span style.
    UnknownSpan,
    /// XLIFF inline element that could not be mapped.
    Inline,
    /// Bold/italic/span usage differs from the default language (info).
    DiffersFromSource,
}

/// A markup problem in an imported value.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct MarkupIssue {
    pub lang: String,
    pub key: String,
    pub kind: IssueKind,
    pub message: String,
}

/// A layout finding after the import (validation of the changed languages).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct LayoutFinding {
    /// Key of the text element (if any).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub key: Option<String>,
    /// Not present before the import.
    pub new: bool,
    #[serde(flatten)]
    pub finding: Finding,
}

/// Result of comparing import files with the project.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ImportPlan {
    pub files: Vec<FileReport>,
    pub langs: Vec<LangReport>,
    pub changes: Vec<StringChange>,
    #[serde(default)]
    pub unknown_keys: Vec<UnknownKey>,
    #[serde(default)]
    pub issues: Vec<MarkupIssue>,
    /// `text_overflow` / `emergency_break` after the import ([`check_layout`]).
    #[serde(default)]
    pub layout: Vec<LayoutFinding>,
}

impl ImportPlan {
    /// The `set_string` ops of all changes (one transaction).
    pub fn ops(&self) -> Result<Vec<Op>, String> {
        self.changes
            .iter()
            .map(|c| {
                Ok(Op::SetString {
                    lang: Lang::parse(&c.lang)?,
                    key: StringKey::parse(&c.key)?,
                    value: c.new.clone(),
                })
            })
            .collect()
    }

    /// Changes whose `old` value no longer matches the project (it was
    /// edited since the plan was made).
    pub fn conflicts(&self, p: &Project) -> Vec<String> {
        self.changes
            .iter()
            .filter(|c| p.string(&c.lang, &c.key) != c.old.as_deref())
            .map(|c| format!("{} ({})", c.key, c.lang))
            .collect()
    }

    pub fn is_empty(&self) -> bool {
        self.changes.is_empty()
    }
}

fn read(p: &Project, input: &ImportInput) -> Result<(Format, Parsed), ExchangeError> {
    let text = decode_text(&input.data)?;
    let format = detect_format(&input.name, &text);
    let parsed = match format {
        Format::Csv => table::parse(p, &text)?,
        Format::Xliff12 | Format::Xliff20 => xliff::parse(p, &text)?,
    };
    Ok((format, parsed))
}

fn markup_issues(p: &Project, lang: &str, key: &str, value: &str, out: &mut Vec<MarkupIssue>) {
    let issue = |kind, message| MarkupIssue {
        lang: lang.to_owned(),
        key: key.to_owned(),
        kind,
        message,
    };
    let m = markup::parse(value);
    for d in &m.diagnostics {
        out.push(issue(IssueKind::Unbalanced, d.to_string()));
    }
    let mut unknown = BTreeSet::new();
    for s in &m.spans {
        if let Some(name) = &s.style
            && !p.styles.spans.contains_key(name)
        {
            unknown.insert(name.clone());
        }
    }
    for name in unknown {
        out.push(issue(
            IssueKind::UnknownSpan,
            format!("span style `{name}` does not exist"),
        ));
    }
    if lang != p.languages.default.as_str()
        && let Some(src) = p.string(p.languages.default.as_str(), key)
    {
        let (a, b) = (
            markup_signature(&parse_tree(src)),
            markup_signature(&parse_tree(value)),
        );
        if a != b {
            out.push(issue(
                IssueKind::DiffersFromSource,
                "bold/italic/span markup differs from the default language".to_owned(),
            ));
        }
    }
}

/// Value, file name and inline problems of one `(lang, key)`.
type Planned = (Option<String>, String, Vec<String>);

/// Reads `inputs` and compares them with `p`. Files that cannot be read
/// are reported in [`FileReport::error`]; the rest is still planned.
#[allow(clippy::too_many_lines)]
pub fn plan_import(p: &Project, inputs: &[ImportInput], opts: &ImportOptions) -> ImportPlan {
    let mut plan = ImportPlan::default();
    // (lang, key) → (value, file, problems); the last file wins.
    let mut values: BTreeMap<(String, String), Planned> = BTreeMap::new();
    for input in inputs {
        let mut report = FileReport {
            name: input.name.clone(),
            ..FileReport::default()
        };
        match read(p, input) {
            Err(e) => report.error = Some(e.to_string()),
            Ok((format, parsed)) => {
                report.format = Some(format);
                report.langs = parsed.langs;
                report.entries = parsed.entries.len();
                report.unknown_columns = parsed.unknown_columns;
                report.warnings = parsed.warnings;
                let mut invalid = BTreeSet::new();
                for e in parsed.entries {
                    if StringKey::parse(&e.key).is_err() {
                        invalid.insert(e.key);
                        continue;
                    }
                    let slot = (e.lang, e.key);
                    if let Some((_, other, _)) = values.get(&slot)
                        && *other == input.name
                    {
                        report.warnings.push(format!(
                            "`{}` ({}) appears twice; the last value is used",
                            slot.1, slot.0
                        ));
                    }
                    values.insert(slot, (e.value, input.name.clone(), e.problems));
                }
                for k in invalid {
                    report
                        .warnings
                        .push(format!("invalid key `{k}` (empty or with spaces) ignored"));
                }
            }
        }
        plan.files.push(report);
    }

    let known = all_keys(p);
    let default = p.languages.default.as_str();
    let mut langs: BTreeMap<String, LangReport> = BTreeMap::new();
    let mut unknown: BTreeMap<String, String> = BTreeMap::new();
    for ((lang, key), (value, file, problems)) in values {
        let is_known = known.contains(&key);
        if !is_known && !opts.create_keys {
            if value.is_some() {
                unknown.entry(key).or_insert(file);
            }
            continue;
        }
        let r = langs.entry(lang.clone()).or_insert_with(|| LangReport {
            lang: lang.clone(),
            ..LangReport::default()
        });
        let old = p.string(&lang, &key).map(ToOwned::to_owned);
        for problem in problems {
            plan.issues.push(MarkupIssue {
                lang: lang.clone(),
                key: key.clone(),
                kind: IssueKind::Inline,
                message: problem,
            });
        }
        let change = |kind, new: Option<String>| StringChange {
            lang: lang.clone(),
            key: key.clone(),
            kind,
            old: old.clone(),
            new,
            created_key: !is_known,
        };
        match value {
            None => {
                if opts.empty == EmptyCells::Remove && old.is_some() && lang != default {
                    r.removed.push(key.clone());
                    plan.changes.push(change(ChangeKind::Removed, None));
                } else {
                    r.skipped.push(key.clone());
                }
            }
            Some(v) => {
                markup_issues(p, &lang, &key, &v, &mut plan.issues);
                match &old {
                    Some(o) if same_markup(o, &v) => r.unchanged += 1,
                    Some(_) => {
                        r.changed.push(key.clone());
                        plan.changes.push(change(ChangeKind::Changed, Some(v)));
                    }
                    None => {
                        r.new.push(key.clone());
                        plan.changes.push(change(ChangeKind::New, Some(v)));
                    }
                }
            }
        }
    }
    // Project language order.
    plan.langs = p
        .languages
        .available
        .iter()
        .filter_map(|l| langs.remove(l.as_str()))
        .collect();
    plan.unknown_keys = unknown
        .into_iter()
        .map(|(key, file)| UnknownKey { key, file })
        .collect();
    plan
}

/// Runs validation for the languages the plan changes, before and after
/// applying it, and stores the `text_overflow` / `emergency_break`
/// findings after the import in [`ImportPlan::layout`] (`new` = caused by
/// the import).
pub fn check_layout(plan: &mut ImportPlan, p: &Project, opts: &ValidateOptions<'_>) {
    plan.layout.clear();
    let langs: BTreeSet<String> = plan.changes.iter().map(|c| c.lang.clone()).collect();
    if langs.is_empty() {
        return;
    }
    let Ok(ops) = plan.ops() else {
        return;
    };
    let mut after = p.clone();
    if after.apply_ops(ops).is_err() {
        return;
    }
    let mut opts = opts.clone();
    opts.langs = Some(langs.into_iter().collect());
    let relevant = |f: &Finding| {
        matches!(
            f.code,
            FindingCode::TextOverflow | FindingCode::EmergencyBreak
        )
    };
    let before: std::collections::HashSet<Finding> = validate(p, &opts)
        .findings
        .into_iter()
        .filter(relevant)
        .collect();
    let mut keys = BTreeMap::new();
    for s in &after.strips {
        s.walk_elements(&mut |e| {
            if let ElementKind::Text(t) = &e.kind {
                keys.insert(e.id.to_string(), t.text.to_string());
            }
        });
    }
    plan.layout = validate(&after, &opts)
        .findings
        .into_iter()
        .filter(relevant)
        .map(|f| LayoutFinding {
            key: f
                .element
                .as_ref()
                .and_then(|e| keys.get(e.as_str()).cloned()),
            new: !before.contains(&f),
            finding: f,
        })
        .collect();
}
