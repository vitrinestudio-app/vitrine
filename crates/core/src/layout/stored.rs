//! System fonts stored in the project (ADR 0028, the font counterpart of
//! ADR 0027): "Store used system fonts in the project" copies the files of
//! the installed families a project uses into `fonts/system/`, so a build
//! server (CI) or a colleague without those fonts renders the same.
//!
//! ```text
//! <project>/fonts/system/
//!   README.md     licence notice (generated)
//!   fonts.yaml    manifest: file, family, role (named / fallback + language),
//!                 original path, size, SHA-256, licence from the name table
//!   <file>.ttf|.otf|.ttc …   copied byte for byte
//! ```
//!
//! Only the files of the faces the project's weights pick are copied (CSS
//! matching is monotone: the face chosen from the full family is also
//! chosen from the copied subset). The folder is never part of a template
//! (`template::extract` only reads files directly in `fonts/`).
//!
//! Font licences vary: OFL fonts may be shared, Apple's and Microsoft's
//! system fonts and most commercial fonts may not be redistributed. The
//! plan reports each family's licence as read from the font; front ends
//! warn before storing (private repositories only).

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use skrifa::{FontRef, MetadataProvider, string::StringId};

use super::fonts::{FontDirReport, FontLibrary, FontSource};
use super::system::SystemFontIndex;
use crate::io::{from_yaml, to_yaml, write_atomic};
use crate::model::Project;

/// Folder below the project's `fonts/`.
pub const STORED_DIR: &str = "system";
pub const MANIFEST_FILE: &str = "fonts.yaml";
pub const README_FILE: &str = "README.md";

/// `<project>/fonts/system`.
pub fn stored_root(project: &Path) -> PathBuf {
    project.join("fonts").join(STORED_DIR)
}

/// What the licence of a font allows, as far as its name table tells.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LicenseKind {
    /// SIL Open Font License: may be shared (also in templates).
    Ofl,
    /// A platform vendor's system font (Apple, Microsoft): licensed for
    /// use on that platform, no redistribution.
    Restricted,
    /// Another licence text: read it before sharing.
    Other,
    /// The font states no licence: treat it as not redistributable.
    Unknown,
}

/// Licence and origin of a font, from its name table.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct FontLicenseInfo {
    pub kind: LicenseKind,
    /// Manufacturer (name ID 8), else the copyright holder.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub vendor: String,
    /// Name ID 0.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub copyright: String,
    /// Name ID 13 (shortened).
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub license: String,
    /// Name ID 14.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub license_url: String,
}

/// Reads the licence information of face `index` of a font file.
pub fn license_info(data: &[u8], index: u32) -> FontLicenseInfo {
    let Ok(font) = FontRef::from_index(data, index) else {
        return FontLicenseInfo {
            kind: LicenseKind::Unknown,
            vendor: String::new(),
            copyright: String::new(),
            license: String::new(),
            license_url: String::new(),
        };
    };
    let get = |id: StringId| {
        font.localized_strings(id)
            .english_or_first()
            .map(|s| s.chars().collect::<String>().trim().to_owned())
            .unwrap_or_default()
    };
    let copyright = get(StringId::COPYRIGHT_NOTICE);
    let manufacturer = get(StringId::MANUFACTURER);
    let license = get(StringId::LICENSE_DESCRIPTION);
    let license_url = get(StringId::LICENSE_URL);
    let names = crate::template::fonts::FontNames {
        family: String::new(),
        copyright: copyright.clone(),
        license: license.clone(),
        license_url: license_url.clone(),
    };
    let all = format!("{copyright} {manufacturer} {license}").to_ascii_lowercase();
    let kind = if names.declares_ofl() {
        LicenseKind::Ofl
    } else if ["apple inc", "apple computer", "microsoft corp"]
        .iter()
        .any(|v| all.contains(v))
    {
        LicenseKind::Restricted
    } else if license.is_empty() && license_url.is_empty() {
        LicenseKind::Unknown
    } else {
        LicenseKind::Other
    };
    let vendor = if manufacturer.is_empty() {
        holder(&copyright)
    } else {
        manufacturer
    };
    FontLicenseInfo {
        kind,
        vendor,
        copyright: shorten(&copyright, 200),
        license: shorten(&license, 300),
        license_url,
    }
}

/// "© 2017 Apple Inc. All rights reserved." → "Apple Inc."
fn holder(copyright: &str) -> String {
    let s = copyright
        .replace("Copyright", "")
        .replace("(c)", "")
        .replace("(C)", "")
        .replace('©', "");
    let s = s.trim_start_matches(|c: char| c.is_ascii_digit() || " ,-–".contains(c));
    let end = s
        .find(". All rights")
        .or_else(|| s.find(", All rights"))
        .map_or(s.len(), |i| i + 1);
    shorten(s[..end].trim(), 80)
}

fn shorten(s: &str, n: usize) -> String {
    let mut chars = s.chars();
    let head: String = chars.by_ref().take(n).collect();
    if chars.next().is_some() {
        format!("{head}…")
    } else {
        head
    }
}

/// Why a family is stored.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FontRole {
    /// A style names the family.
    Named,
    /// Fallback for characters no other font has (`lang`: only for that
    /// language).
    Fallback,
}

/// One stored family file (`fonts.yaml`).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct StoredFont {
    /// File name in `fonts/system/`.
    pub file: String,
    pub family: String,
    pub role: FontRole,
    /// Language of a fallback (`None` = every language).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lang: Option<String>,
    /// Where the file came from (on the computer that stored it).
    pub source: String,
    pub bytes: u64,
    pub sha256: String,
    pub license: FontLicenseInfo,
}

/// `fonts/system/fonts.yaml`.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct StoredManifest {
    #[serde(default)]
    pub fonts: Vec<StoredFont>,
}

/// Reads the manifest (`None` when missing or unreadable).
pub fn read_manifest(project: &Path) -> Option<StoredManifest> {
    fs::read_to_string(stored_root(project).join(MANIFEST_FILE))
        .ok()
        .and_then(|t| from_yaml(&t).ok())
}

/// Does the project hold stored system fonts?
pub fn has_stored(project: &Path) -> bool {
    read_manifest(project).is_some_and(|m| !m.fonts.is_empty())
}

fn is_font_file(p: &Path) -> bool {
    p.is_file()
        && p.extension().and_then(|x| x.to_str()).is_some_and(|x| {
            ["ttf", "otf", "ttc", "otc"].contains(&x.to_ascii_lowercase().as_str())
        })
}

/// Loads the stored system fonts into `lib` as project fonts; fallback
/// families join the fallback chains (their language's or every one).
/// Without a readable manifest every font file of the folder is loaded as
/// a plain project font.
pub fn load_stored(lib: &mut FontLibrary, project: &Path) -> FontDirReport {
    let root = stored_root(project);
    let mut report = FontDirReport::default();
    if !root.is_dir() {
        return report;
    }
    let manifest = read_manifest(project);
    let files: BTreeSet<String> = match &manifest {
        Some(m) => m.fonts.iter().map(|f| f.file.clone()).collect(),
        None => fs::read_dir(&root)
            .into_iter()
            .flatten()
            .flatten()
            .map(|e| e.path())
            .filter(|p| is_font_file(p))
            .filter_map(|p| p.file_name().map(|n| n.to_string_lossy().into_owned()))
            .collect(),
    };
    for file in files {
        if file.contains(['/', '\\']) || file.starts_with('.') {
            continue;
        }
        match lib.load_file(&root.join(&file)) {
            Ok(ids) => report.added.extend(ids),
            Err(e) => report.errors.push(e),
        }
    }
    for f in manifest.iter().flat_map(|m| &m.fonts) {
        if lib.has_family(&f.family) {
            lib.mark_stored(&f.family);
            if f.role == FontRole::Fallback {
                lib.add_chain_fallback(f.lang.as_deref(), &f.family);
            }
        }
    }
    report
}

/// What [`plan`] / [`apply`] does with one stored file.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum StoreAction {
    /// Copied from this computer (not stored yet).
    Add,
    /// Stored, but this computer's file differs: replaced.
    Update,
    /// Stored and current (or not installed on this computer).
    Keep,
    /// Stored but no longer used: removed.
    Remove,
}

/// One file of a [`FontStorePlan`].
#[derive(Clone, Debug, Serialize)]
pub struct FontStoreItem {
    pub family: String,
    pub role: FontRole,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub lang: Option<String>,
    /// File name in `fonts/system/`.
    pub file: String,
    /// The installed file (empty when this computer does not have it).
    pub source: String,
    pub action: StoreAction,
    pub bytes: u64,
    pub license: FontLicenseInfo,
}

/// Result of [`plan`] / [`apply`].
#[derive(Clone, Debug, Serialize)]
pub struct FontStorePlan {
    /// `<project>/fonts/system`.
    pub dir: PathBuf,
    /// The project holds stored fonts (before this call for a plan, after
    /// it for [`apply`]).
    pub stored: bool,
    pub items: Vec<FontStoreItem>,
    /// Families from this computer's system fonts the project uses (named
    /// or as fallback), whether stored or not.
    pub system_families: Vec<String>,
    /// Families whose licence does not allow sharing or is unclear (all
    /// but OFL): the front ends warn about them.
    pub restricted: Vec<String>,
    pub bytes_now: u64,
    pub bytes_after: u64,
    /// Nothing to add, update or remove.
    pub up_to_date: bool,
    /// `false` = dry run.
    pub applied: bool,
}

/// Weights (and their `**bold**` weight) the project's styles use.
fn used_weights(project: &Project) -> BTreeSet<u16> {
    let mut w: BTreeSet<u16> = project
        .styles
        .text
        .values()
        .map(|s| s.weight.unwrap_or(400))
        .chain(project.styles.spans.values().filter_map(|s| s.weight))
        .collect();
    w.insert(400);
    let bold: Vec<u16> = w.iter().map(|x| super::bolder(*x)).collect();
    w.extend(bold);
    w
}

/// Wanted copies: (family, role, lang, source path).
type Want = (String, FontRole, Option<String>, PathBuf);

/// The installed files the project's system families need on this
/// computer (bundled and top-level project fonts loaded first, stored
/// copies not, so a stored family is still recognised as a system one).
fn wanted(
    project: &Project,
    project_dir: &Path,
    index: &SystemFontIndex,
    mut lib: FontLibrary,
) -> (Vec<Want>, FontLibrary) {
    lib.load_dir(&project_dir.join("fonts"));
    super::setup::add_system_fonts(&mut lib, project, index);
    let weights = used_weights(project);
    let mut roles: Vec<(String, FontRole, Option<String>)> = lib
        .system_families()
        .iter()
        .map(|f| (f.clone(), FontRole::Named, None))
        .collect();
    roles.extend(
        lib.system_fallbacks()
            .iter()
            .map(|f| (f.clone(), FontRole::Fallback, None)),
    );
    for (lang, fams) in lib.lang_fallbacks() {
        roles.extend(
            fams.iter()
                .map(|f| (f.clone(), FontRole::Fallback, Some(lang.clone()))),
        );
    }
    let mut out: Vec<Want> = Vec::new();
    for (family, role, lang) in roles {
        let mut files = BTreeSet::new();
        for w in &weights {
            for italic in [false, true] {
                if let Some(face) = lib
                    .match_face(&family, *w, italic)
                    .and_then(|id| lib.face(id))
                    && let FontSource::System(p) = &face.source
                {
                    files.insert(p.clone());
                }
            }
        }
        out.extend(
            files
                .into_iter()
                .map(|p| (family.clone(), role, lang.clone(), p)),
        );
    }
    (out, lib)
}

fn sha256_file(p: &Path) -> Option<(u64, String)> {
    let data = fs::read(p).ok()?;
    Some((data.len() as u64, crate::samples::sha256_hex(&data)))
}

fn file_len(p: &Path) -> u64 {
    fs::metadata(p).map_or(0, |m| m.len())
}

/// A file name for `source` in the stored folder, unique among `taken`.
fn stored_name(source: &Path, taken: &BTreeMap<String, PathBuf>) -> String {
    let name = source.file_name().map_or_else(
        || "font.ttf".to_owned(),
        |n| n.to_string_lossy().into_owned(),
    );
    let name: String = name
        .chars()
        .map(|c| {
            if c == '/' || c == '\\' || c == ':' {
                '_'
            } else {
                c
            }
        })
        .collect();
    if taken.get(&name).is_none_or(|p| p == source) {
        return name;
    }
    let (stem, ext) = name.rsplit_once('.').unwrap_or((&name, "ttf"));
    (2..10_000)
        .map(|i| format!("{stem}-{i}.{ext}"))
        .find(|n| taken.get(n).is_none_or(|p| p == source))
        .unwrap_or(name)
}

/// What storing the used system fonts would do (dry run).
pub fn plan(project: &Project, project_dir: &Path, index: &SystemFontIndex) -> FontStorePlan {
    plan_with(project, project_dir, index, FontLibrary::bundled())
}

/// [`plan`] over `base` (the bundled fonts; tests: an empty library).
#[allow(clippy::too_many_lines, reason = "one pass: wanted, stored, totals")]
fn plan_with(
    project: &Project,
    project_dir: &Path,
    index: &SystemFontIndex,
    base: FontLibrary,
) -> FontStorePlan {
    let root = stored_root(project_dir);
    let manifest = read_manifest(project_dir).unwrap_or_default();
    let (want, lib) = wanted(project, project_dir, index, base);
    // File names: keep the stored name of a source, else derive one.
    let mut names: BTreeMap<String, PathBuf> = BTreeMap::new();
    for f in &manifest.fonts {
        names.insert(f.file.clone(), PathBuf::from(&f.source));
    }
    let mut items: Vec<FontStoreItem> = Vec::new();
    let mut seen_files = BTreeSet::new();
    let mut hashes: BTreeMap<PathBuf, Option<(u64, String)>> = BTreeMap::new();
    for (family, role, lang, src) in &want {
        let file = manifest
            .fonts
            .iter()
            .find(|f| Path::new(&f.source) == src)
            .map_or_else(|| stored_name(src, &names), |f| f.file.clone());
        names.insert(file.clone(), src.clone());
        let stored = manifest.fonts.iter().find(|f| f.file == file);
        let action = match stored {
            None => StoreAction::Add,
            Some(s) => {
                let current = hashes
                    .entry(src.clone())
                    .or_insert_with(|| sha256_file(src));
                match current {
                    Some((_, h)) if *h != s.sha256 => StoreAction::Update,
                    _ if !root.join(&file).is_file() => StoreAction::Add,
                    _ => StoreAction::Keep,
                }
            }
        };
        let face_index = lib
            .faces()
            .iter()
            .find(|f| matches!(&f.source, FontSource::System(p) if p == src))
            .map_or(0, |f| f.index);
        let license = stored.map_or_else(
            || {
                fs::read(src)
                    .map_or_else(|_| license_info(&[], 0), |d| license_info(&d, face_index))
            },
            |s| s.license.clone(),
        );
        seen_files.insert((file.clone(), family.to_lowercase(), *role, lang.clone()));
        items.push(FontStoreItem {
            family: family.clone(),
            role: *role,
            lang: lang.clone(),
            file,
            source: src.display().to_string(),
            action,
            bytes: file_len(src),
            license,
        });
    }
    // Stored entries not wanted: kept when this computer does not have the
    // family (another machine stored it), else removed.
    for s in &manifest.fonts {
        let key = (
            s.file.clone(),
            s.family.to_lowercase(),
            s.role,
            s.lang.clone(),
        );
        if seen_files.contains(&key) {
            continue;
        }
        let installed_here = index.family(&s.family).is_some();
        let named = super::setup::named_families(&project.styles)
            .iter()
            .any(|f| f.eq_ignore_ascii_case(&s.family));
        let action = if installed_here || (s.role == FontRole::Named && !named) {
            StoreAction::Remove
        } else {
            StoreAction::Keep
        };
        items.push(FontStoreItem {
            family: s.family.clone(),
            role: s.role,
            lang: s.lang.clone(),
            file: s.file.clone(),
            source: String::new(),
            action,
            bytes: s.bytes,
            license: s.license.clone(),
        });
    }
    let file_bytes = |pred: &dyn Fn(&FontStoreItem) -> bool| -> u64 {
        let mut seen = BTreeSet::new();
        items
            .iter()
            .filter(|i| pred(i) && seen.insert(i.file.clone()))
            .map(|i| i.bytes)
            .sum()
    };
    let bytes_now: u64 = manifest
        .fonts
        .iter()
        .map(|f| (f.file.clone(), file_len(&root.join(&f.file))))
        .collect::<BTreeMap<_, _>>()
        .values()
        .sum();
    let bytes_after = file_bytes(&|i| i.action != StoreAction::Remove);
    let mut system_families: Vec<String> = Vec::new();
    let mut restricted: Vec<String> = Vec::new();
    for i in items.iter().filter(|i| i.action != StoreAction::Remove) {
        if !system_families.contains(&i.family) {
            system_families.push(i.family.clone());
        }
        if i.license.kind != LicenseKind::Ofl && !restricted.contains(&i.family) {
            restricted.push(i.family.clone());
        }
    }
    let up_to_date = items.iter().all(|i| i.action == StoreAction::Keep);
    FontStorePlan {
        dir: root,
        stored: !manifest.fonts.is_empty(),
        items,
        system_families,
        restricted,
        bytes_now,
        bytes_after,
        up_to_date,
        applied: false,
    }
}

/// Stores the used system fonts: copies added/updated files, deletes
/// removed ones, writes the manifest and the licence notice; removes the
/// folder when nothing is left. Returns the plan with `applied: true`.
pub fn apply(
    project: &Project,
    project_dir: &Path,
    index: &SystemFontIndex,
) -> Result<FontStorePlan, String> {
    apply_with(project, project_dir, index, FontLibrary::bundled())
}

fn apply_with(
    project: &Project,
    project_dir: &Path,
    index: &SystemFontIndex,
    base: FontLibrary,
) -> Result<FontStorePlan, String> {
    let mut plan = plan_with(project, project_dir, index, base);
    let root = stored_root(project_dir);
    let kept: Vec<&FontStoreItem> = plan
        .items
        .iter()
        .filter(|i| i.action != StoreAction::Remove)
        .collect();
    if kept.is_empty() {
        remove(project_dir)?;
        plan.applied = true;
        plan.stored = false;
        return Ok(plan);
    }
    fs::create_dir_all(&root).map_err(|e| format!("{}: {e}", root.display()))?;
    let old = read_manifest(project_dir).unwrap_or_default();
    let mut manifest = StoredManifest::default();
    let mut copied = BTreeSet::new();
    for i in &kept {
        let dest = root.join(&i.file);
        let entry = match i.action {
            StoreAction::Add | StoreAction::Update => {
                if copied.insert(i.file.clone()) {
                    let data = fs::read(&i.source).map_err(|e| format!("{}: {e}", i.source))?;
                    write_atomic(&dest, &data).map_err(|e| e.to_string())?;
                }
                let (bytes, sha256) =
                    sha256_file(&dest).ok_or_else(|| format!("{}: unreadable", dest.display()))?;
                StoredFont {
                    file: i.file.clone(),
                    family: i.family.clone(),
                    role: i.role,
                    lang: i.lang.clone(),
                    source: i.source.clone(),
                    bytes,
                    sha256,
                    license: i.license.clone(),
                }
            }
            StoreAction::Keep | StoreAction::Remove => {
                let prev = old.fonts.iter().find(|f| f.file == i.file);
                match prev {
                    Some(p) => StoredFont {
                        family: i.family.clone(),
                        role: i.role,
                        lang: i.lang.clone(),
                        ..p.clone()
                    },
                    None => continue,
                }
            }
        };
        manifest.fonts.push(entry);
    }
    // Files no entry refers to any more.
    let keep_files: BTreeSet<&str> = manifest.fonts.iter().map(|f| f.file.as_str()).collect();
    for e in fs::read_dir(&root).into_iter().flatten().flatten() {
        let name = e.file_name().to_string_lossy().into_owned();
        if is_font_file(&e.path()) && !keep_files.contains(name.as_str()) {
            let _ = fs::remove_file(e.path());
        }
    }
    let yaml = to_yaml(&manifest).map_err(|e| e.to_string())?;
    write_atomic(&root.join(MANIFEST_FILE), yaml.as_bytes()).map_err(|e| e.to_string())?;
    write_atomic(&root.join(README_FILE), readme().as_bytes()).map_err(|e| e.to_string())?;
    plan.applied = true;
    plan.stored = true;
    Ok(plan)
}

/// Removes the stored system fonts (the whole folder). Returns the bytes
/// freed.
pub fn remove(project_dir: &Path) -> Result<u64, String> {
    let root = stored_root(project_dir);
    if !root.exists() {
        return Ok(0);
    }
    let bytes = crate::frame_import::embed::dir_bytes(&root);
    fs::remove_dir_all(&root).map_err(|e| format!("{}: {e}", root.display()))?;
    Ok(bytes)
}

/// The licence notice written into the folder.
pub fn readme() -> String {
    "# System fonts stored in this project\n\
     \n\
     Vitrine Studio copied the installed system fonts this project uses into\n\
     this folder (\"Store used system fonts in the project\"), so that\n\
     `vitrine export`, `render` and `validate` draw the same text on a build\n\
     server (CI) or on a computer without these fonts. `fonts.yaml` lists each\n\
     file with its family, the computer path it came from, its SHA-256 and the\n\
     licence the font states.\n\
     \n\
     **Licence.** Font licences differ. Fonts under the SIL Open Font License\n\
     may be shared. The system fonts of macOS and Windows (San Francisco, New\n\
     York, Helvetica Neue, PingFang, Hiragino, Segoe UI, Microsoft YaHei …) and\n\
     most commercial fonts are licensed for use on your computer only and must\n\
     **not** be redistributed.\n\
     \n\
     - Keep this project in a **private** repository and check each font's\n  \
     licence (`fonts.yaml`, `license` / `license_url`).\n\
     - Do **not** publish it with this folder: no public repository, no\n  \
     template, no gallery upload, no archive shared outside your team.\n  \
     Vitrine Studio templates never contain this folder.\n\
     - For a public repository, add `fonts/system/` to `.gitignore` (a build\n  \
     server then falls back to the bundled fonts) or remove the copies:\n  \
     `vitrine fonts unembed`.\n\
     \n\
     Update or remove the copies in Project settings → Fonts, or with\n\
     `vitrine fonts embed` / `vitrine fonts unembed`. This folder is\n\
     generated; do not edit it by hand.\n"
        .to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn index_with(dir: &Path, files: &[(&str, &[u8])]) -> SystemFontIndex {
        for (name, data) in files {
            fs::write(dir.join(name), data).unwrap();
        }
        SystemFontIndex::scan(&[dir.to_owned()])
    }

    fn project(font: &str) -> Project {
        let mut p = Project::new("p", crate::model::Lang::new("en"));
        p.styles.text.insert(
            "title".into(),
            crate::model::TextStyle {
                font: Some(font.into()),
                weight: Some(700),
                ..Default::default()
            },
        );
        p
    }

    #[test]
    fn licence_of_bundled_and_unknown_fonts() {
        let info = license_info(super::super::bundled::FILES[8].data(), 0);
        assert_eq!(info.kind, LicenseKind::Ofl);
        assert!(info.license_url.contains("OFL") || info.license_url.contains("ofl"));
        assert_eq!(license_info(b"nope", 0).kind, LicenseKind::Unknown);
        assert_eq!(
            holder("© 2017 Apple Inc. All rights reserved."),
            "Apple Inc."
        );
        assert_eq!(
            holder("Copyright 2011 The Lora Project Authors"),
            "The Lora Project Authors"
        );
    }

    #[test]
    fn store_update_and_remove_used_system_fonts() {
        // Lora (bundled) acts as a "system" family over an empty base.
        let sys = tempfile::tempdir().unwrap();
        let lora = super::super::bundled::FILES[8].data();
        let index = index_with(sys.path(), &[("Lora.ttf", lora)]);
        let proj = tempfile::tempdir().unwrap();
        let p = project("Lora");

        // Bundled base: Lora is bundled, so never a system font.
        let bundled = plan(&p, proj.path(), &index);
        assert!(bundled.items.is_empty() && bundled.up_to_date);

        let dry = plan_with(&p, proj.path(), &index, FontLibrary::empty());
        assert_eq!(dry.items.len(), 1);
        let item = &dry.items[0];
        assert_eq!(
            (
                item.family.as_str(),
                item.role,
                item.action,
                item.file.as_str()
            ),
            ("Lora", FontRole::Named, StoreAction::Add, "Lora.ttf")
        );
        assert_eq!(item.license.kind, LicenseKind::Ofl);
        assert!(dry.restricted.is_empty(), "{:?}", dry.restricted);
        assert!(!dry.applied && !dry.stored && !dry.up_to_date);
        assert!(!stored_root(proj.path()).exists());

        let done = apply_with(&p, proj.path(), &index, FontLibrary::empty()).unwrap();
        assert!(done.applied && done.stored);
        let root = stored_root(proj.path());
        assert_eq!(fs::read(root.join("Lora.ttf")).unwrap(), lora);
        assert!(root.join(README_FILE).is_file());
        let m = read_manifest(proj.path()).unwrap();
        assert_eq!(m.fonts.len(), 1);
        assert_eq!(m.fonts[0].bytes, lora.len() as u64);
        assert!(has_stored(proj.path()));

        // Loaded as a project font on a machine without the system font.
        let mut lib = FontLibrary::empty();
        let report = load_stored(&mut lib, proj.path());
        assert_eq!(report.errors.len(), 0);
        assert!(lib.has_family("Lora") && !lib.is_system_family("Lora"));
        assert_eq!(lib.stored_families(), ["Lora"]);

        // Up to date now; a changed system file is an update.
        let again = plan_with(&p, proj.path(), &index, FontLibrary::empty());
        assert!(again.up_to_date, "{:?}", again.items);
        fs::write(
            sys.path().join("Lora.ttf"),
            super::super::bundled::FILES[9].data(),
        )
        .unwrap();
        let index2 = SystemFontIndex::scan(&[sys.path().to_owned()]);
        let upd = plan_with(&p, proj.path(), &index2, FontLibrary::empty());
        assert_eq!(upd.items[0].action, StoreAction::Update);

        // Not used any more (style uses a bundled family): removed.
        let p2 = project("Inter");
        let rm = plan_with(&p2, proj.path(), &index, FontLibrary::empty());
        assert_eq!(rm.items[0].action, StoreAction::Remove);
        let rm = apply_with(&p2, proj.path(), &index, FontLibrary::empty()).unwrap();
        assert!(!rm.stored);
        assert!(!root.exists());

        // Stored on another machine, this one lacks the family: kept.
        apply_with(&p, proj.path(), &index, FontLibrary::empty()).unwrap();
        let empty = SystemFontIndex::default();
        let keep = plan_with(&p, proj.path(), &empty, FontLibrary::empty());
        assert_eq!(keep.items[0].action, StoreAction::Keep);
        // README and manifest count too.
        assert!(remove(proj.path()).unwrap() > keep.bytes_now);
        assert!(!root.exists());
    }
}
