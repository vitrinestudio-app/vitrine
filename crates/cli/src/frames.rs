//! `vitrine frames import|list|remove`: the app-wide store of imported
//! vendor frames (Apple Product Bezels, KONZEPT §4.2, ADR 0012). The store
//! lives in the app data folder (same as the GUI; `VITRINE_APP_DIR`
//! overrides it), never in a project.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use anyhow::{Context, bail};
use clap::{Args, Subcommand};
use vitrine_core::frame_import::pack::{
    self, PACKS_DIR, PROJECT_FRAMES_DIR, PackInfo, PackOrigin, PackTarget,
};
use vitrine_core::frame_import::{
    APP_DIR_ENV, Action, FRAMES_DIR, FrameIndex, FrameStore, ImportPlan, Package, RemoveFilter,
    Rules, import, orientation_name,
};
use vitrine_core::model::Orientation;
use vitrine_render::frame_packs::{ResvgRasterizer, load_packs};

use crate::term::{Style, plural};
use crate::{EXIT_OK, EXIT_PROBLEMS, Format, Global};

#[derive(Debug, Args)]
pub struct FramesArgs {
    #[command(subcommand)]
    pub command: FramesCommand,
}

#[derive(Debug, Subcommand)]
pub enum FramesCommand {
    /// Import a downloaded frame package (folder, .zip or, on macOS, .dmg)
    /// or an own frame pack (folder or .zip with `frame.yaml`).
    Import(ImportArgs),
    /// List the installed frames.
    List(ListArgs),
    /// Remove installed frames.
    Remove(RemoveArgs),
}

#[derive(Debug, Args)]
pub struct ImportArgs {
    /// The package: extracted folder, .zip or .dmg.
    pub path: PathBuf,
    /// Only show what would be imported.
    #[arg(long)]
    pub dry_run: bool,
    /// Frame source (rules in `crates/core/data/frame-import/<source>.yaml`).
    #[arg(long, default_value = "apple")]
    pub source: String,
    /// Own frame packs (a folder or .zip with `frame.yaml`, ADR 0024): where
    /// to put it, `app` (this computer) or `project` (the project's
    /// `frames/` folder, travels with it).
    #[arg(long, value_enum, default_value_t = PackTo::App)]
    pub to: PackTo,
    /// Own frame packs: the pack id (default: from its name).
    #[arg(long)]
    pub id: Option<String>,
    #[arg(long, value_enum, default_value_t = Format::Text)]
    pub format: Format,
}

/// Where a frame pack goes / comes from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
pub enum PackTo {
    App,
    Project,
}

impl From<PackTo> for PackTarget {
    fn from(t: PackTo) -> Self {
        match t {
            PackTo::App => PackTarget::App,
            PackTo::Project => PackTarget::Project,
        }
    }
}

#[derive(Debug, Args)]
pub struct ListArgs {
    #[arg(long, default_value = "apple")]
    pub source: String,
    /// List own frame packs (app-wide and the project's) instead.
    #[arg(long)]
    pub packs: bool,
    #[arg(long, value_enum, default_value_t = Format::Text)]
    pub format: Format,
}

#[derive(Debug, Args)]
pub struct RemoveArgs {
    #[arg(long, default_value = "apple")]
    pub source: String,
    /// Only this model.
    #[arg(long)]
    pub model: Option<String>,
    /// Only this colour id.
    #[arg(long)]
    pub color: Option<String>,
    /// Only this orientation (`portrait`, `landscape`).
    #[arg(long)]
    pub orientation: Option<String>,
    /// Only this vendor model (`"iPhone 18 Pro"` or `iphone-18-pro`).
    #[arg(long)]
    pub bezel: Option<String>,
    /// Required to remove everything of the source (no filter given).
    #[arg(long)]
    pub all: bool,
    /// Remove the own frame pack with this id instead (see `--from`).
    #[arg(long, value_name = "ID")]
    pub pack: Option<String>,
    /// With `--pack`: `app` or `project`.
    #[arg(long, value_enum, default_value_t = PackTo::App)]
    pub from: PackTo,
}

/// The app data folder (as the GUI's): `$VITRINE_APP_DIR/data` or the
/// platform data folder.
pub fn app_data_dir() -> Option<PathBuf> {
    if let Some(dir) = std::env::var_os(APP_DIR_ENV).filter(|v| !v.is_empty()) {
        return Some(PathBuf::from(dir).join("data"));
    }
    directories::ProjectDirs::from("app", "", "vitrinestudio").map(|d| d.data_dir().to_owned())
}

/// The frame store root.
pub fn frames_root() -> anyhow::Result<PathBuf> {
    app_data_dir()
        .map(|d| d.join(FRAMES_DIR))
        .context("no app data folder (no home folder?); set VITRINE_APP_DIR")
}

/// Installed frames plus frame packs (app-wide and the project's in
/// `project/frames/`, ADR 0024) for rendering and validation (`None` if
/// none).
pub fn installed(project: &Path) -> Option<Arc<FrameIndex>> {
    let root = frames_root().ok();
    vitrine_render::frame_packs::frame_index(root.as_deref(), Some(project))
}

/// Render context of the project in `root` with the installed frames.
pub fn render_context(
    root: &Path,
) -> (
    vitrine_render::scene::RenderContext,
    Option<vitrine_core::layout::FontDirReport>,
) {
    let (ctx, fonts) = vitrine_render::scene::RenderContext::new(root);
    (ctx.with_frames(installed(root)), fonts)
}

pub fn run(args: &FramesArgs, global: &Global) -> anyhow::Result<u8> {
    let store = FrameStore::new(frames_root()?);
    match &args.command {
        FramesCommand::Import(a) if pack::is_pack(&a.path) => run_pack_import(a, &store, global),
        FramesCommand::Import(a) => run_import(a, &store, global),
        FramesCommand::List(a) if a.packs => run_pack_list(a, &store, global),
        FramesCommand::List(a) => run_list(a, &store),
        FramesCommand::Remove(a) if a.pack.is_some() => run_pack_remove(a, &store, global),
        FramesCommand::Remove(a) => run_remove(a, &store, global),
    }
}

/// The project folder when it holds a project (for project packs).
fn project_dir(global: &Global) -> Option<&Path> {
    let p = global.project.as_path();
    p.join("project.yaml").is_file().then_some(p)
}

fn packs_dir(to: PackTo, store: &FrameStore, global: &Global) -> anyhow::Result<PathBuf> {
    Ok(match to {
        PackTo::App => store.root().join(PACKS_DIR),
        PackTo::Project => project_dir(global)
            .context("--to/--from project needs a project (--project <DIR> with project.yaml)")?
            .join(PROJECT_FRAMES_DIR),
    })
}

fn run_pack_import(a: &ImportArgs, store: &FrameStore, global: &Global) -> anyhow::Result<u8> {
    let dest = packs_dir(a.to, store, global)?;
    let r = pack::import_pack(
        &a.path,
        &dest,
        a.to.into(),
        a.id.as_deref(),
        !a.dry_run,
        Some(&ResvgRasterizer),
    )
    .map_err(anyhow::Error::msg)?;
    match a.format {
        Format::Json => println!("{}", serde_json::to_string_pretty(&r)?),
        Format::Text => {
            let st = Style::stdout();
            print_pack(&r.pack, global);
            println!(
                "{} {} pack `{}` ({}, {}){}",
                st.bold("frames:"),
                if r.applied {
                    "imported"
                } else {
                    "would import"
                },
                r.id,
                plural(r.pack.frames.len(), "frame"),
                plural(r.files, "file"),
                if r.applied {
                    format!(" → {}", r.pack.dir.display())
                } else {
                    " (dry run)".to_owned()
                }
            );
            if r.replaces {
                println!("{} replaces the pack `{}` there", st.warn("note:"), r.id);
            }
            println!("{} use it with `frame: {}`", st.dim("hint:"), r.pack.frame);
        }
    }
    Ok(if r.pack.errors.is_empty() {
        EXIT_OK
    } else {
        EXIT_PROBLEMS
    })
}

fn print_pack(p: &PackInfo, global: &Global) {
    let st = Style::stdout();
    let origin = match p.origin {
        PackOrigin::App => "app",
        PackOrigin::Project => "project",
    };
    println!(
        "{} {}  {}{}",
        st.bold(&p.frame),
        p.name,
        st.dim(&format!(
            "({origin}{}{})",
            p.license
                .as_deref()
                .map(|l| format!(", licence {l}"))
                .unwrap_or_default(),
            if p.shadows_app_pack {
                ", hides the app pack"
            } else {
                ""
            }
        )),
        if p.models.is_empty() {
            "  any device".to_owned()
        } else {
            format!("  {}", p.models.join(", "))
        }
    );
    let mut seen = std::collections::BTreeSet::new();
    for f in &p.frames {
        // One line per image (a frame for several models is listed once).
        if !seen.insert(&f.file) {
            continue;
        }
        println!(
            "  {:<12} {:<10} {:<20} {}×{}  screen {}×{} at {},{}  {}",
            f.color,
            orientation_name(f.orientation),
            f.apple_model,
            f.size.w,
            f.size.h,
            f.screen.w,
            f.screen.h,
            f.screen.x,
            f.screen.y,
            st.dim(&f.source_file)
        );
    }
    for e in &p.errors {
        println!("  {} {e}", st.error("error:"));
    }
    if !global.quiet {
        for w in &p.warnings {
            println!("  {} {w}", st.warn("warning:"));
        }
    }
}

fn run_pack_list(a: &ListArgs, store: &FrameStore, global: &Global) -> anyhow::Result<u8> {
    let packs = load_packs(Some(store.root()), project_dir(global));
    match a.format {
        Format::Json => println!("{}", serde_json::to_string_pretty(&packs)?),
        Format::Text => {
            if packs.is_empty() {
                println!(
                    "no frame packs in {} or the project's frames/ folder",
                    store.root().join(PACKS_DIR).display()
                );
            }
            for p in &packs {
                print_pack(p, global);
            }
        }
    }
    Ok(if packs.iter().any(|p| !p.errors.is_empty()) {
        EXIT_PROBLEMS
    } else {
        EXIT_OK
    })
}

fn run_pack_remove(a: &RemoveArgs, store: &FrameStore, global: &Global) -> anyhow::Result<u8> {
    let id = a.pack.as_deref().unwrap_or_default();
    let dir = packs_dir(a.from, store, global)?;
    let project = match a.from {
        PackTo::Project => project_dir(global),
        PackTo::App => None,
    };
    if !pack::remove_pack(&dir, id, project).map_err(anyhow::Error::msg)? {
        bail!("no frame pack `{id}` in {}", dir.display());
    }
    if !global.quiet {
        println!("removed frame pack `{id}`");
    }
    Ok(EXIT_OK)
}

fn rules(source: &str) -> anyhow::Result<&'static Rules> {
    Rules::builtin(source)
        .with_context(|| format!("unknown frame source `{source}` (known: apple)"))
}

fn run_import(a: &ImportArgs, store: &FrameStore, global: &Global) -> anyhow::Result<u8> {
    let rules = rules(&a.source)?;
    // A disk image with a licence agreement gives a clear explanation
    // (never accepted on the user's behalf).
    let package = Package::open(&a.path).map_err(anyhow::Error::new)?;
    let plan = import(store, &package, rules, !a.dry_run).map_err(anyhow::Error::msg)?;
    drop(package);
    let failed = plan
        .items
        .iter()
        .any(|i| matches!(i.action, Action::Failed));
    match a.format {
        Format::Json => println!("{}", serde_json::to_string_pretty(&plan)?),
        Format::Text => print_plan(&plan, store.root(), global),
    }
    Ok(if failed { EXIT_PROBLEMS } else { EXIT_OK })
}

fn print_plan(plan: &ImportPlan, root: &Path, global: &Global) {
    let st = Style::stdout();
    if plan.unverified_rules && !global.quiet {
        println!(
            "{} the `{}` file-name rules are not yet verified with a real package",
            st.warn("note:"),
            plan.source
        );
    }
    for i in &plan.items {
        let what = format!(
            "{} / {} / {} / {}",
            i.matched.model,
            i.matched.color,
            orientation_name(i.matched.orientation),
            i.matched.apple_model
        );
        let (tag, extra) = match &i.action {
            Action::Install => (st.ok("install "), String::new()),
            Action::Replace => (st.warn("replace "), String::new()),
            Action::Superseded { by } => (st.dim("skip    "), format!(" (superseded by `{by}`)")),
            Action::Failed => (
                st.error("failed  "),
                format!(" ({})", i.error.as_deref().unwrap_or_default()),
            ),
        };
        if global.quiet && matches!(i.action, Action::Superseded { .. }) {
            continue;
        }
        let screen = i
            .screen
            .map(|s| format!("  screen {}×{} at {},{}", s.w, s.h, s.x, s.y))
            .unwrap_or_default();
        println!("{tag} {what:<62} ← {}{extra}", i.name);
        if global.verbose && !screen.is_empty() {
            println!("         {}", st.dim(screen.trim()));
        }
        for w in &i.warnings {
            println!("         {} {w}", st.warn("warning:"));
        }
    }
    // Skipped on purpose by a `skip` rule of the source (not a failure),
    // ignored by the user's mapping, or not recognised by any rule.
    for u in &plan.unmatched {
        let (tag, why) = match (&u.reason, u.ignored) {
            (Some(r), _) => ("skipped ", format!("  (on purpose: {r})")),
            (None, true) => ("ignored ", "  (your mapping)".to_owned()),
            (None, false) => ("unknown ", "  (no rule matched)".to_owned()),
        };
        println!("{} {}{}", st.dim(tag), u.file, st.dim(&why));
    }
    let n = plan.installs().count();
    let verb = if plan.applied {
        "imported"
    } else {
        "would import"
    };
    let mut counts = vec![format!(
        "{} unmatched",
        plan.unmatched.iter().filter(|u| !u.ignored).count()
    )];
    let on_purpose = plan.unmatched.iter().filter(|u| u.reason.is_some()).count();
    if on_purpose > 0 {
        counts.push(format!("{on_purpose} skipped on purpose"));
    }
    let by_you = plan
        .unmatched
        .iter()
        .filter(|u| u.ignored && u.reason.is_none())
        .count();
    if by_you > 0 {
        counts.push(format!("{by_you} ignored by your mapping"));
    }
    if plan.psd > 0 {
        counts.push(format!("{} Photoshop file(s) ignored", plan.psd));
    }
    if plan.skipped > 0 {
        counts.push(format!("{} other file(s) skipped", plan.skipped));
    }
    println!(
        "{} {verb} {} from {} ({}){}",
        st.bold("frames:"),
        plural(n, "frame"),
        plan.package,
        counts.join(", "),
        if plan.applied {
            format!(" → {}", root.join(&plan.source).display())
        } else {
            " (dry run)".to_owned()
        }
    );
    print_license_notice(plan);
}

/// "These frames are subject to Apple's licence …" when the package has a
/// licence file.
fn print_license_notice(plan: &ImportPlan) {
    let Some(first) = plan.licenses.first() else {
        return;
    };
    let st = Style::stdout();
    let name = first.rsplit(['/', '\\']).next().unwrap_or(first);
    println!(
        "{} these frames are subject to Apple's licence ({name} in the package). \
         They are stored only on this computer and never in projects.",
        st.warn("licence:")
    );
    for l in &plan.licenses {
        println!("         {}", st.dim(l));
    }
}

fn run_list(a: &ListArgs, store: &FrameStore) -> anyhow::Result<u8> {
    // Stores imported before swatches existed: measure them once.
    let _ = store.fill_swatches(&a.source);
    let index = store.index();
    let frames = index.frames(&a.source);
    match a.format {
        Format::Json => println!("{}", serde_json::to_string_pretty(frames)?),
        Format::Text => {
            let st = Style::stdout();
            if frames.is_empty() {
                println!(
                    "no `{}` frames installed in {}",
                    a.source,
                    store.root().display()
                );
            }
            for f in frames {
                println!(
                    "{:<14} {:<20} {:<8} {:<10} {:<20} {}×{}  screen {}×{}  {}",
                    f.model,
                    f.color,
                    f.swatch.as_deref().unwrap_or("-"),
                    orientation_name(f.orientation),
                    f.apple_model,
                    f.size.w,
                    f.size.h,
                    f.screen.w,
                    f.screen.h,
                    st.dim(&format!(
                        "{} ({}, {})",
                        f.source_file, f.package, f.imported
                    ))
                );
            }
        }
    }
    Ok(EXIT_OK)
}

fn run_remove(a: &RemoveArgs, store: &FrameStore, global: &Global) -> anyhow::Result<u8> {
    let orientation = match a.orientation.as_deref() {
        None => None,
        Some("portrait") => Some(Orientation::Portrait),
        Some("landscape") => Some(Orientation::Landscape),
        Some(o) => bail!("invalid orientation `{o}` (portrait, landscape)"),
    };
    let filter = RemoveFilter {
        model: a.model.as_deref(),
        color: a.color.as_deref(),
        orientation,
        bezel: a.bezel.as_deref(),
    };
    if filter.model.is_none()
        && filter.color.is_none()
        && filter.orientation.is_none()
        && filter.bezel.is_none()
        && !a.all
    {
        bail!(
            "pass --model/--color/--orientation/--bezel, or --all to remove every `{}` frame",
            a.source
        );
    }
    let n = store
        .remove(&a.source, &filter)
        .map_err(anyhow::Error::msg)?;
    if !global.quiet {
        println!("removed {}", plural(n, "frame"));
    }
    Ok(EXIT_OK)
}
