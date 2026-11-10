//! The open project (KONZEPT §8.3): model + undo history + render context +
//! preview cache, saved atomically after every transaction and reloaded
//! live when the folder changes on disk.
//!
//! - [`Session::apply_ops`] applies ops as one transaction on a copy,
//!   saves it (revision check, atomic writes) and only then commits it and
//!   the history step. A revision conflict reloads the project from disk,
//!   emits `project-changed` (`reason: conflict`) and returns
//!   [`ErrorCode::Conflict`]; other save errors leave everything unchanged.
//! - The watcher reloads `project.yaml`/`i18n` on external changes. Our own
//!   writes are recognised because the folder then loads to exactly the
//!   in-memory project. Screenshot/asset changes invalidate previews that
//!   use them; font changes rebuild the render context.
//!
//! Locking: one `Mutex` for model + history (short critical sections; no
//! rendering under it), an `RwLock` for the render context. Rendering works
//! on a cloned project snapshot.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError, RwLock, Weak};

use arrangia_core::Project;
use arrangia_core::io::PROJECT_FILE;
use arrangia_core::model::Element;
use arrangia_core::ops::{History, Op};
use arrangia_render::scene::RenderContext;
use serde::Serialize;
use serde_json::json;

use crate::error::{CmdResult, CommandError, ErrorCode};
use crate::events::{EventSink, PROJECT_CHANGED, PROJECT_ERROR};
use crate::preview::PreviewCache;
use crate::watch::{self, FsChange, Watcher};
use arrangia_core::frame_import::FrameIndex;

/// A translation that changed.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize)]
pub struct StringRef {
    pub lang: String,
    pub key: String,
}

/// What differs between two versions of a project (ids in project order).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
#[allow(clippy::struct_excessive_bools)] // flags of independent parts
pub struct Changes {
    pub name: bool,
    pub languages: bool,
    pub styles: bool,
    /// Strips whose spec, backgrounds, variants or elements changed, plus
    /// added and removed strips.
    pub strips: Vec<String>,
    /// Added, removed or changed elements (a group also counts as changed
    /// when one of its children changed).
    pub elements: Vec<String>,
    pub strings: Vec<StringRef>,
    pub targets: Vec<String>,
    /// Changed screenshots/assets (project-relative paths; watcher only).
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub assets: Vec<String>,
    /// Project fonts changed (watcher only).
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub fonts: bool,
}

impl Changes {
    pub fn is_empty(&self) -> bool {
        *self == Changes::default()
    }

    /// Computes what differs between `old` and `new`.
    pub fn between(old: &Project, new: &Project) -> Changes {
        let mut c = Changes {
            name: old.name != new.name,
            languages: old.languages != new.languages,
            styles: old.styles != new.styles,
            ..Changes::default()
        };
        let ids = |a: Vec<String>, b: Vec<String>| -> Vec<String> {
            let mut seen = BTreeSet::new();
            a.into_iter()
                .chain(b)
                .filter(|i| seen.insert(i.clone()))
                .collect()
        };
        let strip_ids = ids(
            old.strips.iter().map(|s| s.id.to_string()).collect(),
            new.strips.iter().map(|s| s.id.to_string()).collect(),
        );
        c.strips = strip_ids
            .into_iter()
            .filter(|id| old.strip(id) != new.strip(id))
            .collect();
        let (old_el, new_el) = (elements(old), elements(new));
        let el_ids = ids(
            old_el.keys().cloned().collect(),
            new_el.keys().cloned().collect(),
        );
        c.elements = el_ids
            .into_iter()
            .filter(|id| old_el.get(id) != new_el.get(id))
            .collect();
        let target_ids = ids(
            old.targets.iter().map(|t| t.id.to_string()).collect(),
            new.targets.iter().map(|t| t.id.to_string()).collect(),
        );
        c.targets = target_ids
            .into_iter()
            .filter(|id| old.target(id) != new.target(id))
            .collect();
        let mut strings = BTreeSet::new();
        for (a, b) in [(old, new), (new, old)] {
            for (lang, map) in &a.strings {
                for (key, text) in map {
                    if b.string(lang.as_str(), key.as_str()) != Some(text.as_str()) {
                        strings.insert(StringRef {
                            lang: lang.to_string(),
                            key: key.to_string(),
                        });
                    }
                }
            }
        }
        c.strings = strings.into_iter().collect();
        c
    }
}

/// All elements by id (including group children).
fn elements(p: &Project) -> BTreeMap<String, &Element> {
    let mut out = BTreeMap::new();
    for s in &p.strips {
        s.walk_elements(&mut |e| {
            out.insert(e.id.to_string(), e);
        });
    }
    out
}

/// Undo/redo availability and step labels (most recent last).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct HistoryState {
    pub can_undo: bool,
    pub can_redo: bool,
    pub undo: Vec<String>,
    pub redo: Vec<String>,
}

/// Result of an apply, undo or redo.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ApplyResult {
    /// `false` for an undo/redo with nothing to do.
    pub applied: bool,
    /// Revision now on disk.
    pub revision: u64,
    pub changes: Changes,
    pub history: HistoryState,
}

/// Payload of the `project-changed` event.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ProjectChanged {
    /// `external` (watcher), `conflict` (save refused, reloaded) or `assets`.
    pub reason: &'static str,
    pub revision: u64,
    pub changes: Changes,
    /// The undo history was cleared (model reloaded).
    pub history_cleared: bool,
}

struct Inner {
    project: Project,
    history: History,
    undo_labels: Vec<String>,
    redo_labels: Vec<String>,
}

impl Inner {
    fn history_state(&self) -> HistoryState {
        HistoryState {
            can_undo: self.history.can_undo(),
            can_redo: self.history.can_redo(),
            undo: self.undo_labels.clone(),
            redo: self.redo_labels.clone(),
        }
    }
}

/// One open project.
pub struct Session {
    dir: PathBuf,
    inner: Mutex<Inner>,
    ctx: RwLock<Arc<RenderContext>>,
    /// Installed vendor frames (app-wide, ADR 0009), kept across render
    /// context rebuilds.
    frames: RwLock<Option<Arc<FrameIndex>>>,
    preview: PreviewCache,
    events: Arc<dyn EventSink>,
    watcher: Mutex<Option<Watcher>>,
    /// Last scratch project of an edit preview (key, revision, project).
    edit_preview: Mutex<Option<(String, u64, Arc<Project>)>>,
}

impl std::fmt::Debug for Session {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Session")
            .field("dir", &self.dir)
            .finish_non_exhaustive()
    }
}

/// Short label of a transaction (`update_element hero`, `3 changes`).
pub fn label_for(ops: &[Op]) -> String {
    let one = |op: &Op| -> String {
        let v = serde_json::to_value(op).unwrap_or_default();
        let tag = v.get("op").and_then(|t| t.as_str()).unwrap_or("change");
        let id = ["id", "key", "strip"]
            .iter()
            .find_map(|k| v.get(k).and_then(|x| x.as_str()))
            .or_else(|| {
                ["element", "target", "strip"]
                    .iter()
                    .find_map(|k| v.get(k).and_then(|x| x.get("id")).and_then(|x| x.as_str()))
            });
        match id {
            Some(id) => format!("{tag} {id}"),
            None => tag.to_owned(),
        }
    };
    match ops {
        [] => "nothing".to_owned(),
        [op] => one(op),
        [first, ..] => format!("{} (+{} more)", one(first), ops.len() - 1),
    }
}

fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(PoisonError::into_inner)
}

impl Session {
    /// Opens the project in `dir` (without watching; see [`Self::watch`]).
    pub fn open(dir: &Path, events: Arc<dyn EventSink>) -> CmdResult<Arc<Session>> {
        if !dir.join(PROJECT_FILE).is_file() {
            return Err(CommandError::new(
                ErrorCode::Load,
                format!("no {PROJECT_FILE} in `{}`", dir.display()),
            ));
        }
        let dir = dir
            .canonicalize()
            .map_err(|e| CommandError::io(format!("{}: {e}", dir.display())))?;
        let project = Project::load(&dir)?;
        let (ctx, _fonts) = RenderContext::new(&dir);
        Ok(Arc::new(Session {
            dir,
            inner: Mutex::new(Inner {
                project,
                history: History::new(),
                undo_labels: Vec::new(),
                redo_labels: Vec::new(),
            }),
            ctx: RwLock::new(Arc::new(ctx)),
            frames: RwLock::new(None),
            preview: PreviewCache::default(),
            events,
            watcher: Mutex::new(None),
            edit_preview: Mutex::new(None),
        }))
    }

    /// Starts the file watcher (reloads and events on external changes).
    pub fn watch(self: &Arc<Self>) -> CmdResult<()> {
        let weak: Weak<Session> = Arc::downgrade(self);
        let w = watch::watch(&self.dir, move |paths| {
            if let Some(s) = weak.upgrade() {
                s.handle_fs_changes(&paths);
            }
        })
        .map_err(|e| CommandError::io(format!("cannot watch the project folder: {e}")))?;
        *lock(&self.watcher) = Some(w);
        Ok(())
    }

    /// Stops the watcher (also on drop).
    pub fn stop_watching(&self) {
        lock(&self.watcher).take();
    }

    /// Canonical project folder.
    pub fn dir(&self) -> &Path {
        &self.dir
    }

    /// A copy of the current project (for rendering, summaries).
    pub fn snapshot(&self) -> Project {
        lock(&self.inner).project.clone()
    }

    /// Runs `f` on the current project under the lock (keep it short).
    pub fn with_project<R>(&self, f: impl FnOnce(&Project) -> R) -> R {
        f(&lock(&self.inner).project)
    }

    pub fn revision(&self) -> u64 {
        lock(&self.inner).project.revision
    }

    pub fn render_context(&self) -> Arc<RenderContext> {
        self.ctx
            .read()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }

    /// Renders `frame: imported:*` devices with `frames` from now on;
    /// `notify` tells the UI to reload (element sizes may change).
    pub fn set_frames(&self, frames: Option<Arc<FrameIndex>>, notify: bool) {
        *self.frames.write().unwrap_or_else(PoisonError::into_inner) = frames;
        self.rebuild_context();
        self.preview.clear();
        if notify {
            let revision = self.revision();
            self.emit_changed("frames", revision, Changes::default(), false);
        }
    }

    /// New render context (fonts re-read) with the current frames.
    fn rebuild_context(&self) {
        let frames = self
            .frames
            .read()
            .unwrap_or_else(PoisonError::into_inner)
            .clone();
        let (ctx, _) = RenderContext::new(&self.dir);
        *self.ctx.write().unwrap_or_else(PoisonError::into_inner) =
            Arc::new(ctx.with_frames(frames));
    }

    pub fn preview(&self) -> &PreviewCache {
        &self.preview
    }

    pub fn history_state(&self) -> HistoryState {
        lock(&self.inner).history_state()
    }

    /// Result of an edit that produced no ops (nothing applied or saved).
    pub fn unchanged(&self) -> ApplyResult {
        let inner = lock(&self.inner);
        ApplyResult {
            applied: false,
            revision: inner.project.revision,
            changes: Changes::default(),
            history: inner.history_state(),
        }
    }

    /// The scratch project of an edit preview (a gesture applied to a copy
    /// of the current project, never saved). The last one is memoised by
    /// `key` and revision, so the images of one drag frame share it.
    pub fn edit_preview(
        &self,
        key: String,
        make: impl FnOnce(&Project) -> CmdResult<Project>,
    ) -> CmdResult<Arc<Project>> {
        let (revision, project) = {
            let inner = lock(&self.inner);
            if let Some((k, rev, p)) = lock(&self.edit_preview).as_ref()
                && *k == key
                && *rev == inner.project.revision
            {
                return Ok(p.clone());
            }
            (inner.project.revision, inner.project.clone())
        };
        let scratch = Arc::new(make(&project)?);
        *lock(&self.edit_preview) = Some((key, revision, scratch.clone()));
        Ok(scratch)
    }

    /// Applies `ops` as one undo step and saves. `label` defaults to
    /// [`label_for`].
    pub fn apply_ops(&self, ops: Vec<Op>, label: Option<String>) -> CmdResult<ApplyResult> {
        if ops.is_empty() {
            return Err(CommandError::invalid("no ops given"));
        }
        let label = label.unwrap_or_else(|| label_for(&ops));
        self.transact(|project, history| {
            history.apply(project, ops)?;
            Ok(true)
        })
        .map(|(r, mut inner)| {
            if r.applied {
                inner.undo_labels.push(label);
                trim(&mut inner.undo_labels);
                inner.redo_labels.clear();
            }
            r.with_history(inner.history_state())
        })
    }

    /// Undoes the last step and saves.
    pub fn undo(&self) -> CmdResult<ApplyResult> {
        self.transact(|project, history| Ok(history.undo(project)?))
            .map(|(r, mut inner)| {
                if r.applied
                    && let Some(l) = inner.undo_labels.pop()
                {
                    inner.redo_labels.push(l);
                }
                r.with_history(inner.history_state())
            })
    }

    /// Redoes the last undone step and saves.
    pub fn redo(&self) -> CmdResult<ApplyResult> {
        self.transact(|project, history| Ok(history.redo(project)?))
            .map(|(r, mut inner)| {
                if r.applied
                    && let Some(l) = inner.redo_labels.pop()
                {
                    inner.undo_labels.push(l);
                }
                r.with_history(inner.history_state())
            })
    }

    /// Runs `f` on copies of project and history, saves the copy and
    /// commits both. Returns the result with the still-locked state for
    /// label bookkeeping.
    fn transact(
        &self,
        f: impl FnOnce(&mut Project, &mut History) -> CmdResult<bool>,
    ) -> CmdResult<(ApplyResult, MutexGuard<'_, Inner>)> {
        let mut inner = lock(&self.inner);
        let mut project = inner.project.clone();
        let mut history = inner.history.clone();
        if !f(&mut project, &mut history)? {
            let r = ApplyResult {
                applied: false,
                revision: inner.project.revision,
                changes: Changes::default(),
                history: HistoryState::default(),
            };
            return Ok((r, inner));
        }
        if let Err(e) = project.save(&self.dir) {
            let err = CommandError::from(e);
            if err.code == ErrorCode::Conflict {
                self.reload_locked(&mut inner, "conflict");
            }
            return Err(err);
        }
        let changes = Changes::between(&inner.project, &project);
        inner.project = project;
        inner.history = history;
        let r = ApplyResult {
            applied: true,
            revision: inner.project.revision,
            changes,
            history: HistoryState::default(),
        };
        Ok((r, inner))
    }

    /// Reloads the model from disk if it differs from memory; emits
    /// `project-changed` (or `project-error` when it does not load).
    /// Returns the changes (`None`: identical, e.g. our own write).
    pub fn reload(&self, reason: &'static str) -> Option<Changes> {
        let mut inner = lock(&self.inner);
        self.reload_locked(&mut inner, reason)
    }

    fn reload_locked(&self, inner: &mut Inner, reason: &'static str) -> Option<Changes> {
        let loaded = match Project::load(&self.dir) {
            Ok(p) => p,
            Err(e) => {
                self.events
                    .emit(PROJECT_ERROR, json!({ "message": e.to_string() }));
                return None;
            }
        };
        if loaded == inner.project {
            return None;
        }
        let changes = Changes::between(&inner.project, &loaded);
        inner.project = loaded;
        inner.history.clear();
        inner.undo_labels.clear();
        inner.redo_labels.clear();
        self.preview.clear();
        self.emit_changed(reason, inner.project.revision, changes.clone(), true);
        Some(changes)
    }

    fn emit_changed(&self, reason: &'static str, revision: u64, changes: Changes, cleared: bool) {
        let payload = ProjectChanged {
            reason,
            revision,
            changes,
            history_cleared: cleared,
        };
        self.events.emit(
            PROJECT_CHANGED,
            serde_json::to_value(payload).unwrap_or_default(),
        );
    }

    /// Reacts to changed paths (absolute) from the watcher.
    pub fn handle_fs_changes(&self, paths: &[PathBuf]) {
        let mut model = false;
        let mut fonts = false;
        let mut assets: Vec<String> = Vec::new();
        for p in paths {
            match watch::classify(&self.dir, &self.canonical(p)) {
                Some(FsChange::Model) => model = true,
                Some(FsChange::Font) => fonts = true,
                Some(FsChange::Asset(a)) if !assets.contains(&a) => assets.push(a),
                Some(FsChange::Asset(_)) | None => {}
            }
        }
        if fonts {
            self.rebuild_context();
            self.preview.clear();
        } else if !assets.is_empty() {
            self.preview.invalidate_assets(&assets);
        }
        if model {
            self.reload("external");
        }
        if fonts || !assets.is_empty() {
            let revision = self.revision();
            self.emit_changed(
                "assets",
                revision,
                Changes {
                    assets,
                    fonts,
                    ..Changes::default()
                },
                false,
            );
        }
    }
}

impl Session {
    /// `path` below the canonical project folder (watchers may report
    /// paths through symlinks, e.g. `/var` vs `/private/var` on macOS; the
    /// file itself may be gone).
    fn canonical(&self, path: &Path) -> PathBuf {
        if path.starts_with(&self.dir) {
            return path.to_owned();
        }
        match (path.parent().map(Path::canonicalize), path.file_name()) {
            (Some(Ok(parent)), Some(name)) => parent.join(name),
            _ => path.to_owned(),
        }
    }
}

fn trim(labels: &mut Vec<String>) {
    if labels.len() > History::DEFAULT_LIMIT {
        labels.remove(0);
    }
}

impl ApplyResult {
    fn with_history(mut self, history: HistoryState) -> Self {
        self.history = history;
        self
    }
}

#[cfg(test)]
mod tests;
