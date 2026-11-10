//! Inline markup of translated texts (`docs/KONZEPT.md` §3.5,
//! `docs/adr/0003-text-markup-and-layout.md`).
//!
//! Deliberately minimal and translator friendly:
//!
//! | Source            | Meaning                                              |
//! |-------------------|------------------------------------------------------|
//! | `**bold**`        | bold (CSS `bolder` of the current weight)            |
//! | `_italic_`        | italic; `_` inside a word (`snake_case`) is literal  |
//! | `[name]…[/]`      | named span style from `styles.spans`; may nest       |
//! | `\n` or newline   | hard line break                                      |
//! | `\-` or U+00AD    | soft hyphen: optional break point, see layout        |
//! | `\*` `\_` `\[` `\]` `\\` | the literal character                         |
//!
//! The parser never fails: unbalanced markers are kept as literal text and
//! reported as [`MarkupDiagnostic`]s.

use std::fmt;
use std::ops::Range;

use serde::{Deserialize, Serialize};

/// The soft hyphen (U+00AD). `\-` in the source is turned into this.
pub const SOFT_HYPHEN: char = '\u{AD}';

/// A styled range of [`Markup::text`] (byte offsets, on char boundaries).
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Span {
    pub range: Range<usize>,
    pub bold: bool,
    pub italic: bool,
    /// Innermost named span style (`[accent]…[/]`).
    pub style: Option<String>,
}

/// An unbalanced marker that was rendered literally.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct MarkupDiagnostic {
    /// The marker as written: `**`, `_`, `[name]` or `[/]`.
    pub marker: String,
    /// Byte offset of the marker in the source.
    pub offset: usize,
}

impl fmt::Display for MarkupDiagnostic {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "unbalanced `{}` at byte {} (rendered literally)",
            self.marker, self.offset
        )
    }
}

/// Parsed markup: plain text plus styled spans covering it without gaps.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Markup {
    /// Text without markup; hard breaks are `\n`, soft hyphens U+00AD.
    pub text: String,
    /// Consecutive spans; adjacent spans always differ in style.
    pub spans: Vec<Span>,
    pub diagnostics: Vec<MarkupDiagnostic>,
}

impl Markup {
    /// Text of a span.
    pub fn span_text(&self, span: &Span) -> &str {
        &self.text[span.range.clone()]
    }

    /// Plain text with soft hyphens removed (e.g. for search or reports).
    pub fn visible_text(&self) -> String {
        self.text.chars().filter(|&c| c != SOFT_HYPHEN).collect()
    }
}

#[derive(Debug)]
enum Tok {
    Text(String),
    Bold,
    Italic,
    Open(String),
    Close,
}

#[derive(Debug)]
struct Token {
    tok: Tok,
    offset: usize,
    /// Source text used when the marker stays unmatched.
    literal: String,
    matched: bool,
}

fn is_span_name_char(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '-' || c == '_'
}

fn tokenize(src: &str) -> Vec<Token> {
    let mut out: Vec<Token> = Vec::new();
    let text = |out: &mut Vec<Token>, s: &str, offset: usize| {
        if let Some(Token {
            tok: Tok::Text(t), ..
        }) = out.last_mut()
        {
            t.push_str(s);
        } else {
            out.push(Token {
                tok: Tok::Text(s.to_owned()),
                offset,
                literal: String::new(),
                matched: true,
            });
        }
    };
    let marker = |out: &mut Vec<Token>, tok: Tok, offset: usize, literal: &str| {
        out.push(Token {
            tok,
            offset,
            literal: literal.to_owned(),
            matched: false,
        });
    };

    let mut i = 0;
    while let Some(c) = src[i..].chars().next() {
        let rest = &src[i..];
        let mut consumed = c.len_utf8();
        match c {
            '\\' => match rest[1..].chars().next() {
                Some('n') => {
                    text(&mut out, "\n", i);
                    consumed = 2;
                }
                Some('-') => {
                    text(&mut out, "\u{AD}", i);
                    consumed = 2;
                }
                Some(e @ ('*' | '_' | '[' | ']' | '\\')) => {
                    text(&mut out, e.encode_utf8(&mut [0; 4]), i);
                    consumed = 2;
                }
                _ => text(&mut out, "\\", i),
            },
            '*' if rest.starts_with("**") => {
                marker(&mut out, Tok::Bold, i, "**");
                consumed = 2;
            }
            '_' => {
                let (can_open, can_close) = italic_flanks(src, i);
                if can_open || can_close {
                    marker(&mut out, Tok::Italic, i, "_");
                } else {
                    text(&mut out, "_", i);
                }
            }
            '[' if rest.starts_with("[/]") => {
                marker(&mut out, Tok::Close, i, "[/]");
                consumed = 3;
            }
            '[' => {
                let name = rest[1..]
                    .split_once(']')
                    .map(|(n, _)| n)
                    .filter(|n| !n.is_empty() && n.chars().all(is_span_name_char));
                if let Some(name) = name {
                    consumed = name.len() + 2;
                    marker(&mut out, Tok::Open(name.to_owned()), i, &rest[..consumed]);
                } else {
                    text(&mut out, "[", i);
                }
            }
            '\r' => {
                text(&mut out, "\n", i);
                if rest[1..].starts_with('\n') {
                    consumed = 2;
                }
            }
            _ => text(&mut out, c.encode_utf8(&mut [0; 4]), i),
        }
        i += consumed;
    }
    out
}

/// Can an `_` at byte `offset` open / close an italic pair? Like `CommonMark`'s
/// flanking rules: an opener is not preceded by a letter/digit and not
/// followed by whitespace; a closer is the mirror image.
fn italic_flanks(src: &str, offset: usize) -> (bool, bool) {
    let prev = src[..offset].chars().next_back();
    let next = src[offset + 1..].chars().next();
    let can_open =
        next.is_some_and(|n| !n.is_whitespace()) && !prev.is_some_and(char::is_alphanumeric);
    let can_close =
        prev.is_some_and(|p| !p.is_whitespace()) && !next.is_some_and(char::is_alphanumeric);
    (can_open, can_close)
}

fn match_markers(src: &str, toks: &mut [Token]) {
    let mut bold: Option<usize> = None;
    let mut italic: Option<usize> = None;
    let mut spans: Vec<usize> = Vec::new();
    for i in 0..toks.len() {
        match toks[i].tok {
            Tok::Text(_) => {}
            Tok::Bold => {
                if let Some(o) = bold.take() {
                    toks[o].matched = true;
                    toks[i].matched = true;
                } else {
                    bold = Some(i);
                }
            }
            Tok::Italic => {
                let (can_open, can_close) = italic_flanks(src, toks[i].offset);
                if let (Some(o), true) = (italic, can_close) {
                    toks[o].matched = true;
                    toks[i].matched = true;
                    italic = None;
                } else if can_open {
                    // A previous unclosed opener stays literal.
                    italic = Some(i);
                }
            }
            Tok::Open(_) => spans.push(i),
            Tok::Close => {
                if let Some(o) = spans.pop() {
                    toks[o].matched = true;
                    toks[i].matched = true;
                }
            }
        }
    }
}

/// A token of inline markup after marker matching (for `core::exchange`):
/// unmatched markers are literal text; text keeps hard breaks as `\n` and
/// soft hyphens as U+00AD (escapes resolved).
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum MarkupToken {
    Text(String),
    /// `**` (toggles).
    Bold,
    /// `_` (toggles).
    Italic,
    Open(String),
    Close,
}

/// Tokens of `src` as [`parse`] sees them.
pub(crate) fn matched_tokens(src: &str) -> Vec<MarkupToken> {
    let mut toks = tokenize(src);
    match_markers(src, &mut toks);
    toks.into_iter()
        .map(|t| match t.tok {
            _ if !t.matched => MarkupToken::Text(t.literal),
            Tok::Text(s) => MarkupToken::Text(s),
            Tok::Bold => MarkupToken::Bold,
            Tok::Italic => MarkupToken::Italic,
            Tok::Open(name) => MarkupToken::Open(name),
            Tok::Close => MarkupToken::Close,
        })
        .collect()
}

/// Parses inline markup. Never fails and never panics; see module docs.
pub fn parse(src: &str) -> Markup {
    let mut toks = tokenize(src);
    match_markers(src, &mut toks);

    let mut m = Markup::default();
    let mut bold = false;
    let mut italic = false;
    let mut styles: Vec<String> = Vec::new();
    let push = |m: &mut Markup, s: &str, bold: bool, italic: bool, style: Option<&String>| {
        if s.is_empty() {
            return;
        }
        let start = m.text.len();
        m.text.push_str(s);
        let end = m.text.len();
        if let Some(last) = m.spans.last_mut()
            && last.bold == bold
            && last.italic == italic
            && last.style.as_ref() == style
        {
            last.range.end = end;
            return;
        }
        m.spans.push(Span {
            range: start..end,
            bold,
            italic,
            style: style.cloned(),
        });
    };
    for t in toks {
        if !t.matched {
            m.diagnostics.push(MarkupDiagnostic {
                marker: t.literal.clone(),
                offset: t.offset,
            });
            push(&mut m, &t.literal, bold, italic, styles.last());
            continue;
        }
        match t.tok {
            Tok::Text(s) => push(&mut m, &s, bold, italic, styles.last()),
            Tok::Bold => bold = !bold,
            Tok::Italic => italic = !italic,
            Tok::Open(name) => styles.push(name),
            Tok::Close => {
                styles.pop();
            }
        }
    }
    m
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    fn parts(m: &Markup) -> Vec<(String, bool, bool, Option<String>)> {
        m.spans
            .iter()
            .map(|s| (m.span_text(s).to_owned(), s.bold, s.italic, s.style.clone()))
            .collect()
    }

    #[test]
    fn plain_text() {
        let m = parse("Plane deine Beete");
        assert_eq!(m.text, "Plane deine Beete");
        assert_eq!(m.spans.len(), 1);
        assert_eq!(m.diagnostics, Vec::<MarkupDiagnostic>::new());
        assert_eq!(parse("").spans, Vec::<Span>::new());
    }

    #[test]
    fn bold_italic_and_spans() {
        let m = parse("Plane deine Beete **in Minuten**");
        assert_eq!(
            parts(&m),
            [
                ("Plane deine Beete ".into(), false, false, None),
                ("in Minuten".into(), true, false, None),
            ]
        );
        let m = parse("Mit [accent]Mischkultur[/] zu _mehr_ Ernte");
        assert_eq!(m.text, "Mit Mischkultur zu mehr Ernte");
        assert_eq!(m.spans[1].style.as_deref(), Some("accent"));
        assert!(m.spans[3].italic && m.span_text(&m.spans[3]) == "mehr");
        assert_eq!(m.diagnostics, Vec::<MarkupDiagnostic>::new());
    }

    #[test]
    fn nesting_and_overlap() {
        let m = parse("[accent]x **y [hl]z[/]**[/] w");
        assert_eq!(
            parts(&m),
            [
                ("x ".into(), false, false, Some("accent".into())),
                ("y ".into(), true, false, Some("accent".into())),
                ("z".into(), true, false, Some("hl".into())),
                (" w".into(), false, false, None),
            ]
        );
        // Bold and italic may overlap; each pair toggles independently.
        let m = parse("**a _b** c_");
        assert_eq!(m.text, "a b c");
        assert_eq!(m.diagnostics, Vec::<MarkupDiagnostic>::new());
        assert!(m.spans[1].bold && m.spans[1].italic);
        assert!(!m.spans[2].bold && m.spans[2].italic);
    }

    #[test]
    fn breaks_and_soft_hyphens() {
        let m = parse("Misch\\-kultur\\nzweite\u{AD}Zeile\r\ndritte");
        assert_eq!(m.text, "Misch\u{AD}kultur\nzweite\u{AD}Zeile\ndritte");
        assert_eq!(m.visible_text(), "Mischkultur\nzweiteZeile\ndritte");
    }

    #[test]
    fn escapes() {
        let m = parse(r"\*\*not bold\*\* \_x\_ \[accent\] a\\b \q");
        assert_eq!(m.text, r"**not bold** _x_ [accent] a\b \q");
        assert_eq!(m.spans.len(), 1);
        assert_eq!(m.diagnostics, Vec::<MarkupDiagnostic>::new());
    }

    #[test]
    fn intraword_underscore_is_literal() {
        let m = parse("snake_case and file_name.txt");
        assert_eq!(m.text, "snake_case and file_name.txt");
        assert_eq!(m.diagnostics, Vec::<MarkupDiagnostic>::new());
        assert!(m.spans.iter().all(|s| !s.italic));
    }

    #[test]
    fn brackets_without_style_name_are_literal() {
        let m = parse("a [not a style] b [] c [x");
        assert_eq!(m.text, "a [not a style] b [] c [x");
        assert_eq!(m.diagnostics, Vec::<MarkupDiagnostic>::new());
    }

    #[test]
    fn unbalanced_markers_are_literal_with_diagnostics() {
        let m = parse("**open bold");
        assert_eq!(m.text, "**open bold");
        assert_eq!(
            m.diagnostics,
            [MarkupDiagnostic {
                marker: "**".into(),
                offset: 0
            }]
        );
        let m = parse("a **b** **c");
        assert_eq!(m.text, "a b **c");
        assert_eq!(m.diagnostics[0].offset, 8);

        let m = parse("[accent]never closed");
        assert_eq!(m.text, "[accent]never closed");
        assert_eq!(m.diagnostics[0].marker, "[accent]");

        let m = parse("stray [/] close");
        assert_eq!(m.text, "stray [/] close");
        assert_eq!(m.diagnostics[0].marker, "[/]");

        let m = parse("_open italic");
        assert_eq!(m.text, "_open italic");
        assert_eq!(m.diagnostics[0].marker, "_");
        assert!(m.diagnostics[0].to_string().contains("unbalanced `_`"));
    }

    proptest! {
        #[test]
        fn never_panics_and_spans_cover_text(
            s in "(\\*\\*|_|\\[accent\\]|\\[/\\]|\\\\|\\\\-|\\\\n|\u{AD}|[a-zä ]|\\[|\\]|\r|\n|\\*){0,40}"
        ) {
            let m = parse(&s);
            let mut pos = 0;
            for span in &m.spans {
                prop_assert_eq!(span.range.start, pos);
                prop_assert!(span.range.end > span.range.start);
                prop_assert!(m.text.is_char_boundary(span.range.end));
                pos = span.range.end;
            }
            prop_assert_eq!(pos, m.text.len());
            for w in m.spans.windows(2) {
                prop_assert!(
                    (w[0].bold, w[0].italic, &w[0].style) != (w[1].bold, w[1].italic, &w[1].style)
                );
            }
        }

        #[test]
        fn never_panics_on_arbitrary_input(s in any::<String>()) {
            let _ = parse(&s);
        }
    }
}
