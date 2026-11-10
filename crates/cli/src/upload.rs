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
            "file_done" => format!(
                "{name}: {} uploaded ({}/{} images)",
                p["file"]["file_name"].as_str().unwrap_or_default(),
                p["images_done"],
                p["images_total"]
            ),
            "set_finished" => format!(
                "{name}: {}{}",
                set["status"].as_str().unwrap_or_default(),
                set["error"]
                    .as_str()
                    .map(|e| format!(" – {e}"))
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
    }
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
                "(account {}, app {}, flow {})",
                sp["account"].as_str().unwrap_or("-"),
                sp["app"].as_str().unwrap_or("-"),
                sp["flow"].as_str().unwrap_or_default()
            ))
        );
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
            if let Some(r) = s["reason"].as_str()
                && (global.verbose || action == "skip")
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
