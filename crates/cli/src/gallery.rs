//! `vitrine gallery login|logout|list|upload|download|admin`: the template
//! gallery at `api.vitrinestudio.app` (ADR 0025).
//!
//! Browsing and downloading need no account. `login` runs the GitHub
//! device flow through the API and keeps the server's session token in the
//! system keychain (service `app.vitrinestudio`, account
//! `gallery/<api host>`). `upload` (one or many files, `--dry-run` checks
//! only) uses that session, or a static token read from an environment
//! variable (`--token-env`) – never a token on the command line. `admin`
//! (moderation) reads the admin token only from an environment variable
//! and never prints it. Network code: `vitrine_publish::gallery`.

use std::path::PathBuf;
use std::time::{Duration, Instant};

use anyhow::{Context, bail};
use clap::{Args, Subcommand};
use vitrine_core::template::{EXTENSION, Limits, Package};
use vitrine_publish::credentials::{CredentialError, Secret, SecretStore};
use vitrine_publish::gallery::{
    DevicePoll, GalleryClient, GalleryError, ListQuery, TERMS_URL, api_url, keychain_user,
};

use crate::term::{Style, plural};
use crate::{EXIT_OK, EXIT_PROBLEMS, Format, Global};

/// Default environment variable of the admin token.
pub const ADMIN_TOKEN_ENV: &str = "VITRINE_API_ADMIN_TOKEN";

#[derive(Debug, Args)]
pub struct GalleryArgs {
    /// API base URL (https, or http on 127.0.0.1/localhost).
    /// Default: `https://api.vitrinestudio.app` (`VITRINE_API_URL` overrides
    /// it with a loopback URL, for tests).
    #[arg(long, global = true, value_name = "URL")]
    pub api: Option<String>,
    #[command(subcommand)]
    pub command: GalleryCommand,
}

#[derive(Debug, Subcommand)]
pub enum GalleryCommand {
    /// Sign in with GitHub (device flow): shows a code to enter on
    /// github.com, then keeps the session in the system keychain.
    Login,
    /// Revoke the session on the server and remove it from the keychain.
    Logout,
    /// List approved gallery templates (no login needed).
    List(ListArgs),
    /// Upload `.vitrine-template` files (checked locally first; the server
    /// checks them again and queues them for moderation). Continues after
    /// a failed file and ends with a summary.
    Upload(UploadArgs),
    /// Download a template file (no login needed; checked after download).
    Download(DownloadArgs),
    /// Moderation (owner): the queue, approve, reject, re-render. The
    /// admin token is read from an environment variable only.
    Admin(AdminArgs),
}

#[derive(Debug, Args)]
pub struct AdminArgs {
    /// Environment variable holding the admin token.
    #[arg(long, value_name = "VAR", default_value = ADMIN_TOKEN_ENV, global = true)]
    pub admin_token_env: String,
    #[command(subcommand)]
    pub command: AdminCommand,
}

#[derive(Debug, Subcommand)]
pub enum AdminCommand {
    /// Pending templates and open abuse reports.
    Queue {
        #[arg(long, value_enum, default_value_t = Format::Text)]
        format: Format,
    },
    /// Publish templates.
    Approve(ModerateArgs),
    /// Reject and delete templates at once (also to take down a reported one)
    Reject(ModerateArgs),
    /// Render previews and gallery sprites again.
    Rerender {
        #[arg(required = true, value_name = "ID")]
        ids: Vec<String>,
    },
}

#[derive(Debug, Args)]
pub struct ModerateArgs {
    #[arg(required = true, value_name = "ID")]
    pub ids: Vec<String>,
    /// Moderation note stored with the decision.
    #[arg(long, default_value = "")]
    pub note: String,
}

#[derive(Debug, Args)]
pub struct ListArgs {
    /// Search in name, description and tags.
    #[arg(long)]
    pub query: Option<String>,
    /// Any tag (facet value or device).
    #[arg(long)]
    pub tag: Option<String>,
    /// Style filter; repeat or separate with commas (any of them).
    #[arg(long, value_delimiter = ',')]
    pub style: Vec<String>,
    /// Colour filter.
    #[arg(long, value_delimiter = ',')]
    pub color: Vec<String>,
    /// App category filter.
    #[arg(long = "use", value_delimiter = ',')]
    pub use_: Vec<String>,
    /// Device filter (phone, tablet, watch, mac, web, tv, banner).
    #[arg(long, value_delimiter = ',')]
    pub devices: Vec<String>,
    /// 1–100.
    #[arg(long, default_value_t = 50)]
    pub limit: u32,
    #[arg(long, default_value_t = 0)]
    pub offset: u32,
    #[arg(long, value_enum, default_value_t = Format::Text)]
    pub format: Format,
}

#[derive(Debug, Args)]
pub struct UploadArgs {
    /// The `.vitrine-template` files (e.g. from `vitrine template export`).
    #[arg(required = true, value_name = "FILE")]
    pub files: Vec<PathBuf>,
    /// Run every local check (package, tags, sample screenshots) and print
    /// the summary; send nothing (no login needed).
    #[arg(long)]
    pub dry_run: bool,
    /// Confirm that the sample screenshots in the templates are your own
    /// and published as CC0 (required for templates with samples).
    #[arg(long)]
    pub confirm_own_screenshots: bool,
    /// Read a static upload token from this environment variable instead
    /// of using the `vitrine gallery login` session.
    #[arg(long, value_name = "VAR")]
    pub token_env: Option<String>,
    #[arg(long, value_enum, default_value_t = Format::Text)]
    pub format: Format,
}

#[derive(Debug, Args)]
pub struct DownloadArgs {
    /// Template id.
    pub id: String,
    /// Output file (default: `<id>.vitrine-template` in the current folder).
    #[arg(short, long, value_name = "FILE")]
    pub output: Option<PathBuf>,
}

fn runtime() -> anyhow::Result<tokio::runtime::Runtime> {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .context("cannot start the network runtime")
}

/// Network errors: a short hint when the server is not reachable.
fn net(e: GalleryError) -> anyhow::Error {
    match e {
        GalleryError::Unreachable(_) => {
            anyhow::anyhow!("{e} – check the network connection or `--api`")
        }
        other => anyhow::Error::new(other),
    }
}

fn keychain() -> anyhow::Result<SecretStore> {
    SecretStore::from_env().map_err(|e| anyhow::anyhow!("{e}"))
}

pub fn run(args: &GalleryArgs, global: &Global) -> anyhow::Result<u8> {
    let base = api_url(args.api.as_deref())?;
    let client = GalleryClient::new(base)?;
    let rt = runtime()?;
    match &args.command {
        GalleryCommand::Login => login(&rt, &client, global),
        GalleryCommand::Logout => logout(&rt, &client, global),
        GalleryCommand::List(a) => list(&rt, &client, a),
        GalleryCommand::Upload(a) => upload(&rt, &client, a, global),
        GalleryCommand::Download(a) => download(&rt, &client, a, global),
        GalleryCommand::Admin(a) => admin(&rt, &client, a, global),
    }
}

fn login(
    rt: &tokio::runtime::Runtime,
    client: &GalleryClient,
    global: &Global,
) -> anyhow::Result<u8> {
    let style = Style::stderr();
    let store = keychain()?;
    let code = rt.block_on(client.device_start()).map_err(net)?;
    eprintln!(
        "Open {} and enter the code {}",
        style.bold(&code.verification_uri),
        style.bold(&code.user_code)
    );
    eprintln!("{}", style.dim("Waiting for the confirmation on GitHub …"));
    let deadline = Instant::now() + Duration::from_secs(code.expires_in.max(1));
    let mut interval = code.interval.max(1);
    let session = loop {
        if Instant::now() >= deadline {
            bail!("the code expired; run `vitrine gallery login` again");
        }
        std::thread::sleep(Duration::from_secs(interval));
        match rt
            .block_on(client.device_poll(&code.device_code))
            .map_err(net)?
        {
            DevicePoll::Done(s) => break s,
            DevicePoll::Pending => {}
            DevicePoll::SlowDown(n) => interval = n.max(interval + 1),
        }
    };
    store
        .set_named(
            &keychain_user(client.base()),
            &Secret::new(session.token.clone()),
        )
        .map_err(|e| anyhow::anyhow!("{e}"))?;
    let days = (session.expires_at - now()).max(0) / 86_400;
    if !global.quiet {
        println!(
            "{} logged in as {}{} (session valid for {days} days)",
            Style::stdout().ok("ok"),
            session.login,
            if session.official { " (official)" } else { "" },
        );
    }
    Ok(EXIT_OK)
}

fn now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| i64::try_from(d.as_secs()).unwrap_or(i64::MAX))
}

fn logout(
    rt: &tokio::runtime::Runtime,
    client: &GalleryClient,
    global: &Global,
) -> anyhow::Result<u8> {
    let store = keychain()?;
    let user = keychain_user(client.base());
    let token = match store.get_named(&user) {
        Ok(t) => t,
        Err(CredentialError::Missing(_)) => {
            if !global.quiet {
                println!("not logged in at {}", client.base());
            }
            return Ok(EXIT_OK);
        }
        Err(e) => bail!("{e}"),
    };
    // Revoke on the server; a session the server no longer knows is fine.
    match rt.block_on(client.logout(token.expose())) {
        Ok(()) | Err(GalleryError::Api { status: 401, .. }) => {}
        Err(e) => {
            eprintln!(
                "{} could not revoke the session on the server: {e}",
                Style::stderr().warn("warn")
            );
        }
    }
    store
        .delete_named(&user)
        .map_err(|e| anyhow::anyhow!("{e}"))?;
    if !global.quiet {
        println!("{} logged out", Style::stdout().ok("ok"));
    }
    Ok(EXIT_OK)
}

fn list(rt: &tokio::runtime::Runtime, client: &GalleryClient, a: &ListArgs) -> anyhow::Result<u8> {
    let page = rt
        .block_on(client.list(&ListQuery {
            query: a.query.clone(),
            tag: a.tag.clone(),
            style: a.style.clone(),
            color: a.color.clone(),
            use_: a.use_.clone(),
            devices: a.devices.clone(),
            limit: Some(a.limit.clamp(1, 100)),
            offset: Some(a.offset),
        }))
        .map_err(net)?;
    if a.format == Format::Json {
        println!("{}", serde_json::to_string_pretty(&page)?);
        return Ok(EXIT_OK);
    }
    let style = Style::stdout();
    if page.templates.is_empty() {
        println!("no templates found");
        return Ok(EXIT_OK);
    }
    for t in &page.templates {
        let canvases: Vec<&str> = t.canvases.iter().map(|c| c.id.as_str()).collect();
        println!(
            "{}  {}  {}{}",
            style.bold(&t.id),
            t.version,
            t.name,
            if t.official {
                format!(" {}", style.info("[official]"))
            } else {
                String::new()
            }
        );
        println!(
            "    {}",
            style.dim(&format!(
                "by {} · {} · {} downloads",
                t.uploader,
                canvases.join(", "),
                t.downloads
            ))
        );
        let mut tags = t.tags.all();
        tags.extend(t.devices.iter().cloned());
        if !tags.is_empty() {
            println!("    {}", style.dim(&tags.join(", ")));
        }
    }
    println!("{}", style.dim(&plural(page.templates.len(), "template")));
    Ok(EXIT_OK)
}

/// One file of a (bulk) upload and what happened to it.
#[derive(Debug, Clone, Default, serde::Serialize)]
struct UploadRow {
    file: String,
    id: String,
    version: String,
    canvases: usize,
    tags: String,
    devices: String,
    samples: usize,
    bytes: u64,
    /// `ok` (dry run), `uploaded`, `error`.
    result: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    status: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    message: String,
}

/// Every local check of one file: the package (same code as the server),
/// complete tags, the sample screenshot rule. The package bytes on success.
fn check_file(
    path: &std::path::Path,
    confirm: bool,
    row: &mut UploadRow,
) -> Result<Vec<u8>, String> {
    let bytes = std::fs::read(path).map_err(|e| e.to_string())?;
    row.bytes = bytes.len() as u64;
    let pkg = Package::from_zip_bytes(&bytes, Limits::default())
        .map_err(|e| format!("not a valid template: {e}"))?;
    let m = &pkg.manifest;
    row.id.clone_from(&m.id);
    row.version.clone_from(&m.version);
    row.canvases = m.canvases.len();
    row.tags = m.tags.all().join(",");
    row.samples = m.samples.len();
    if let Ok(project) = pkg.project() {
        row.devices = vitrine_core::template::tags::devices(&project).join(",");
    }
    if !m.tags.dropped.is_empty() {
        return Err(format!(
            "unknown tags {:?}: save the template again with --style/--color/--use",
            m.tags.dropped
        ));
    }
    m.tags
        .check(true)
        .map_err(|e| format!("tags: {e} (save again with --style/--color/--use)"))?;
    if pkg.has_samples() && !confirm {
        return Err(format!(
            "contains {}: pass --confirm-own-screenshots to confirm they are your own and CC0, or save without --sample-screenshots",
            plural(m.samples.len(), "sample screenshot")
        ));
    }
    Ok(bytes)
}

/// The message for an upload the server refused as too large (HTTP 413):
/// the API names its limit (`too_large`); any other 413 comes from a
/// reverse proxy in front of it.
fn too_large_message(code: &str, message: &str, bytes: u64) -> String {
    let size = format!("{:.1} MB", bytes as f64 / (1024.0 * 1024.0));
    if code == "too_large" {
        format!(
            "{message} (this template: {size}; the server operator sets VITRINE_API_MAX_UPLOAD_MB)"
        )
    } else {
        format!(
            "the server refused the {size} template as too large (HTTP 413): a proxy in front of the API limits the request size (nginx `client_max_body_size`)"
        )
    }
}

fn upload_token(client: &GalleryClient, a: &UploadArgs) -> anyhow::Result<String> {
    Ok(match &a.token_env {
        Some(var) => std::env::var(var)
            .ok()
            .map(|t| t.trim().to_owned())
            .filter(|t| !t.is_empty())
            .with_context(|| format!("the environment variable `{var}` is not set"))?,
        None => match keychain()?.get_named(&keychain_user(client.base())) {
            Ok(t) => t.expose().to_owned(),
            Err(CredentialError::Missing(_)) => {
                bail!("not logged in: run `vitrine gallery login` (or pass --token-env VAR)")
            }
            Err(e) => bail!("{e}"),
        },
    })
}

fn upload(
    rt: &tokio::runtime::Runtime,
    client: &GalleryClient,
    a: &UploadArgs,
    global: &Global,
) -> anyhow::Result<u8> {
    let mut rows = Vec::new();
    let mut ready = Vec::new();
    for path in &a.files {
        let mut row = UploadRow {
            file: path.display().to_string(),
            ..UploadRow::default()
        };
        match check_file(path, a.confirm_own_screenshots, &mut row) {
            Ok(bytes) => {
                row.result = "ok".into();
                ready.push((rows.len(), bytes));
            }
            Err(e) => {
                row.result = "error".into();
                row.message = e;
            }
        }
        rows.push(row);
    }
    // Two files with the same id would overwrite each other.
    for i in 0..rows.len() {
        if rows[i].result == "ok"
            && rows[..i]
                .iter()
                .any(|r| r.result == "ok" && r.id == rows[i].id)
        {
            rows[i].result = "error".into();
            rows[i].message = format!("template id `{}` is given twice", rows[i].id);
            ready.retain(|(n, _)| *n != i);
        }
    }
    if !a.dry_run && !ready.is_empty() {
        let token = upload_token(client, a)?;
        if !global.quiet && a.format == Format::Text {
            let style = Style::stderr();
            eprintln!(
                "{}",
                style.dim(
                    "Templates in the gallery are CC0-1.0; you confirm you own the rights to everything in them."
                )
            );
            eprintln!(
                "{}",
                style.dim(&format!(
                    "By uploading you accept the gallery terms of use: {TERMS_URL}"
                ))
            );
        }
        for (i, bytes) in ready {
            let row = &mut rows[i];
            match rt.block_on(client.upload(&token, bytes, a.confirm_own_screenshots)) {
                Ok(res) => {
                    row.result = "uploaded".into();
                    row.status = res.status;
                    if res.samples_removed > 0 {
                        row.message = format!(
                            "{} sample screenshots replaced by placeholders",
                            res.samples_removed
                        );
                    }
                }
                Err(GalleryError::Api { status: 401, .. }) => {
                    row.result = "error".into();
                    row.message = "the token was refused (expired or revoked): run `vitrine gallery login` again".into();
                }
                Err(GalleryError::Api {
                    status: 413,
                    code,
                    message,
                    ..
                }) => {
                    row.result = "error".into();
                    row.message = too_large_message(&code, &message, row.bytes);
                }
                Err(e) => {
                    row.result = "error".into();
                    row.message = net(e).to_string();
                }
            }
            if !global.quiet && a.format == Format::Text {
                eprintln!("{} {} {}", row.result, row.id, row.message);
            }
        }
    }
    let failed = rows.iter().filter(|r| r.result == "error").count();
    if a.format == Format::Json {
        println!(
            "{}",
            serde_json::to_string_pretty(&serde_json::json!({
                "dry_run": a.dry_run,
                "files": rows,
                "failed": failed,
            }))?
        );
    } else {
        print_upload_table(&rows, a.dry_run);
    }
    Ok(if failed > 0 { EXIT_PROBLEMS } else { EXIT_OK })
}

fn print_upload_table(rows: &[UploadRow], dry_run: bool) {
    let style = Style::stdout();
    let width = |f: &dyn Fn(&UploadRow) -> usize, min: usize| {
        rows.iter().map(f).max().unwrap_or(0).max(min)
    };
    let wid = width(&|r| r.id.chars().count(), 2);
    let wver = width(&|r| r.version.chars().count(), 7);
    println!(
        "{:<wid$}  {:<wver$}  {:>3}  {:>7}  {:>7}  result / tags",
        "id", "version", "cv", "samples", "KB"
    );
    for r in rows {
        let result = match r.result.as_str() {
            "error" => style.error("error"),
            "uploaded" => style.ok(&format!("uploaded ({})", r.status)),
            _ => style.ok("ok"),
        };
        let id = if r.id.is_empty() {
            r.file.as_str()
        } else {
            r.id.as_str()
        };
        println!(
            "{id:<wid$}  {:<wver$}  {:>3}  {:>7}  {:>7}  {result}  {}",
            r.version,
            r.canvases,
            r.samples,
            r.bytes.div_ceil(1024),
            style.dim(&[r.tags.as_str(), r.devices.as_str()].join(" | "))
        );
        if !r.message.is_empty() {
            println!("    {}", r.message);
        }
    }
    let failed = rows.iter().filter(|r| r.result == "error").count();
    let ok = rows.len() - failed;
    println!(
        "{}{} {}, {} failed",
        if dry_run { "dry run: " } else { "" },
        plural(rows.len(), "file"),
        if dry_run {
            format!("{ok} ready to upload")
        } else {
            format!("{ok} uploaded")
        },
        failed
    );
}

/// The admin token from the environment (never an argument, never printed).
fn admin_token(var: &str) -> anyhow::Result<String> {
    std::env::var(var)
        .ok()
        .map(|t| t.trim().to_owned())
        .filter(|t| !t.is_empty())
        .with_context(|| {
            format!("the environment variable `{var}` with the admin token is not set")
        })
}

fn admin(
    rt: &tokio::runtime::Runtime,
    client: &GalleryClient,
    a: &AdminArgs,
    global: &Global,
) -> anyhow::Result<u8> {
    use vitrine_publish::gallery::AdminAction;
    let token = admin_token(&a.admin_token_env)?;
    let style = Style::stdout();
    let (action, ids, note) = match &a.command {
        AdminCommand::Queue { format } => {
            let q = rt.block_on(client.admin_queue(&token)).map_err(net)?;
            if *format == Format::Json {
                println!("{}", serde_json::to_string_pretty(&q)?);
                return Ok(EXIT_OK);
            }
            if q.pending.is_empty() {
                println!("no pending templates");
            }
            for t in &q.pending {
                let mut tags = t.tags.all();
                tags.extend(t.devices.iter().cloned());
                println!(
                    "{}  {}  {}  {}",
                    style.bold(&t.id),
                    t.version,
                    t.name,
                    style.dim(&format!(
                        "by {} · {} · {} samples",
                        t.uploader,
                        tags.join(", "),
                        t.samples
                    ))
                );
            }
            for r in &q.reports {
                println!(
                    "{} {} – {}: {}",
                    style.warn("report"),
                    r["template_id"].as_str().unwrap_or("?"),
                    r["reason"].as_str().unwrap_or("?"),
                    r["message"].as_str().unwrap_or("")
                );
            }
            return Ok(EXIT_OK);
        }
        AdminCommand::Approve(m) => (AdminAction::Approve, &m.ids, m.note.as_str()),
        AdminCommand::Reject(m) => (AdminAction::Reject, &m.ids, m.note.as_str()),
        AdminCommand::Rerender { ids } => (AdminAction::Rerender, ids, ""),
    };
    let mut failed = 0;
    for id in ids {
        match rt.block_on(client.admin_action(&token, id, action, note)) {
            Ok(_) => {
                if !global.quiet {
                    println!("{} {id} {}", style.ok("ok"), action.as_str());
                }
            }
            Err(e) => {
                failed += 1;
                eprintln!("{} {id}: {}", Style::stderr().error("error"), net(e));
            }
        }
    }
    Ok(if failed > 0 { EXIT_PROBLEMS } else { EXIT_OK })
}

fn download(
    rt: &tokio::runtime::Runtime,
    client: &GalleryClient,
    a: &DownloadArgs,
    global: &Global,
) -> anyhow::Result<u8> {
    let bytes = rt.block_on(client.download(&a.id)).map_err(net)?;
    let pkg = Package::from_zip_bytes(&bytes, Limits::default())
        .context("the downloaded template did not pass the checks")?;
    if pkg.manifest.id != a.id {
        bail!(
            "the server sent template `{}` instead of `{}`",
            pkg.manifest.id,
            a.id
        );
    }
    let out = a
        .output
        .clone()
        .unwrap_or_else(|| PathBuf::from(format!("{}.{EXTENSION}", a.id)));
    std::fs::write(&out, &bytes).with_context(|| format!("{}", out.display()))?;
    if !global.quiet {
        println!(
            "{} {} {} → {} (install: `vitrine template import {}`)",
            Style::stdout().ok("ok"),
            pkg.manifest.id,
            pkg.manifest.version,
            out.display(),
            out.display()
        );
    }
    Ok(EXIT_OK)
}
