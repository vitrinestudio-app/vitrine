//! `vitrine new`: a project folder (KONZEPT §3.1) from a built-in template.

use std::path::PathBuf;

use anyhow::Context;
use clap::{Args, ValueEnum};
use vitrine_core::model::Lang;
use vitrine_core::templates::{self, create_project, default_name, parse_langs};

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

/// Command line names of the built-in templates (`vitrine_core::templates`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum Template {
    /// Three phone pages with gradient, headlines and devices; targets
    /// iPhone 6.9″ (App Store), Google Play phone and a blog banner.
    Phone,
    /// One page with a headline and a device; target iPhone 6.9″.
    Minimal,
}

impl From<Template> for templates::Template {
    fn from(t: Template) -> Self {
        match t {
            Template::Phone => Self::Phone,
            Template::Minimal => Self::Minimal,
        }
    }
}

pub fn run(args: &NewArgs, global: &Global) -> anyhow::Result<u8> {
    let dir = &args.dir;
    let langs = parse_langs(&args.langs)?;
    let name = args.name.clone().unwrap_or_else(|| default_name(dir));
    let template: templates::Template = args.template.into();
    let project = create_project(dir, &name, template, &langs)
        .with_context(|| format!("cannot create project `{}`", dir.display()))?;

    if !global.quiet {
        let style = crate::term::Style::stdout();
        let lang_list: Vec<&str> = langs.iter().map(Lang::as_str).collect();
        println!(
            "{} project `{}` in {} (template {}, languages {})",
            style.ok("Created"),
            project.name,
            dir.display(),
            template.name(),
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
        println!("  vitrine validate --project {p}");
        println!("  vitrine render --project {p} --canvas");
        println!("  vitrine export --project {p}");
    }
    Ok(EXIT_OK)
}
