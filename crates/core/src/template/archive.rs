//! Reading and writing packages: zip archives and extracted folders, with
//! size limits and safe paths (ADR 0023 "Safe import").

use std::collections::BTreeMap;
use std::fs;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

use super::{MANIFEST_FILE, Package, TemplateError, manifest_from_yaml, manifest_to_yaml};

/// Size limits for reading a package.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Limits {
    /// The packed file (`.vitrine-template`).
    pub max_package: u64,
    /// All files unpacked together.
    pub max_total: u64,
    /// One unpacked file.
    pub max_file: u64,
    /// Number of files.
    pub max_files: usize,
    /// Path depth (`assets/a/b/c.png` = 4).
    pub max_depth: usize,
}

const MB: u64 = 1024 * 1024;

impl Limits {
    /// Largest package the app, the CLI and the gallery client read (MB):
    /// generous, so a local pre-check never refuses what a gallery server
    /// with a raised `VITRINE_API_MAX_UPLOAD_MB` accepts. The server
    /// enforces its own limit ([`Self::for_package`]).
    pub const CLIENT_PACKAGE_MB: u64 = 250;

    /// The limits for packages of at most `max_package` bytes: unpacked at
    /// most twice that (at least 150 MB) – images and fonts barely
    /// compress, so more points to a zip bomb –, 50 MB per file, 1000
    /// files, depth 6.
    pub fn for_package(max_package: u64) -> Self {
        Self {
            max_package,
            max_total: max_package.saturating_mul(2).max(150 * MB),
            max_file: 50 * MB,
            max_files: 1000,
            max_depth: 6,
        }
    }
}

impl Default for Limits {
    /// The client limits: [`Limits::for_package`] of
    /// [`Limits::CLIENT_PACKAGE_MB`] (250 MB packed, 500 MB unpacked).
    fn default() -> Self {
        Self::for_package(Self::CLIENT_PACKAGE_MB * MB)
    }
}

/// `12.3 MB`.
fn mb(bytes: u64) -> String {
    format!("{:.1} MB", bytes as f64 / MB as f64)
}

/// Checks a package path: relative, `/`-separated, no empty, `.`, `..` or
/// hidden components, no backslashes, colons or control characters.
pub(crate) fn check_path(path: &str) -> Result<(), String> {
    let bad = |why: &str| Err(format!("`{path}`: {why}"));
    if path.is_empty() || path.len() > 255 {
        return bad("empty or too long path");
    }
    if path.starts_with('/') || path.contains('\\') || path.contains(':') {
        return bad("absolute paths, backslashes and colons are not allowed");
    }
    if path.chars().any(char::is_control) {
        return bad("control characters in the path");
    }
    for c in path.split('/') {
        if c.is_empty() || c == "." || c == ".." {
            return bad("path traversal or empty path components are not allowed");
        }
        if c.starts_with('.') {
            return bad("hidden files are not allowed");
        }
    }
    Ok(())
}

/// Junk that archivers add (macOS resource forks, Finder files): skipped.
fn is_junk(path: &str) -> bool {
    path.starts_with("__MACOSX/")
        || path
            .rsplit('/')
            .next()
            .is_some_and(|n| n == ".DS_Store" || n == "Thumbs.db")
}

/// Collects files while enforcing the limits.
struct Collector {
    limits: Limits,
    total: u64,
    manifest: Option<String>,
    files: BTreeMap<String, Vec<u8>>,
}

impl Collector {
    fn new(limits: Limits) -> Self {
        Self {
            limits,
            total: 0,
            manifest: None,
            files: BTreeMap::new(),
        }
    }

    fn add(
        &mut self,
        path: &str,
        declared: u64,
        reader: &mut dyn Read,
    ) -> Result<(), TemplateError> {
        check_path(path).map_err(TemplateError::Invalid)?;
        if path.split('/').count() > self.limits.max_depth {
            return Err(TemplateError::invalid(format!("`{path}`: nested too deep")));
        }
        if self.files.len() + 1 > self.limits.max_files {
            return Err(TemplateError::invalid(format!(
                "more than {} files in the template",
                self.limits.max_files
            )));
        }
        if declared > self.limits.max_file {
            return Err(too_big(path, self.limits.max_file));
        }
        // Never trust the declared size: read at most the limit + 1.
        let mut data = Vec::new();
        reader
            .take(self.limits.max_file + 1)
            .read_to_end(&mut data)
            .map_err(|e| TemplateError::invalid(format!("`{path}`: {e}")))?;
        if data.len() as u64 > self.limits.max_file {
            return Err(too_big(path, self.limits.max_file));
        }
        self.total += data.len() as u64;
        if self.total > self.limits.max_total {
            return Err(TemplateError::invalid(format!(
                "the template unpacks to more than {} (limit for unpacked files)",
                mb(self.limits.max_total)
            )));
        }
        if path == MANIFEST_FILE {
            let text = String::from_utf8(data)
                .map_err(|_| TemplateError::invalid("template.yaml is not UTF-8"))?;
            self.manifest = Some(text);
        } else {
            super::FileKind::of(path).map_err(TemplateError::Invalid)?;
            if self.files.insert(path.to_owned(), data).is_some() {
                return Err(TemplateError::invalid(format!("`{path}` exists twice")));
            }
        }
        Ok(())
    }

    fn finish(self) -> Result<Package, TemplateError> {
        let text = self.manifest.ok_or_else(|| {
            TemplateError::invalid("not a Vitrine Studio template: template.yaml is missing")
        })?;
        let manifest = manifest_from_yaml(&text)?;
        Package::new(manifest, self.files)
    }
}

fn too_big(path: &str, max: u64) -> TemplateError {
    TemplateError::invalid(format!("`{path}` is larger than {}", mb(max)))
}

impl Package {
    /// Reads and checks a packed template. A single top-level folder
    /// wrapping everything (`my-template/template.yaml`) is accepted.
    pub fn from_zip_bytes(bytes: &[u8], limits: Limits) -> Result<Self, TemplateError> {
        if bytes.len() as u64 > limits.max_package {
            return Err(TemplateError::invalid(format!(
                "the template file is {}, larger than the limit of {}",
                mb(bytes.len() as u64),
                mb(limits.max_package)
            )));
        }
        let mut zip = zip::ZipArchive::new(std::io::Cursor::new(bytes))
            .map_err(|e| TemplateError::invalid(format!("not a template (zip) file: {e}")))?;
        if zip.len() > limits.max_files * 2 {
            return Err(TemplateError::invalid(format!(
                "more than {} files in the template",
                limits.max_files
            )));
        }
        let names: Vec<String> = zip
            .file_names()
            .map(|n| n.map(std::borrow::Cow::into_owned))
            .collect::<Result<_, _>>()
            .map_err(|e| TemplateError::invalid(format!("unreadable zip entry name: {e}")))?;
        let prefix = wrapper_prefix(names.iter().map(String::as_str));
        let mut c = Collector::new(limits);
        for i in 0..zip.len() {
            let mut f = zip
                .by_index(i)
                .map_err(|e| TemplateError::invalid(format!("unreadable zip entry: {e}")))?;
            let name = f
                .name()
                .map_err(|e| TemplateError::invalid(format!("unreadable zip entry name: {e}")))?
                .into_owned();
            if f.is_symlink() {
                return Err(TemplateError::invalid(format!(
                    "`{name}`: symbolic links are not allowed"
                )));
            }
            if f.is_dir() || is_junk(&name) {
                continue;
            }
            let path = name.strip_prefix(&prefix).unwrap_or(&name).to_owned();
            let declared = f.size();
            c.add(&path, declared, &mut f)?;
        }
        c.finish()
    }

    /// Reads and checks a packed template file.
    pub fn from_zip_file(path: &Path, limits: Limits) -> Result<Self, TemplateError> {
        let meta = fs::metadata(path).map_err(TemplateError::io(path))?;
        if meta.len() > limits.max_package {
            return Err(TemplateError::invalid(format!(
                "{}: the template file is {}, larger than the limit of {}",
                path.display(),
                mb(meta.len()),
                mb(limits.max_package)
            )));
        }
        let bytes = fs::read(path).map_err(TemplateError::io(path))?;
        Self::from_zip_bytes(&bytes, limits)
    }

    /// Reads and checks an extracted template folder (the store). Symbolic
    /// links anywhere are refused.
    pub fn from_dir(dir: &Path, limits: Limits) -> Result<Self, TemplateError> {
        let mut c = Collector::new(limits);
        let mut stack = vec![(dir.to_owned(), String::new())];
        while let Some((d, rel)) = stack.pop() {
            let mut entries: Vec<_> = fs::read_dir(&d)
                .map_err(TemplateError::io(&d))?
                .collect::<Result<_, _>>()
                .map_err(TemplateError::io(&d))?;
            entries.sort_by_key(fs::DirEntry::file_name);
            for e in entries {
                let name = e.file_name().to_string_lossy().into_owned();
                let path = if rel.is_empty() {
                    name.clone()
                } else {
                    format!("{rel}/{name}")
                };
                let ft = e.file_type().map_err(TemplateError::io(&e.path()))?;
                if ft.is_symlink() {
                    return Err(TemplateError::invalid(format!(
                        "`{path}`: symbolic links are not allowed"
                    )));
                }
                if is_junk(&path) {
                    continue;
                }
                if ft.is_dir() {
                    stack.push((e.path(), path));
                } else {
                    let len = e.metadata().map_or(0, |m| m.len());
                    let mut f = fs::File::open(e.path()).map_err(TemplateError::io(&e.path()))?;
                    c.add(&path, len, &mut f)?;
                }
            }
        }
        c.finish()
    }

    /// Reads a `.vitrine-template` file or an extracted folder.
    pub fn open(path: &Path, limits: Limits) -> Result<Self, TemplateError> {
        if path.is_dir() {
            Self::from_dir(path, limits)
        } else {
            Self::from_zip_file(path, limits)
        }
    }

    /// The packed form (`template.yaml` first, deflate).
    pub fn to_zip_bytes(&self) -> Result<Vec<u8>, TemplateError> {
        let mut out = std::io::Cursor::new(Vec::new());
        {
            let mut zip = zip::ZipWriter::new(&mut out);
            let opts = zip::write::SimpleFileOptions::default()
                .compression_method(zip::CompressionMethod::Deflated)
                .unix_permissions(0o644);
            let mut put = |name: &str, data: &[u8]| -> Result<(), TemplateError> {
                zip.start_file(name, opts)
                    .and_then(|()| zip.write_all(data).map_err(Into::into))
                    .map_err(|e| TemplateError::invalid(format!("cannot pack `{name}`: {e}")))
            };
            put(MANIFEST_FILE, manifest_to_yaml(&self.manifest)?.as_bytes())?;
            for (path, data) in &self.files {
                put(path, data)?;
            }
            zip.finish()
                .map_err(|e| TemplateError::invalid(format!("cannot pack: {e}")))?;
        }
        Ok(out.into_inner())
    }

    /// Writes the packed form to `path` (atomically).
    pub fn write_zip(&self, path: &Path) -> Result<(), TemplateError> {
        let bytes = self.to_zip_bytes()?;
        if let Some(parent) = path.parent().filter(|p| !p.as_os_str().is_empty()) {
            fs::create_dir_all(parent).map_err(TemplateError::io(parent))?;
        }
        crate::io::write_atomic(path, &bytes).map_err(|e| TemplateError::invalid(e.to_string()))
    }

    /// Writes the extracted form into `dir` (which must not exist).
    pub fn write_dir(&self, dir: &Path) -> Result<(), TemplateError> {
        if dir.exists() {
            return Err(TemplateError::Conflict(format!("{} exists", dir.display())));
        }
        fs::create_dir_all(dir).map_err(TemplateError::io(dir))?;
        let write = |rel: &str, data: &[u8]| -> Result<(), TemplateError> {
            let path = join(dir, rel);
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent).map_err(TemplateError::io(parent))?;
            }
            fs::write(&path, data).map_err(TemplateError::io(&path))
        };
        write(MANIFEST_FILE, manifest_to_yaml(&self.manifest)?.as_bytes())?;
        for (path, data) in &self.files {
            write(path, data)?;
        }
        Ok(())
    }
}

/// `dir` joined with a checked package path.
pub(crate) fn join(dir: &Path, rel: &str) -> PathBuf {
    rel.split('/').fold(dir.to_owned(), |p, c| p.join(c))
}

/// `name/` when every entry lives below one top-level folder that holds
/// `template.yaml` (zips made by "Compress folder").
fn wrapper_prefix<'a>(names: impl Iterator<Item = &'a str>) -> String {
    let names: Vec<&str> = names.filter(|n| !is_junk(n)).collect();
    if names.contains(&MANIFEST_FILE) {
        return String::new();
    }
    let Some(first) = names
        .first()
        .and_then(|n| n.split_once('/'))
        .map(|(a, _)| a)
    else {
        return String::new();
    };
    let prefix = format!("{first}/");
    if names.iter().all(|n| n.starts_with(&prefix))
        && names
            .iter()
            .any(|n| *n == format!("{prefix}{MANIFEST_FILE}"))
    {
        prefix
    } else {
        String::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn paths_are_checked() {
        for ok in ["project.yaml", "assets/a b/ü.png", "fonts/X-Regular.ttf"] {
            assert!(check_path(ok).is_ok(), "{ok}");
        }
        for bad in [
            "/etc/passwd",
            "../x",
            "a/../b",
            "a//b",
            "./a",
            "a\\b",
            "C:/x",
            ".hidden",
            "a/.git/config",
            "",
            "a\u{0}b",
        ] {
            assert!(check_path(bad).is_err(), "{bad:?}");
        }
    }

    #[test]
    fn wrapper_folder_is_stripped() {
        let names = ["t/template.yaml", "t/project.yaml", "__MACOSX/t/._x"];
        assert_eq!(wrapper_prefix(names.into_iter()), "t/");
        let names = ["template.yaml", "t/project.yaml"];
        assert_eq!(wrapper_prefix(names.into_iter()), "");
        let names = ["t/template.yaml", "u/project.yaml"];
        assert_eq!(wrapper_prefix(names.into_iter()), "");
    }

    #[test]
    fn junk_is_recognised() {
        assert!(is_junk("__MACOSX/a/b"));
        assert!(is_junk("assets/.DS_Store"));
        assert!(!is_junk("assets/x.png"));
    }
}
