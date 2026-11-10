//! Who made a change: the user (GUI) or an assistant over MCP (ADR 0013).
//!
//! The actor labels history steps ("Claude: move title") and is part of the
//! `project-edited` event, so the GUI can show and highlight changes made
//! by Claude. Commands do not take an actor argument; a front end runs
//! them inside [`Actor::scope`] (or [`crate::commands::dispatch_as`]) and
//! [`crate::session::Session::apply_ops`] picks up [`Actor::current`].
//! Outside a scope the actor is [`Actor::User`].

use std::cell::RefCell;

use serde::{Deserialize, Serialize};

/// The originator of an edit (`{"kind": "user"}`,
/// `{"kind": "claude", "client": "claude-desktop"}`).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Actor {
    /// The person at the GUI (default).
    #[default]
    User,
    /// An assistant connected over MCP; `client` names the MCP client.
    Claude { client: String },
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
    pub fn is_user(&self) -> bool {
        matches!(self, Actor::User)
    }

    /// History label of a step by this actor: unchanged for the user,
    /// `Claude: <label>` for Claude.
    pub fn label(&self, label: &str) -> String {
        match self {
            Actor::User => label.to_owned(),
            Actor::Claude { .. } => format!("Claude: {label}"),
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
    use super::Actor;

    #[test]
    fn labels_and_scope() {
        let claude = Actor::Claude {
            client: "claude-code".into(),
        };
        assert_eq!(Actor::User.label("move a"), "move a");
        assert_eq!(claude.label("move a"), "Claude: move a");
        assert_eq!(Actor::current(), Actor::User);
        let inner = claude.clone().scope(|| {
            assert_eq!(Actor::current(), claude);
            Actor::User.scope(Actor::current)
        });
        assert_eq!(inner, Actor::User);
        assert_eq!(Actor::current(), Actor::User);
        assert_eq!(
            serde_json::to_value(&claude).unwrap(),
            serde_json::json!({"kind": "claude", "client": "claude-code"})
        );
    }
}
