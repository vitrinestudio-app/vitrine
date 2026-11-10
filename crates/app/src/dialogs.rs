//! Native file dialogs behind a trait: the Tauri app implements it with
//! `tauri-plugin-dialog` (used from Rust only, the web view keeps
//! `core:default`); the dev bridge uses [`NoDialogs`] or a fixed answer.

use std::path::PathBuf;

/// A file type filter (`name`, extensions without dot).
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct FileFilter {
    pub name: String,
    pub extensions: Vec<String>,
}

/// Blocking file dialogs. Called from worker threads, never from the main
/// (UI) thread.
pub trait Dialogs: Send + Sync {
    /// A folder (`None` = cancelled).
    fn pick_folder(&self, title: &str) -> Option<PathBuf>;
    /// Files to open (empty = cancelled).
    fn pick_files(&self, title: &str, filters: &[FileFilter], multiple: bool) -> Vec<PathBuf>;
    /// A file to save to, proposing `default_name` (`None` = cancelled).
    fn save_file(
        &self,
        _title: &str,
        _default_name: &str,
        _filters: &[FileFilter],
    ) -> Option<PathBuf> {
        None
    }
}

/// No dialogs: every dialog is cancelled.
#[derive(Debug, Default, Clone, Copy)]
pub struct NoDialogs;

impl Dialogs for NoDialogs {
    fn pick_folder(&self, _: &str) -> Option<PathBuf> {
        None
    }

    fn pick_files(&self, _: &str, _: &[FileFilter], _: bool) -> Vec<PathBuf> {
        Vec::new()
    }
}

/// Fixed answers (bridge `--pick-folder`/`--pick-file`/`--save-file`, tests).
#[derive(Debug, Default, Clone)]
pub struct FixedDialogs {
    pub folder: Option<PathBuf>,
    pub files: Vec<PathBuf>,
    /// Answer of the save dialog.
    pub save: Option<PathBuf>,
}

impl Dialogs for FixedDialogs {
    fn pick_folder(&self, _: &str) -> Option<PathBuf> {
        self.folder.clone()
    }

    fn pick_files(&self, _: &str, _: &[FileFilter], _: bool) -> Vec<PathBuf> {
        self.files.clone()
    }

    fn save_file(&self, _: &str, _: &str, _: &[FileFilter]) -> Option<PathBuf> {
        self.save.clone()
    }
}
