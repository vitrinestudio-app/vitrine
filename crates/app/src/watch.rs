//! Debounced watcher of the project folder (`notify`, KONZEPT §8.3) and
//! the classification of changed paths.

use std::path::{Path, PathBuf};
use std::time::Duration;

use arrangia_core::io::{I18N_DIR, PROJECT_FILE};
use notify::RecursiveMode;
use notify_debouncer_full::{DebounceEventResult, Debouncer, RecommendedCache, new_debouncer};

/// Debounce window: bursts of writes (atomic saves, copies) become one batch.
pub const DEBOUNCE: Duration = Duration::from_millis(150);

/// What a changed path means for the session.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FsChange {
    /// `project.yaml` or an `i18n/*.yaml` file.
    Model,
    /// A screenshot or asset (project-relative, `/`-separated).
    Asset(String),
    /// A file in `fonts/`.
    Font,
}

/// Classifies a changed path below `root` (both absolute; `root` should be
/// canonical). `None` for paths the session does not care about: export
/// output, caches, temp files of atomic saves, hidden files.
pub fn classify(root: &Path, path: &Path) -> Option<FsChange> {
    let rel = path.strip_prefix(root).ok()?;
    let parts: Vec<String> = rel
        .components()
        .map(|c| c.as_os_str().to_string_lossy().into_owned())
        .collect();
    let name = parts.last()?;
    if parts.iter().any(|p| p.starts_with('.')) || name.ends_with('~') {
        return None;
    }
    match parts[0].as_str() {
        PROJECT_FILE if parts.len() == 1 => Some(FsChange::Model),
        I18N_DIR
            if parts.len() == 2
                && Path::new(name)
                    .extension()
                    .is_some_and(|e| e.eq_ignore_ascii_case("yaml")) =>
        {
            Some(FsChange::Model)
        }
        "screenshots" | "assets" if parts.len() > 1 => Some(FsChange::Asset(parts.join("/"))),
        "fonts" => Some(FsChange::Font),
        _ => None,
    }
}

/// A running watcher; dropping it stops watching.
pub struct Watcher {
    _debouncer: Debouncer<notify::RecommendedWatcher, RecommendedCache>,
}

impl std::fmt::Debug for Watcher {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Watcher").finish_non_exhaustive()
    }
}

/// Watches `dir` recursively and calls `on_change` with the (deduplicated)
/// paths of every debounced batch, on the watcher's thread.
pub fn watch(
    dir: &Path,
    on_change: impl Fn(Vec<PathBuf>) + Send + 'static,
) -> Result<Watcher, notify::Error> {
    let mut debouncer = new_debouncer(DEBOUNCE, None, move |res: DebounceEventResult| {
        let Ok(events) = res else {
            return;
        };
        let mut paths: Vec<PathBuf> = events
            .into_iter()
            .filter(|e| !e.kind.is_access())
            .flat_map(|e| e.event.paths)
            .collect();
        paths.sort();
        paths.dedup();
        if !paths.is_empty() {
            on_change(paths);
        }
    })?;
    debouncer.watch(dir, RecursiveMode::Recursive)?;
    Ok(Watcher {
        _debouncer: debouncer,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classifies_project_paths() {
        let root = Path::new("/p");
        let c = |rel: &str| classify(root, &root.join(rel));
        assert_eq!(c("project.yaml"), Some(FsChange::Model));
        assert_eq!(c("i18n/de.yaml"), Some(FsChange::Model));
        assert_eq!(c("i18n/.de.yaml.123.tmp"), None);
        assert_eq!(c(".project.yaml.42.tmp"), None);
        assert_eq!(
            c("screenshots/en/ios-phone/home.png"),
            Some(FsChange::Asset("screenshots/en/ios-phone/home.png".into()))
        );
        assert_eq!(
            c("assets/logo.svg"),
            Some(FsChange::Asset("assets/logo.svg".into()))
        );
        assert_eq!(c("fonts/Brand.ttf"), Some(FsChange::Font));
        assert_eq!(c("export/x/en/01.png"), None);
        assert_eq!(c(".cache/render/a.png"), None);
        assert_eq!(c("screenshots"), None);
        assert_eq!(classify(root, Path::new("/elsewhere/project.yaml")), None);
    }
}
