//! `vitrine new`: a project folder (KONZEPT §3.1) from a template (built
//! in, installed in the app-wide store, or a `.vitrine-template` file).

use std::path::PathBuf;

use anyhow::Context;
use clap::Args;
use vitrine_core::model::Lang;
use vitrine_core::template::builtin;
use vitrine_core::template::instantiate::{create_project, default_name, parse_langs};

use crate::{EXIT_OK, Global};

#[derive(Debug, Args)]
pub struct NewArgs {
    /// Folder to create (must not exist or be empty).
    pub dir: PathBuf,
    /// Project name (default: the folder name).
    #[arg(long)]
    pub name: Option<String>,
    /// Template: built in (`phone`, `minimal`), the id of an installed
    /// template (`vitrine template list`) or a `.vitrine-template` file.
    #[arg(long, default_value = builtin::DEFAULT, value_name = "NAME|FILE")]
    pub template: String,
    /// Project language; repeat for more. The first is the default language
    /// (fallback for texts and screenshots). Default: `en`.
    #[arg(long = "lang", value_name = "LANG")]
    pub langs: Vec<String>,
}

pub fn run(args: &NewArgs, global: &Global) -> anyhow::Result<u8> {
    let dir = &args.dir;
    let langs = parse_langs(&args.langs)?;
    let name = args.name.clone().unwrap_or_else(|| default_name(dir));
    let store = crate::template::store()?;
    let pkg = store.find(&args.template)?;
    let created = create_project(&pkg, dir, &name, &langs)
        .with_context(|| format!("cannot create project `{}`", dir.display()))?;
    let project = created.project;

    if !global.quiet {
        let style = crate::term::Style::stdout();
        let lang_list: Vec<&str> = langs.iter().map(Lang::as_str).collect();
        println!(
            "{} project `{}` in {} (template {}, languages {})",
            style.ok("Created"),
            project.name,
            dir.display(),
            pkg.manifest.name,
            lang_list.join(", ")
        );
        for (lang, from) in &created.copied_texts {
            println!("  texts for {lang} copied from {from} – translate them in i18n/{lang}.yaml");
        }
        let mut shots: Vec<String> = Vec::new();
        for s in project.targets.iter().filter_map(|t| t.screenshots.clone()) {
            if !shots.contains(&s) {
                shots.push(s);
            }
        }
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
        println!("  vitrine validate --project {p}");
        println!("  vitrine render --project {p} --canvas");
        println!("  vitrine export --project {p}");
    }
    Ok(EXIT_OK)
}
