//! `vitrine fonts`: the font families text styles can use – the bundled
//! set (always available, `vitrine_core::layout::bundled`) and the
//! families in the project's `fonts/` folder.

use std::collections::BTreeMap;

use clap::Args;
use serde::Serialize;
use vitrine_core::layout::FontLibrary;
use vitrine_core::layout::bundled::{self, FontCategory};

use crate::term::Style;
use crate::{EXIT_OK, Format, Global};

#[derive(Debug, Args)]
pub struct FontsArgs {
    #[arg(long, value_enum, default_value_t = Format::Text)]
    pub format: Format,
}

#[derive(Debug, Serialize)]
struct FamilyEntry {
    family: String,
    /// `bundled` or `project`.
    source: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    category: Option<FontCategory>,
    /// Lightest and heaviest weight (variable fonts: any weight between).
    weights: [u16; 2],
    italic: bool,
}

fn category_name(c: FontCategory) -> &'static str {
    match c {
        FontCategory::SansSerif => "sans serif",
        FontCategory::Serif => "serif",
        FontCategory::Display => "display",
        FontCategory::Handwriting => "handwriting",
        FontCategory::Monospace => "monospace",
    }
}

fn families(global: &Global) -> Vec<FamilyEntry> {
    let mut out: Vec<FamilyEntry> = bundled::FAMILIES
        .iter()
        .map(|fam| {
            let files: Vec<_> = bundled::files_of(fam.family).collect();
            FamilyEntry {
                family: fam.family.to_owned(),
                source: "bundled",
                category: Some(fam.category),
                weights: [
                    files.iter().map(|f| f.weights.0).min().unwrap_or(400),
                    files.iter().map(|f| f.weights.1).max().unwrap_or(400),
                ],
                italic: files.iter().any(|f| f.italic),
            }
        })
        .collect();
    let mut project = FontLibrary::empty();
    let report = project.load_dir(&global.project.join("fonts"));
    if !global.quiet {
        crate::project::report_font_errors(&report.errors);
    }
    let mut by_family: BTreeMap<&str, FamilyEntry> = BTreeMap::new();
    for face in project.faces() {
        let e = by_family
            .entry(&face.family)
            .or_insert_with(|| FamilyEntry {
                family: face.family.clone(),
                source: "project",
                category: None,
                weights: [face.weights.0, face.weights.1],
                italic: false,
            });
        e.weights = [
            e.weights[0].min(face.weights.0),
            e.weights[1].max(face.weights.1),
        ];
        e.italic |= face.italic;
    }
    out.extend(by_family.into_values());
    out
}

pub fn run(args: &FontsArgs, global: &Global) -> anyhow::Result<u8> {
    let list = families(global);
    if args.format == Format::Json {
        println!("{}", serde_json::to_string_pretty(&list)?);
        return Ok(EXIT_OK);
    }
    let style = Style::stdout();
    for f in &list {
        let weights = if f.weights[0] == f.weights[1] {
            f.weights[0].to_string()
        } else {
            format!("{}–{}", f.weights[0], f.weights[1])
        };
        let kind = f.category.map_or(f.source, category_name);
        println!(
            "{:<18} {}",
            f.family,
            style.dim(&format!(
                "{} · {} · weight {weights}{}",
                f.source,
                kind,
                if f.italic { " · italic" } else { "" }
            ))
        );
    }
    Ok(EXIT_OK)
}
