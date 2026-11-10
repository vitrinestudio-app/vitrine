//! Tool routers by area. Each module adds `#[tool]` methods to
//! [`crate::ArrangiaMcp`] through its own `#[tool_router]` (combined in
//! `ArrangiaMcp::new`).

pub mod assets;
pub mod catalog;
pub mod docs;
pub mod elements;
pub mod history;
pub mod output;
pub mod project;
pub mod strings;
pub mod structure;
pub mod styles;
