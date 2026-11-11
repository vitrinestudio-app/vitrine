//! Read-only resources: the project file, translation files, catalogs and
//! the documentation topics (`vitrine://docs/<topic>`, the same text as the
//! `docs_get` tool).

use rmcp::ErrorData;
use rmcp::model::{ReadResourceResult, Resource, ResourceContents, ResourceTemplate};
use serde_json::Value;
use vitrine_app::commands::AppState;
use vitrine_core::io::PROJECT_FILE;
use vitrine_core::model::Lang;

use crate::server::cmd;

pub const PROJECT_URI: &str = "vitrine://project/project.yaml";
pub const I18N_PREFIX: &str = "vitrine://project/i18n/";
pub const DEVICES_URI: &str = "vitrine://catalog/devices";
pub const TARGETS_URI: &str = "vitrine://catalog/targets";
pub const OPS_URI: &str = "vitrine://docs/ops";
pub const DOCS_PREFIX: &str = "vitrine://docs/";

/// Reference of the ops accepted by `apply_ops`.
pub const OPS_DOC: &str = include_str!("ops.md");

fn res(uri: &str, name: &str, mime: &str, description: &str) -> Resource {
    let mut r = Resource::new(uri, name).with_description(description);
    r.mime_type = Some(mime.to_owned());
    r
}

/// Resources available now (project files only with an open project).
pub fn list(app: &AppState) -> Vec<Resource> {
    let mut out = Vec::new();
    if let Ok(session) = app.session() {
        out.push(res(
            PROJECT_URI,
            "project.yaml",
            "application/yaml",
            "The open project's file (strips, elements, styles, targets)",
        ));
        let langs: Vec<String> = session.with_project(|p| {
            p.languages
                .available
                .iter()
                .map(ToString::to_string)
                .collect()
        });
        for lang in langs {
            out.push(res(
                &format!("{I18N_PREFIX}{lang}.yaml"),
                &format!("i18n/{lang}.yaml"),
                "application/yaml",
                &format!("Translations of `{lang}` (key: text)"),
            ));
        }
    }
    out.push(res(
        DEVICES_URI,
        "device catalog",
        "application/json",
        "Device models for device elements",
    ));
    out.push(res(
        TARGETS_URI,
        "target presets",
        "application/json",
        "Export target presets (store sizes, slots, galleries)",
    ));
    for (topic, what) in crate::docs::TOPICS {
        out.push(res(
            &format!("{DOCS_PREFIX}{topic}"),
            &format!("docs: {topic}"),
            "text/markdown",
            what,
        ));
    }
    out
}

/// URI templates (translation files of any language).
pub fn templates() -> Vec<ResourceTemplate> {
    let mut t = ResourceTemplate::new(format!("{I18N_PREFIX}{{lang}}.yaml"), "translations")
        .with_description("Translations of one language (key: text)");
    t.mime_type = Some("application/yaml".to_owned());
    vec![t]
}

fn text(uri: &str, mime: &str, body: String) -> ReadResourceResult {
    ReadResourceResult::new(vec![ResourceContents::TextResourceContents {
        uri: uri.to_owned(),
        mime_type: Some(mime.to_owned()),
        text: body,
        meta: None,
    }])
}

fn json_text(v: &Value) -> String {
    serde_json::to_string_pretty(v).unwrap_or_default()
}

fn mcp_err(e: &vitrine_app::CommandError) -> ErrorData {
    ErrorData::resource_not_found(e.message.clone(), None)
}

/// Reads resource `uri`.
pub fn read(app: &AppState, uri: &str) -> Result<ReadResourceResult, ErrorData> {
    match uri {
        _ if uri.starts_with(DOCS_PREFIX) => {
            return crate::docs::topic(&uri[DOCS_PREFIX.len()..])
                .map(|body| text(uri, "text/markdown", body))
                .ok_or_else(|| {
                    ErrorData::resource_not_found(format!("unknown resource `{uri}`"), None)
                });
        }
        DEVICES_URI => {
            let v = cmd(app, "catalog_devices", Value::Null).map_err(|e| mcp_err(&e))?;
            return Ok(text(uri, "application/json", json_text(&v)));
        }
        TARGETS_URI => {
            let v = cmd(app, "catalog_targets", Value::Null).map_err(|e| mcp_err(&e))?;
            return Ok(text(uri, "application/json", json_text(&v)));
        }
        _ => {}
    }
    let session = app.session().map_err(|e| mcp_err(&e))?;
    let path = if uri == PROJECT_URI {
        session.dir().join(PROJECT_FILE)
    } else if let Some(lang) = uri
        .strip_prefix(I18N_PREFIX)
        .and_then(|rest| rest.strip_suffix(".yaml"))
    {
        // A valid language tag cannot leave the i18n folder.
        let lang = Lang::parse(lang)
            .map_err(|e| ErrorData::invalid_params(format!("invalid language: {e}"), None))?;
        session.dir().join("i18n").join(format!("{lang}.yaml"))
    } else {
        return Err(ErrorData::resource_not_found(
            format!("unknown resource `{uri}`"),
            None,
        ));
    };
    let body = std::fs::read_to_string(&path)
        .map_err(|e| ErrorData::resource_not_found(format!("{}: {e}", path.display()), None))?;
    Ok(text(uri, "application/yaml", body))
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use vitrine_app::dialogs::NoDialogs;
    use vitrine_app::events::NullSink;
    use vitrine_app::settings::AppPaths;

    use super::*;

    #[test]
    fn lists_and_reads_resources() {
        let tmp = tempfile::tempdir().unwrap();
        let app = AppState::new(
            AppPaths::in_dir(&tmp.path().join("app")),
            Arc::new(NullSink),
            Arc::new(NoDialogs),
        )
        .without_watcher();
        // Without a project: catalogs and docs only.
        let uris: Vec<String> = list(&app).into_iter().map(|r| r.uri).collect();
        assert_eq!(uris[..2], [DEVICES_URI, TARGETS_URI]);
        assert!(uris.contains(&OPS_URI.to_owned()));
        assert_eq!(uris.len(), 2 + crate::docs::TOPICS.len());
        assert!(read(&app, PROJECT_URI).is_err());
        let dir = tmp.path().join("p");
        vitrine_app::testutil::copy_project(&vitrine_app::testutil::example_fixture(), &dir)
            .unwrap();
        app.open_project(&dir).unwrap();
        let uris: Vec<String> = list(&app).into_iter().map(|r| r.uri).collect();
        assert!(uris.contains(&format!("{I18N_PREFIX}fr-CA.yaml")));
        let r = read(&app, PROJECT_URI).unwrap();
        let ResourceContents::TextResourceContents { text, .. } = &r.contents[0] else {
            panic!("text expected");
        };
        assert!(text.contains("format: vitrine/1"));
        assert!(read(&app, &format!("{I18N_PREFIX}de.yaml")).is_ok());
        assert!(read(&app, &format!("{I18N_PREFIX}../project.yaml")).is_err());
        assert!(read(&app, "vitrine://nope").is_err());
        let ResourceContents::TextResourceContents { text, .. } =
            &read(&app, OPS_URI).unwrap().contents[0]
        else {
            panic!("text expected");
        };
        assert!(text.contains("set_string"));
    }
}
