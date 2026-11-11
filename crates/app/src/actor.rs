//! Who made a change: the user (GUI) or an assistant over MCP (ADR 0013).
//!
//! The actor labels history steps ("Claude Desktop: move title", by the
//! client's display name) and is part of the `project-edited` event, so the
//! GUI can show and highlight changes made by an assistant. Commands do not take an actor argument; a front end runs
//! them inside [`Actor::scope`] (or [`crate::commands::dispatch_as`]) and
//! [`crate::session::Session::apply_ops`] picks up [`Actor::current`].
//! Outside a scope the actor is [`Actor::User`].

use std::cell::RefCell;

use serde::{Deserialize, Serialize};

/// The originator of an edit (`{"kind": "user"}`,
/// `{"kind": "assistant", "client": "claude-ai", "name": "Claude Desktop"}`).
/// The former kind `"claude"` (before 2026-11-10) is still accepted when
/// deserializing (actors are not persisted; this only keeps old payloads
/// readable).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Actor {
    /// The person at the GUI (default).
    #[default]
    User,
    /// An AI assistant connected over MCP: `client` is the MCP client's
    /// `clientInfo.name`, `name` its display name ([`client_display_name`]).
    #[serde(alias = "claude")]
    Assistant {
        client: String,
        #[serde(default)]
        name: String,
    },
}

/// Fallback display name of an MCP client that names itself neither by
/// `title` nor by `name`.
pub const ASSISTANT_FALLBACK: &str = "Assistant";

/// Display name of an MCP client from its `clientInfo`: known clients by
/// name (`claude-ai` → "Claude Desktop", `claude-code` → "Claude Code"),
/// else its `title`, else its `name` prettified (`my-client` → "My
/// Client"), else [`ASSISTANT_FALLBACK`].
pub fn client_display_name(name: &str, title: Option<&str>) -> String {
    let name = name.trim();
    match name {
        "claude-ai" => return "Claude Desktop".to_owned(),
        "claude-code" => return "Claude Code".to_owned(),
        _ => {}
    }
    if let Some(title) = title.map(str::trim).filter(|t| !t.is_empty()) {
        return title.to_owned();
    }
    let words: Vec<String> = name
        .split(|c: char| c == '-' || c == '_' || c.is_whitespace())
        .filter(|w| !w.is_empty())
        .map(|w| {
            let mut chars = w.chars();
            chars.next().map_or_else(String::new, |first| {
                first.to_uppercase().chain(chars).collect()
            })
        })
        .collect();
    if words.is_empty() || name == "mcp" {
        ASSISTANT_FALLBACK.to_owned()
    } else {
        words.join(" ")
    }
}

thread_local! {
    static CURRENT: RefCell<Actor> = const { RefCell::new(Actor::User) };
}

/// Restores the previous actor when a scope ends (also on panic).
struct Restore(Option<Actor>);

impl Drop for Restore {
    fn drop(&mut self) {
        if let Some(prev) = self.0.take() {
            CURRENT.with(|c| *c.borrow_mut() = prev);
        }
    }
}

impl Actor {
    /// An assistant for the MCP client `client` (`clientInfo.name`) with
    /// the optional `clientInfo.title`.
    pub fn assistant(client: &str, title: Option<&str>) -> Actor {
        Actor::Assistant {
            client: client.to_owned(),
            name: client_display_name(client, title),
        }
    }

    pub fn is_user(&self) -> bool {
        matches!(self, Actor::User)
    }

    /// History label of a step by this actor: unchanged for the user,
    /// `<display name>: <label>` for an assistant.
    pub fn label(&self, label: &str) -> String {
        match self {
            Actor::User => label.to_owned(),
            Actor::Assistant { name, .. } if !name.is_empty() => format!("{name}: {label}"),
            Actor::Assistant { .. } => format!("{ASSISTANT_FALLBACK}: {label}"),
        }
    }

    /// The actor of the commands running on this thread.
    pub fn current() -> Actor {
        CURRENT.with(|c| c.borrow().clone())
    }

    /// Runs `f` with `self` as the current actor of this thread.
    pub fn scope<R>(self, f: impl FnOnce() -> R) -> R {
        let prev = CURRENT.with(|c| std::mem::replace(&mut *c.borrow_mut(), self));
        let _restore = Restore(Some(prev));
        f()
    }
}

#[cfg(test)]
mod tests {
    use super::{Actor, client_display_name};

    #[test]
    fn display_names() {
        assert_eq!(client_display_name("claude-ai", None), "Claude Desktop");
        assert_eq!(client_display_name("claude-code", Some("X")), "Claude Code");
        assert_eq!(
            client_display_name("cursor-vscode", Some("Cursor")),
            "Cursor"
        );
        assert_eq!(client_display_name("vitrine-test", None), "Vitrine Test");
        assert_eq!(
            client_display_name("my_client app", Some(" ")),
            "My Client App"
        );
        assert_eq!(client_display_name("mcp", None), "Assistant");
        assert_eq!(client_display_name(" ", None), "Assistant");
    }

    #[test]
    fn old_kind_claude_still_reads() {
        let a: Actor =
            serde_json::from_value(serde_json::json!({"kind": "claude", "client": "claude-code"}))
                .unwrap();
        assert_eq!(
            a,
            Actor::Assistant {
                client: "claude-code".into(),
                name: String::new()
            }
        );
        assert_eq!(a.label("x"), "Assistant: x");
    }

    #[test]
    fn labels_and_scope() {
        let claude = Actor::assistant("claude-code", None);
        assert_eq!(Actor::User.label("move a"), "move a");
        assert_eq!(claude.label("move a"), "Claude Code: move a");
        assert_eq!(Actor::current(), Actor::User);
        let inner = claude.clone().scope(|| {
            assert_eq!(Actor::current(), claude);
            Actor::User.scope(Actor::current)
        });
        assert_eq!(inner, Actor::User);
        assert_eq!(Actor::current(), Actor::User);
        assert_eq!(
            serde_json::to_value(&claude).unwrap(),
            serde_json::json!({"kind": "assistant", "client": "claude-code", "name": "Claude Code"})
        );
    }
}
