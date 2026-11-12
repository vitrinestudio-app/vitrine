//! CI report formats of `vitrine validate` (`--format github|gitlab|junit`).
//!
//! - **github**: GitHub Actions workflow commands
//!   (`::error file=…,line=…,title=…::message`), shown as annotations on
//!   the run and on the pull request diff. GitHub shows at most 10 errors
//!   and 10 warnings per step (more are in the log only), so errors come
//!   first.
//! - **gitlab**: GitLab Code Quality report (JSON array; `artifacts:
//!   reports: codequality`), shown in the merge request widget.
//! - **junit**: `JUnit` XML (GitLab `artifacts: reports: junit`, Jenkins
//!   `junit`, Azure `PublishTestResults@2`, `CircleCI` `store_test_results`,
//!   Bitrise test reports). One test suite per target (plus `project`),
//!   one failing test case per error, warnings as skipped test cases
//!   (failing with `--strict`), a passing case for a target without
//!   findings.
//!
//! Findings carry no file position; [`Locator`] points them at the line
//! of `project.yaml` that defines the element or target (or the
//! `store:` / `languages:` section), relative to the working directory
//! (the repository root in CI).

use std::fmt::Write as _;
use std::path::{Component, Path, PathBuf};

use serde_json::{Value, json};
use sha2::{Digest as _, Sha256};
use vitrine_core::validate::{Finding, Severity};

/// Where a finding points in the repository.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Location {
    /// `/`-separated, relative to the working directory when possible.
    pub path: String,
    /// 1-based line in `project.yaml`, if found.
    pub line: Option<usize>,
}

/// Finds lines in `project.yaml` for findings.
pub struct Locator {
    path: String,
    lines: Vec<String>,
}

impl Locator {
    /// For the project in `dir` (reads `project.yaml`; a missing file gives
    /// locations without lines).
    pub fn new(dir: &Path) -> Self {
        let file = dir.join("project.yaml");
        let text = std::fs::read_to_string(&file).unwrap_or_default();
        Self::from_text(repo_path(&file), &text)
    }

    pub fn from_text(path: String, text: &str) -> Self {
        Self {
            path,
            lines: text.lines().map(str::to_owned).collect(),
        }
    }

    /// Line range (start, end exclusive) of the top-level key `key:`.
    fn section(&self, key: &str) -> Option<(usize, usize)> {
        let head = format!("{key}:");
        let start = self
            .lines
            .iter()
            .position(|l| l == &head || l.starts_with(&format!("{head} ")))?;
        let end = self.lines[start + 1..]
            .iter()
            .position(|l| l.chars().next().is_some_and(|c| c.is_ascii_alphabetic()))
            .map_or(self.lines.len(), |n| start + 1 + n);
        Some((start, end))
    }

    /// Line (0-based) of `id: <id>` inside the section `key`.
    fn id_line(&self, key: &str, id: &str) -> Option<usize> {
        let (start, end) = self.section(key)?;
        (start..end).find(|&i| {
            let l = self.lines[i].trim_start();
            let l = l.strip_prefix("- ").unwrap_or(l).trim_start();
            l.strip_prefix("id:").is_some_and(|v| {
                let v = v.trim();
                let v = v.split(" #").next().unwrap_or(v).trim();
                v.trim_matches(|c| c == '"' || c == '\'') == id
            })
        })
    }

    pub fn locate(&self, f: &Finding) -> Location {
        let code = f.code.as_str();
        let line = f
            .element
            .as_ref()
            .and_then(|e| self.id_line("canvases", e.as_str()))
            .or_else(|| {
                f.target
                    .as_ref()
                    .and_then(|t| self.id_line("targets", t.as_str()))
            })
            .or_else(|| {
                if code.starts_with("store_locale") {
                    self.section("store").map(|(s, _)| s)
                } else if matches!(code, "lang_not_available" | "invalid_lang") {
                    self.section("languages").map(|(s, _)| s)
                } else if code.starts_with("unknown_font") {
                    self.section("styles").map(|(s, _)| s)
                } else {
                    None
                }
            })
            .map(|i| i + 1);
        Location {
            path: self.path.clone(),
            line,
        }
    }
}

/// `file` relative to the working directory, `/`-separated (absolute when
/// it lies outside).
fn repo_path(file: &Path) -> String {
    let rel: PathBuf = if file.is_absolute() {
        let cwd = std::env::current_dir().ok();
        let canon = |p: &Path| std::fs::canonicalize(p).unwrap_or_else(|_| p.to_owned());
        cwd.and_then(|c| {
            file.strip_prefix(&c)
                .map(Path::to_owned)
                .ok()
                .or_else(|| canon(file).strip_prefix(canon(&c)).map(Path::to_owned).ok())
        })
        .unwrap_or_else(|| file.to_owned())
    } else {
        file.to_owned()
    };
    if rel.is_absolute() {
        return rel.display().to_string().replace('\\', "/");
    }
    rel.components()
        .filter_map(|c| match c {
            Component::Normal(s) => Some(s.to_string_lossy().into_owned()),
            Component::ParentDir => Some("..".to_owned()),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("/")
}

/// Where the finding applies, for titles: ``target (lang) page 2 `element` ``.
fn scope(f: &Finding) -> String {
    let mut parts = Vec::new();
    match (&f.target, &f.lang) {
        (Some(t), Some(l)) => parts.push(format!("{t} ({l})")),
        (Some(t), None) => parts.push(t.to_string()),
        (None, Some(l)) => parts.push(format!("({l})")),
        (None, None) => {}
    }
    if let Some(p) = f.page {
        parts.push(format!("page {}", p + 1));
    }
    if let Some(e) = &f.element {
        parts.push(format!("`{e}`"));
    }
    parts.join(" ")
}

fn level_name(l: Severity) -> &'static str {
    match l {
        Severity::Error => "error",
        Severity::Warn => "warning",
        Severity::Info => "info",
    }
}

// ----- GitHub ---------------------------------------------------------------------------

fn gh_data(s: &str) -> String {
    s.replace('%', "%25")
        .replace('\r', "%0D")
        .replace('\n', "%0A")
}

fn gh_prop(s: &str) -> String {
    gh_data(s).replace(':', "%3A").replace(',', "%2C")
}

/// GitHub workflow commands, errors first; infos only with `infos`.
pub fn github(findings: &[Finding], loc: &Locator, infos: bool) -> String {
    let mut out = String::new();
    for level in [Severity::Error, Severity::Warn, Severity::Info] {
        if level == Severity::Info && !infos {
            continue;
        }
        let cmd = match level {
            Severity::Error => "error",
            Severity::Warn => "warning",
            Severity::Info => "notice",
        };
        for f in findings.iter().filter(|f| f.level == level) {
            let l = loc.locate(f);
            let mut props = format!("file={}", gh_prop(&l.path));
            if let Some(line) = l.line {
                let _ = write!(props, ",line={line}");
            }
            let s = scope(f);
            let title = if s.is_empty() {
                format!("vitrine: {}", f.code.as_str())
            } else {
                format!("vitrine: {} – {s}", f.code.as_str())
            };
            let _ = writeln!(
                out,
                "::{cmd} {props},title={}::{}",
                gh_prop(&title),
                gh_data(&f.message)
            );
        }
    }
    out
}

// ----- GitLab Code Quality ----------------------------------------------------------------

/// Stable id of a finding (same finding → same fingerprint across runs).
fn fingerprint(f: &Finding) -> String {
    let mut h = Sha256::new();
    for part in [
        f.code.as_str(),
        f.target.as_ref().map_or("", |t| t.as_str()),
        f.lang.as_ref().map_or("", |l| l.as_str()),
        &f.page.map(|p| p.to_string()).unwrap_or_default(),
        f.element.as_ref().map_or("", |e| e.as_str()),
        &f.message,
    ] {
        h.update(part.as_bytes());
        h.update([0]);
    }
    h.finalize().iter().fold(String::new(), |mut s, b| {
        let _ = write!(s, "{b:02x}");
        s
    })
}

/// GitLab Code Quality report: error → `critical`, warning → `minor`,
/// info → `info`.
pub fn gitlab(findings: &[Finding], loc: &Locator) -> Value {
    let issues: Vec<Value> = findings
        .iter()
        .map(|f| {
            let l = loc.locate(f);
            let s = scope(f);
            json!({
                "type": "issue",
                "check_name": format!("vitrine/{}", f.code.as_str()),
                "description": if s.is_empty() { f.message.clone() } else { format!("{s}: {}", f.message) },
                "categories": ["Style"],
                "severity": match f.level {
                    Severity::Error => "critical",
                    Severity::Warn => "minor",
                    Severity::Info => "info",
                },
                "fingerprint": fingerprint(f),
                "location": {"path": l.path, "lines": {"begin": l.line.unwrap_or(1)}},
            })
        })
        .collect();
    Value::Array(issues)
}

// ----- JUnit ------------------------------------------------------------------------------

fn xml(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&apos;"),
            // XML 1.0 forbids most control characters.
            c if (c as u32) < 0x20 && !matches!(c, '\n' | '\r' | '\t') => {}
            c => out.push(c),
        }
    }
    out
}

/// `JUnit` XML: one suite per target in `targets` (checked targets, project
/// order) plus `project`; infos are left out.
pub fn junit(findings: &[Finding], targets: &[String], loc: &Locator, strict: bool) -> String {
    let mut suites: Vec<(String, Vec<&Finding>)> = std::iter::once("project".to_owned())
        .chain(targets.iter().cloned())
        .map(|t| (t, Vec::new()))
        .collect();
    for f in findings.iter().filter(|f| f.level != Severity::Info) {
        let key = f
            .target
            .as_ref()
            .map_or_else(|| "project".to_owned(), ToString::to_string);
        match suites.iter_mut().find(|(k, _)| *k == key) {
            Some((_, list)) => list.push(f),
            None => suites.push((key, vec![f])),
        }
    }
    let mut body = String::new();
    let (mut total, mut failures, mut skipped) = (0, 0, 0);
    for (name, list) in &suites {
        let mut cases = String::new();
        let (mut t, mut fl, mut sk) = (0, 0, 0);
        if list.is_empty() {
            t += 1;
            let _ = writeln!(
                cases,
                "    <testcase classname=\"vitrine.{n}\" name=\"{n}: no errors or warnings\" />",
                n = xml(name)
            );
        }
        for f in list {
            t += 1;
            let l = loc.locate(f);
            let s = scope(f);
            let case_name = format!("{} {}", f.code.as_str(), s);
            let file_attr = match l.line {
                Some(line) => format!(" file=\"{}\" line=\"{line}\"", xml(&l.path)),
                None => format!(" file=\"{}\"", xml(&l.path)),
            };
            let fails = f.level == Severity::Error || strict;
            let detail = format!(
                "{}: {}\n{}{}",
                level_name(f.level),
                f.message,
                l.path,
                l.line.map(|n| format!(":{n}")).unwrap_or_default()
            );
            let _ = writeln!(
                cases,
                "    <testcase classname=\"vitrine.{}\" name=\"{}\"{file_attr}>",
                xml(name),
                xml(case_name.trim())
            );
            if fails {
                fl += 1;
                let _ = writeln!(
                    cases,
                    "      <failure type=\"{}\" message=\"{}\">{}</failure>",
                    xml(f.code.as_str()),
                    xml(&f.message),
                    xml(&detail)
                );
            } else {
                sk += 1;
                let _ = writeln!(
                    cases,
                    "      <skipped message=\"{}\" />",
                    xml(&format!("warning: {} (fails with --strict)", f.message))
                );
            }
            let _ = writeln!(cases, "    </testcase>");
        }
        total += t;
        failures += fl;
        skipped += sk;
        let _ = write!(
            body,
            "  <testsuite name=\"{}\" tests=\"{t}\" failures=\"{fl}\" errors=\"0\" skipped=\"{sk}\">\n{cases}  </testsuite>\n",
            xml(name)
        );
    }
    format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<testsuites name=\"vitrine validate\" tests=\"{total}\" failures=\"{failures}\" errors=\"0\" skipped=\"{skipped}\">\n{body}</testsuites>\n"
    )
}

#[cfg(test)]
mod tests {
    use vitrine_core::model::{ElementId, Lang, TargetId};
    use vitrine_core::validate::FindingCode;

    use super::*;

    const YAML: &str = "format: vitrine/1
languages:
  default: en
styles:
  colors: {}
canvases:
- id: phone
  elements:
  - id: title
    kind: text
  - id: \"shot\"
targets:
- id: phone
  canvas: phone
- id: ipad # tablet
  canvas: phone
store:
  apple: {}
";

    fn finding(
        level: Severity,
        code: FindingCode,
        target: Option<&str>,
        el: Option<&str>,
    ) -> Finding {
        Finding {
            level,
            code,
            target: target.map(TargetId::new),
            lang: target.map(|_| Lang::new("de")),
            page: el.map(|_| 1),
            element: el.map(ElementId::new),
            message: "50% too long, \"really\" <b>\nsecond line".to_owned(),
        }
    }

    fn loc() -> Locator {
        Locator::from_text("app/screens/project.yaml".into(), YAML)
    }

    #[test]
    fn locates_elements_targets_and_sections() {
        let l = loc();
        let at = |f: &Finding| l.locate(f).line;
        assert_eq!(
            at(&finding(
                Severity::Error,
                FindingCode::TextOverflow,
                Some("phone"),
                Some("title")
            )),
            Some(9)
        );
        assert_eq!(
            at(&finding(
                Severity::Error,
                FindingCode::TextOverflow,
                Some("phone"),
                Some("shot")
            )),
            Some(11)
        );
        // `phone` is a canvas id too: targets are looked up in `targets:`.
        assert_eq!(
            at(&finding(
                Severity::Warn,
                FindingCode::StoreSize,
                Some("phone"),
                None
            )),
            Some(13)
        );
        assert_eq!(
            at(&finding(
                Severity::Warn,
                FindingCode::StoreSize,
                Some("ipad"),
                None
            )),
            Some(15)
        );
        assert_eq!(
            at(&finding(
                Severity::Warn,
                FindingCode::StoreLocaleMissing,
                None,
                None
            )),
            Some(17)
        );
        assert_eq!(
            at(&finding(Severity::Warn, FindingCode::NoGallery, None, None)),
            None
        );
        assert_eq!(
            l.locate(&finding(Severity::Warn, FindingCode::NoGallery, None, None))
                .path,
            "app/screens/project.yaml"
        );
    }

    #[test]
    fn repo_paths_are_relative_and_slashed() {
        assert_eq!(
            repo_path(Path::new("./demo/project.yaml")),
            "demo/project.yaml"
        );
        let cwd = std::env::current_dir().unwrap();
        assert_eq!(
            repo_path(&cwd.join("x").join("project.yaml")),
            "x/project.yaml"
        );
    }

    #[test]
    fn github_commands_are_escaped_and_errors_come_first() {
        let fs = [
            finding(Severity::Warn, FindingCode::StoreSize, Some("ipad"), None),
            finding(
                Severity::Error,
                FindingCode::TextOverflow,
                Some("phone"),
                Some("title"),
            ),
            finding(
                Severity::Info,
                FindingCode::Hyphenated,
                Some("phone"),
                Some("title"),
            ),
        ];
        let out = github(&fs, &loc(), false);
        let lines: Vec<&str> = out.lines().collect();
        assert_eq!(lines.len(), 2, "{out}");
        assert!(lines[0].starts_with("::error file=app/screens/project.yaml,line=9,title=vitrine%3A text_overflow – phone (de) page 2 `title`::"), "{out}");
        assert!(
            lines[0].ends_with("::50%25 too long, \"really\" <b>%0Asecond line"),
            "{out}"
        );
        assert!(
            lines[1].starts_with("::warning file=app/screens/project.yaml,line=15,"),
            "{out}"
        );
        assert!(github(&fs, &loc(), true).contains("::notice "));
    }

    #[test]
    fn gitlab_code_quality_fields() {
        let fs = [
            finding(
                Severity::Error,
                FindingCode::TextOverflow,
                Some("phone"),
                Some("title"),
            ),
            finding(Severity::Warn, FindingCode::NoGallery, None, None),
        ];
        let v = gitlab(&fs, &loc());
        let a = v.as_array().unwrap();
        assert_eq!(a.len(), 2);
        assert_eq!(a[0]["severity"], "critical");
        assert_eq!(a[0]["check_name"], "vitrine/text_overflow");
        assert_eq!(a[0]["location"]["path"], "app/screens/project.yaml");
        assert_eq!(a[0]["location"]["lines"]["begin"], 9);
        assert_eq!(a[1]["severity"], "minor");
        assert_eq!(a[1]["location"]["lines"]["begin"], 1);
        assert_eq!(a[0]["fingerprint"].as_str().unwrap().len(), 64);
        assert_ne!(a[0]["fingerprint"], a[1]["fingerprint"]);
        assert_eq!(
            gitlab(&fs, &loc())[0]["fingerprint"],
            a[0]["fingerprint"],
            "stable"
        );
    }

    #[test]
    fn junit_suites_failures_and_skips() {
        let fs = [
            finding(
                Severity::Error,
                FindingCode::TextOverflow,
                Some("phone"),
                Some("title"),
            ),
            finding(Severity::Warn, FindingCode::StoreSize, Some("phone"), None),
            finding(
                Severity::Info,
                FindingCode::Hyphenated,
                Some("phone"),
                Some("title"),
            ),
        ];
        let targets = vec!["phone".to_owned(), "ipad".to_owned()];
        let x = junit(&fs, &targets, &loc(), false);
        assert!(x.starts_with("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<testsuites name=\"vitrine validate\" tests=\"4\" failures=\"1\" errors=\"0\" skipped=\"1\">"), "{x}");
        assert!(
            x.contains(
                "<testsuite name=\"phone\" tests=\"2\" failures=\"1\" errors=\"0\" skipped=\"1\">"
            ),
            "{x}"
        );
        assert!(x.contains("name=\"ipad: no errors or warnings\""), "{x}");
        assert!(x.contains("name=\"project: no errors or warnings\""), "{x}");
        assert!(
            x.contains("message=\"50% too long, &quot;really&quot; &lt;b&gt;\nsecond line\""),
            "{x}"
        );
        assert!(x.contains("line=\"9\""), "{x}");
        let strict = junit(&fs, &targets, &loc(), true);
        assert!(
            strict.contains("failures=\"2\" errors=\"0\" skipped=\"0\""),
            "{strict}"
        );
    }
}
