//! File-name rules (`data/frame-import/<source>.yaml`): which model,
//! colour and orientation a vendor file stands for.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::model::Orientation;

/// The embedded Apple rules (`data/frame-import/apple.yaml`).
const APPLE_YAML: &str = include_str!("../../data/frame-import/apple.yaml");

/// A rule file: one per frame source.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Rules {
    /// Source id (`apple`), used in `frame: imported:<source>`.
    pub source: String,
    pub label: String,
    /// `true` when every rule is checked against a real package; single
    /// rules say so with `verified: <date>`.
    #[serde(default)]
    pub verified: bool,
    /// Vendor colour name (case-insensitive) → colour id.
    #[serde(default)]
    pub color_aliases: BTreeMap<String, String>,
    pub rules: Vec<Rule>,
}

/// One file-name pattern.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Rule {
    pub pattern: String,
    /// Catalog model id.
    pub model: String,
    /// Orientation when the pattern has no `{orientation}`.
    #[serde(default)]
    pub orientation: Option<Orientation>,
    #[serde(default)]
    pub priority: i32,
    /// Date the rule was checked against a real package (`2026-11-10`).
    #[serde(default)]
    pub verified: Option<String>,
    #[serde(default)]
    pub note: Option<String>,
}

/// What a file name stands for.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FileMatch {
    pub model: String,
    /// Colour id (slug).
    pub color: String,
    /// Colour as written in the file name (`Black Titanium`).
    pub color_label: String,
    pub orientation: Orientation,
    /// The vendor's model name: the file name before the colour
    /// (`iPhone 18 Pro`, `MacBook Pro 14 - M4`). Different vendor models
    /// of one catalog model are installed side by side (ADR 0012).
    pub apple_model: String,
    pub priority: i32,
    /// The pattern that matched (`None` = a user assignment).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rule: Option<String>,
    /// The rule is checked against a real package (user assignments are
    /// trusted).
    #[serde(default)]
    pub verified: bool,
}

impl Rules {
    /// The built-in rules of `source` (`apple`), if there are any.
    pub fn builtin(source: &str) -> Option<&'static Rules> {
        static APPLE: std::sync::OnceLock<Rules> = std::sync::OnceLock::new();
        match source {
            "apple" => Some(
                APPLE
                    .get_or_init(|| Self::parse(APPLE_YAML).expect("embedded apple.yaml is valid")),
            ),
            _ => None,
        }
    }

    /// Does any rule map to `model`? (Models a source has no frames for,
    /// like `browser`, silently keep their parametric frame.)
    pub fn covers(&self, model: &str) -> bool {
        self.rules.iter().any(|r| r.model == model)
    }

    pub fn parse(yaml: &str) -> Result<Rules, String> {
        let rules: Rules = crate::io::from_yaml(yaml)?;
        for r in &rules.rules {
            Pattern::parse(&r.pattern)?;
            if r.orientation.is_none() && !r.pattern.contains("{orientation}") {
                return Err(format!(
                    "rule `{}`: no {{orientation}} in the pattern and no `orientation:`",
                    r.pattern
                ));
            }
        }
        Ok(rules)
    }

    /// The first rule matching `file_name` (folders and extension are
    /// ignored).
    pub fn match_file(&self, file_name: &str) -> Option<FileMatch> {
        let name = normalize_name(file_name);
        self.rules.iter().find_map(|r| {
            let pattern = Pattern::parse(&r.pattern).ok()?;
            let caps = pattern.matches(&name)?;
            let orientation = caps.orientation.or(r.orientation)?;
            let label = caps.color?;
            Some(FileMatch {
                model: r.model.clone(),
                color: self.color_id(&label),
                apple_model: vendor_model(&name, &label),
                color_label: label,
                orientation,
                priority: r.priority,
                rule: Some(r.pattern.clone()),
                verified: self.verified || r.verified.is_some(),
            })
        })
    }

    /// Colour id of a vendor colour name: alias or slug.
    pub fn color_id(&self, label: &str) -> String {
        self.color_aliases
            .iter()
            .find(|(k, _)| k.eq_ignore_ascii_case(label.trim()))
            .map_or_else(|| slug(label), |(_, v)| v.clone())
    }
}

/// The file name without folders and `.png`, with dashes, quotes and white
/// space normalised (en/em dash → `-`, typographic quotes → `"`, one space).
pub fn normalize_name(file: &str) -> String {
    let base = file.rsplit(['/', '\\']).next().unwrap_or(file);
    let base = match base.len().checked_sub(4) {
        Some(i) if base.is_char_boundary(i) && base[i..].eq_ignore_ascii_case(".png") => &base[..i],
        _ => base,
    };
    let mapped: String = base
        .chars()
        .map(|c| match c {
            '\u{2010}'..='\u{2015}' | '\u{2212}' => '-',
            '\u{201C}' | '\u{201D}' | '\u{2033}' => '"',
            '\u{2018}' | '\u{2019}' | '\u{2032}' => '\'',
            c if c.is_whitespace() => ' ',
            c => c,
        })
        .collect();
    mapped
        .split(' ')
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join(" ")
}

/// The vendor model name in a normalised file name: everything before
/// the last ` - <colour>` (`iPhone 18 Pro - Black - Portrait` →
/// `iPhone 18 Pro`). Without the colour in the name: the name without a
/// trailing ` - Portrait` / ` - Landscape`.
pub fn vendor_model(name: &str, color_label: &str) -> String {
    let lower = name.to_lowercase();
    let needle = format!(" - {}", color_label.trim().to_lowercase());
    let cut = if needle.len() > 3 {
        lower.rfind(&needle)
    } else {
        None
    }
    .or_else(|| {
        [" - portrait", " - landscape"]
            .iter()
            .find(|o| lower.ends_with(*o))
            .map(|o| lower.len() - o.len())
    });
    // Lower-casing may change byte lengths; fall back to the whole name.
    let base = cut
        .filter(|&i| lower.len() == name.len() && name.is_char_boundary(i))
        .map_or(name, |i| &name[..i]);
    let out = base.trim();
    if out.is_empty() { name.trim() } else { out }.to_owned()
}

/// Lower-case id: alphanumerics kept, everything else becomes one `-`.
pub fn slug(s: &str) -> String {
    let mut out = String::new();
    for c in s.chars().flat_map(char::to_lowercase) {
        if c.is_ascii_alphanumeric() {
            out.push(c);
        } else if !out.is_empty() && !out.ends_with('-') {
            out.push('-');
        }
    }
    while out.ends_with('-') {
        out.pop();
    }
    out
}

#[derive(Debug, Clone, PartialEq)]
enum Token {
    Lit(Vec<char>),
    Color,
    Orientation,
    Any,
}

/// A parsed pattern (`iPhone 1{*} - {color} - {orientation}`).
#[derive(Debug, Clone)]
pub struct Pattern(Vec<Token>);

#[derive(Debug, Default, Clone)]
struct Captures {
    color: Option<String>,
    orientation: Option<Orientation>,
}

impl Pattern {
    pub fn parse(p: &str) -> Result<Pattern, String> {
        let mut tokens = Vec::new();
        let mut lit = String::new();
        let mut rest = normalize_name(p);
        // `normalize_name` would strip a `.png`; patterns never have one.
        if rest.is_empty() {
            return Err("empty pattern".into());
        }
        while let Some(i) = rest.find('{') {
            lit.push_str(&rest[..i]);
            let end = rest[i..]
                .find('}')
                .ok_or_else(|| format!("pattern `{p}`: unclosed `{{`"))?;
            let tok = match &rest[i + 1..i + end] {
                "color" => Token::Color,
                "orientation" => Token::Orientation,
                "*" => Token::Any,
                other => return Err(format!("pattern `{p}`: unknown placeholder `{{{other}}}`")),
            };
            if !lit.is_empty() {
                tokens.push(Token::Lit(std::mem::take(&mut lit).chars().collect()));
            }
            tokens.push(tok);
            rest = rest[i + end + 1..].to_owned();
        }
        lit.push_str(&rest);
        if !lit.is_empty() {
            tokens.push(Token::Lit(lit.chars().collect()));
        }
        if tokens.iter().filter(|t| **t == Token::Color).count() != 1 {
            return Err(format!("pattern `{p}`: needs exactly one {{color}}"));
        }
        Ok(Pattern(tokens))
    }

    fn matches(&self, name: &str) -> Option<Captures> {
        let chars: Vec<char> = name.chars().collect();
        let mut caps = Captures::default();
        step(&self.0, &chars, &mut caps).then_some(caps)
    }
}

fn eq_ci(a: char, b: char) -> bool {
    a == b || a.to_lowercase().eq(b.to_lowercase())
}

fn starts_with_ci(s: &[char], lit: &[char]) -> bool {
    s.len() >= lit.len() && s.iter().zip(lit).all(|(a, b)| eq_ci(*a, *b))
}

fn contains_separator(s: &[char]) -> bool {
    s.windows(3).any(|w| w == [' ', '-', ' '])
}

fn step(tokens: &[Token], s: &[char], caps: &mut Captures) -> bool {
    let Some((tok, rest)) = tokens.split_first() else {
        return s.is_empty();
    };
    match tok {
        Token::Lit(lit) => starts_with_ci(s, lit) && step(rest, &s[lit.len()..], caps),
        Token::Any => (0..=s.len()).rev().any(|n| step(rest, &s[n..], caps)),
        Token::Color => (1..=s.len()).any(|n| {
            let c = &s[..n];
            if contains_separator(c) || c[0] == ' ' || c[n - 1] == ' ' {
                return false;
            }
            let ok = step(rest, &s[n..], caps);
            if ok {
                caps.color = Some(c.iter().collect());
            }
            ok
        }),
        Token::Orientation => [
            ("portrait", Orientation::Portrait),
            ("landscape", Orientation::Landscape),
        ]
        .into_iter()
        .any(|(word, o)| {
            let w: Vec<char> = word.chars().collect();
            let ok = starts_with_ci(s, &w) && step(rest, &s[w.len()..], caps);
            if ok {
                caps.orientation = Some(o);
            }
            ok
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn apple() -> &'static Rules {
        Rules::builtin("apple").unwrap()
    }

    fn m(name: &str) -> Option<(String, String, Orientation)> {
        apple()
            .match_file(name)
            .map(|f| (f.model, f.color, f.orientation))
    }

    #[test]
    fn apple_rules_name_catalog_models() {
        for r in &apple().rules {
            assert!(
                crate::catalog::device(&r.model).is_some(),
                "unknown model {}",
                r.model
            );
        }
    }

    #[test]
    fn iphone_names() {
        let island = |c: &str, o| Some(("iphone-island".into(), c.into(), o));
        assert_eq!(
            m("iPhone 16 Pro - Black Titanium - Portrait.png"),
            island("black-titanium", Orientation::Portrait)
        );
        assert_eq!(
            m("Bezel/iPhone 17 Pro Max – Cosmic Orange – Landscape.PNG"),
            island("cosmic-orange", Orientation::Landscape)
        );
        assert_eq!(
            m("iPhone 14 Pro - Space Black - Portrait.png"),
            island("space-black", Orientation::Portrait)
        );
        assert_eq!(
            m("iPhone 13 Pro Max - Graphite - Portrait.png").unwrap().0,
            "iphone-notch"
        );
        assert_eq!(
            m("iPhone 14 Plus - Midnight - Portrait.png").unwrap().0,
            "iphone-notch"
        );
        assert_eq!(
            m("iPhone 16e - White - Portrait.png").unwrap().0,
            "iphone-notch"
        );
        assert_eq!(
            m("iPhone SE - (PRODUCT)RED - Portrait.png").unwrap().1,
            "product-red"
        );
        assert_eq!(
            m("iPhone Air - Sky Blue - Portrait.png").unwrap().0,
            "iphone-island"
        );
        assert_eq!(m("iPhone 16 Pro - Black Titanium.png"), None);
        assert_eq!(m("Readme.png"), None);
    }

    #[test]
    fn ipad_and_mac_names() {
        assert_eq!(
            m("iPad Pro 13 - M4 - Space Black - Landscape.png"),
            Some((
                "ipad-modern".into(),
                "space-black".into(),
                Orientation::Landscape
            ))
        );
        assert_eq!(
            m("iPad (9th generation) - Silver - Portrait.png")
                .unwrap()
                .0,
            "ipad-home"
        );
        assert_eq!(
            m("iPad (10th generation) - Blue - Portrait.png").unwrap().0,
            "ipad-modern"
        );
        assert_eq!(
            m("MacBook Pro 14 - M4 - Space Black.png"),
            Some((
                "macbook".into(),
                "space-black".into(),
                Orientation::Landscape
            ))
        );
        assert_eq!(
            m("MacBook Air 13\u{201D} - M3 - Midnight.png").unwrap().1,
            "midnight"
        );
        assert_eq!(m("iMac 24\" - Blue.png").unwrap().0, "imac");
    }

    #[test]
    fn vendor_models() {
        let v = |n: &str| apple().match_file(n).unwrap().apple_model;
        assert_eq!(v("iPhone 18 Pro - Black - Portrait.png"), "iPhone 18 Pro");
        assert_eq!(
            v("iPhone 18 Pro Max \u{2013} Glacier \u{2013} Landscape.png"),
            "iPhone 18 Pro Max"
        );
        assert_eq!(
            v("MacBook Pro 14 - M4 - Space Black.png"),
            "MacBook Pro 14 - M4"
        );
        assert_eq!(vendor_model("Foo Bar - Portrait", "x"), "Foo Bar");
        assert_eq!(vendor_model("Foo", "Foo"), "Foo");
    }

    #[test]
    fn iphone_18_names() {
        for c in ["Black", "Burgundy", "Glacier", "Silver"] {
            for (o, want) in [
                ("Portrait", Orientation::Portrait),
                ("Landscape", Orientation::Landscape),
            ] {
                for model in ["iPhone 18 Pro", "iPhone 18 Pro Max"] {
                    let f = apple()
                        .match_file(&format!("{model} - {c} - {o}.png"))
                        .unwrap();
                    assert_eq!(f.model, "iphone-island");
                    assert_eq!(f.color, slug(c));
                    assert_eq!(f.orientation, want);
                    assert_eq!(f.apple_model, model);
                }
            }
        }
    }

    #[test]
    fn helpers() {
        assert_eq!(slug("Black Titanium"), "black-titanium");
        assert_eq!(slug(" (PRODUCT)RED "), "product-red");
        assert_eq!(normalize_name("a/b/X  –  Y.png"), "X - Y");
        assert!(Pattern::parse("{color} {nope}").is_err());
        assert!(Pattern::parse("no colour").is_err());
        assert!(
            Rules::parse(
                "source: x\nlabel: x\nrules:\n  - {pattern: \"A - {color}\", model: imac}\n"
            )
            .is_err()
        );
    }
}
