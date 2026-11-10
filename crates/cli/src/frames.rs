//! `arrangia frames import|list|remove`: the app-wide store of imported
//! vendor frames (Apple Product Bezels, KONZEPT §4.2, ADR 0012). The store
//! lives in the app data folder (same as the GUI; `ARRANGIA_APP_DIR`
//! overrides it), never in a project.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use anyhow::{Context, bail};
use arrangia_core::frame_import::{
    APP_DIR_ENV, Action, FRAMES_DIR, FrameIndex, FrameStore, ImportPlan, Package, RemoveFilter,
    Rules, import, orientation_name,
};
use arrangia_core::model::Orientation;
use clap::{Args, Subcommand};

use crate::term::{Style, plural};
use crate::{EXIT_OK, EXIT_PROBLEMS, Format, Global};

#[derive(Debug, Args)]
pub struct FramesArgs {
    #[command(subcommand)]
    pub command: FramesCommand,
}

#[derive(Debug, Subcommand)]
pub enum FramesCommand {
    /// Import a downloaded frame package (folder, .zip or, on macOS, .dmg).
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
    #[arg(long, value_enum, default_value_t = Format::Text)]
    pub format: Format,
}

#[derive(Debug, Args)]
pub struct ListArgs {
    #[arg(long, default_value = "apple")]
    pub source: String,
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
}

/// The app data folder (as the GUI's): `$ARRANGIA_APP_DIR/data` or the
/// platform data folder.
pub fn app_data_dir() -> Option<PathBuf> {
    if let Some(dir) = std::env::var_os(APP_DIR_ENV).filter(|v| !v.is_empty()) {
        return Some(PathBuf::from(dir).join("data"));
    }
    directories::ProjectDirs::from("app", "Arrangia", "Arrangia").map(|d| d.data_dir().to_owned())
}

/// The frame store root.
pub fn frames_root() -> anyhow::Result<PathBuf> {
    app_data_dir()
        .map(|d| d.join(FRAMES_DIR))
        .context("no app data folder (no home folder?); set ARRANGIA_APP_DIR")
}

/// Installed frames for rendering and validation (`None` if none).
pub fn installed() -> Option<Arc<FrameIndex>> {
    let root = frames_root().ok()?;
    let index = FrameIndex::load(&root);
    (!index.is_empty()).then(|| Arc::new(index))
}

/// Render context of the project in `root` with the installed frames.
pub fn render_context(
    root: &Path,
) -> (
    arrangia_render::scene::RenderContext,
    Option<arrangia_core::layout::FontDirReport>,
) {
    let (ctx, fonts) = arrangia_render::scene::RenderContext::new(root);
    (ctx.with_frames(installed()), fonts)
}

pub fn run(args: &FramesArgs, global: &Global) -> anyhow::Result<u8> {
    let store = FrameStore::new(frames_root()?);
    match &args.command {
        FramesCommand::Import(a) => run_import(a, &store, global),
        FramesCommand::List(a) => run_list(a, &store),
        FramesCommand::Remove(a) => run_remove(a, &store, global),
    }
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
    for u in &plan.unmatched {
        let tag = if u.ignored { "ignored " } else { "unknown " };
        println!("{} {}", st.dim(tag), u.file);
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
