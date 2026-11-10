//! Translation exchange with translators (KONZEPT §5.1, ADR 0011): CSV and
//! XLIFF 1.2 / 2.0 export and import.
//!
//! - **Export** ([`export`]) builds files in memory; front ends write them
//!   ([`output_paths`] picks the names). CSV: one table `key, <default>,
//!   <lang>…` (+ `max_chars, context, notes` with [`ExportOptions::context`]).
//!   XLIFF: one file per target language, source = default language.
//! - **Import** ([`plan_import`]) reads files of any of the formats (detected
//!   from extension/content) and compares them with the project: an
//!   [`ImportPlan`] lists new/changed/removed/unchanged/skipped strings per
//!   language, unknown keys and columns, and markup issues.
//!   [`check_layout`] adds the overflow findings of the changed languages
//!   (`text_overflow`, `emergency_break`). [`ImportPlan::ops`] gives the
//!   `set_string` ops to apply as one transaction (one undo step).
//!
//! Inline markup in XLIFF ([`MarkupMode::Inline`], default): bold, italic
//! and `[span]…[/]` become `<pc>` (2.0) / `<g>` (1.2), line breaks and soft
//! hyphens `<ph>` / `<x/>`, so CAT tools protect them; the round trip is
//! lossless. [`MarkupMode::Plain`] keeps the Arrangia markup as text.
//! CSV always holds the markup as text.

mod context;
mod inline;
mod plan;
mod table;
#[cfg(test)]
mod tests;
mod xliff;

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

pub use context::{AVG_CHAR_EM, KeyContext, KeyUsage, all_keys, key_contexts};
pub use inline::{Node, parse_tree, same_markup, to_markup};
pub use plan::{
    ChangeKind, EmptyCells, FileReport, ImportInput, ImportOptions, ImportPlan, IssueKind,
    LangReport, LayoutFinding, MarkupIssue, StringChange, UnknownKey, check_layout, plan_import,
};

use crate::model::Project;

/// Exchange file format.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Format {
    Csv,
    Xliff12,
    Xliff20,
}

impl Format {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Csv => "csv",
            Self::Xliff12 => "xliff12",
            Self::Xliff20 => "xliff20",
        }
    }

    pub fn extension(self) -> &'static str {
        match self {
            Self::Csv => "csv",
            Self::Xliff12 | Self::Xliff20 => "xlf",
        }
    }
}

impl std::str::FromStr for Format {
    type Err = ExchangeError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_ascii_lowercase().as_str() {
            "csv" => Ok(Self::Csv),
            "xliff12" | "xliff-1.2" | "xlf12" => Ok(Self::Xliff12),
            "xliff20" | "xliff-2.0" | "xliff" | "xlf" => Ok(Self::Xliff20),
            _ => Err(ExchangeError::UnknownFormat(s.to_owned())),
        }
    }
}

/// How inline markup is written to XLIFF.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MarkupMode {
    /// Inline elements (`<pc>`/`<ph>`, `<g>`/`<x/>`).
    #[default]
    Inline,
    /// Arrangia markup as plain text (`**…**`, `[accent]…[/]`).
    Plain,
}

/// CSV field delimiter.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Delimiter {
    #[default]
    Comma,
    /// What Excel uses in locales with a decimal comma.
    Semicolon,
    Tab,
}

impl Delimiter {
    pub fn byte(self) -> u8 {
        match self {
            Self::Comma => b',',
            Self::Semicolon => b';',
            Self::Tab => b'\t',
        }
    }
}

/// Export options.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct ExportOptions {
    /// Target languages (empty = all except the default language). CSV
    /// always has the default language as the first text column.
    pub langs: Vec<String>,
    /// Only keys missing in a target language.
    pub missing_only: bool,
    /// Context for translators (elements, pages, box width, length hint).
    pub context: bool,
    pub delimiter: Delimiter,
    /// UTF-8 byte order mark (Excel needs it to detect UTF-8).
    pub bom: bool,
    pub markup: MarkupMode,
}

/// One exported file (in memory).
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct ExportFile {
    /// Default file name (`<project>-de.xlf`, `<project>-translations.csv`).
    pub name: String,
    /// Target language (XLIFF only).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub lang: Option<String>,
    /// Keys written.
    pub keys: usize,
    /// Keys left out because they have no text in the default language
    /// (XLIFF needs a source).
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub skipped: Vec<String>,
    #[serde(skip)]
    pub content: Vec<u8>,
}

/// Exchange errors (a whole file or request failed).
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ExchangeError {
    #[error("unknown format `{0}` (csv, xliff12, xliff20)")]
    UnknownFormat(String),
    #[error("`{0}` is not a project language")]
    UnknownLanguage(String),
    #[error("no target language: the project has only its default language")]
    NoTargetLanguage,
    #[error("the file is not UTF-8 text (in Excel, save as “CSV UTF-8”)")]
    NotUtf8,
    #[error("CSV: {0}")]
    Csv(String),
    #[error("XLIFF: {0}")]
    Xliff(String),
}

/// File name stem from the project name (`Fernwood`, `grove.eco`).
fn file_stem(p: &Project) -> String {
    let s: String = p
        .name
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || matches!(c, '.' | '-' | '_') {
                c
            } else {
                '-'
            }
        })
        .collect();
    let s = s.trim_matches(['-', '.']).to_owned();
    if s.is_empty() {
        "strings".to_owned()
    } else {
        s
    }
}

/// Validated target languages in project order (default excluded).
fn target_langs(p: &Project, langs: &[String]) -> Result<Vec<String>, ExchangeError> {
    for l in langs {
        if !p.languages.is_available(l) {
            return Err(ExchangeError::UnknownLanguage(l.clone()));
        }
    }
    Ok(p.languages
        .available
        .iter()
        .map(ToString::to_string)
        .filter(|l| *l != p.languages.default.as_str())
        .filter(|l| langs.is_empty() || langs.contains(l))
        .collect())
}

/// Builds the export files.
pub fn export(
    p: &Project,
    format: Format,
    opts: &ExportOptions,
) -> Result<Vec<ExportFile>, ExchangeError> {
    let targets = target_langs(p, &opts.langs)?;
    match format {
        Format::Csv => Ok(vec![table::export(p, &targets, opts)?]),
        Format::Xliff12 | Format::Xliff20 => {
            if targets.is_empty() {
                return Err(ExchangeError::NoTargetLanguage);
            }
            Ok(targets
                .iter()
                .map(|l| xliff::export(p, l, format, opts))
                .collect())
        }
    }
}

/// Where to write `files`: into `out` when it is a folder; else `out`
/// itself for a single file, or `<out stem>-<lang>.<ext>` next to it.
pub fn output_paths(files: &[ExportFile], out: &Path, out_is_dir: bool) -> Vec<PathBuf> {
    if out_is_dir {
        return files.iter().map(|f| out.join(&f.name)).collect();
    }
    if files.len() == 1 {
        return vec![out.to_path_buf()];
    }
    let dir = out.parent().unwrap_or(Path::new(""));
    let stem = out.file_stem().map_or_else(
        || "strings".to_owned(),
        |s| s.to_string_lossy().into_owned(),
    );
    files
        .iter()
        .map(|f| {
            let ext = Path::new(&f.name)
                .extension()
                .map(|e| e.to_string_lossy().into_owned())
                .unwrap_or_default();
            let lang = f.lang.as_deref().unwrap_or("all");
            dir.join(format!("{stem}-{lang}.{ext}"))
        })
        .collect()
}

/// Bytes → text: strips a UTF-8 BOM, decodes UTF-16 with BOM.
pub(crate) fn decode_text(data: &[u8]) -> Result<String, ExchangeError> {
    if let Some(rest) = data.strip_prefix(b"\xEF\xBB\xBF") {
        return String::from_utf8(rest.to_vec()).map_err(|_| ExchangeError::NotUtf8);
    }
    let utf16 = |rest: &[u8], le: bool| {
        let units = rest.as_chunks::<2>().0.iter().map(|c| {
            if le {
                u16::from_le_bytes([c[0], c[1]])
            } else {
                u16::from_be_bytes([c[0], c[1]])
            }
        });
        char::decode_utf16(units)
            .collect::<Result<String, _>>()
            .map_err(|_| ExchangeError::NotUtf8)
    };
    if let Some(rest) = data.strip_prefix(b"\xFF\xFE") {
        return utf16(rest, true);
    }
    if let Some(rest) = data.strip_prefix(b"\xFE\xFF") {
        return utf16(rest, false);
    }
    String::from_utf8(data.to_vec()).map_err(|_| ExchangeError::NotUtf8)
}

/// Format of an import file: by extension, else by content.
pub fn detect_format(name: &str, text: &str) -> Format {
    let ext = Path::new(name)
        .extension()
        .map(|e| e.to_string_lossy().to_ascii_lowercase());
    match ext.as_deref() {
        Some("csv" | "tsv" | "txt") => Format::Csv,
        _ if text.trim_start().starts_with('<') => {
            if xliff::looks_like_12(text) {
                Format::Xliff12
            } else {
                Format::Xliff20
            }
        }
        _ => Format::Csv,
    }
}
