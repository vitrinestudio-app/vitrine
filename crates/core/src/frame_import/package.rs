//! Frame packages as the user drops them: a folder, a `.zip` or (macOS) a
//! `.dmg`, which is attached read-only to a temporary mount point and
//! detached again when the [`Package`] is dropped.

use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use serde::Serialize;

/// Kind of a package.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PackageKind {
    Folder,
    Zip,
    Dmg,
}

/// A PNG inside a package.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackageEntry {
    /// Path inside the package (`/`-separated).
    pub path: String,
    /// File name only.
    pub name: String,
}

enum Source {
    Folder(PathBuf),
    Zip(Mutex<zip::ZipArchive<fs::File>>),
    Dmg { mount: PathBuf },
}

/// An opened package.
pub struct Package {
    pub kind: PackageKind,
    /// What the user picked.
    pub path: PathBuf,
    pub entries: Vec<PackageEntry>,
    /// Files that are not PNGs (PSDs, licences …).
    pub skipped: usize,
    source: Source,
}

impl std::fmt::Debug for Package {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Package")
            .field("kind", &self.kind)
            .field("path", &self.path)
            .field("entries", &self.entries.len())
            .finish_non_exhaustive()
    }
}

/// Ignored paths: hidden files, macOS resource forks, `__MACOSX`.
fn ignored(path: &str) -> bool {
    path.split(['/', '\\'])
        .any(|c| c.starts_with('.') || c == "__MACOSX")
}

fn is_png(path: &str) -> bool {
    Path::new(path)
        .extension()
        .is_some_and(|e| e.eq_ignore_ascii_case("png"))
}

fn entry(path: String) -> PackageEntry {
    let name = path.rsplit('/').next().unwrap_or(&path).to_owned();
    PackageEntry { path, name }
}

impl Package {
    /// Opens a folder, `.zip` or `.dmg` (macOS only).
    pub fn open(path: &Path) -> Result<Package, String> {
        let meta = fs::metadata(path).map_err(|e| format!("{}: {e}", path.display()))?;
        let ext = path
            .extension()
            .map(|e| e.to_string_lossy().to_ascii_lowercase())
            .unwrap_or_default();
        if meta.is_dir() {
            return Self::folder(path, path, PackageKind::Folder, Source::Folder(path.into()));
        }
        match ext.as_str() {
            "zip" => Self::zip(path),
            "dmg" => Self::dmg(path),
            "png" => Err(format!(
                "{}: pick the whole bezel package (folder, .zip or .dmg), not a single PNG",
                path.display()
            )),
            _ => Err(format!("{}: not a folder, .zip or .dmg", path.display())),
        }
    }

    fn folder(
        path: &Path,
        root: &Path,
        kind: PackageKind,
        source: Source,
    ) -> Result<Package, String> {
        let mut entries = Vec::new();
        let mut skipped = 0;
        let mut stack = vec![root.to_owned()];
        while let Some(dir) = stack.pop() {
            let rd = fs::read_dir(&dir).map_err(|e| format!("{}: {e}", dir.display()))?;
            for item in rd {
                let item = item.map_err(|e| format!("{}: {e}", dir.display()))?;
                let p = item.path();
                let rel = p
                    .strip_prefix(root)
                    .unwrap_or(&p)
                    .components()
                    .map(|c| c.as_os_str().to_string_lossy())
                    .collect::<Vec<_>>()
                    .join("/");
                if ignored(&rel) {
                    continue;
                }
                let ft = item.file_type().map_err(|e| e.to_string())?;
                if ft.is_dir() {
                    stack.push(p);
                } else if is_png(&rel) {
                    entries.push(entry(rel));
                } else {
                    skipped += 1;
                }
            }
        }
        entries.sort_by(|a, b| a.path.cmp(&b.path));
        Ok(Package {
            kind,
            path: path.to_owned(),
            entries,
            skipped,
            source,
        })
    }

    fn zip(path: &Path) -> Result<Package, String> {
        let file = fs::File::open(path).map_err(|e| format!("{}: {e}", path.display()))?;
        let archive = zip::ZipArchive::new(file).map_err(|e| format!("{}: {e}", path.display()))?;
        let mut entries = Vec::new();
        let mut skipped = 0;
        for name in archive.file_names() {
            let name = name.map_err(|e| format!("{}: {e}", path.display()))?;
            if name.ends_with('/') || ignored(&name) {
                continue;
            }
            if is_png(&name) {
                entries.push(entry(name.into_owned()));
            } else {
                skipped += 1;
            }
        }
        entries.sort_by(|a, b| a.path.cmp(&b.path));
        Ok(Package {
            kind: PackageKind::Zip,
            path: path.to_owned(),
            entries,
            skipped,
            source: Source::Zip(Mutex::new(archive)),
        })
    }

    #[cfg(target_os = "macos")]
    fn dmg(path: &Path) -> Result<Package, String> {
        let mount = std::env::temp_dir().join(format!(
            "arrangia-dmg-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_or(0, |d| d.as_nanos())
        ));
        fs::create_dir_all(&mount).map_err(|e| format!("{}: {e}", mount.display()))?;
        let out = std::process::Command::new("hdiutil")
            .args([
                "attach",
                "-readonly",
                "-nobrowse",
                "-noautoopen",
                "-mountpoint",
            ])
            .arg(&mount)
            .arg(path)
            .output()
            .map_err(|e| format!("hdiutil: {e}"))?;
        if !out.status.success() {
            let _ = fs::remove_dir(&mount);
            return Err(format!(
                "{}: cannot attach the disk image: {}",
                path.display(),
                String::from_utf8_lossy(&out.stderr).trim()
            ));
        }
        Self::folder(
            path,
            &mount.clone(),
            PackageKind::Dmg,
            Source::Dmg { mount },
        )
    }

    #[cfg(not(target_os = "macos"))]
    fn dmg(path: &Path) -> Result<Package, String> {
        Err(format!(
            "{}: disk images can only be opened on macOS; please extract the .dmg first \
             (e.g. with 7-Zip) and import the extracted folder",
            path.display()
        ))
    }

    /// The bytes of `entry`.
    pub fn read(&self, entry: &PackageEntry) -> Result<Vec<u8>, String> {
        match &self.source {
            Source::Folder(root) | Source::Dmg { mount: root } => {
                let p = entry.path.split('/').fold(root.clone(), |p, c| p.join(c));
                fs::read(&p).map_err(|e| format!("{}: {e}", p.display()))
            }
            Source::Zip(archive) => {
                let mut a = archive
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner);
                let mut f = a
                    .by_name(&entry.path)
                    .map_err(|e| format!("{}: {e}", entry.path))?;
                let mut buf = Vec::new();
                f.read_to_end(&mut buf)
                    .map_err(|e| format!("{}: {e}", entry.path))?;
                Ok(buf)
            }
        }
    }

    /// File name of the package (`Bezel-iPhone-17.dmg`).
    pub fn display_name(&self) -> String {
        self.path.file_name().map_or_else(
            || self.path.display().to_string(),
            |n| n.to_string_lossy().into_owned(),
        )
    }
}

impl Drop for Package {
    fn drop(&mut self) {
        if let Source::Dmg { mount } = &self.source {
            let _ = std::process::Command::new("hdiutil")
                .args(["detach", "-quiet"])
                .arg(mount)
                .output();
            let _ = fs::remove_dir(mount);
        }
    }
}
