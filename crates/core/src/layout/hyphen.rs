//! Automatic hyphenation (ADR 0009): Liang's algorithm over the TeX
//! patterns of the hyph-utf8 project, embedded for the languages whose
//! licence allows redistribution (`assets/hyphenation/LICENSES.md`).
//!
//! Layout uses it only as a last step of auto-fit: when a text with
//! `hyphenate: true` does not fit at `fit.shrink_to`, its words get soft
//! hyphens at the pattern positions and it is laid out again. Words that
//! already contain a soft hyphen (`\-`) are left alone, so manual breaks
//! keep priority.

use std::collections::HashMap;
use std::sync::OnceLock;

use crate::markup::SOFT_HYPHEN;

/// Words shorter than this (in characters) are never hyphenated.
pub const MIN_WORD_CHARS: usize = 6;
/// Minimum characters before the first hyphen of a word.
pub const MIN_LEFT: usize = 2;
/// Minimum characters after the last hyphen of a word.
pub const MIN_RIGHT: usize = 3;

macro_rules! patterns {
    ($($id:literal => $pat:literal $(, $hyp:literal)?;)*) => {
        /// Bundled pattern sets: id, patterns, exceptions.
        const SETS: &[(&str, &str, &str)] = &[$(
            (
                $id,
                include_str!(concat!("../../../../assets/hyphenation/", $pat)),
                patterns!(@hyp $($hyp)?),
            ),
        )*];
    };
    (@hyp) => { "" };
    (@hyp $hyp:literal) => { include_str!(concat!("../../../../assets/hyphenation/", $hyp)) };
}

patterns! {
    "da" => "hyph-da.pat.txt";
    "de-1996" => "hyph-de-1996.pat.txt";
    "en-gb" => "hyph-en-gb.pat.txt", "hyph-en-gb.hyp.txt";
    "en-us" => "hyph-en-us.pat.txt", "hyph-en-us.hyp.txt";
    "es" => "hyph-es.pat.txt";
    "fi" => "hyph-fi.pat.txt";
    "fr" => "hyph-fr.pat.txt";
    "it" => "hyph-it.pat.txt";
    "nb" => "hyph-nb.pat.txt", "hyph-nb.hyp.txt";
    "nl" => "hyph-nl.pat.txt", "hyph-nl.hyp.txt";
    "pt" => "hyph-pt.pat.txt", "hyph-pt.hyp.txt";
}

/// Ids of the bundled pattern sets (`de-1996`, `en-us` …).
pub fn pattern_sets() -> impl Iterator<Item = &'static str> {
    SETS.iter().map(|s| s.0)
}

/// The pattern set used for a project language (BCP 47 tag), if any:
/// `en` → `en-us` (`en-GB`, `-AU`, `-NZ`, `-IE`, `-ZA`, `-IN` → `en-gb`),
/// `de-*` → `de-1996`, `no`/`nb` → `nb`, `pt-*` → `pt`, otherwise the
/// primary subtag.
pub fn pattern_set_for(lang: &str) -> Option<&'static str> {
    let lower = lang.to_ascii_lowercase();
    let mut parts = lower.split(['-', '_']);
    let primary = parts.next()?;
    let region = parts.find(|p| p.len() == 2);
    let id = match primary {
        "en" => match region {
            Some("gb" | "au" | "nz" | "ie" | "za" | "in") => "en-gb",
            _ => "en-us",
        },
        "de" => "de-1996",
        "no" | "nb" => "nb",
        other => other,
    };
    SETS.iter().map(|s| s.0).find(|s| *s == id)
}

/// A compiled pattern set.
#[derive(Debug)]
pub struct Hyphenator {
    /// Letters of a pattern → its inter-letter values (`letters + 1`).
    patterns: HashMap<Box<str>, Box<[u8]>>,
    /// Longest pattern, in characters.
    max_len: usize,
    /// Lower-case word → break positions (character indices).
    exceptions: HashMap<Box<str>, Vec<usize>>,
}

impl Hyphenator {
    /// Compiles patterns in the hyph-utf8 `.pat.txt` format (whitespace
    /// separated, digits between letters) and exceptions in the
    /// `.hyp.txt` format (`hy-phen-ation`).
    pub fn new(patterns: &str, exceptions: &str) -> Self {
        let mut map = HashMap::new();
        let mut max_len = 0;
        for pat in patterns.split_whitespace() {
            let mut letters = String::new();
            let mut values = vec![0u8];
            for c in pat.chars() {
                if let Some(d) = c.to_digit(10) {
                    if let Some(last) = values.last_mut() {
                        *last = d as u8;
                    }
                } else {
                    letters.push(c);
                    values.push(0);
                }
            }
            max_len = max_len.max(letters.chars().count());
            map.insert(letters.into_boxed_str(), values.into_boxed_slice());
        }
        let mut exc = HashMap::new();
        for word in exceptions.split_whitespace() {
            let mut plain = String::new();
            let mut breaks = Vec::new();
            for c in word.chars() {
                if c == '-' {
                    breaks.push(plain.chars().count());
                } else {
                    plain.push(c);
                }
            }
            exc.insert(plain.to_lowercase().into_boxed_str(), breaks);
        }
        Self {
            patterns: map,
            max_len,
            exceptions: exc,
        }
    }

    /// Break positions of `word` as **character** indices (a break at `i`
    /// goes between characters `i - 1` and `i`), honouring
    /// [`MIN_WORD_CHARS`], [`MIN_LEFT`] and [`MIN_RIGHT`].
    pub fn breaks(&self, word: &str) -> Vec<usize> {
        let chars: Vec<char> = word.chars().collect();
        let n = chars.len();
        if n < MIN_WORD_CHARS || n < MIN_LEFT + MIN_RIGHT {
            return Vec::new();
        }
        // Lower case per character; skip words whose lower case changes
        // the character count (positions would not map back).
        let mut lower = Vec::with_capacity(n);
        for c in &chars {
            let mut l = c.to_lowercase();
            match (l.next(), l.next()) {
                (Some(x), None) => lower.push(x),
                _ => return Vec::new(),
            }
        }
        let keep = |i: &usize| (MIN_LEFT..=n - MIN_RIGHT).contains(i);
        let key: String = lower.iter().collect();
        if let Some(b) = self.exceptions.get(key.as_str()) {
            return b.iter().copied().filter(keep).collect();
        }
        let mut dotted = Vec::with_capacity(n + 2);
        dotted.push('.');
        dotted.extend(&lower);
        dotted.push('.');
        let mut points = vec![0u8; dotted.len() + 1];
        let mut buf = String::new();
        for i in 0..dotted.len() {
            buf.clear();
            for &c in &dotted[i..dotted.len().min(i + self.max_len)] {
                buf.push(c);
                if let Some(values) = self.patterns.get(buf.as_str()) {
                    for (k, v) in values.iter().enumerate() {
                        let p = &mut points[i + k];
                        *p = (*p).max(*v);
                    }
                }
            }
        }
        // points[k] is the value before dotted[k]; a break before
        // word char i sits before dotted[i + 1].
        (1..n)
            .filter(|i| points[i + 1] % 2 == 1)
            .filter(keep)
            .collect()
    }

    /// `word` with soft hyphens at its break positions.
    pub fn hyphenate_word(&self, word: &str) -> String {
        let breaks = self.breaks(word);
        let mut out = String::with_capacity(word.len() + 2 * breaks.len());
        for (i, c) in word.chars().enumerate() {
            if breaks.contains(&i) {
                out.push(SOFT_HYPHEN);
            }
            out.push(c);
        }
        out
    }
}

/// The compiled hyphenator for a project language (see
/// [`pattern_set_for`]); compiled once on first use.
pub fn hyphenator(lang: &str) -> Option<&'static Hyphenator> {
    static CACHE: [OnceLock<Hyphenator>; 11] = [const { OnceLock::new() }; 11];
    let id = pattern_set_for(lang)?;
    let i = SETS.iter().position(|s| s.0 == id)?;
    let (_, pat, hyp) = SETS[i];
    Some(CACHE.get(i)?.get_or_init(|| Hyphenator::new(pat, hyp)))
}

/// A soft hyphen inserted by [`hyphenate_text`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Inserted {
    /// Byte offset of the inserted soft hyphen in the new text.
    pub at: usize,
    /// The word it belongs to (without soft hyphens).
    pub word: String,
}

/// `text` with soft hyphens inserted into every word (maximal run of
/// alphabetic characters) of at least [`MIN_WORD_CHARS`] characters that
/// does not already contain a soft hyphen. Returns the new text, the
/// inserted positions and a byte-offset map (old offset → new offset,
/// `text.len() + 1` entries).
pub fn hyphenate_text(h: &Hyphenator, text: &str) -> (String, Vec<Inserted>, Vec<usize>) {
    let mut out = String::with_capacity(text.len() + text.len() / 4);
    let mut inserted = Vec::new();
    let mut map = vec![0; text.len() + 1];
    let is_word = |c: char| c.is_alphabetic() || c == SOFT_HYPHEN;
    let mut rest = text.char_indices().peekable();
    while let Some(&(start, c)) = rest.peek() {
        if !is_word(c) {
            map[start] = out.len();
            out.push(c);
            rest.next();
            continue;
        }
        let mut end = start;
        while let Some(&(i, c)) = rest.peek() {
            if !is_word(c) {
                break;
            }
            end = i + c.len_utf8();
            rest.next();
        }
        let word = &text[start..end];
        let breaks = if word.contains(SOFT_HYPHEN) {
            Vec::new()
        } else {
            h.breaks(word)
        };
        for (ci, (bi, c)) in word.char_indices().enumerate() {
            if breaks.contains(&ci) {
                inserted.push(Inserted {
                    at: out.len(),
                    word: word.to_owned(),
                });
                out.push(SOFT_HYPHEN);
            }
            map[start + bi] = out.len();
            out.push(c);
        }
    }
    map[text.len()] = out.len();
    // Offsets inside multi-byte characters map like their start.
    for i in 1..text.len() {
        if !text.is_char_boundary(i) {
            map[i] = map[i - 1] + 1;
        }
    }
    (out, inserted, map)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn shy(lang: &str, word: &str) -> String {
        hyphenator(lang)
            .unwrap()
            .hyphenate_word(word)
            .replace(SOFT_HYPHEN, "-")
    }

    #[test]
    fn language_mapping() {
        assert_eq!(pattern_set_for("de"), Some("de-1996"));
        assert_eq!(pattern_set_for("de-AT"), Some("de-1996"));
        assert_eq!(pattern_set_for("en"), Some("en-us"));
        assert_eq!(pattern_set_for("en-GB"), Some("en-gb"));
        assert_eq!(pattern_set_for("pt-BR"), Some("pt"));
        assert_eq!(pattern_set_for("no"), Some("nb"));
        assert_eq!(pattern_set_for("sv"), None, "LPPL-only, not bundled");
        assert_eq!(pattern_set_for("ja"), None);
        assert_eq!(pattern_sets().count(), 11);
    }

    #[test]
    fn every_bundled_set_compiles() {
        for id in pattern_sets() {
            let h = hyphenator(id).unwrap();
            assert!(h.max_len > 3, "{id}");
        }
    }

    #[test]
    fn known_words() {
        assert_eq!(shy("en", "hyphenation"), "hy-phen-ation");
        assert_eq!(shy("en", "university"), "uni-ver-sity", "exception list");
        assert_eq!(
            shy("de", "Gartenplanungswerkzeug"),
            "Gar-ten-pla-nungs-werk-zeug"
        );
        assert_eq!(shy("de", "MISCHKULTUR"), "MISCH-KUL-TUR");
        assert_eq!(shy("fr", "jardinage"), "jar-di-nage");
        // Too short / fragment limits.
        assert_eq!(shy("de", "Beete"), "Beete");
        assert!(!shy("en", "garden").starts_with("g-"));
    }

    #[test]
    fn text_insertion_keeps_manual_hyphens_and_maps_offsets() {
        let h = hyphenator("de").unwrap();
        let text = "Ernte\u{AD}kalender für Gartenplanung";
        let (out, ins, map) = hyphenate_text(h, text);
        assert!(out.starts_with("Ernte\u{AD}kalender für "), "{out}");
        assert!(ins.iter().all(|i| i.word == "Gartenplanung"));
        assert_ne!(ins.len(), 0);
        for i in &ins {
            assert_eq!(&out[i.at..i.at + 2], "\u{AD}");
        }
        assert_eq!(map[text.len()], out.len());
        let g = text.find('G').unwrap();
        assert!(out[map[g]..].starts_with('G'));
    }
}
