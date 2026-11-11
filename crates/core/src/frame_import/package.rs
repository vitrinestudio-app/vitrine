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
    /// A disk image attached by `hdiutil` (macOS only); detached on drop.
    #[cfg(target_os = "macos")]
    Dmg {
        mount: PathBuf,
    },
}

/// Why a package cannot be opened.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PackageError {
    /// The disk image has a software licence agreement (SLA) that macOS
    /// shows before mounting. Vitrine Studio never accepts it on the user's
    /// behalf: the user opens the image in Finder, accepts, and imports
    /// the mounted volume.
    LicenseAgreement {
        /// The `.dmg`.
        path: PathBuf,
        /// Where Finder will most likely mount it (`/Volumes/<name>`).
        volume: PathBuf,
    },
    Other(String),
}

impl std::fmt::Display for PackageError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::LicenseAgreement { volume, .. } => write!(
                f,
                "This disk image contains a licence agreement. Open it in Finder, accept the \
                 licence, then import the mounted volume (e.g. {}).",
                volume.display()
            ),
            Self::Other(s) => f.write_str(s),
        }
    }
}

impl std::error::Error for PackageError {}

impl From<String> for PackageError {
    fn from(s: String) -> Self {
        Self::Other(s)
    }
}

impl From<PackageError> for String {
    fn from(e: PackageError) -> Self {
        e.to_string()
    }
}

/// The volume Finder mounts `dmg` at: `/Volumes/<file stem>`.
pub fn likely_volume(dmg: &Path) -> PathBuf {
    let stem = dmg
        .file_stem()
        .map_or_else(|| "…".into(), |s| s.to_string_lossy().into_owned());
    Path::new("/Volumes").join(stem)
}

/// Does `hdiutil imageinfo -plist` output say the image has a licence
/// agreement?
pub fn imageinfo_has_sla(plist: &str) -> bool {
    let Some(i) = plist.find("<key>Software License Agreement</key>") else {
        return false;
    };
    plist[i..]
        .split_once("</key>")
        .is_some_and(|(_, rest)| rest.trim_start().starts_with("<true"))
}

/// Maps a failed `hdiutil attach` (its stderr) to an error: a cancelled
/// attach means the licence agreement was not accepted (stdin is closed,
/// so `hdiutil` cannot ask).
pub fn attach_error(dmg: &Path, stderr: &str) -> PackageError {
    let lower = stderr.to_lowercase();
    if lower.contains("attach canceled")
        || lower.contains("attach cancelled")
        || lower.contains("license agreement")
        || lower.contains("licence agreement")
    {
        return PackageError::LicenseAgreement {
            path: dmg.to_owned(),
            volume: likely_volume(dmg),
        };
    }
    PackageError::Other(format!(
        "{}: cannot attach the disk image: {}",
        dmg.display(),
        stderr.trim()
    ))
}

/// An opened package.
pub struct Package {
    pub kind: PackageKind,
    /// What the user picked.
    pub path: PathBuf,
    pub entries: Vec<PackageEntry>,
    /// Photoshop files (`.psd`), ignored.
    pub psd: usize,
    /// Licence files (`License.rtf` …), paths inside the package.
    pub licenses: Vec<String>,
    /// Other files that are not PNGs.
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

fn has_ext(path: &str, ext: &str) -> bool {
    Path::new(path)
        .extension()
        .is_some_and(|e| e.eq_ignore_ascii_case(ext))
}

/// `License.rtf`, `Licence.pdf`, `LICENSE.txt`, `Apple Design Resources
/// License.rtf` …
fn is_license(path: &str) -> bool {
    let name = path.rsplit('/').next().unwrap_or(path).to_lowercase();
    (name.contains("license") || name.contains("licence"))
        && ["rtf", "txt", "pdf", "html", "htm", "md"]
            .iter()
            .any(|e| has_ext(&name, e))
}

/// Non-PNG files of a package, counted by kind.
#[derive(Default)]
struct Others {
    psd: usize,
    licenses: Vec<String>,
    skipped: usize,
}

impl Others {
    fn add(&mut self, rel: &str) {
        if has_ext(rel, "psd") || has_ext(rel, "psb") {
            self.psd += 1;
        } else if is_license(rel) {
            self.licenses.push(rel.to_owned());
        } else {
            self.skipped += 1;
        }
    }
}

fn entry(path: String) -> PackageEntry {
    let name = path.rsplit('/').next().unwrap_or(&path).to_owned();
    PackageEntry { path, name }
}

impl Package {
    /// Opens a folder, `.zip` or `.dmg` (macOS only).
    pub fn open(path: &Path) -> Result<Package, PackageError> {
        let meta = fs::metadata(path).map_err(|e| format!("{}: {e}", path.display()))?;
        let ext = path
            .extension()
            .map(|e| e.to_string_lossy().to_ascii_lowercase())
            .unwrap_or_default();
        if meta.is_dir() {
            return Ok(Self::folder(
                path,
                path,
                PackageKind::Folder,
                Source::Folder(path.into()),
            )?);
        }
        match ext.as_str() {
            "zip" => Ok(Self::zip(path)?),
            "dmg" => Self::dmg(path),
            "png" => Err(PackageError::Other(format!(
                "{}: pick the whole bezel package (folder, .zip or .dmg), not a single PNG",
                path.display()
            ))),
            _ => Err(PackageError::Other(format!(
                "{}: not a folder, .zip or .dmg",
                path.display()
            ))),
        }
    }

    fn folder(
        path: &Path,
        root: &Path,
        kind: PackageKind,
        source: Source,
    ) -> Result<Package, String> {
        let mut entries = Vec::new();
        let mut others = Others::default();
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
                    others.add(&rel);
                }
            }
        }
        entries.sort_by(|a, b| a.path.cmp(&b.path));
        others.licenses.sort();
        Ok(Package {
            kind,
            path: path.to_owned(),
            entries,
            psd: others.psd,
            licenses: others.licenses,
            skipped: others.skipped,
            source,
        })
    }

    fn zip(path: &Path) -> Result<Package, String> {
        let file = fs::File::open(path).map_err(|e| format!("{}: {e}", path.display()))?;
        let archive = zip::ZipArchive::new(file).map_err(|e| format!("{}: {e}", path.display()))?;
        let mut entries = Vec::new();
        let mut others = Others::default();
        for name in archive.file_names() {
            let name = name.map_err(|e| format!("{}: {e}", path.display()))?;
            if name.ends_with('/') || ignored(&name) {
                continue;
            }
            if is_png(&name) {
                entries.push(entry(name.into_owned()));
            } else {
                others.add(&name);
            }
        }
        entries.sort_by(|a, b| a.path.cmp(&b.path));
        others.licenses.sort();
        Ok(Package {
            kind: PackageKind::Zip,
            path: path.to_owned(),
            entries,
            psd: others.psd,
            licenses: others.licenses,
            skipped: others.skipped,
            source: Source::Zip(Mutex::new(archive)),
        })
    }

    #[cfg(target_os = "macos")]
    fn dmg(path: &Path) -> Result<Package, PackageError> {
        // A licence agreement would make `hdiutil attach` wait for an answer
        // (stdin is closed, so it cancels). Detect it up front; never answer.
        if let Ok(info) = std::process::Command::new("hdiutil")
            .args(["imageinfo", "-plist"])
            .arg(path)
            .stdin(std::process::Stdio::null())
            .output()
            && info.status.success()
            && imageinfo_has_sla(&String::from_utf8_lossy(&info.stdout))
        {
            return Err(PackageError::LicenseAgreement {
                path: path.to_owned(),
                volume: likely_volume(path),
            });
        }
        let mount = std::env::temp_dir().join(format!(
            "vitrine-dmg-{}-{}",
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
            .stdin(std::process::Stdio::null())
            .output()
            .map_err(|e| format!("hdiutil: {e}"))?;
        if !out.status.success() {
            let _ = fs::remove_dir(&mount);
            return Err(attach_error(path, &String::from_utf8_lossy(&out.stderr)));
        }
        Ok(Self::folder(
            path,
            &mount.clone(),
            PackageKind::Dmg,
            Source::Dmg { mount },
        )?)
    }

    #[cfg(not(target_os = "macos"))]
    fn dmg(path: &Path) -> Result<Package, PackageError> {
        Err(PackageError::Other(format!(
            "{}: disk images can only be opened on macOS; please extract the .dmg first \
             (e.g. with 7-Zip) and import the extracted folder",
            path.display()
        )))
    }

    /// The bytes of `entry`.
    pub fn read(&self, entry: &PackageEntry) -> Result<Vec<u8>, String> {
        match &self.source {
            Source::Folder(root) => read_below(root, entry),
            #[cfg(target_os = "macos")]
            Source::Dmg { mount } => read_below(mount, entry),
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

    /// Licence files for display: an absolute path for folders,
    /// `<package>/<path inside>` for archives and disk images (their mount
    /// point is temporary).
    pub fn license_display(&self) -> Vec<String> {
        self.licenses
            .iter()
            .map(|rel| match self.kind {
                PackageKind::Folder => rel
                    .split('/')
                    .fold(self.path.clone(), |p, c| p.join(c))
                    .display()
                    .to_string(),
                PackageKind::Zip | PackageKind::Dmg => format!("{}/{rel}", self.display_name()),
            })
            .collect()
    }

    /// File name of the package (`Bezel-iPhone-17.dmg`).
    pub fn display_name(&self) -> String {
        self.path.file_name().map_or_else(
            || self.path.display().to_string(),
            |n| n.to_string_lossy().into_owned(),
        )
    }
}

/// The bytes of `entry` below the folder `root`.
fn read_below(root: &Path, entry: &PackageEntry) -> Result<Vec<u8>, String> {
    let p = entry
        .path
        .split('/')
        .fold(root.to_owned(), |p, c| p.join(c));
    fs::read(&p).map_err(|e| format!("{}: {e}", p.display()))
}

#[cfg(target_os = "macos")]
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
