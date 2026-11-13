//! Shared argument pieces of the tools and their mapping onto the view
//! context of `vitrine_app` edit commands.

use std::fmt::Write as _;
use std::path::{Component, Path, PathBuf};

use schemars::JsonSchema;
use serde::Deserialize;
use serde_json::{Map, Value, json};
use vitrine_app::CommandError;
use vitrine_app::commands::AppState;
use vitrine_core::Project;

/// Where an edit goes.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Scope {
    /// The base element: all targets and languages (default).
    #[default]
    Base,
    /// The override of a variant (`variant`, default: the target's own
    /// variant), e.g. a different position on iPad landscape.
    Variant,
    /// The language override of `lang` (text elements: at, width, height,
    /// box, `font_size`, fit, align …), e.g. a wider box for German.
    Lang,
}

/// Declares a tool argument struct with the edit context fields
/// (`target`, `scope`, `variant`, `lang`) in front of its own fields.
macro_rules! edit_params {
    ($(#[$meta:meta])* pub struct $name:ident { $($body:tt)* }) => {
        $(#[$meta])*
        #[derive(Debug, Clone, serde::Deserialize, schemars::JsonSchema)]
        #[serde(deny_unknown_fields)]
        pub struct $name {
            $($body)*
            /// Target whose page geometry is used (page heights, gaps,
            /// grouping). Default: the target the user looks at in the GUI
            /// (when it shows the element's canvas), else the first target
            /// showing the canvas.
            #[serde(default)]
            pub target: Option<String>,
            /// Where the change goes: `base` (default; all targets and
            /// languages), `variant` (override of `variant` or the target's
            /// own variant), `lang` (language override of `lang`; text
            /// elements).
            #[serde(default)]
            pub scope: $crate::params::Scope,
            /// Variant name for `scope: variant` (default: the target's
            /// variant).
            #[serde(default)]
            pub variant: Option<String>,
            /// Language: required for `scope: lang`; otherwise the language
            /// whose text layout is used (default: the GUI's language, else
            /// the default language).
            #[serde(default)]
            pub lang: Option<String>,
        }

        impl $name {
            /// The edit context of these arguments.
            pub fn edit(&self) -> $crate::params::EditCtx {
                $crate::params::EditCtx {
                    target: self.target.clone(),
                    scope: self.scope,
                    variant: self.variant.clone(),
                    lang: self.lang.clone(),
                }
            }
        }
    };
}

pub(crate) use edit_params;

/// Edit context as given by a tool call.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct EditCtx {
    pub target: Option<String>,
    pub scope: Scope,
    pub variant: Option<String>,
    pub lang: Option<String>,
}

thread_local! {
    /// The GUI's view while a tool of a GUI-hosted server runs (see
    /// [`with_gui_view`]).
    static GUI_VIEW: std::cell::RefCell<Option<vitrine_app::ViewState>> =
        const { std::cell::RefCell::new(None) };
}

/// Runs `f` with `view` as the GUI's view: tools of a GUI-hosted server
/// default to the target and language the user looks at when the caller
/// omits them (ADR 0015). Headless servers never set it.
pub fn with_gui_view<R>(view: Option<vitrine_app::ViewState>, f: impl FnOnce() -> R) -> R {
    let prev = GUI_VIEW.with(|v| std::mem::replace(&mut *v.borrow_mut(), view));
    let _restore = RestoreView(prev);
    f()
}

/// Puts the previous GUI view back (also on panic).
struct RestoreView(Option<vitrine_app::ViewState>);

impl Drop for RestoreView {
    fn drop(&mut self) {
        let prev = self.0.take();
        GUI_VIEW.with(|v| *v.borrow_mut() = prev);
    }
}

/// The GUI's current target, when it exists in `project` and shows
/// `canvas` (any canvas when `None`).
pub fn gui_target(project: &Project, canvas: Option<&str>) -> Option<String> {
    let t = GUI_VIEW.with(|v| v.borrow().as_ref().and_then(|v| v.target.clone()))?;
    project
        .target(&t)
        .filter(|tg| canvas.is_none_or(|s| tg.canvas.as_str() == s))
        .map(|_| t)
}

/// The GUI's current language, when `project` has it.
pub fn gui_lang(project: &Project) -> Option<String> {
    let l = GUI_VIEW.with(|v| v.borrow().as_ref().and_then(|v| v.lang.clone()))?;
    project.languages.is_available(&l).then_some(l)
}

/// Canvas holding element `id`.
pub fn canvas_of_element(project: &Project, id: &str) -> Option<String> {
    project.canvases.iter().find_map(|s| {
        let mut found = false;
        s.walk_elements(&mut |e| found |= e.id.as_str() == id);
        found.then(|| s.id.to_string())
    })
}

/// The target for an edit on `canvas`: `given`, else (GUI-hosted) the
/// target the user looks at when it shows the canvas, else the first target
/// showing the canvas.
pub fn target_for(
    project: &Project,
    given: Option<&str>,
    canvas: Option<&str>,
) -> Result<String, CommandError> {
    if let Some(t) = given {
        if project.target(t).is_none() {
            return Err(CommandError::not_found(format!("unknown target `{t}`")));
        }
        return Ok(t.to_owned());
    }
    if let Some(t) = gui_target(project, canvas) {
        return Ok(t);
    }
    let pick = match canvas {
        Some(s) => project.targets.iter().find(|t| t.canvas.as_str() == s),
        None => project.targets.first(),
    };
    pick.map(|t| t.id.to_string()).ok_or_else(|| match canvas {
        Some(s) => CommandError::invalid(format!(
            "no target shows canvas `{s}` yet; add one with target_add {{preset, canvas: \"{s}\"}}"
        )),
        None => CommandError::invalid("the project has no targets; add one with target_add"),
    })
}

impl EditCtx {
    /// The view context JSON (`target`, `lang`, `variant`, `scope`) of the
    /// app's edit commands for an edit of `element` (or on `canvas`).
    pub fn view(
        &self,
        app: &AppState,
        element: Option<&str>,
        canvas: Option<&str>,
    ) -> Result<Map<String, Value>, CommandError> {
        let session = app.session()?;
        let project = session.snapshot();
        let canvas = match (canvas, element) {
            (Some(s), _) => Some(s.to_owned()),
            (None, Some(id)) => Some(
                canvas_of_element(&project, id)
                    .ok_or_else(|| CommandError::not_found(format!("unknown element `{id}`")))?,
            ),
            (None, None) => None,
        };
        let target = target_for(&project, self.target.as_deref(), canvas.as_deref())?;
        let mut v = Map::new();
        v.insert("target".into(), json!(target));
        if let Some(lang) = self.lang.clone().or_else(|| gui_lang(&project)) {
            v.insert("lang".into(), json!(lang));
        }
        match self.scope {
            Scope::Base => {
                v.insert("variant".into(), Value::Null);
            }
            Scope::Variant => {
                if let Some(name) = &self.variant {
                    v.insert("variant".into(), json!(name));
                } else if project.target(&target).is_none_or(|t| t.variant.is_none()) {
                    return Err(CommandError::invalid(format!(
                        "target `{target}` shows no variant; pass `variant` or use scope `base`"
                    )));
                }
            }
            Scope::Lang => {
                if self.lang.is_none() {
                    return Err(CommandError::invalid("scope `lang` needs `lang`"));
                }
                v.insert("variant".into(), Value::Null);
                v.insert("scope".into(), json!("lang"));
            }
        }
        Ok(v)
    }
}

/// `base` with the entries of `extra` (an object) added.
pub fn merged(mut base: Map<String, Value>, extra: Value) -> Value {
    if let Value::Object(m) = extra {
        base.extend(m);
    }
    Value::Object(base)
}

/// Compact form of an apply/edit result: `applied`, `revision`, the undo
/// label of the step, created ids and what changed (non-empty parts only).
pub fn edit_summary(r: &Value) -> Value {
    let mut out = json!({
        "applied": r["applied"],
        "revision": r["revision"],
    });
    if r["applied"] == true
        && let Some(label) = r["history"]["undo"].as_array().and_then(|u| u.last())
    {
        out["step"] = label.clone();
    }
    if let Some(c) = r.get("created").filter(|c| !c.is_null()) {
        out["created"] = c.clone();
    }
    let mut changed = Map::new();
    if let Some(c) = r["changes"].as_object() {
        for (k, v) in c {
            let keep = match v {
                Value::Bool(b) => *b,
                Value::Array(a) => !a.is_empty(),
                _ => false,
            };
            if keep {
                changed.insert(k.clone(), v.clone());
            }
        }
    }
    if !changed.is_empty() {
        out["changed"] = Value::Object(changed);
    }
    if let Some(d) = r
        .get("dropped_override_fields")
        .filter(|d| d.as_array().is_some_and(|a| !a.is_empty()))
    {
        out["dropped_override_fields"] = d.clone();
    }
    out
}

/// An existing file inside the project folder `dir`, given relative to it:
/// absolute paths, `..` and symlinks leading out of the folder are refused.
pub fn project_file(dir: &Path, rel: &str) -> Result<PathBuf, CommandError> {
    let path = Path::new(rel);
    if rel.is_empty()
        || path
            .components()
            .any(|c| !matches!(c, Component::Normal(_) | Component::CurDir))
    {
        return Err(CommandError::invalid(format!(
            "`{rel}`: give a path relative to the project folder (no absolute path, no `..`)"
        )));
    }
    let root = dir
        .canonicalize()
        .map_err(|e| CommandError::io(format!("{}: {e}", dir.display())))?;
    let full = root
        .join(path)
        .canonicalize()
        .map_err(|e| CommandError::not_found(format!("`{rel}` in the project folder: {e}")))?;
    if !full.starts_with(&root) {
        return Err(CommandError::invalid(format!(
            "`{rel}` leads out of the project folder (symlink)"
        )));
    }
    if !full.is_file() {
        return Err(CommandError::invalid(format!("`{rel}` is not a file")));
    }
    Ok(full)
}

/// One-line text for an edit result.
pub fn edit_line(r: &Value) -> String {
    if r["applied"] != true {
        return "nothing changed".to_owned();
    }
    let mut line = format!(
        "done: {} (revision {})",
        r["step"].as_str().unwrap_or("change"),
        r["revision"]
    );
    // `set_variant_override` replaces the whole override: say which
    // fields it removed, in case that was not meant.
    for d in r["dropped_override_fields"]
        .as_array()
        .into_iter()
        .flatten()
    {
        let fields: Vec<&str> = d["fields"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(Value::as_str)
            .collect();
        let _ = write!(
            line,
            "\nwarning: set_variant_override replaced the override of `{}` in variant `{}` of \
             `{}` and dropped {} – re-add them, or use element_update {{scope: \"variant\"}} \
             (merges) next time",
            d["element"].as_str().unwrap_or_default(),
            d["variant"].as_str().unwrap_or_default(),
            d["canvas"].as_str().unwrap_or_default(),
            fields.join(", ")
        );
    }
    line
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn summary_keeps_the_essentials() {
        let r = json!({
            "applied": true, "revision": 4,
            "changes": {"name": false, "elements": ["a"], "strings": [], "styles": false},
            "history": {"undo": ["x", "Claude Code: Move a"]},
            "created": ["a"],
        });
        let s = edit_summary(&r);
        assert_eq!(
            s,
            json!({"applied": true, "revision": 4, "step": "Claude Code: Move a",
                   "created": ["a"], "changed": {"elements": ["a"]}})
        );
        assert_eq!(edit_line(&s), "done: Claude Code: Move a (revision 4)");
        let none = edit_summary(&json!({"applied": false, "revision": 4, "changes": {}}));
        assert_eq!(edit_line(&none), "nothing changed");
    }

    #[test]
    fn dropped_override_fields_are_kept_and_warned() {
        let r = json!({
            "applied": true, "revision": 5, "changes": {},
            "history": {"undo": ["Apply ops"]},
            "dropped_override_fields": [
                {"canvas": "phone", "variant": "android", "element": "shot", "fields": ["at", "rotation"]}
            ],
        });
        let s = edit_summary(&r);
        assert_eq!(s["dropped_override_fields"][0]["element"], "shot");
        let line = edit_line(&s);
        assert!(
            line.contains("dropped at, rotation") && line.contains("element_update"),
            "{line}"
        );
        let none = edit_summary(&json!({"applied": true, "revision": 5,
                                        "dropped_override_fields": []}));
        assert!(none.get("dropped_override_fields").is_none());
    }

    #[test]
    fn project_files_stay_inside_the_project() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path().join("p");
        std::fs::create_dir_all(dir.join("ops")).unwrap();
        std::fs::write(dir.join("ops/a.json"), "[]").unwrap();
        std::fs::write(tmp.path().join("outside.json"), "[]").unwrap();
        assert!(
            project_file(&dir, "ops/a.json")
                .unwrap()
                .ends_with("a.json")
        );
        assert!(project_file(&dir, "./ops/a.json").is_ok());
        let abs = dir.join("ops/a.json");
        for bad in [
            abs.to_str().unwrap(),
            "../outside.json",
            "ops/../../outside.json",
            "",
            "ops",
            "ops/missing.json",
        ] {
            assert!(project_file(&dir, bad).is_err(), "{bad}");
        }
        #[cfg(unix)]
        {
            std::os::unix::fs::symlink(tmp.path().join("outside.json"), dir.join("link.json"))
                .unwrap();
            let e = project_file(&dir, "link.json").unwrap_err();
            assert!(e.message.contains("leads out"), "{e:?}");
        }
    }

    #[test]
    fn scope_parses() {
        let s: Scope = serde_json::from_value(json!("lang")).unwrap();
        assert_eq!(s, Scope::Lang);
        assert_eq!(Scope::default(), Scope::Base);
    }
}
