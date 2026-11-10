//! Arrangia inline markup as a tree ([`Node`]) and back, for exchange
//! formats that represent bold/italic/spans as inline elements (XLIFF
//! `<pc>`/`<g>`) and line breaks/soft hyphens as placeholders.
//!
//! Text nodes produced by [`parse_tree`] hold *plain* text (escapes
//! resolved). [`escape_tree`] turns them into source form again (minimal
//! escaping, verified against [`crate::markup::parse`]) so that
//! concatenating text and markers ([`concat`]) gives an equivalent markup
//! string. XLIFF writes the escaped text nodes, so an import only needs to
//! concatenate.

use crate::markup::{self, MarkupToken};

/// One node of a markup tree.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Node {
    Text(String),
    Bold(Vec<Node>),
    Italic(Vec<Node>),
    /// `[style]…[/]`.
    Span(String, Vec<Node>),
    /// `\n` (hard line break).
    LineBreak,
    /// `\-` / U+00AD.
    SoftHyphen,
}

/// What a pair of markers means.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum PairKind {
    Bold,
    Italic,
    Span(String),
}

impl PairKind {
    pub(crate) fn open_marker(&self) -> String {
        match self {
            Self::Bold => "**".to_owned(),
            Self::Italic => "_".to_owned(),
            Self::Span(s) => format!("[{s}]"),
        }
    }

    pub(crate) fn close_marker(&self) -> &'static str {
        match self {
            Self::Bold => "**",
            Self::Italic => "_",
            Self::Span(_) => "[/]",
        }
    }

    /// Kind of an opening marker as written (`**`, `_`, `[name]`).
    pub(crate) fn from_marker(m: &str) -> Option<Self> {
        match m {
            "**" => Some(Self::Bold),
            "_" => Some(Self::Italic),
            _ => m
                .strip_prefix('[')
                .and_then(|r| r.strip_suffix(']'))
                .filter(|n| {
                    !n.is_empty()
                        && n.chars()
                            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
                })
                .map(|n| Self::Span(n.to_owned())),
        }
    }

    fn node(self, children: Vec<Node>) -> Node {
        match self {
            Self::Bold => Node::Bold(children),
            Self::Italic => Node::Italic(children),
            Self::Span(s) => Node::Span(s, children),
        }
    }

    fn same(&self, other: &Self) -> bool {
        matches!(
            (self, other),
            (Self::Bold, Self::Bold)
                | (Self::Italic, Self::Italic)
                | (Self::Span(_), Self::Span(_))
        )
    }
}

/// A flat token for [`TreeBuilder`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum Tok {
    Text(String),
    Open(PairKind),
    /// Closes the innermost open pair of the same kind (any span for spans).
    Close(PairKind),
    LineBreak,
    SoftHyphen,
}

/// Builds a [`Node`] tree from open/close tokens. Overlapping pairs are
/// split (`**a _b** c_` → `**a _b_**_ c_`); unmatched closers and unclosed
/// openers are reported via [`TreeBuilder::problems`].
#[derive(Debug, Default)]
pub(crate) struct TreeBuilder {
    root: Vec<Node>,
    stack: Vec<(PairKind, Vec<Node>)>,
    pub(crate) problems: Vec<String>,
}

impl TreeBuilder {
    fn current(&mut self) -> &mut Vec<Node> {
        match self.stack.last_mut() {
            Some((_, c)) => c,
            None => &mut self.root,
        }
    }

    fn push_node(&mut self, n: Node) {
        let cur = self.current();
        if let (Node::Text(s), Some(Node::Text(last))) = (&n, cur.last_mut()) {
            last.push_str(s);
            return;
        }
        if matches!(&n, Node::Text(s) if s.is_empty()) {
            return;
        }
        cur.push(n);
    }

    pub(crate) fn push(&mut self, t: Tok) {
        match t {
            Tok::Text(s) => self.push_node(Node::Text(s)),
            Tok::LineBreak => self.push_node(Node::LineBreak),
            Tok::SoftHyphen => self.push_node(Node::SoftHyphen),
            Tok::Open(k) => self.stack.push((k, Vec::new())),
            Tok::Close(k) => {
                let Some(pos) = self.stack.iter().rposition(|(o, _)| o.same(&k)) else {
                    self.problems
                        .push(format!("closing `{}` without opener", k.close_marker()));
                    return;
                };
                let mut reopen = Vec::new();
                while self.stack.len() > pos + 1 {
                    let (kind, children) = self.stack.pop().unwrap_or((PairKind::Bold, vec![]));
                    reopen.push(kind.clone());
                    self.close_frame(kind, children);
                }
                if let Some((kind, children)) = self.stack.pop() {
                    self.close_frame(kind, children);
                }
                for kind in reopen.into_iter().rev() {
                    self.stack.push((kind, Vec::new()));
                }
            }
        }
    }

    fn close_frame(&mut self, kind: PairKind, children: Vec<Node>) {
        if children.is_empty() {
            return;
        }
        self.current().push(kind.node(children));
    }

    pub(crate) fn finish(mut self) -> (Vec<Node>, Vec<String>) {
        let mut unclosed = Vec::new();
        while let Some((kind, children)) = self.stack.pop() {
            unclosed.push(kind.open_marker());
            self.close_frame(kind, children);
        }
        for m in unclosed.into_iter().rev() {
            self.problems.push(format!("`{m}` is never closed"));
        }
        (self.root, self.problems)
    }
}

/// Splits plain text into text, line break and soft hyphen tokens.
pub(crate) fn text_tokens(s: &str, out: &mut impl FnMut(Tok)) {
    let mut buf = String::new();
    for c in s.chars() {
        match c {
            '\n' | markup::SOFT_HYPHEN => {
                if !buf.is_empty() {
                    out(Tok::Text(std::mem::take(&mut buf)));
                }
                out(if c == '\n' {
                    Tok::LineBreak
                } else {
                    Tok::SoftHyphen
                });
            }
            _ => buf.push(c),
        }
    }
    if !buf.is_empty() {
        out(Tok::Text(buf));
    }
}

/// Markup source → tree. Unbalanced markers are kept as literal text (as
/// [`crate::markup::parse`] renders them).
pub fn parse_tree(src: &str) -> Vec<Node> {
    let mut b = TreeBuilder::default();
    let (mut bold, mut italic) = (false, false);
    for t in markup::matched_tokens(src) {
        match t {
            MarkupToken::Text(s) => text_tokens(&s, &mut |t| b.push(t)),
            MarkupToken::Bold => {
                b.push(if bold {
                    Tok::Close(PairKind::Bold)
                } else {
                    Tok::Open(PairKind::Bold)
                });
                bold = !bold;
            }
            MarkupToken::Italic => {
                b.push(if italic {
                    Tok::Close(PairKind::Italic)
                } else {
                    Tok::Open(PairKind::Italic)
                });
                italic = !italic;
            }
            MarkupToken::Open(n) => b.push(Tok::Open(PairKind::Span(n))),
            MarkupToken::Close => b.push(Tok::Close(PairKind::Span(String::new()))),
        }
    }
    b.finish().0
}

/// Concatenates a tree whose text nodes are already in source form.
pub(crate) fn concat(nodes: &[Node]) -> String {
    let mut out = String::new();
    concat_into(nodes, &mut out);
    out
}

fn concat_into(nodes: &[Node], out: &mut String) {
    for n in nodes {
        match n {
            Node::Text(s) => out.push_str(s),
            Node::Bold(c) => {
                out.push_str("**");
                concat_into(c, out);
                out.push_str("**");
            }
            Node::Italic(c) => {
                out.push('_');
                concat_into(c, out);
                out.push('_');
            }
            Node::Span(s, c) => {
                out.push('[');
                out.push_str(s);
                out.push(']');
                concat_into(c, out);
                out.push_str("[/]");
            }
            Node::LineBreak => out.push('\n'),
            Node::SoftHyphen => out.push_str("\\-"),
        }
    }
}

fn map_text(nodes: &[Node], f: &impl Fn(&str) -> String) -> Vec<Node> {
    nodes
        .iter()
        .map(|n| match n {
            Node::Text(s) => Node::Text(f(s)),
            Node::Bold(c) => Node::Bold(map_text(c, f)),
            Node::Italic(c) => Node::Italic(map_text(c, f)),
            Node::Span(s, c) => Node::Span(s.clone(), map_text(c, f)),
            Node::LineBreak => Node::LineBreak,
            Node::SoftHyphen => Node::SoftHyphen,
        })
        .collect()
}

const SPECIAL: [char; 5] = ['\\', '*', '_', '[', ']'];

fn escape_all(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        if SPECIAL.contains(&c) {
            out.push('\\');
        }
        out.push(c);
    }
    out
}

/// Escapes only what would obviously be read as markup.
fn escape_minimal(s: &str) -> String {
    let chars: Vec<char> = s.chars().collect();
    let mut out = String::with_capacity(s.len());
    for (i, &c) in chars.iter().enumerate() {
        let prev = i.checked_sub(1).map(|j| chars[j]);
        let next = chars.get(i + 1).copied();
        let escape = match c {
            '\\' => next.is_none_or(|n| matches!(n, 'n' | '-' | '*' | '_' | '[' | ']' | '\\')),
            '*' => prev.is_none() || next.is_none() || prev == Some('*') || next == Some('*'),
            '[' => {
                let rest: String = chars[i + 1..].iter().collect();
                rest.starts_with("/]")
                    || rest.split_once(']').is_some_and(|(n, _)| {
                        !n.is_empty()
                            && n.chars()
                                .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
                    })
            }
            _ => false,
        };
        if escape {
            out.push('\\');
        }
        out.push(c);
    }
    out
}

/// Text and spans of a parse (diagnostics ignored).
fn semantic(src: &str) -> (String, Vec<markup::Span>) {
    let m = markup::parse(src);
    (m.text, m.spans)
}

/// The tree with text nodes in source form: minimal escaping when that
/// parses to the same result as escaping everything, else everything
/// escaped.
pub(crate) fn escape_tree(nodes: &[Node]) -> Vec<Node> {
    let minimal = map_text(nodes, &escape_minimal);
    let full = map_text(nodes, &escape_all);
    let m = markup::parse(&concat(&minimal));
    if m.diagnostics.is_empty() && (m.text, m.spans) == semantic(&concat(&full)) {
        minimal
    } else {
        full
    }
}

/// Tree → markup source (canonical form).
pub fn to_markup(nodes: &[Node]) -> String {
    concat(&escape_tree(nodes))
}

/// Do two markup strings render the same (text, styles, breaks)?
pub fn same_markup(a: &str, b: &str) -> bool {
    a == b || {
        let (ma, mb) = (markup::parse(a), markup::parse(b));
        ma.text == mb.text && ma.spans == mb.spans
    }
}

/// Bold/italic counts and span names of a tree (for comparing a
/// translation's markup with its source).
pub(crate) fn markup_signature(nodes: &[Node]) -> (usize, usize, Vec<String>) {
    fn walk(nodes: &[Node], sig: &mut (usize, usize, Vec<String>)) {
        for n in nodes {
            match n {
                Node::Bold(c) => {
                    sig.0 += 1;
                    walk(c, sig);
                }
                Node::Italic(c) => {
                    sig.1 += 1;
                    walk(c, sig);
                }
                Node::Span(s, c) => {
                    sig.2.push(s.clone());
                    walk(c, sig);
                }
                _ => {}
            }
        }
    }
    let mut sig = (0, 0, Vec::new());
    walk(nodes, &mut sig);
    sig.2.sort();
    sig.2.dedup();
    sig.0 = sig.0.min(1);
    sig.1 = sig.1.min(1);
    sig
}

#[cfg(test)]
mod tests {
    use super::*;

    fn t(s: &str) -> Node {
        Node::Text(s.to_owned())
    }

    #[test]
    fn tree_of_nested_markup() {
        let tree = parse_tree("Mit [accent]Misch\\-kultur **zu [hl]mehr[/]**[/]\\nErnte");
        assert_eq!(
            tree,
            [
                t("Mit "),
                Node::Span(
                    "accent".into(),
                    vec![
                        t("Misch"),
                        Node::SoftHyphen,
                        t("kultur "),
                        Node::Bold(vec![t("zu "), Node::Span("hl".into(), vec![t("mehr")])]),
                    ]
                ),
                Node::LineBreak,
                t("Ernte"),
            ]
        );
    }

    #[test]
    fn round_trip_is_semantically_lossless() {
        for src in [
            "Plain",
            "Plan every bed **by drag & drop**",
            "Know your [accent]plant partners[/]",
            "**Fennel**\nbetter keep apart",
            "Nachbarschafts\\-wert",
            "[a]x **y [b]z[/]**[/] w",
            "snake_case and file_name.txt",
            r"\*\*not bold\*\* \_x\_ \[accent\] a\\b \q",
            "**open bold",
            "stray [/] close",
            "a*b * c ** d",
            "path\\",
            "x [1] y",
        ] {
            let back = to_markup(&parse_tree(src));
            assert!(same_markup(src, &back), "{src:?} → {back:?}");
        }
    }

    /// Known limit: overlapping bold/italic is split into nested pairs; an
    /// italic part starting with a space cannot be written as `_ …_`.
    #[test]
    fn overlap_with_leading_space_is_not_lossless() {
        let back = to_markup(&parse_tree("**a _b** c_"));
        assert_eq!(back, "**a _b_**_ c_");
    }

    #[test]
    fn minimal_escaping_keeps_text_readable() {
        assert_eq!(to_markup(&parse_tree("snake_case a*b")), "snake_case a*b");
        assert_eq!(
            to_markup(&parse_tree(r"\*\*x\*\*")),
            r"\*\*x\*\*",
            "literal double stars stay escaped"
        );
        assert_eq!(to_markup(&parse_tree("a\u{AD}b")), "a\\-b");
    }

    #[test]
    fn overlapping_pairs_are_split() {
        let tree = parse_tree("**a [s]b** c[/]");
        assert_eq!(
            tree,
            [
                Node::Bold(vec![t("a "), Node::Span("s".into(), vec![t("b")])]),
                Node::Span("s".into(), vec![t(" c")]),
            ]
        );
    }

    #[test]
    fn builder_reports_problems() {
        let mut b = TreeBuilder::default();
        b.push(Tok::Close(PairKind::Bold));
        b.push(Tok::Open(PairKind::Span("x".into())));
        b.push(Tok::Text("y".into()));
        let (tree, problems) = b.finish();
        assert_eq!(tree, [Node::Span("x".into(), vec![t("y")])]);
        assert_eq!(problems.len(), 2);
    }
}
