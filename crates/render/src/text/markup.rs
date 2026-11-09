//! Minimal, translator-friendly inline markup (`docs/KONZEPT.md` §3.5):
//! `**bold**`, `[style]…[/]` (named span style) and `\n` (explicit line break).
//! Italic (`_…_`) is not part of the spike (no italic font bundled).

/// A styled piece of text.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Span {
    pub text: String,
    pub bold: bool,
    /// Named span style, e.g. `accent`.
    pub style: Option<String>,
}

/// Parses inline markup into spans. Unknown or unbalanced markers are kept
/// as literal text where possible; never fails.
pub fn parse_markup(src: &str) -> Vec<Span> {
    let mut spans: Vec<Span> = Vec::new();
    let mut bold = false;
    let mut style: Option<String> = None;
    let mut cur = String::new();
    let mut rest = src;

    let flush = |cur: &mut String, spans: &mut Vec<Span>, bold: bool, style: &Option<String>| {
        if !cur.is_empty() {
            spans.push(Span {
                text: std::mem::take(cur),
                bold,
                style: style.clone(),
            });
        }
    };

    while let Some(c) = rest.chars().next() {
        if let Some(r) = rest.strip_prefix("**") {
            flush(&mut cur, &mut spans, bold, &style);
            bold = !bold;
            rest = r;
        } else if let Some(r) = rest.strip_prefix("[/]") {
            flush(&mut cur, &mut spans, bold, &style);
            style = None;
            rest = r;
        } else if let Some(name) = rest
            .strip_prefix('[')
            .and_then(|r| r.split_once(']'))
            .map(|(n, _)| n)
            .filter(|n| {
                !n.is_empty()
                    && n.chars()
                        .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
            })
        {
            flush(&mut cur, &mut spans, bold, &style);
            style = Some(name.to_owned());
            rest = &rest[name.len() + 2..];
        } else if let Some(r) = rest.strip_prefix("\\n") {
            cur.push('\n');
            rest = r;
        } else {
            cur.push(c);
            rest = &rest[c.len_utf8()..];
        }
    }
    flush(&mut cur, &mut spans, bold, &style);
    spans
}

/// Plain text of spans (markup removed).
pub fn plain_text(spans: &[Span]) -> String {
    spans.iter().map(|s| s.text.as_str()).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bold_and_named_spans() {
        let s = parse_markup("Plane deine Beete **in Minuten**");
        assert_eq!(s.len(), 2);
        assert!(!s[0].bold && s[1].bold);
        assert_eq!(s[1].text, "in Minuten");

        let s = parse_markup("Mit [accent]Mischkultur[/] zu mehr Ernte");
        assert_eq!(s.len(), 3);
        assert_eq!(s[1].style.as_deref(), Some("accent"));
        assert_eq!(s[2].style, None);
        assert_eq!(plain_text(&s), "Mit Mischkultur zu mehr Ernte");
    }

    #[test]
    fn literal_brackets_and_newlines() {
        let s = parse_markup("a [not a style] b\\nc");
        assert_eq!(plain_text(&s), "a [not a style] b\nc");
    }

    #[test]
    fn nested_bold_inside_style() {
        let s = parse_markup("[accent]x **y**[/] z");
        assert_eq!(s.len(), 3);
        assert!(s[1].bold && s[1].style.as_deref() == Some("accent"));
    }
}
