//! The serializable error of all commands: `{code, message, details}`.

use serde::Serialize;

/// Machine-readable error class (`snake_case` in JSON).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ErrorCode {
    /// The command needs an open project.
    NoProject,
    /// Arguments missing or malformed.
    InvalidArgs,
    /// No command with this name.
    UnknownCommand,
    /// A named thing (target, file, image …) does not exist.
    NotFound,
    /// The core rejected an op; the project is unchanged.
    OpRejected,
    /// `project.yaml` changed on disk; the session reloaded it.
    Conflict,
    /// The project folder cannot be loaded.
    Load,
    /// Reading or writing a file failed.
    Io,
    /// Resolving or rendering failed.
    Render,
    /// Another export is running.
    Busy,
    /// The user cancelled (dialog, export).
    Cancelled,
    /// The feature is not available in this build yet.
    NotImplemented,
    /// The system keychain failed.
    Keychain,
    /// An upload was not confirmed: unknown/expired/used token, the
    /// project changed since the plan, or the user declined / did not
    /// answer an assistant's request (`details.reason`).
    NotConfirmed,
    /// Not allowed for this caller (e.g. headless MCP uploads).
    Forbidden,
    Internal,
}

/// Error returned by every command.
#[derive(Debug, Clone, PartialEq, Serialize, thiserror::Error)]
#[error("{message}")]
pub struct CommandError {
    pub code: ErrorCode,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub details: Option<serde_json::Value>,
}

impl CommandError {
    pub fn new(code: ErrorCode, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
            details: None,
        }
    }

    #[must_use]
    pub fn with_details(mut self, details: serde_json::Value) -> Self {
        self.details = Some(details);
        self
    }

    pub fn no_project() -> Self {
        Self::new(ErrorCode::NoProject, "no project is open")
    }

    pub fn invalid(message: impl Into<String>) -> Self {
        Self::new(ErrorCode::InvalidArgs, message)
    }

    pub fn not_found(message: impl Into<String>) -> Self {
        Self::new(ErrorCode::NotFound, message)
    }

    pub fn io(message: impl Into<String>) -> Self {
        Self::new(ErrorCode::Io, message)
    }

    pub fn internal(message: impl Into<String>) -> Self {
        Self::new(ErrorCode::Internal, message)
    }
}

impl From<vitrine_core::ops::OpError> for CommandError {
    fn from(e: vitrine_core::ops::OpError) -> Self {
        Self::new(ErrorCode::OpRejected, e.to_string())
    }
}

impl From<vitrine_core::resolve::ResolveError> for CommandError {
    fn from(e: vitrine_core::resolve::ResolveError) -> Self {
        Self::new(ErrorCode::Render, e.to_string())
    }
}

impl From<vitrine_core::io::IoError> for CommandError {
    fn from(e: vitrine_core::io::IoError) -> Self {
        use vitrine_core::io::IoError;
        let code = match &e {
            IoError::Conflict { .. } => ErrorCode::Conflict,
            IoError::Io { .. } | IoError::Serialize(_) => ErrorCode::Io,
            IoError::Parse { .. } | IoError::Invalid { .. } => ErrorCode::Load,
        };
        Self::new(code, e.to_string())
    }
}

impl From<vitrine_core::template::TemplateError> for CommandError {
    fn from(e: vitrine_core::template::TemplateError) -> Self {
        use vitrine_core::template::TemplateError;
        let code = match &e {
            TemplateError::Invalid(_) | TemplateError::FontLicense { .. } => ErrorCode::InvalidArgs,
            TemplateError::NotFound(_) => ErrorCode::NotFound,
            TemplateError::Conflict(_) | TemplateError::Io { .. } => ErrorCode::Io,
            TemplateError::Project(_) => ErrorCode::Load,
        };
        let mut err = Self::new(code, e.to_string());
        if let TemplateError::FontLicense { file, .. } = &e {
            err = err.with_details(serde_json::json!({ "kind": "font_license", "file": file }));
        }
        err
    }
}

impl From<serde_json::Error> for CommandError {
    fn from(e: serde_json::Error) -> Self {
        Self::invalid(format!("invalid arguments: {e}"))
    }
}

impl From<rusqlite::Error> for CommandError {
    fn from(e: rusqlite::Error) -> Self {
        Self::io(format!("app database: {e}"))
    }
}

/// `Result` of all commands.
pub type CmdResult<T> = Result<T, CommandError>;
