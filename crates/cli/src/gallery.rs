//! `vitrine gallery login|logout|list|upload|download`: the template
//! gallery at `api.vitrinestudio.app` (ADR 0025).
//!
//! Browsing and downloading need no account. `login` runs the GitHub
//! device flow through the API and keeps the server's session token in the
//! system keychain (service `app.vitrinestudio`, account
//! `gallery/<api host>`). `upload` uses that session, or a static token
//! read from an environment variable (`--token-env`) – never a token on
//! the command line. Network code: `vitrine_publish::gallery`.

use std::path::PathBuf;
use std::time::{Duration, Instant};

use anyhow::{Context, bail};
use clap::{Args, Subcommand};
use vitrine_core::template::{EXTENSION, Limits, Package};
use vitrine_publish::credentials::{CredentialError, Secret, SecretStore};
use vitrine_publish::gallery::{
    DevicePoll, GalleryClient, GalleryError, ListQuery, api_url, keychain_user,
};

use crate::term::{Style, plural};
use crate::{EXIT_OK, Format, Global};

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
    /// Upload a `.vitrine-template` (checked locally first; the server
    /// checks it again and queues it for moderation).
    Upload(UploadArgs),
    /// Download a template file (no login needed; checked after download).
    Download(DownloadArgs),
}

#[derive(Debug, Args)]
pub struct ListArgs {
    /// Search in name, description and tags.
    #[arg(long)]
    pub query: Option<String>,
    #[arg(long)]
    pub tag: Option<String>,
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
    /// The `.vitrine-template` file (e.g. from `vitrine template export`).
    pub file: PathBuf,
    /// Read a static upload token from this environment variable instead
    /// of using the `vitrine gallery login` session.
    #[arg(long, value_name = "VAR")]
    pub token_env: Option<String>,
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
    }
    println!("{}", style.dim(&plural(page.templates.len(), "template")));
    Ok(EXIT_OK)
}

fn upload(
    rt: &tokio::runtime::Runtime,
    client: &GalleryClient,
    a: &UploadArgs,
    global: &Global,
) -> anyhow::Result<u8> {
    let bytes = std::fs::read(&a.file).with_context(|| format!("{}", a.file.display()))?;
    // The same checks as the server, so a broken file fails here.
    let pkg = Package::from_zip_bytes(&bytes, Limits::default())
        .with_context(|| format!("{} is not a valid template", a.file.display()))?;
    let token = match &a.token_env {
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
    };
    if !global.quiet {
        eprintln!(
            "{}",
            Style::stderr().dim(&format!(
                "Uploading `{}` {} – templates in the gallery are CC0-1.0; you confirm you own the rights to everything in it.",
                pkg.manifest.id, pkg.manifest.version
            ))
        );
    }
    let res = match rt.block_on(client.upload(&token, bytes)) {
        Err(GalleryError::Api { status: 401, .. }) => {
            bail!("the token was refused (expired or revoked): run `vitrine gallery login` again")
        }
        other => other.map_err(net)?,
    };
    println!(
        "{} uploaded {} {} – status: {}",
        Style::stdout().ok("ok"),
        res.id,
        res.version,
        res.status
    );
    Ok(EXIT_OK)
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
