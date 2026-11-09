//! `arrangia new`: a project folder (KONZEPT §3.1) from a built-in template.

use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, bail};
use arrangia_core::Project;
use arrangia_core::io::{I18N_DIR, PROJECT_FILE, project_from_yaml};
use arrangia_core::model::Lang;
use clap::{Args, ValueEnum};

use crate::{EXIT_OK, Global};

#[derive(Debug, Args)]
pub struct NewArgs {
    /// Folder to create (must not exist or be empty).
    pub dir: PathBuf,
    /// Project name (default: the folder name).
    #[arg(long)]
    pub name: Option<String>,
    /// Template to start from.
    #[arg(long, value_enum, default_value_t = Template::Phone)]
    pub template: Template,
    /// Project language; repeat for more. The first is the default language
    /// (fallback for texts and screenshots). Default: `en`.
    #[arg(long = "lang", value_name = "LANG")]
    pub langs: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum Template {
    /// Three phone pages with gradient, headlines and devices; targets
    /// iPhone 6.9″ (App Store), Google Play phone and a blog banner.
    Phone,
    /// One page with a headline and a device; target iPhone 6.9″.
    Minimal,
}

struct Files {
    project: &'static str,
    /// Texts per language; the first entry is used for languages without
    /// their own entry.
    strings: &'static [(&'static str, &'static str)],
}

impl Template {
    fn files(self) -> Files {
        match self {
            Self::Phone => Files {
                project: include_str!("../templates/phone/project.yaml"),
                strings: &[
                    ("en", include_str!("../templates/phone/en.yaml")),
                    ("de", include_str!("../templates/phone/de.yaml")),
                ],
            },
            Self::Minimal => Files {
                project: include_str!("../templates/minimal/project.yaml"),
                strings: &[
                    ("en", include_str!("../templates/minimal/en.yaml")),
                    ("de", include_str!("../templates/minimal/de.yaml")),
                ],
            },
        }
    }

    fn name(self) -> &'static str {
        match self {
            Self::Phone => "phone",
            Self::Minimal => "minimal",
        }
    }
}

/// `project.yaml` text with name and languages filled in.
fn project_yaml(template: &str, name: &str, langs: &[Lang]) -> anyhow::Result<String> {
    let list: Vec<&str> = langs.iter().map(Lang::as_str).collect();
    // A JSON string is a valid double-quoted YAML scalar.
    let yaml = template
        .replace("__NAME__", &serde_json::to_string(name)?)
        .replace("__DEFAULT_LANG__", list[0])
        .replace("__LANGS__", &format!("[{}]", list.join(", ")));
    project_from_yaml(&yaml).map_err(|e| anyhow::anyhow!("template does not parse: {e}"))?;
    Ok(yaml)
}

/// Texts for `lang`: exact match, then the primary subtag (`de-AT` → `de`),
/// else the template's first language marked for translation.
fn strings_for(files: &Files, lang: &str) -> String {
    let primary = lang.split(['-', '_']).next().unwrap_or(lang);
    if let Some((_, s)) = files
        .strings
        .iter()
        .find(|(l, _)| *l == lang)
        .or_else(|| files.strings.iter().find(|(l, _)| *l == primary))
    {
        return (*s).to_owned();
    }
    let (from, s) = files.strings[0];
    format!("# TODO: translate to `{lang}` (copied from `{from}`)\n{s}")
}

fn is_empty_dir(dir: &Path) -> anyhow::Result<bool> {
    Ok(fs::read_dir(dir)
        .with_context(|| format!("cannot read `{}`", dir.display()))?
        .next()
        .is_none())
}

fn default_name(dir: &Path) -> String {
    std::path::absolute(dir)
        .ok()
        .and_then(|p| {
            p.file_name()
                .map(|n| n.to_string_lossy().trim_end_matches(".arrangia").to_owned())
        })
        .filter(|n| !n.is_empty())
        .unwrap_or_else(|| "My App".to_owned())
}

pub fn run(args: &NewArgs, global: &Global) -> anyhow::Result<u8> {
    let dir = &args.dir;
    if dir.exists() {
        if !dir.is_dir() {
            bail!("`{}` exists and is not a folder", dir.display());
        }
        if !is_empty_dir(dir)? {
            bail!(
                "`{}` is not empty; refusing to overwrite (choose a new or empty folder)",
                dir.display()
            );
        }
    }

    let mut langs: Vec<Lang> = Vec::new();
    let default_langs = ["en".to_owned()];
    let given = if args.langs.is_empty() {
        &default_langs[..]
    } else {
        &args.langs[..]
    };
    for l in given {
        let lang = Lang::parse(l).map_err(|e| anyhow::anyhow!("invalid language `{l}`: {e}"))?;
        if langs.contains(&lang) {
            bail!("language `{l}` is given twice");
        }
        langs.push(lang);
    }
    let name = args.name.clone().unwrap_or_else(|| default_name(dir));
    let files = args.template.files();
    let yaml = project_yaml(files.project, &name, &langs)?;

    let write = |rel: &str, text: &str| -> anyhow::Result<()> {
        let path = dir.join(rel);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("cannot create `{}`", parent.display()))?;
        }
        fs::write(&path, text).with_context(|| format!("cannot write `{}`", path.display()))
    };
    write(PROJECT_FILE, &yaml)?;
    for lang in &langs {
        write(
            &format!("{I18N_DIR}/{lang}.yaml"),
            &strings_for(&files, lang.as_str()),
        )?;
    }
    write(".gitignore", "export/\n.cache/\n")?;
    for sub in [
        format!("screenshots/{}", langs[0]),
        "assets".to_owned(),
        "fonts".to_owned(),
    ] {
        let path = dir.join(sub);
        fs::create_dir_all(&path).with_context(|| format!("cannot create `{}`", path.display()))?;
    }

    // The written project must load (guards the embedded templates).
    let project = Project::load(dir)
        .with_context(|| format!("created project `{}` does not load", dir.display()))?;

    if !global.quiet {
        let style = crate::term::Style::stdout();
        let lang_list: Vec<&str> = langs.iter().map(Lang::as_str).collect();
        println!(
            "{} project `{}` in {} (template {}, languages {})",
            style.ok("Created"),
            project.name,
            dir.display(),
            args.template.name(),
            lang_list.join(", ")
        );
        let shots: Vec<String> = project
            .targets
            .iter()
            .filter_map(|t| t.screenshots.clone())
            .collect();
        println!(
            "Put screenshots into {}/screenshots/{}/<{}>/ (or directly into screenshots/{}/), then:",
            dir.display(),
            langs[0],
            if shots.is_empty() {
                "variant".to_owned()
            } else {
                shots.join("|")
            },
            langs[0],
        );
        let p = dir.display();
        println!("  arrangia validate --project {p}");
        println!("  arrangia render --project {p} --canvas");
        println!("  arrangia export --project {p}");
    }
    Ok(EXIT_OK)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn templates_parse_with_names_needing_quotes() {
        let langs = vec![Lang::parse("en").unwrap(), Lang::parse("de").unwrap()];
        for t in [Template::Phone, Template::Minimal] {
            let yaml = project_yaml(t.files().project, "Grove: \"eco\" #1", &langs).unwrap();
            let p = project_from_yaml(&yaml).unwrap();
            assert_eq!(p.name, "Grove: \"eco\" #1");
            assert_eq!(p.languages.available.len(), 2);
        }
    }

    #[test]
    fn strings_fall_back_to_primary_subtag_then_first() {
        let files = Template::Phone.files();
        assert!(strings_for(&files, "de-AT").contains("Deine App"));
        let fr = strings_for(&files, "fr");
        assert!(fr.starts_with("# TODO: translate to `fr`"));
        assert!(fr.contains("Your app"));
    }
}
