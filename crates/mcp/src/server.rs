//! [`VitrineMcp`]: the MCP server handler over an [`AppState`].
//!
//! The handler owns no project state of its own: it holds an
//! `Arc<AppState>` (headless: its own; GUI: the app's live state), so the
//! same tool router can be served over stdio or Streamable HTTP. Every tool
//! call
//!
//! 1. reads the MCP client's `clientInfo` (name, title) for the actor,
//! 2. runs on the blocking pool (`spawn_blocking`) inside
//!    `Actor::scope(Actor::Assistant { client, name }, …)`, so history
//!    labels start with the client's display name (`Claude Desktop: …`)
//!    and `project-edited` events name the assistant,
//! 3. first syncs with the disk (reload when the files changed; a
//!    `project.yaml` that does not load is reported as error `load`),
//! 4. then calls `vitrine_app::commands` – never the model directly.

use std::sync::Arc;

use rmcp::handler::server::router::tool::ToolRouter;
use rmcp::handler::server::tool::ToolCallContext;
use rmcp::model::{
    CallToolRequestParams, CallToolResponse, CallToolResult, Implementation,
    InitializeRequestParams, InitializeResult, ListResourceTemplatesResult, ListResourcesResult,
    ListToolsResult, PaginatedRequestParams, ReadResourceRequestParams, ReadResourceResponse,
    ServerCapabilities, ServerConfig,
};
use rmcp::service::RequestContext;
use rmcp::{ErrorData, RoleServer, ServerHandler};
use serde_json::Value;
use vitrine_app::commands::AppState;
use vitrine_app::{Actor, CommandError, ErrorCode, Reply};
use vitrine_core::Project;

use crate::output::{ToolOutput, error_result};
use crate::progress::ProgressRelay;
use crate::resources;

/// Guide for the model, sent as the server `instructions`.
pub const INSTRUCTIONS: &str = include_str!("instructions.md");

/// Who hosts the server.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Host {
    /// `vitrine mcp --headless`: own session, no GUI.
    Headless,
    /// The GUI's live session (ADR 0015): omitted targets/languages
    /// default to the GUI's view; sessions and tool calls are reported in
    /// the app's MCP status.
    Gui,
}

/// The Vitrine Studio MCP server (tools + resources) over an [`AppState`].
#[derive(Clone)]
pub struct VitrineMcp {
    app: Arc<AppState>,
    progress: Option<Arc<ProgressRelay>>,
    host: Host,
    tool_router: ToolRouter<Self>,
    /// One MCP session of a GUI-hosted server (see [`Self::for_session`]).
    session: Option<Arc<SessionGuard>>,
}

/// Counts an MCP session in the app's MCP status while it lives: the
/// client name is set on `initialize`; dropping the last clone of the
/// session's server (session closed or expired) counts it out.
struct SessionGuard {
    app: Arc<AppState>,
    client: std::sync::Mutex<Option<String>>,
}

impl SessionGuard {
    fn opened(&self, name: &str, title: Option<&str>) {
        let mut client = self
            .client
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if client.is_none() {
            *client = Some(name.to_owned());
            drop(client);
            self.app
                .update_mcp_status(|s| s.session_opened(name, title));
        }
    }
}

impl Drop for SessionGuard {
    fn drop(&mut self) {
        let name = self
            .client
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .take();
        if let Some(name) = name {
            self.app.update_mcp_status(|s| s.session_closed(&name));
        }
    }
}

impl std::fmt::Debug for VitrineMcp {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("VitrineMcp")
            .field("host", &self.host)
            .finish_non_exhaustive()
    }
}

/// The MCP client's `clientInfo.name` (`claude-code`, …; `mcp` when
/// empty) and `clientInfo.title`.
pub fn client_info(ctx: &RequestContext<RoleServer>) -> (String, Option<String>) {
    let info = ctx.client_info();
    let name = info
        .as_ref()
        .map(|i| i.name.trim().to_owned())
        .filter(|n| !n.is_empty())
        .unwrap_or_else(|| "mcp".to_owned());
    (name, info.and_then(|i| i.title))
}

impl VitrineMcp {
    /// A server over `app`. `progress` must be the app's event sink (or
    /// wrap it) for `export` to send progress notifications.
    pub fn new(app: Arc<AppState>, progress: Option<Arc<ProgressRelay>>, host: Host) -> Self {
        let tool_router = Self::project_tools()
            + Self::catalog_tools()
            + Self::structure_tools()
            + Self::element_tools()
            + Self::string_tools()
            + Self::style_tools()
            + Self::asset_tools()
            + Self::output_tools()
            + Self::history_tools()
            + Self::docs_tools()
            + Self::upload_tools()
            + Self::template_tools();
        Self {
            app,
            progress,
            host,
            tool_router,
            session: None,
        }
    }

    /// A clone for a new MCP session: a GUI-hosted server counts its
    /// sessions per client in the app's MCP status (`mcp-status`).
    #[must_use]
    pub fn for_session(&self) -> Self {
        let mut s = self.clone();
        if self.host == Host::Gui {
            s.session = Some(Arc::new(SessionGuard {
                app: self.app.clone(),
                client: std::sync::Mutex::new(None),
            }));
        }
        s
    }

    pub fn app(&self) -> &Arc<AppState> {
        &self.app
    }

    pub fn host(&self) -> Host {
        self.host
    }

    pub(crate) fn progress(&self) -> Option<&Arc<ProgressRelay>> {
        self.progress.as_ref()
    }

    /// All tool definitions (for tests and docs).
    pub fn tools(&self) -> Vec<rmcp::model::Tool> {
        self.tool_router.list_all()
    }

    /// Runs `f` for the client of `ctx` on the blocking pool (see the module
    /// docs) and turns its result into a tool result.
    pub(crate) async fn run<F>(&self, ctx: &RequestContext<RoleServer>, f: F) -> CallToolResult
    where
        F: FnOnce(&AppState) -> Result<ToolOutput, CommandError> + Send + 'static,
    {
        self.run_with(ctx, true, f).await
    }

    /// [`Self::run`] without the disk sync (opening or creating a project
    /// must work while the current one is broken on disk).
    pub(crate) async fn run_unsynced<F>(
        &self,
        ctx: &RequestContext<RoleServer>,
        f: F,
    ) -> CallToolResult
    where
        F: FnOnce(&AppState) -> Result<ToolOutput, CommandError> + Send + 'static,
    {
        self.run_with(ctx, false, f).await
    }

    async fn run_with<F>(
        &self,
        ctx: &RequestContext<RoleServer>,
        sync: bool,
        f: F,
    ) -> CallToolResult
    where
        F: FnOnce(&AppState) -> Result<ToolOutput, CommandError> + Send + 'static,
    {
        let (client, title) = client_info(ctx);
        let actor = Actor::assistant(&client, title.as_deref());
        let app = self.app.clone();
        let gui = self.host == Host::Gui;
        let joined = tokio::task::spawn_blocking(move || {
            actor.scope(|| {
                if sync {
                    sync_with_disk(&app)?;
                }
                // GUI-hosted: omitted targets/languages default to what
                // the user looks at.
                let view = gui.then(|| app.session().ok().map(|s| s.view())).flatten();
                crate::params::with_gui_view(view, || f(&app))
            })
        })
        .await;
        match joined {
            Ok(Ok(out)) => out.into_result(),
            Ok(Err(e)) => error_result(&e),
            Err(e) => error_result(&CommandError::internal(format!("tool panicked: {e}"))),
        }
    }
}

/// Reloads the open project when its files changed on disk (the watcher
/// does this too, but debounced; a tool must see the current state). A
/// `project.yaml` that does not load is an error (code `load`): writing
/// would fail with a conflict anyway, and reading would show stale data.
pub fn sync_with_disk(app: &AppState) -> Result<(), CommandError> {
    let Ok(session) = app.session() else {
        return Ok(());
    };
    session.reload("external");
    let on_disk = vitrine_core::io::disk_revision(session.dir());
    let loaded = session.revision();
    let broken = match &on_disk {
        Ok(Some(rev)) => *rev != loaded,
        Ok(None) => false,
        Err(_) => true,
    };
    if broken {
        let why = Project::load(session.dir())
            .err()
            .map_or_else(|| "unknown reason".to_owned(), |e| e.to_string());
        return Err(CommandError::new(
            ErrorCode::Load,
            format!(
                "project.yaml on disk was changed and does not load: {why}. \
                 Vitrine Studio still holds revision {loaded} in memory; nothing was changed."
            ),
        ));
    }
    Ok(())
}

/// Runs a JSON command of `vitrine_app::commands` by name.
pub fn cmd(app: &AppState, name: &str, args: Value) -> Result<Value, CommandError> {
    match vitrine_app::dispatch(app, name, args)? {
        Reply::Json(v) => Ok(v),
        Reply::Image(_) => Err(CommandError::internal(format!(
            "`{name}` answers with an image"
        ))),
    }
}

/// Runs an image command of `vitrine_app::commands` by name.
pub fn image_cmd(
    app: &AppState,
    name: &str,
    args: Value,
) -> Result<Arc<vitrine_app::preview::PreviewImage>, CommandError> {
    match vitrine_app::dispatch(app, name, args)? {
        Reply::Image(i) => Ok(i),
        Reply::Json(_) => Err(CommandError::internal(format!(
            "`{name}` answers with JSON"
        ))),
    }
}

/// Is `message` rmcp's error for arguments that do not match the tool's
/// schema?
fn is_argument_error(message: &str) -> bool {
    message.starts_with("failed to deserialize parameters")
}

impl ServerHandler for VitrineMcp {
    fn get_info(&self) -> ServerConfig {
        ServerConfig::new(
            ServerCapabilities::builder()
                .enable_tools()
                .enable_resources()
                .build(),
        )
        .with_server_info(
            Implementation::new(crate::SERVER_NAME, env!("CARGO_PKG_VERSION"))
                .with_title("Vitrine Studio"),
        )
        .with_instructions(INSTRUCTIONS)
    }

    async fn call_tool(
        &self,
        request: CallToolRequestParams,
        context: RequestContext<RoleServer>,
    ) -> Result<CallToolResponse, ErrorData> {
        let name = request.name.clone();
        if self.host == Host::Gui {
            let (client, title) = client_info(&context);
            self.app
                .update_mcp_status(|s| s.tool_called(&client, title.as_deref(), &name));
        }
        let tcc = ToolCallContext::new(self, request, context);
        let response = self.tool_router.call(tcc).await;
        // Wrong arguments: rmcp answers with a plain-text tool error (or a
        // protocol error); give the model our structured error instead.
        let argument_error = |message: &str| {
            Ok(error_result(&CommandError::invalid(format!("{name}: {message}"))).into())
        };
        match response {
            Err(e) if is_argument_error(&e.message) => argument_error(&e.message),
            Ok(CallToolResponse::Complete(r))
                if r.is_error == Some(true)
                    && r.structured_content.is_none()
                    && r.content
                        .first()
                        .and_then(|c| c.as_text())
                        .is_some_and(|t| is_argument_error(&t.text)) =>
            {
                let message = r.content[0]
                    .as_text()
                    .map(|t| t.text.clone())
                    .unwrap_or_default();
                argument_error(&message)
            }
            other => other,
        }
    }

    #[allow(clippy::unused_async_trait_impl)] // the trait wants a future
    async fn initialize(
        &self,
        request: InitializeRequestParams,
        context: RequestContext<RoleServer>,
    ) -> Result<InitializeResult, ErrorData> {
        if let Some(guard) = &self.session {
            let name = request.client_info.name.trim();
            let title = request.client_info.title.as_deref();
            guard.opened(if name.is_empty() { "mcp" } else { name }, title);
        }
        context.peer.set_peer_info(request.clone());
        self.negotiate_initialize(&request)
    }

    #[allow(clippy::unused_async_trait_impl)] // the trait wants a future
    async fn list_tools(
        &self,
        _request: Option<PaginatedRequestParams>,
        _context: RequestContext<RoleServer>,
    ) -> Result<ListToolsResult, ErrorData> {
        Ok(ListToolsResult::with_all_items(self.tool_router.list_all()))
    }

    fn get_tool(&self, name: &str) -> Option<rmcp::model::Tool> {
        self.tool_router.get(name).cloned()
    }

    async fn list_resources(
        &self,
        _request: Option<PaginatedRequestParams>,
        _context: RequestContext<RoleServer>,
    ) -> Result<ListResourcesResult, ErrorData> {
        let app = self.app.clone();
        let list = tokio::task::spawn_blocking(move || resources::list(&app))
            .await
            .map_err(|e| ErrorData::internal_error(e.to_string(), None))?;
        Ok(ListResourcesResult::with_all_items(list))
    }

    #[allow(clippy::unused_async_trait_impl)] // the trait wants a future
    async fn list_resource_templates(
        &self,
        _request: Option<PaginatedRequestParams>,
        _context: RequestContext<RoleServer>,
    ) -> Result<ListResourceTemplatesResult, ErrorData> {
        Ok(ListResourceTemplatesResult::with_all_items(
            resources::templates(),
        ))
    }

    async fn read_resource(
        &self,
        request: ReadResourceRequestParams,
        _context: RequestContext<RoleServer>,
    ) -> Result<ReadResourceResponse, ErrorData> {
        let app = self.app.clone();
        let uri = request.uri;
        tokio::task::spawn_blocking(move || {
            sync_with_disk(&app).ok();
            resources::read(&app, &uri)
        })
        .await
        .map_err(|e| ErrorData::internal_error(e.to_string(), None))?
        .map(Into::into)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn argument_errors_are_recognised() {
        assert!(is_argument_error(
            "failed to deserialize parameters: missing field `id`"
        ));
        assert!(!is_argument_error("tool not found"));
    }

    fn headless_example() -> (tempfile::TempDir, VitrineMcp) {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path().join("p");
        vitrine_app::testutil::copy_project(&vitrine_app::testutil::example_fixture(), &dir)
            .unwrap();
        let app = Arc::new(
            AppState::new(
                vitrine_app::settings::AppPaths::in_dir(&tmp.path().join("app")),
                Arc::new(vitrine_app::events::NullSink),
                Arc::new(vitrine_app::dialogs::NoDialogs),
            )
            .without_watcher(),
        );
        app.open_project(&dir).unwrap();
        (tmp, VitrineMcp::new(app, None, Host::Headless))
    }

    #[test]
    fn every_tool_has_a_description_and_an_object_schema() {
        let (_tmp, server) = headless_example();
        let tools = server.tools();
        assert!(tools.len() >= 45, "{} tools", tools.len());
        for t in &tools {
            let d = t.description.as_deref().unwrap_or_default();
            assert!(d.len() > 20, "{}: description too short", t.name);
            assert_eq!(t.input_schema["type"], "object", "{}", t.name);
            assert!(
                t.name.chars().all(|c| c.is_ascii_lowercase() || c == '_'),
                "{} is not snake_case",
                t.name
            );
        }
        for name in [
            "template_list",
            "template_save",
            "template_insert",
            "template_import",
        ] {
            assert!(tools.iter().any(|t| t.name == name), "{name} missing");
        }
        let preview = tools.iter().find(|t| t.name == "render_preview").unwrap();
        let max_edge = &preview.input_schema["properties"]["max_edge"];
        assert_eq!(max_edge["default"], 1200, "{max_edge}");
        let view = serde_json::to_string(&preview.input_schema).unwrap();
        assert!(view.contains("gallery") && view.contains("canvas"));
    }

    #[test]
    fn conflicting_writes_and_broken_files_give_readable_errors() {
        let (_tmp, server) = headless_example();
        let app = server.app().clone();
        let session = app.session().unwrap();
        let file = session.dir().join("project.yaml");
        let text = std::fs::read_to_string(&file).unwrap();
        // Another writer bumped the revision behind our back (the race
        // between sync and save): the write is refused as a conflict and
        // the session reloads.
        std::fs::write(&file, text.replace("revision: 1", "revision: 7")).unwrap();
        let e = cmd(
            &app,
            "element_patch",
            serde_json::json!({"id": "title-plan", "patch": {"align": "left"}}),
        )
        .unwrap_err();
        assert_eq!(e.code, ErrorCode::Conflict);
        let r = error_result(&e);
        let body = r.structured_content.unwrap();
        assert_eq!(body["error"]["code"], "conflict");
        assert!(body["hint"].as_str().unwrap().contains("reloaded"));
        assert_eq!(session.revision(), 7);
        // After the reload the same write succeeds.
        sync_with_disk(&app).unwrap();
        cmd(
            &app,
            "element_patch",
            serde_json::json!({"id": "title-plan", "patch": {"align": "left"}}),
        )
        .unwrap();
        // A file that does not load is reported before any write.
        std::fs::write(&file, "format: vitrine/1\nrevision: 99\nbogus: [\n").unwrap();
        let e = sync_with_disk(&app).unwrap_err();
        assert_eq!(e.code, ErrorCode::Load);
        assert!(e.message.contains("does not load"), "{}", e.message);
    }

    #[test]
    fn instructions_are_compact() {
        let lines = INSTRUCTIONS.lines().count();
        assert!((30..=60).contains(&lines), "{lines} lines");
        for word in ["render_preview", "1000 lu", "\\-", "[/]", "clockwise"] {
            assert!(INSTRUCTIONS.contains(word), "instructions mention {word}");
        }
    }
}
