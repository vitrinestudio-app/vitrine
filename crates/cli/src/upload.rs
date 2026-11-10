//! `arrangia upload plan|execute|log`: store upload from the command line
//! (Phase 5.5, ADR 0018) over `arrangia_app::upload` – the same plan,
//! confirm token, executor and log as the GUI and MCP.
//!
//! Credentials come from the store account referenced in `project.yaml`
//! (`store.<store>.account`): its public facts from the app settings
//! (`--app-dir`, `$ARRANGIA_APP_DIR` or the platform folder), the key from
//! the system keychain (`ARRANGIA_KEYRING=mock` = in-memory, tests).

use std::io::{BufRead, IsTerminal, Write};
use std::path::PathBuf;
use std::sync::Arc;

use anyhow::{Context, anyhow, bail};
use arrangia_app::dialogs::NoDialogs;
use arrangia_app::events::EventSink;
use arrangia_app::settings::AppPaths;
use arrangia_app::{AppState, CommandError, Reply};
use clap::{Args, Subcommand, ValueEnum};
use serde_json::{Value, json};

use crate::term::Style;
use crate::{EXIT_OK, EXIT_PROBLEMS, Format, Global, project};

#[derive(Debug, Args)]
pub struct UploadArgs {
    /// App data folder (settings with the store accounts). Default:
    /// `$ARRANGIA_APP_DIR` or the platform folder.
    #[arg(long, value_name = "DIR", global = true)]
    pub app_dir: Option<PathBuf>,
    #[command(subcommand)]
    pub command: UploadCommand,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum StoreArg {
    Apple,
    Google,
}

#[derive(Debug, Subcommand)]
pub enum UploadCommand {
    /// Dry run: what would be uploaded, problems, and the confirm token.
    Plan {
        /// Only this store (default: every store the project is mapped to).
        #[arg(long, value_enum)]
        store: Option<StoreArg>,
        /// Only this target; repeat for more.
        #[arg(long = "target", value_name = "ID")]
        targets: Vec<String>,
        /// Only this language; repeat for more.
        #[arg(long = "lang", value_name = "LANG")]
        langs: Vec<String>,
        /// Also read the store state (network; uses the account's key).
        #[arg(long)]
        remote: bool,
        /// Upload sets even when they look unchanged.
        #[arg(long)]
        force: bool,
        #[arg(long, value_enum, default_value_t = Format::Text)]
        format: Format,
    },
    /// Upload a plan: needs its confirm token (same project state, 30 min).
    Execute {
        /// The `confirm_token` printed by `upload plan`.
        #[arg(long = "confirm", value_name = "TOKEN")]
        token: String,
        /// Do not ask before uploading.
        #[arg(long)]
        yes: bool,
        /// Google: commit with "changes not sent for review".
        #[arg(long)]
        changes_not_sent_for_review: bool,
        #[arg(long, value_enum, default_value_t = Format::Text)]
        format: Format,
    },
    /// App Store Connect asset types (read-only): the App Asset Library
    /// reference data – placement types, groups, display classes, image
    /// specs – and how Arrangia's slots resolve against it (ADR 0019).
    Refdata {
        /// Apple account id (default: the project's, else the only one).
        #[arg(long, value_name = "ID")]
        account: Option<String>,
        /// App: numeric Apple ID or bundle id (default: the project's);
        /// also checks that the app can use the asset library.
        #[arg(long, value_name = "ID")]
        app: Option<String>,
        #[arg(long, value_enum, default_value_t = Format::Text)]
        format: Format,
    },
    /// Past uploads from `<project>/.cache/uploads.sqlite`.
    Log {
        /// Only the last run.
        #[arg(long)]
        last: bool,
        /// Number of runs (default 10).
        #[arg(long, value_name = "N")]
        limit: Option<usize>,
        /// Include the HTTP events (method, path, status).
        #[arg(long)]
        events: bool,
        #[arg(long, value_enum, default_value_t = Format::Text)]
        format: Format,
    },
}

/// Prints `upload-progress` lines on stderr.
struct Progress {
    quiet: bool,
}

impl EventSink for Progress {
    fn emit(&self, event: &str, p: Value) {
        if self.quiet || event != arrangia_app::upload::UPLOAD_PROGRESS {
            return;
        }
        let style = Style::stderr();
        let set = &p["set"];
        let name = if set.is_null() {
            String::new()
        } else {
            format!(
                "{} {}/{} ({})",
                set["store"].as_str().unwrap_or_default(),
                set["target"].as_str().unwrap_or_default(),
                set["lang"].as_str().unwrap_or_default(),
                set["locale"].as_str().unwrap_or_default()
            )
        };
        let line = match p["step"].as_str().unwrap_or_default() {
            "prepare" => format!(
                "connecting to {}",
                p["file"]["store"].as_str().unwrap_or_default()
            ),
            "set_started" => format!("{name}: started"),
            "file_uploaded" => format!(
                "{name}: {} uploaded, Apple is processing it",
                p["file"]["file_name"].as_str().unwrap_or_default(),
            ),
            "file_done" => format!(
                "{name}: {} in the store ({}/{} images)",
                p["file"]["file_name"].as_str().unwrap_or_default(),
                p["images_done"],
                p["images_total"]
            ),
            "processing" => {
                let secs = p["file"]["elapsed_secs"].as_u64().unwrap_or(0);
                format!(
                    "{name}: Apple is processing {} image(s) ({}, {}:{:02})",
                    p["file"]["pending"],
                    p["file"]["state"].as_str().unwrap_or_default(),
                    secs / 60,
                    secs % 60
                )
            }
            "file_failed" => style.error(&format!(
                "{name}: Apple could not process {}: {}",
                p["file"]["file_name"].as_str().unwrap_or_default(),
                p["file"]["error"].as_str().unwrap_or_default()
            )),
            "set_finished" => format!(
                "{name}: {}{}{}",
                set["status"].as_str().unwrap_or_default(),
                set["error"]
                    .as_str()
                    .map(|e| format!(" – {e}"))
                    .unwrap_or_default(),
                set["note"]
                    .as_str()
                    .map(|n| format!(" ({n})"))
                    .unwrap_or_default()
            ),
            "paused" => style.warn(&format!(
                "{name}: rate limit – paused until {}",
                p["paused_until"]
            )),
            "commit" => "google: committing the edit".to_owned(),
            _ => return,
        };
        eprintln!("{line}");
    }
}

fn app_state(
    app_dir: Option<&PathBuf>,
    quiet: bool,
    dir: &std::path::Path,
) -> anyhow::Result<AppState> {
    let paths = match app_dir {
        Some(d) => AppPaths::in_dir(d),
        None => {
            AppPaths::platform().ok_or_else(|| anyhow!("no app data folder (set --app-dir)"))?
        }
    };
    project::load(dir)?;
    let app =
        AppState::new(paths, Arc::new(Progress { quiet }), Arc::new(NoDialogs)).without_watcher();
    app.open_project(dir).map_err(cmd_error)?;
    Ok(app)
}

#[allow(clippy::needless_pass_by_value)] // used as `map_err(cmd_error)`
fn cmd_error(e: CommandError) -> anyhow::Error {
    match &e.details {
        Some(d) => anyhow!("{} ({d})", e.message),
        None => anyhow!("{}", e.message),
    }
}

fn call(app: &AppState, name: &str, args: Value) -> anyhow::Result<Value> {
    match arrangia_app::dispatch(app, name, args).map_err(cmd_error)? {
        Reply::Json(v) => Ok(v),
        Reply::Image(_) => bail!("unexpected image"),
    }
}

pub fn run(args: &UploadArgs, global: &Global) -> anyhow::Result<u8> {
    if let UploadCommand::Refdata {
        account,
        app,
        format,
    } = &args.command
    {
        return refdata(
            args.app_dir.as_ref(),
            global,
            account.as_deref(),
            app.as_deref(),
            *format,
        );
    }
    let app = app_state(args.app_dir.as_ref(), global.quiet, &global.project)?;
    match &args.command {
        UploadCommand::Plan {
            store,
            targets,
            langs,
            remote,
            force,
            format,
        } => {
            let store = store.map(|s| match s {
                StoreArg::Apple => "apple",
                StoreArg::Google => "google",
            });
            let plan = call(
                &app,
                "upload_plan",
                json!({"store": store, "targets": targets, "langs": langs,
                       "remote": remote, "force": force}),
            )?;
            match format {
                Format::Json => println!("{}", serde_json::to_string_pretty(&plan)?),
                Format::Text => print_plan(&plan, global),
            }
            Ok(if plan["blocked"] == true {
                EXIT_PROBLEMS
            } else {
                EXIT_OK
            })
        }
        UploadCommand::Execute {
            token,
            yes,
            changes_not_sent_for_review,
            format,
        } => {
            let plan = arrangia_app::commands::upload_verify(&app, token).map_err(cmd_error)?;
            if !yes {
                confirm(&plan)?;
            }
            let out = call(
                &app,
                "upload_execute",
                json!({"token": token, "wait": true,
                       "changes_not_sent_for_review": changes_not_sent_for_review}),
            )?;
            let run = &out["run"];
            match format {
                Format::Json => println!("{}", serde_json::to_string_pretty(run)?),
                Format::Text => print_run(run),
            }
            Ok(if run["status"] == "completed" {
                EXIT_OK
            } else {
                EXIT_PROBLEMS
            })
        }
        UploadCommand::Log {
            last,
            limit,
            events,
            format,
        } => {
            let limit = if *last { 1 } else { limit.unwrap_or(10) };
            let log = call(
                &app,
                "upload_log",
                json!({"limit": limit, "events": events}),
            )?;
            match format {
                Format::Json => println!("{}", serde_json::to_string_pretty(&log)?),
                Format::Text => print_log(&log, *events),
            }
            Ok(EXIT_OK)
        }
        UploadCommand::Refdata { .. } => unreachable!("handled above"),
    }
}

/// `upload refdata`: works without a project (then `--account` or the only
/// Apple account); with one, its account and app are the defaults.
fn refdata(
    app_dir: Option<&PathBuf>,
    global: &Global,
    account: Option<&str>,
    app_id: Option<&str>,
    format: Format,
) -> anyhow::Result<u8> {
    let paths = match app_dir {
        Some(d) => AppPaths::in_dir(d),
        None => {
            AppPaths::platform().ok_or_else(|| anyhow!("no app data folder (set --app-dir)"))?
        }
    };
    let app = AppState::new(
        paths,
        Arc::new(Progress { quiet: true }),
        Arc::new(NoDialogs),
    )
    .without_watcher();
    if global.project.join("project.yaml").is_file() {
        app.open_project(&global.project).map_err(cmd_error)?;
    }
    let report = call(
        &app,
        "upload_refdata",
        json!({"account": account, "app": app_id}),
    )?;
    match format {
        Format::Json => println!("{}", serde_json::to_string_pretty(&report)?),
        Format::Text => print!("{}", format_refdata(&report, Style::stdout())),
    }
    Ok(EXIT_OK)
}

fn strs(v: &Value) -> Vec<&str> {
    v.as_array()
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .collect()
}

fn spec_size(spec: &Value) -> String {
    let d = &spec["dimensions"];
    let range = |a: &Value, b: &Value| {
        if a == b {
            a.to_string()
        } else {
            format!("{a}-{b}")
        }
    };
    format!(
        "{}x{}",
        range(&d["minWidth"], &d["maxWidth"]),
        range(&d["minHeight"], &d["maxHeight"])
    )
}

/// The text report of `upload refdata`.
#[allow(clippy::too_many_lines)] // one linear report
fn format_refdata(r: &Value, style: Style) -> String {
    use std::fmt::Write as _;
    let mut out = String::new();
    let rd = &r["ref_data"];
    let _ = writeln!(
        out,
        "{} {}",
        style.bold("App Store Connect asset types"),
        style.dim(&format!(
            "(account {})",
            r["account"].as_str().unwrap_or("-")
        ))
    );
    if let Some(a) = r.get("app") {
        let _ = writeln!(
            out,
            "App: {} ({}, {})",
            a["name"].as_str().unwrap_or_default(),
            a["id"].as_str().unwrap_or_default(),
            a["bundle_id"].as_str().unwrap_or_default()
        );
        match (a["library_id"].as_str(), a["library_error"].as_str()) {
            (Some(id), _) => {
                let _ = writeln!(out, "  asset library: {}", style.ok(id));
            }
            (None, Some(e)) => {
                let _ = writeln!(out, "  asset library: {}", style.warn(e));
            }
            _ => {}
        }
    }
    let specs = rd["imageSpecs"].as_array().cloned().unwrap_or_default();
    let _ = writeln!(out, "\n{}", style.bold("Placement types"));
    for t in rd["placementTypes"].as_array().into_iter().flatten() {
        let _ = writeln!(
            out,
            "  {} {}",
            t["placementTypeId"].as_str().unwrap_or_default(),
            style.dim(&format!(
                "[{}]",
                strs(&t["acceptsAssetCategories"]).join(", ")
            ))
        );
        for m in t["specMappings"].as_array().into_iter().flatten() {
            let sizes: Vec<String> = strs(&m["specs"])
                .iter()
                .filter_map(|id| specs.iter().find(|s| s["specId"] == *id).map(spec_size))
                .collect();
            let _ = writeln!(
                out,
                "    {:<48} {}",
                m["placementGroupId"].as_str().unwrap_or_default(),
                style.dim(&sizes.join(" "))
            );
        }
    }
    let _ = writeln!(out, "\n{}", style.bold("Placement groups"));
    for g in rd["placementProfileGroups"]
        .as_array()
        .into_iter()
        .flatten()
    {
        let _ = writeln!(
            out,
            "  {:<48} {:<22} {}",
            g["placementProfileGroupId"].as_str().unwrap_or_default(),
            g["platform"].as_str().unwrap_or_default(),
            g["displayClassId"].as_str().unwrap_or_default()
        );
    }
    let _ = writeln!(out, "\n{}", style.bold("Display classes"));
    for d in rd["displayClasses"].as_array().into_iter().flatten() {
        let _ = writeln!(
            out,
            "  {:<40} {:<8} {}",
            d["displayClassId"].as_str().unwrap_or_default(),
            d["deviceFamily"].as_str().unwrap_or_default(),
            strs(&d["screenDimensions"]).join(", ")
        );
    }
    let _ = writeln!(
        out,
        "\n{} {}",
        style.bold("Arrangia slots"),
        style.dim(&format!(
            "(data/asset-library.yaml, checked {})",
            r["mapping_checked"].as_str().unwrap_or_default()
        ))
    );
    for m in r["mapping"].as_array().into_iter().flatten() {
        let groups = strs(&m["groups"]);
        let slot = m["slot"].as_str().unwrap_or_default();
        if groups.is_empty() {
            let _ = writeln!(
                out,
                "  {:<30} {}",
                slot,
                style.warn(&format!(
                    "no group ({} · {})",
                    m["display_class"].as_str().unwrap_or("any display class"),
                    strs(&m["platforms"]).join("/")
                ))
            );
        }
        for (i, g) in groups.iter().enumerate() {
            let _ = writeln!(out, "  {:<30} {g}", if i == 0 { slot } else { "" });
        }
    }
    out
}

fn confirm(plan: &arrangia_app::upload::UploadPlan) -> anyhow::Result<()> {
    let stdin = std::io::stdin();
    if !stdin.is_terminal() {
        bail!("not a terminal: pass --yes to upload without asking");
    }
    eprint!(
        "Upload to the stores now? {}\nThis changes the public store listing. [y/N] ",
        plan.summary()
    );
    std::io::stderr().flush().ok();
    let mut line = String::new();
    stdin
        .lock()
        .read_line(&mut line)
        .context("reading the answer")?;
    if matches!(line.trim(), "y" | "Y" | "yes") {
        Ok(())
    } else {
        bail!("cancelled – nothing was uploaded")
    }
}

fn store_name(s: &str) -> &'static str {
    match s {
        "apple" => "App Store Connect",
        _ => "Google Play",
    }
}

fn print_problems(problems: &Value, indent: &str) {
    let style = Style::stdout();
    for p in problems.as_array().into_iter().flatten() {
        let label = match p["level"].as_str() {
            Some("error") => style.error("error"),
            Some("warn") => style.warn("warning"),
            _ => style.info("info"),
        };
        println!(
            "{indent}{label} {} {}",
            style.dim(p["code"].as_str().unwrap_or_default()),
            p["message"].as_str().unwrap_or_default()
        );
        if let Some(link) = p["link"].as_str() {
            println!("{indent}  → {link}");
        }
    }
}

fn kb(v: &Value) -> String {
    let b = v.as_u64().unwrap_or(0);
    if b >= 1024 * 1024 {
        format!("{:.1} MB", b as f64 / 1_048_576.0)
    } else {
        format!("{} KB", b.div_ceil(1024))
    }
}

fn print_plan(plan: &Value, global: &Global) {
    let style = Style::stdout();
    print_problems(&plan["problems"], "");
    for sp in plan["stores"].as_array().into_iter().flatten() {
        println!(
            "{} {}",
            style.bold(store_name(sp["store"].as_str().unwrap_or_default())),
            style.dim(&format!(
                "(account {}, app {}, flow {}{})",
                sp["account"].as_str().unwrap_or("-"),
                sp["app"].as_str().unwrap_or("-"),
                sp["flow"].as_str().unwrap_or_default(),
                sp["flow_setting"]
                    .as_str()
                    .map(|s| format!(", setting {s}"))
                    .unwrap_or_default()
            ))
        );
        if let Some(n) = sp["flow_note"].as_str() {
            println!("  {}", style.warn(n));
        }
        if let Some(r) = sp.get("remote") {
            if let Some(v) = r["version"].as_str() {
                println!(
                    "  store: {} version {v}",
                    r["app_name"].as_str().unwrap_or("")
                );
            }
            if let Some(e) = r["error"].as_str() {
                println!("  store: {}", style.warn(e));
            }
        }
        print_problems(&sp["problems"], "  ");
        println!(
            "  {:<22} {:<6} {:<8} {:<10} {:>6} {:>9}  SLOT",
            "TARGET", "LANG", "LOCALE", "ACTION", "IMAGES", "SIZE"
        );
        for s in sp["sets"].as_array().into_iter().flatten() {
            let action = s["action"].as_str().unwrap_or_default();
            let shown = match action {
                "new" | "replace" => style.ok(&format!("{action:<10}")),
                "blocked" => style.error(&format!("{action:<10}")),
                _ => style.dim(&format!("{action:<10}")),
            };
            println!(
                "  {:<22} {:<6} {:<8} {shown} {:>6} {:>9}  {}",
                s["target"].as_str().unwrap_or_default(),
                s["lang"].as_str().unwrap_or_default(),
                s["locale"].as_str().unwrap_or("-"),
                s["images"].as_array().map_or(0, Vec::len),
                kb(&s["bytes"]),
                s["slot"].as_str().unwrap_or_default()
            );
            if let Some(pl) = s["placement"].as_str() {
                let flow = s["flow"]
                    .as_str()
                    .map(|f| format!(" ({f})"))
                    .unwrap_or_default();
                println!("    {}", style.dim(&format!("{pl}{flow}")));
            } else if let Some(f) = s["flow"].as_str() {
                println!("    {}", style.dim(&format!("flow {f}")));
            }
            if let Some(r) = s["reason"].as_str()
                && (global.verbose
                    || action == "skip"
                    || s.get("processing").is_some()
                    || s.get("reusable").is_some())
            {
                println!("    {}", style.dim(r));
            }
            print_problems(&s["problems"], "    ");
        }
    }
    let t = &plan["totals"];
    println!(
        "\n{} set(s) to upload ({} image(s), {}), {} unchanged, {} skipped, {} blocked",
        t["upload_sets"],
        t["images"],
        kb(&t["bytes"]),
        t["unchanged_sets"],
        t["skipped_sets"],
        t["blocked_sets"]
    );
    let token = plan["confirm_token"].as_str().unwrap_or_default();
    if plan["blocked"] == true {
        println!(
            "{}",
            style.error("Blocked: fix the problems above and plan again.")
        );
    } else if t["upload_sets"].as_u64().unwrap_or(0) == 0 {
        println!("Nothing to upload.");
    } else {
        println!("Confirm token: {token} (valid 30 min, for this project state)");
        println!("Upload with:   arrangia upload execute --confirm {token}");
    }
}

fn print_run(run: &Value) {
    let style = Style::stdout();
    let status = run["status"].as_str().unwrap_or_default();
    let shown = if status == "completed" {
        style.ok(status)
    } else {
        style.error(status)
    };
    println!(
        "Upload {shown}: {}",
        run["message"].as_str().unwrap_or_default()
    );
    for f in run["store_failures"].as_array().into_iter().flatten() {
        println!(
            "  {} {}",
            style.error(f["store"].as_str().unwrap_or_default()),
            f["message"].as_str().unwrap_or_default()
        );
        if let Some(link) = f["link"].as_str() {
            println!("    → {link}");
        }
    }
    for s in run["sets"].as_array().into_iter().flatten() {
        if s["status"] == "skipped" {
            continue;
        }
        println!(
            "  {} {}/{} ({}): {}{}",
            s["store"].as_str().unwrap_or_default(),
            s["target"].as_str().unwrap_or_default(),
            s["lang"].as_str().unwrap_or_default(),
            s["locale"].as_str().unwrap_or_default(),
            s["status"].as_str().unwrap_or_default(),
            s["error"]
                .as_str()
                .map(|e| format!(" – {e}"))
                .unwrap_or_default()
        );
        if let Some(n) = s["note"].as_str() {
            println!("    {}", style.dim(n));
        }
    }
    if run["images_processing"].as_u64().unwrap_or(0) > 0 {
        println!(
            "Apple is still processing some images. Check later with `arrangia upload plan --remote` (they are not uploaded again)."
        );
    }
    if status != "completed" {
        println!("Plan and execute again to upload what is missing (completed sets are skipped).");
    }
}

fn print_log(log: &Value, events: bool) {
    let style = Style::stdout();
    let runs = log["runs"].as_array().cloned().unwrap_or_default();
    if runs.is_empty() {
        println!(
            "No uploads yet ({}).",
            log["path"].as_str().unwrap_or_default()
        );
        return;
    }
    for r in runs {
        println!(
            "{} run {} – {}",
            style.bold(&format!("#{}", r["id"])),
            r["started_at"],
            r["status"].as_str().unwrap_or_default()
        );
        for s in r["sets"].as_array().into_iter().flatten() {
            println!(
                "  {} {} {} {}/{}: {} ({} image(s)){}",
                s["store"].as_str().unwrap_or_default(),
                s["locale"].as_str().unwrap_or_default(),
                s["slot"].as_str().unwrap_or_default(),
                s["target"].as_str().unwrap_or_default(),
                s["lang"].as_str().unwrap_or_default(),
                s["status"].as_str().unwrap_or_default(),
                s["images"],
                s["error"]
                    .as_str()
                    .map(|e| format!(" – {e}"))
                    .unwrap_or_default()
            );
        }
        if events {
            for e in r["events"].as_array().into_iter().flatten() {
                println!(
                    "    {} {}",
                    style.dim(e["level"].as_str().unwrap_or_default()),
                    e["message"].as_str().unwrap_or_default()
                );
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn refdata_text_report() {
        let report = json!({
            "account": "apple-team",
            "app": {"id": "1234567890", "name": "Fernwood", "bundle_id": "com.example.fernwood",
                    "library_id": "lib-1"},
            "ref_data": {
                "features": [],
                "placementProfileGroups": [
                    {"placementProfileGroupId": "IPHONE_DUO_OUTER_PROFILE",
                     "platform": "IPHONE_APP_STORE", "displayClassId": "IPHONE_DUO"}],
                "imageSpecs": [{"specId": "s1", "dimensions":
                    {"minWidth": 1398, "maxWidth": 1398, "minHeight": 2034, "maxHeight": 2034}}],
                "videoSpecs": [],
                "placementTypes": [{"placementTypeId": "APP_SCREENSHOT",
                    "acceptsAssetCategories": ["APP_SCREENSHOTS_AND_PREVIEWS"],
                    "specMappings": [{"placementGroupId": "IPHONE_DUO_OUTER_PROFILE", "specs": ["s1"]}]}],
                "displayClasses": [{"displayClassId": "IPHONE_DUO", "deviceFamily": "IPHONE",
                                    "screenDimensions": ["5.4", "7.6"]}]
            },
            "mapping": [
                {"slot": "APP_IPHONE_DUO", "placement_types": ["APP_SCREENSHOT"],
                 "display_class": "IPHONE_DUO", "platforms": ["IPHONE_APP_STORE"],
                 "groups": ["APP_SCREENSHOT → IPHONE_DUO_OUTER_PROFILE 1398x2034"],
                 "asset_library_only": true},
                {"slot": "header", "placement_types": ["PRODUCT_PAGE_HEADER_ASSET"],
                 "platforms": ["IPHONE_APP_STORE", "IPAD_APP_STORE"], "groups": [],
                 "asset_library_only": true}
            ],
            "mapping_checked": "2026-11-10"
        });
        let text = format_refdata(&report, Style::plain());
        for want in [
            "App Store Connect asset types (account apple-team)",
            "App: Fernwood (1234567890, com.example.fernwood)",
            "asset library: lib-1",
            "APP_SCREENSHOT [APP_SCREENSHOTS_AND_PREVIEWS]",
            "IPHONE_DUO_OUTER_PROFILE",
            "1398x2034",
            "IPHONE_DUO",
            "5.4, 7.6",
            "APP_IPHONE_DUO",
            "header",
            "no group (any display class · IPHONE_APP_STORE/IPAD_APP_STORE)",
        ] {
            assert!(text.contains(want), "{want:?} missing in\n{text}");
        }
    }
}
