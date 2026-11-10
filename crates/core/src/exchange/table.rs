//! CSV export and import (RFC 4180 quoting via the `csv` crate).
//!
//! Layout: `key, <default lang>, <lang>…` and, with context,
//! `max_chars, context, notes`. Import maps columns by language code
//! (case-insensitive); `key` is required, `max_chars`/`context`/`notes`
//! are ignored, anything else is reported as unknown.

use super::context::{all_keys, key_contexts};
use super::plan::{Entry, Parsed};
use super::{ExchangeError, ExportFile, ExportOptions, file_stem};
use crate::model::Project;

/// Columns that are written for translators and ignored on import.
const INFO_COLUMNS: [&str; 6] = [
    "max_chars",
    "context",
    "notes",
    "note",
    "comment",
    "comments",
];

fn csv_err(e: impl std::fmt::Display) -> ExchangeError {
    ExchangeError::Csv(e.to_string())
}

pub(super) fn export(
    p: &Project,
    targets: &[String],
    opts: &ExportOptions,
) -> Result<ExportFile, ExchangeError> {
    let default = p.languages.default.to_string();
    let mut langs = vec![default.clone()];
    langs.extend(targets.iter().cloned());
    let contexts = opts.context.then(|| key_contexts(p));

    let mut out = Vec::new();
    if opts.bom {
        out.extend_from_slice(b"\xEF\xBB\xBF");
    }
    let mut w = csv::WriterBuilder::new()
        .delimiter(opts.delimiter.byte())
        .terminator(csv::Terminator::CRLF)
        .from_writer(out);
    let mut header: Vec<&str> = vec!["key"];
    header.extend(langs.iter().map(String::as_str));
    if contexts.is_some() {
        header.extend(["max_chars", "context", "notes"]);
    }
    w.write_record(&header).map_err(csv_err)?;

    let mut n = 0;
    for key in all_keys(p) {
        let missing = |l: &String| p.string(l, &key).is_none();
        if opts.missing_only
            && !(if targets.is_empty() {
                missing(&default)
            } else {
                targets.iter().any(missing)
            })
        {
            continue;
        }
        let mut row: Vec<String> = vec![key.clone()];
        row.extend(
            langs
                .iter()
                .map(|l| p.string(l, &key).unwrap_or_default().to_owned()),
        );
        if let Some(c) = &contexts {
            let ctx = c.get(&key).cloned().unwrap_or_default();
            row.push(ctx.max_chars().map(|m| m.to_string()).unwrap_or_default());
            row.push(ctx.lines().join("; "));
            row.push(String::new());
        }
        w.write_record(&row).map_err(csv_err)?;
        n += 1;
    }
    let content = w.into_inner().map_err(csv_err)?;
    Ok(ExportFile {
        name: format!("{}-translations.csv", file_stem(p)),
        lang: None,
        keys: n,
        skipped: Vec::new(),
        content,
    })
}

/// The most frequent of `;`, `,` and tab in the first line (outside quotes).
fn sniff_delimiter(text: &str) -> u8 {
    let mut counts = [0usize; 3];
    let mut quoted = false;
    for c in text.chars() {
        match c {
            '"' => quoted = !quoted,
            '\n' if !quoted => break,
            ',' if !quoted => counts[0] += 1,
            ';' if !quoted => counts[1] += 1,
            '\t' if !quoted => counts[2] += 1,
            _ => {}
        }
    }
    let best = (0..3)
        .max_by_key(|&i| (counts[i], usize::from(i == 0)))
        .unwrap_or(0);
    b",;\t"[best]
}

pub(super) fn parse(p: &Project, text: &str) -> Result<Parsed, ExchangeError> {
    let mut r = csv::ReaderBuilder::new()
        .delimiter(sniff_delimiter(text))
        .has_headers(false)
        .flexible(true)
        .from_reader(text.as_bytes());
    let mut records = r.records();
    let header = match records.next() {
        Some(h) => h.map_err(csv_err)?,
        None => return Err(ExchangeError::Csv("the file is empty".to_owned())),
    };
    let mut parsed = Parsed::default();
    let mut key_col = None;
    let mut lang_cols: Vec<(usize, String)> = Vec::new();
    for (i, cell) in header.iter().enumerate() {
        let name = cell.trim();
        let lower = name.to_ascii_lowercase();
        if lower == "key" && key_col.is_none() {
            key_col = Some(i);
        } else if let Some(lang) = p
            .languages
            .available
            .iter()
            .find(|l| l.as_str().eq_ignore_ascii_case(name))
        {
            if lang_cols.iter().any(|(_, l)| l == lang.as_str()) {
                parsed.warnings.push(format!(
                    "column `{name}` appears twice; only the first is used"
                ));
            } else {
                lang_cols.push((i, lang.to_string()));
            }
        } else if !name.is_empty() && !INFO_COLUMNS.contains(&lower.as_str()) {
            parsed.unknown_columns.push(name.to_owned());
        }
    }
    let Some(key_col) = key_col else {
        return Err(ExchangeError::Csv(
            "no `key` column in the first row".to_owned(),
        ));
    };
    if lang_cols.is_empty() {
        return Err(ExchangeError::Csv(format!(
            "no column named after a project language ({})",
            p.languages
                .available
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>()
                .join(", ")
        )));
    }
    parsed.langs = lang_cols.iter().map(|(_, l)| l.clone()).collect();
    for rec in records {
        let rec = rec.map_err(csv_err)?;
        let line = rec.position().map_or(0, csv::Position::line);
        let key = rec.get(key_col).unwrap_or_default().trim();
        if key.is_empty() {
            if rec.iter().any(|c| !c.trim().is_empty()) {
                parsed
                    .warnings
                    .push(format!("line {line}: row without key ignored"));
            }
            continue;
        }
        for (col, lang) in &lang_cols {
            let cell = rec.get(*col).unwrap_or_default();
            parsed.entries.push(Entry {
                lang: lang.clone(),
                key: key.to_owned(),
                value: (!cell.is_empty()).then(|| cell.to_owned()),
                problems: Vec::new(),
            });
        }
    }
    Ok(parsed)
}
