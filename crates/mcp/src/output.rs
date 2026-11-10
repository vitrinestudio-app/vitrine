//! Tool results: `structuredContent` JSON plus a short text, PNG images as
//! image content blocks, and [`CommandError`]s as tool errors
//! (`isError: true`) the model can read and recover from.

use arrangia_app::{CommandError, ErrorCode};
use base64::Engine;
use rmcp::model::{CallToolResult, ContentBlock};
use serde::Serialize;
use serde_json::{Value, json};

/// What a tool hands back before it becomes a [`CallToolResult`].
#[derive(Debug, Clone, Default)]
pub struct ToolOutput {
    /// One or two lines for humans (and the model).
    pub summary: String,
    /// The machine-readable result (`structuredContent`).
    pub data: Value,
    /// PNG images, shown after the text.
    pub images: Vec<Vec<u8>>,
}

impl ToolOutput {
    /// `data` serialized; `summary` describes it.
    pub fn new(summary: impl Into<String>, data: impl Serialize) -> Self {
        Self {
            summary: summary.into(),
            data: serde_json::to_value(data).unwrap_or(Value::Null),
            images: Vec::new(),
        }
    }

    /// Appends a PNG image.
    #[must_use]
    pub fn with_png(mut self, png: Vec<u8>) -> Self {
        self.images.push(png);
        self
    }

    /// The MCP result. The text block carries the summary and the JSON (for
    /// clients that ignore `structuredContent`).
    pub fn into_result(self) -> CallToolResult {
        let structured = match self.data {
            Value::Object(_) => self.data,
            // structuredContent must be an object.
            other => json!({ "result": other }),
        };
        let mut content = vec![ContentBlock::text(format!(
            "{}\n{}",
            self.summary,
            compact(&structured)
        ))];
        for png in self.images {
            content.push(ContentBlock::image(
                base64::engine::general_purpose::STANDARD.encode(png),
                "image/png",
            ));
        }
        let mut r = CallToolResult::success(content);
        r.structured_content = Some(structured);
        r
    }
}

/// JSON without whitespace.
fn compact(v: &Value) -> String {
    serde_json::to_string(v).unwrap_or_default()
}

/// The `snake_case` name of an error code.
pub fn code_name(code: ErrorCode) -> String {
    serde_json::to_value(code)
        .ok()
        .and_then(|v| v.as_str().map(ToOwned::to_owned))
        .unwrap_or_else(|| "internal".to_owned())
}

/// A hint how the model can recover from an error class.
fn hint(code: ErrorCode) -> Option<&'static str> {
    Some(match code {
        ErrorCode::NoProject => {
            "open a project with project_open or create one with project_create"
        }
        ErrorCode::NotFound => "check the ids with project_summary / element_list",
        ErrorCode::Conflict => {
            "the project changed on disk and was reloaded; read it again (project_summary) and retry"
        }
        ErrorCode::Load => "fix project.yaml on disk (or undo the external change), then retry",
        ErrorCode::Busy => "wait for the running export to finish",
        ErrorCode::InvalidArgs => "check the tool's input schema and the argument values",
        ErrorCode::OpRejected => "the project is unchanged; adjust the change and retry",
        _ => return None,
    })
}

/// A command error as a tool error result: `{error: {code, message,
/// details}, hint}` plus the text `error (code): message`.
pub fn error_result(e: &CommandError) -> CallToolResult {
    let code = code_name(e.code);
    let mut body = json!({ "error": { "code": code, "message": e.message } });
    if let Some(d) = &e.details {
        body["error"]["details"] = d.clone();
    }
    if let Some(h) = hint(e.code) {
        body["hint"] = json!(h);
    }
    let mut text = format!("error ({code}): {}", e.message);
    if let Some(h) = hint(e.code) {
        text.push_str("\nhint: ");
        text.push_str(h);
    }
    if let Some(d) = &e.details {
        text.push('\n');
        text.push_str(&compact(d));
    }
    let mut r = CallToolResult::error(vec![ContentBlock::text(text)]);
    r.structured_content = Some(body);
    r
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn output_has_text_json_and_images() {
        let r = ToolOutput::new("2 elements", json!({"n": 2}))
            .with_png(vec![1, 2, 3])
            .into_result();
        assert_eq!(r.is_error, Some(false));
        assert_eq!(r.structured_content, Some(json!({"n": 2})));
        let text = r.content[0].as_text().unwrap().text.clone();
        assert_eq!(text, "2 elements\n{\"n\":2}");
        let img = r.content[1].as_image().unwrap();
        assert_eq!(img.mime_type, "image/png");
        assert_eq!(img.data, "AQID");
    }

    #[test]
    fn non_object_data_is_wrapped() {
        let r = ToolOutput::new("x", vec![1, 2]).into_result();
        assert_eq!(r.structured_content, Some(json!({"result": [1, 2]})));
    }

    #[test]
    fn errors_keep_code_message_and_details() {
        let e = CommandError::not_found("unknown element `x`").with_details(json!({"id": "x"}));
        let r = error_result(&e);
        assert_eq!(r.is_error, Some(true));
        let body = r.structured_content.unwrap();
        assert_eq!(body["error"]["code"], "not_found");
        assert_eq!(body["error"]["message"], "unknown element `x`");
        assert_eq!(body["error"]["details"]["id"], "x");
        assert!(body["hint"].as_str().unwrap().contains("element_list"));
        let text = r.content[0].as_text().unwrap().text.clone();
        assert!(text.starts_with("error (not_found): unknown element `x`"));
        assert_eq!(code_name(ErrorCode::OpRejected), "op_rejected");
        let conflict = error_result(&CommandError::new(ErrorCode::Conflict, "changed"));
        assert_eq!(
            conflict.structured_content.unwrap()["error"]["code"],
            "conflict"
        );
    }
}
