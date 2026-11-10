//! XLIFF 1.2 and 2.0: one file per target language, source = default
//! language. Written by hand (tidy, stable output), read with `quick-xml`
//! into a small element tree.
//!
//! Inline markup ([`MarkupMode::Inline`]):
//!
//! | Arrangia      | XLIFF 2.0                                              | XLIFF 1.2                         |
//! |---------------|--------------------------------------------------------|-----------------------------------|
//! | `**…**`       | `<pc type="fmt" subType="xlf:b" dataRefStart…>`        | `<g ctype="bold">`                |
//! | `_…_`         | `<pc type="fmt" subType="xlf:i" …>`                    | `<g ctype="italic">`              |
//! | `[name]…[/]`  | `<pc type="fmt" dataRefStart="[name]" dataRefEnd="[/]">` | `<g ctype="x-span-name">`       |
//! | `\n`          | `<ph type="fmt" subType="xlf:lb" dataRef…/>`           | `<x ctype="lb"/>`                 |
//! | `\-`          | `<ph dataRef="\-"/>`                                   | `<x ctype="x-soft-hyphen"/>`      |
//!
//! 2.0 keeps the original markers in `<originalData>`, so import maps
//! `dataRef*` back to the exact marker. Text between tags is Arrangia
//! source text (escapes like `\*` kept), so import only concatenates.

use std::collections::HashMap;
use std::fmt::Write as _;

use quick_xml::events::Event;

use super::context::{KeyContext, all_keys, key_contexts};
use super::inline::{
    Node, PairKind, Tok, TreeBuilder, concat, escape_tree, parse_tree, text_tokens,
};
use super::plan::{Entry, Parsed};
use super::{ExchangeError, ExportFile, ExportOptions, Format, MarkupMode, file_stem};
use crate::model::Project;

const NS_12: &str = "urn:oasis:names:tc:xliff:document:1.2";
const NS_20: &str = "urn:oasis:names:tc:xliff:document:2.0";

const MARKUP_NOTE: &str = "Inline markup is Arrangia syntax: **bold**, _italic_, \
[style]styled text[/], \\n line break, \\- optional hyphen. Keep the markers.";

// ----- writing -------------------------------------------------------------------------

fn esc_text(s: &str, out: &mut String) {
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '\r' => out.push_str("&#13;"),
            '\t' | '\n' => out.push(c),
            c if (c as u32) < 0x20 || c == '\u{FFFE}' || c == '\u{FFFF}' => {}
            c => out.push(c),
        }
    }
}

fn esc_attr(s: &str) -> String {
    let mut out = String::new();
    for c in s.chars() {
        match c {
            '"' => out.push_str("&quot;"),
            '\n' => out.push_str("&#10;"),
            '\t' => out.push_str("&#9;"),
            c => esc_text(c.encode_utf8(&mut [0; 4]), &mut out),
        }
    }
    out
}

/// Ids of inline elements: the n-th element of a kind gets the same id in
/// source and target.
#[derive(Default)]
struct InlineIds {
    ids: HashMap<(String, usize), usize>,
    next: usize,
    seen: HashMap<String, usize>,
    /// 2.0 `<originalData>`: marker → data id (in order of first use).
    data: Vec<String>,
}

impl InlineIds {
    fn start_pass(&mut self) {
        self.seen.clear();
    }

    fn id(&mut self, sig: &str) -> usize {
        let occ = self.seen.entry(sig.to_owned()).or_default();
        *occ += 1;
        let key = (sig.to_owned(), *occ);
        if let Some(&id) = self.ids.get(&key) {
            return id;
        }
        self.next += 1;
        self.ids.insert(key, self.next);
        self.next
    }

    fn data(&mut self, marker: &str) -> String {
        let i = self
            .data
            .iter()
            .position(|d| d == marker)
            .unwrap_or_else(|| {
                self.data.push(marker.to_owned());
                self.data.len() - 1
            });
        format!("d{}", i + 1)
    }
}

fn pair_of(n: &Node) -> Option<(PairKind, &[Node])> {
    match n {
        Node::Bold(c) => Some((PairKind::Bold, c)),
        Node::Italic(c) => Some((PairKind::Italic, c)),
        Node::Span(s, c) => Some((PairKind::Span(s.clone()), c)),
        _ => None,
    }
}

fn write_nodes(nodes: &[Node], v12: bool, ids: &mut InlineIds, out: &mut String) {
    for n in nodes {
        match n {
            Node::Text(s) => esc_text(s, out),
            Node::LineBreak | Node::SoftHyphen => {
                let lb = matches!(n, Node::LineBreak);
                let id = ids.id(if lb { "lb" } else { "shy" });
                if v12 {
                    let ctype = if lb { "lb" } else { "x-soft-hyphen" };
                    let _ = write!(out, r#"<x id="{id}" ctype="{ctype}"/>"#);
                } else if lb {
                    let d = ids.data("\\n");
                    let _ = write!(
                        out,
                        r#"<ph id="{id}" dataRef="{d}" type="fmt" subType="xlf:lb"/>"#
                    );
                } else {
                    let d = ids.data("\\-");
                    let _ = write!(out, r#"<ph id="{id}" dataRef="{d}"/>"#);
                }
            }
            _ => {
                let Some((kind, children)) = pair_of(n) else {
                    continue;
                };
                let id = ids.id(&kind.open_marker());
                if v12 {
                    let ctype = match &kind {
                        PairKind::Bold => "bold".to_owned(),
                        PairKind::Italic => "italic".to_owned(),
                        PairKind::Span(s) => format!("x-span-{s}"),
                    };
                    let _ = write!(out, r#"<g id="{id}" ctype="{}">"#, esc_attr(&ctype));
                    write_nodes(children, v12, ids, out);
                    out.push_str("</g>");
                } else {
                    let start = ids.data(&kind.open_marker());
                    let end = ids.data(kind.close_marker());
                    let sub = match kind {
                        PairKind::Bold => r#" subType="xlf:b""#,
                        PairKind::Italic => r#" subType="xlf:i""#,
                        PairKind::Span(_) => "",
                    };
                    let _ = write!(
                        out,
                        r#"<pc id="{id}" dataRefStart="{start}" dataRefEnd="{end}" type="fmt"{sub}>"#
                    );
                    write_nodes(children, v12, ids, out);
                    out.push_str("</pc>");
                }
            }
        }
    }
}

/// Source/target content of a unit.
fn content(text: &str, mode: MarkupMode, v12: bool, ids: &mut InlineIds) -> String {
    let mut out = String::new();
    ids.start_pass();
    match mode {
        MarkupMode::Plain => esc_text(text, &mut out),
        MarkupMode::Inline => write_nodes(&escape_tree(&parse_tree(text)), v12, ids, &mut out),
    }
    out
}

/// `(category, text)` notes of a unit.
fn notes(ctx: Option<&KeyContext>) -> Vec<(&'static str, String)> {
    let Some(ctx) = ctx else {
        return Vec::new();
    };
    let mut notes: Vec<_> = ctx.lines().into_iter().map(|l| ("context", l)).collect();
    if let Some(m) = ctx.max_chars() {
        notes.push((
            "length",
            format!(
                "Length: at most ≈ {m} visible characters (approximate; the text shrinks to fit)."
            ),
        ));
    }
    notes
}

/// NMTOKEN-safe unit id (2.0); keys may contain other characters.
fn unit_id(key: &str, index: usize) -> String {
    if key
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_' | ':'))
    {
        key.to_owned()
    } else {
        format!("u{index}")
    }
}

#[allow(clippy::too_many_lines)]
pub(super) fn export(p: &Project, lang: &str, format: Format, opts: &ExportOptions) -> ExportFile {
    let v12 = format == Format::Xliff12;
    let src_lang = p.languages.default.to_string();
    let contexts = opts.context.then(|| key_contexts(p));
    let mut skipped = Vec::new();
    let mut units = String::new();
    let mut n = 0;
    for (index, key) in all_keys(p).into_iter().enumerate() {
        let target = p.string(lang, &key);
        if opts.missing_only && target.is_some() {
            continue;
        }
        let Some(source) = p.string(&src_lang, &key).filter(|s| !s.is_empty()) else {
            skipped.push(key);
            continue;
        };
        n += 1;
        let mut ids = InlineIds::default();
        let src = content(source, opts.markup, v12, &mut ids);
        let tgt = target.map(|t| content(t, opts.markup, v12, &mut ids));
        let notes = notes(
            contexts
                .as_ref()
                .map(|c| c.get(&key).cloned().unwrap_or_default())
                .as_ref(),
        );
        let k = esc_attr(&key);
        if v12 {
            let _ = writeln!(
                units,
                "      <trans-unit id=\"{k}\" resname=\"{k}\" xml:space=\"preserve\">\n        <source>{src}</source>"
            );
            match tgt {
                Some(t) => {
                    let _ = writeln!(units, "        <target state=\"translated\">{t}</target>");
                }
                None => units.push_str("        <target state=\"needs-translation\"/>\n"),
            }
            for (_, note) in &notes {
                let mut s = String::new();
                esc_text(note, &mut s);
                let _ = writeln!(units, "        <note from=\"arrangia\">{s}</note>");
            }
            units.push_str("      </trans-unit>\n");
        } else {
            let _ = writeln!(
                units,
                "    <unit id=\"{}\" name=\"{k}\">",
                esc_attr(&unit_id(&key, index))
            );
            if !notes.is_empty() {
                units.push_str("      <notes>\n");
                for (category, note) in &notes {
                    let mut s = String::new();
                    esc_text(note, &mut s);
                    let _ = writeln!(units, "        <note category=\"{category}\">{s}</note>");
                }
                units.push_str("      </notes>\n");
            }
            if !ids.data.is_empty() {
                units.push_str("      <originalData>\n");
                for (i, d) in ids.data.iter().enumerate() {
                    let mut s = String::new();
                    esc_text(d, &mut s);
                    let _ = writeln!(units, "        <data id=\"d{}\">{s}</data>", i + 1);
                }
                units.push_str("      </originalData>\n");
            }
            let state = if tgt.is_some() {
                "translated"
            } else {
                "initial"
            };
            let _ = writeln!(
                units,
                "      <segment state=\"{state}\">\n        <source>{src}</source>"
            );
            if let Some(t) = tgt {
                let _ = writeln!(units, "        <target>{t}</target>");
            }
            units.push_str("      </segment>\n    </unit>\n");
        }
    }
    let original = esc_attr(&p.name);
    let mut doc = String::from("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n");
    let plain_note = opts.markup == MarkupMode::Plain;
    if v12 {
        let _ = writeln!(
            doc,
            "<xliff xmlns=\"{NS_12}\" version=\"1.2\">\n  <file original=\"{original}\" source-language=\"{}\" target-language=\"{}\" datatype=\"plaintext\">\n    <header>\n      <tool tool-id=\"arrangia\" tool-name=\"Arrangia\" tool-version=\"{}\"/>",
            esc_attr(&src_lang),
            esc_attr(lang),
            env!("CARGO_PKG_VERSION"),
        );
        if plain_note {
            let _ = writeln!(doc, "      <note>{MARKUP_NOTE}</note>");
        }
        doc.push_str("    </header>\n    <body>\n");
        doc.push_str(&units);
        doc.push_str("    </body>\n  </file>\n</xliff>\n");
    } else {
        let _ = writeln!(
            doc,
            "<xliff xmlns=\"{NS_20}\" version=\"2.0\" srcLang=\"{}\" trgLang=\"{}\">\n  <file id=\"f1\" original=\"{original}\" xml:space=\"preserve\">",
            esc_attr(&src_lang),
            esc_attr(lang),
        );
        if plain_note {
            let _ = writeln!(
                doc,
                "    <notes>\n      <note category=\"format\">{MARKUP_NOTE}</note>\n    </notes>"
            );
        }
        doc.push_str(&units);
        doc.push_str("  </file>\n</xliff>\n");
    }
    ExportFile {
        name: format!("{}-{lang}.xlf", file_stem(p)),
        lang: Some(lang.to_owned()),
        keys: n,
        skipped,
        content: doc.into_bytes(),
    }
}

// ----- reading -------------------------------------------------------------------------

/// A parsed XML element (local names; `xml:` attributes keep the prefix).
#[derive(Debug, Default)]
pub(super) struct El {
    pub name: String,
    pub attrs: Vec<(String, String)>,
    pub children: Vec<XNode>,
}

#[derive(Debug)]
pub(super) enum XNode {
    El(El),
    Text(String),
}

impl El {
    pub fn attr(&self, name: &str) -> Option<&str> {
        self.attrs
            .iter()
            .find(|(k, _)| k == name)
            .map(|(_, v)| v.as_str())
    }

    pub fn elements(&self) -> impl Iterator<Item = &El> {
        self.children.iter().filter_map(|c| match c {
            XNode::El(e) => Some(e),
            XNode::Text(_) => None,
        })
    }

    pub fn child(&self, name: &str) -> Option<&El> {
        self.elements().find(|e| e.name == name)
    }

    fn is_empty(&self) -> bool {
        self.children.iter().all(|c| match c {
            XNode::Text(t) => t.is_empty(),
            XNode::El(_) => false,
        })
    }
}

fn xml_err(e: impl std::fmt::Display) -> ExchangeError {
    ExchangeError::Xliff(e.to_string())
}

fn local(s: &str) -> String {
    if s.starts_with("xml:") {
        return s.to_owned();
    }
    s.rsplit(':').next().unwrap_or_default().to_owned()
}

fn element(e: &quick_xml::events::BytesStart<'_>) -> Result<El, ExchangeError> {
    let mut el = El {
        name: local(e.name().as_ref()),
        ..El::default()
    };
    for a in e.attributes() {
        let a = a.map_err(xml_err)?;
        let raw: String = a.key.as_ref().to_owned();
        let key = if raw.starts_with("xmlns") {
            raw
        } else {
            local(a.key.as_ref())
        };
        let value = a
            .normalized_value(quick_xml::XmlVersion::Implicit1_0)
            .map_err(xml_err)?
            .into_owned();
        el.attrs.push((key, value));
    }
    Ok(el)
}

/// Parses a document into its root element.
pub(super) fn parse_dom(text: &str) -> Result<El, ExchangeError> {
    let mut reader = quick_xml::Reader::from_str(text);
    let mut stack: Vec<El> = vec![El::default()];
    let push_text = |stack: &mut Vec<El>, s: &str| {
        if let Some(top) = stack.last_mut() {
            if let Some(XNode::Text(t)) = top.children.last_mut() {
                t.push_str(s);
            } else {
                top.children.push(XNode::Text(s.to_owned()));
            }
        }
    };
    loop {
        match reader.read_event().map_err(|e| {
            ExchangeError::Xliff(format!("{e} (at byte {})", reader.buffer_position()))
        })? {
            Event::Start(e) => stack.push(element(&e)?),
            Event::Empty(e) => {
                let el = element(&e)?;
                if let Some(top) = stack.last_mut() {
                    top.children.push(XNode::El(el));
                }
            }
            Event::End(_) => {
                let el = stack.pop().ok_or_else(|| xml_err("unbalanced end tag"))?;
                match stack.last_mut() {
                    Some(top) => top.children.push(XNode::El(el)),
                    None => return Err(xml_err("unbalanced end tag")),
                }
            }
            Event::Text(t) => push_text(&mut stack, &t.xml10_content()),
            Event::CData(t) => push_text(&mut stack, &t.into_inner()),
            Event::GeneralRef(r) => {
                let s = if let Some(c) = r.resolve_char_ref().map_err(xml_err)? {
                    c.to_string()
                } else {
                    let name = r.into_inner().into_owned();
                    quick_xml::escape::resolve_predefined_entity(&name)
                        .ok_or_else(|| xml_err(format!("unknown entity `&{name};`")))?
                        .to_owned()
                };
                push_text(&mut stack, &s);
            }
            Event::Eof => break,
            Event::Comment(_) | Event::Decl(_) | Event::PI(_) | Event::DocType(_) => {}
        }
    }
    if stack.len() != 1 {
        return Err(xml_err("unexpected end of file (unclosed elements)"));
    }
    let doc = stack.pop().unwrap_or_default();
    doc.children
        .into_iter()
        .find_map(|c| match c {
            XNode::El(e) => Some(e),
            XNode::Text(_) => None,
        })
        .ok_or_else(|| xml_err("no root element"))
}

/// Is this an XLIFF 1.x document (by namespace or version attribute)?
pub(super) fn looks_like_12(text: &str) -> bool {
    // The root tag only: the XML declaration has a `version="1.0"` too.
    let root = text
        .find("<xliff")
        .map(|i| &text[i..])
        .and_then(|t| t.split_once('>'))
        .map(|(tag, _)| tag)
        .unwrap_or_default();
    text.contains(NS_12) || root.contains("version=\"1.") || root.contains("version='1.")
}

/// Converts inline content to builder tokens.
struct InlineReader<'a> {
    v12: bool,
    data: &'a HashMap<String, String>,
    open_ids: HashMap<String, PairKind>,
    problems: Vec<String>,
}

impl InlineReader<'_> {
    fn kind_from_data(&self, id: Option<&str>) -> Option<PairKind> {
        id.and_then(|d| self.data.get(d))
            .and_then(|m| PairKind::from_marker(m))
    }

    fn kind_from_sub(sub: Option<&str>) -> Option<PairKind> {
        match sub? {
            "xlf:b" => Some(PairKind::Bold),
            "xlf:i" => Some(PairKind::Italic),
            _ => None,
        }
    }

    fn kind_from_ctype(ctype: Option<&str>) -> Option<PairKind> {
        match ctype? {
            "bold" | "x-bold" => Some(PairKind::Bold),
            "italic" | "x-italic" => Some(PairKind::Italic),
            c => c
                .strip_prefix("x-span-")
                .filter(|n| PairKind::from_marker(&format!("[{n}]")).is_some())
                .map(|n| PairKind::Span(n.to_owned())),
        }
    }

    fn placeholder(&self, e: &El) -> Option<Tok> {
        if self.v12 {
            return match e.attr("ctype") {
                Some("lb") => Some(Tok::LineBreak),
                Some("x-soft-hyphen") => Some(Tok::SoftHyphen),
                _ => None,
            };
        }
        match e
            .attr("dataRef")
            .and_then(|d| self.data.get(d))
            .map(String::as_str)
        {
            Some("\\n" | "\n") => return Some(Tok::LineBreak),
            Some("\\-" | "\u{AD}") => return Some(Tok::SoftHyphen),
            _ => {}
        }
        (e.attr("subType") == Some("xlf:lb")).then_some(Tok::LineBreak)
    }

    fn read(&mut self, el: &El, out: &mut Vec<Tok>) {
        for c in &el.children {
            match c {
                XNode::Text(t) => text_tokens(t, &mut |t| out.push(t)),
                XNode::El(e) => self.read_el(e, out),
            }
        }
    }

    fn unknown(&mut self, e: &El) {
        let id = e
            .attr("id")
            .map(|i| format!(" id=\"{i}\""))
            .unwrap_or_default();
        self.problems.push(format!(
            "unknown inline element <{}{id}> (markup dropped)",
            e.name
        ));
    }

    fn read_el(&mut self, e: &El, out: &mut Vec<Tok>) {
        let v12 = self.v12;
        match e.name.as_str() {
            "pc" | "g" => {
                let kind = if v12 {
                    Self::kind_from_ctype(e.attr("ctype"))
                } else {
                    self.kind_from_data(e.attr("dataRefStart"))
                        .or_else(|| Self::kind_from_sub(e.attr("subType")))
                };
                if let Some(k) = kind {
                    out.push(Tok::Open(k.clone()));
                    self.read(e, out);
                    out.push(Tok::Close(k));
                } else {
                    self.unknown(e);
                    self.read(e, out);
                }
            }
            "sc" | "bx" => {
                let kind = if v12 {
                    Self::kind_from_ctype(e.attr("ctype"))
                } else {
                    self.kind_from_data(e.attr("dataRef"))
                        .or_else(|| Self::kind_from_sub(e.attr("subType")))
                };
                match kind {
                    Some(k) => {
                        let id = e.attr(if v12 { "rid" } else { "id" }).or(e.attr("id"));
                        if let Some(id) = id {
                            self.open_ids.insert(id.to_owned(), k.clone());
                        }
                        out.push(Tok::Open(k));
                    }
                    None => self.unknown(e),
                }
            }
            "ec" | "ex" => {
                let id = e
                    .attr(if v12 { "rid" } else { "startRef" })
                    .or(e.attr("id"));
                let kind = id.and_then(|i| self.open_ids.get(i).cloned()).or_else(|| {
                    (!v12)
                        .then(|| e.attr("dataRef").and_then(|d| self.data.get(d)))
                        .flatten()
                        .and_then(|m| match m.as_str() {
                            "[/]" => Some(PairKind::Span(String::new())),
                            m => PairKind::from_marker(m),
                        })
                });
                match kind {
                    Some(k) => out.push(Tok::Close(k)),
                    None => self.unknown(e),
                }
            }
            "ph" | "x" => match self.placeholder(e) {
                Some(t) => out.push(t),
                None => self.unknown(e),
            },
            "cp" => {
                if let Some(c) = e
                    .attr("hex")
                    .and_then(|h| u32::from_str_radix(h, 16).ok())
                    .and_then(char::from_u32)
                {
                    text_tokens(c.encode_utf8(&mut [0; 4]), &mut |t| out.push(t));
                }
            }
            "mrk" => self.read(e, out),
            "sm" | "em" => {}
            _ => {
                self.unknown(e);
                self.read(e, out);
            }
        }
    }
}

/// Inline content of a source/target element → markup string + problems.
fn read_content(els: &[&El], v12: bool, data: &HashMap<String, String>) -> (String, Vec<String>) {
    let mut r = InlineReader {
        v12,
        data,
        open_ids: HashMap::new(),
        problems: Vec::new(),
    };
    let mut toks = Vec::new();
    for el in els {
        r.read(el, &mut toks);
    }
    let mut b = TreeBuilder::default();
    for t in toks {
        b.push(t);
    }
    let (tree, mut problems) = b.finish();
    r.problems.append(&mut problems);
    (concat(&tree), r.problems)
}

fn find_all<'a>(el: &'a El, name: &str, out: &mut Vec<&'a El>) {
    for c in el.elements() {
        if c.name == name {
            out.push(c);
        } else {
            find_all(c, name, out);
        }
    }
}

fn project_lang(p: &Project, code: &str) -> Option<String> {
    p.languages
        .available
        .iter()
        .find(|l| l.as_str().eq_ignore_ascii_case(code))
        .map(ToString::to_string)
}

fn check_langs(
    p: &Project,
    src: Option<&str>,
    trg: Option<&str>,
    parsed: &mut Parsed,
) -> Result<String, ExchangeError> {
    let trg = trg.ok_or_else(|| xml_err("no target language (trgLang / target-language)"))?;
    let lang =
        project_lang(p, trg).ok_or_else(|| ExchangeError::UnknownLanguage(trg.to_owned()))?;
    if let Some(src) = src
        && !src.eq_ignore_ascii_case(p.languages.default.as_str())
    {
        parsed.warnings.push(format!(
            "source language `{src}` is not the project's default language `{}`",
            p.languages.default
        ));
    }
    if lang == p.languages.default.as_str() {
        parsed.warnings.push(format!(
            "target language `{lang}` is the default language (source texts will change)"
        ));
    }
    if !parsed.langs.contains(&lang) {
        parsed.langs.push(lang.clone());
    }
    Ok(lang)
}

fn untranslated(state: Option<&str>) -> bool {
    matches!(state, Some("needs-translation" | "new" | "initial"))
}

#[allow(clippy::too_many_lines)]
pub(super) fn parse(p: &Project, text: &str) -> Result<Parsed, ExchangeError> {
    let root = parse_dom(text)?;
    if root.name != "xliff" {
        return Err(xml_err(format!(
            "root element is <{}>, not <xliff>",
            root.name
        )));
    }
    let version = root.attr("version").unwrap_or_default();
    let v12 = version.starts_with('1') || root.attr("xmlns") == Some(NS_12);
    if !v12 && !version.starts_with('2') {
        return Err(xml_err(format!("unsupported XLIFF version `{version}`")));
    }
    let mut parsed = Parsed::default();
    let mut files = Vec::new();
    find_all(&root, "file", &mut files);
    if files.is_empty() {
        return Err(xml_err("no <file> element"));
    }
    let no_data = HashMap::new();
    for file in files {
        if v12 {
            let lang = check_langs(
                p,
                file.attr("source-language"),
                file.attr("target-language").or(root.attr("trgLang")),
                &mut parsed,
            )?;
            let mut units = Vec::new();
            find_all(file, "trans-unit", &mut units);
            for u in units {
                let Some(key) = u.attr("resname").or(u.attr("id")) else {
                    parsed
                        .warnings
                        .push("<trans-unit> without id ignored".to_owned());
                    continue;
                };
                let source = u.child("source");
                let target = u.child("target").filter(|t| !t.is_empty());
                let (value, problems) = match target {
                    Some(t) => {
                        let (v, problems) = read_content(&[t], true, &no_data);
                        let copied = source
                            .map(|s| read_content(&[s], true, &no_data).0)
                            .is_some_and(|s| s == v);
                        if untranslated(t.attr("state")) && copied {
                            (None, Vec::new())
                        } else {
                            (Some(v), problems)
                        }
                    }
                    None => (None, Vec::new()),
                };
                parsed.entries.push(Entry {
                    lang: lang.clone(),
                    key: key.to_owned(),
                    value: value.filter(|v| !v.is_empty()),
                    problems,
                });
            }
        } else {
            let lang = check_langs(p, root.attr("srcLang"), root.attr("trgLang"), &mut parsed)?;
            let mut units = Vec::new();
            find_all(file, "unit", &mut units);
            for u in units {
                let Some(key) = u.attr("name").or(u.attr("id")) else {
                    parsed.warnings.push("<unit> without id ignored".to_owned());
                    continue;
                };
                let data: HashMap<String, String> = u
                    .child("originalData")
                    .map(|od| {
                        od.elements()
                            .filter(|d| d.name == "data")
                            .filter_map(|d| {
                                let text: String = d
                                    .children
                                    .iter()
                                    .filter_map(|c| match c {
                                        XNode::Text(t) => Some(t.as_str()),
                                        XNode::El(_) => None,
                                    })
                                    .collect();
                                d.attr("id").map(|id| (id.to_owned(), text))
                            })
                            .collect()
                    })
                    .unwrap_or_default();
                let mut sources = Vec::new();
                let mut targets = Vec::new();
                let mut all_untranslated = true;
                let mut any_target = false;
                for seg in u
                    .elements()
                    .filter(|e| e.name == "segment" || e.name == "ignorable")
                {
                    let src = seg.child("source");
                    let tgt = seg.child("target").filter(|t| !t.is_empty());
                    if seg.name == "segment" {
                        any_target |= tgt.is_some();
                        all_untranslated &= untranslated(seg.attr("state"));
                    }
                    if let Some(s) = src {
                        sources.push(s);
                    }
                    if let Some(t) = tgt.or(if seg.name == "ignorable" { src } else { None }) {
                        targets.push(t);
                    }
                }
                let (value, problems) = if any_target {
                    let (v, problems) = read_content(&targets, false, &data);
                    let copied = read_content(&sources, false, &data).0 == v;
                    if all_untranslated && copied {
                        (None, Vec::new())
                    } else {
                        (Some(v), problems)
                    }
                } else {
                    (None, Vec::new())
                };
                parsed.entries.push(Entry {
                    lang: lang.clone(),
                    key: key.to_owned(),
                    value: value.filter(|v| !v.is_empty()),
                    problems,
                });
            }
        }
    }
    Ok(parsed)
}

/// Structural self-check used by tests: namespaces, version, required
/// attributes and the id rules of XLIFF 1.2/2.0 core.
#[cfg(test)]
#[allow(clippy::too_many_lines)]
pub(super) fn check_structure(text: &str) -> Result<(), String> {
    use std::collections::HashSet;
    let root = parse_dom(text).map_err(|e| e.to_string())?;
    let v12 = root.attr("version") == Some("1.2");
    let ns = root.attr("xmlns").ok_or("no default namespace")?;
    let need = |e: &El, attrs: &[&str]| -> Result<(), String> {
        for a in attrs {
            if e.attr(a).is_none() {
                return Err(format!("<{}> lacks required `{a}`", e.name));
            }
        }
        Ok(())
    };
    if v12 {
        if ns != NS_12 {
            return Err(format!("namespace {ns}"));
        }
        let mut files = Vec::new();
        find_all(&root, "file", &mut files);
        for f in files {
            need(f, &["original", "source-language", "datatype"])?;
            let order: Vec<&str> = f.elements().map(|e| e.name.as_str()).collect();
            if order != ["header", "body"] && order != ["body"] {
                return Err(format!("<file> children {order:?}"));
            }
            let mut units = Vec::new();
            find_all(f, "trans-unit", &mut units);
            let mut ids = HashSet::new();
            for u in units {
                need(u, &["id"])?;
                if !ids.insert(u.attr("id")) {
                    return Err("duplicate trans-unit id".into());
                }
                let names: Vec<&str> = u.elements().map(|e| e.name.as_str()).collect();
                if names.first() != Some(&"source") {
                    return Err(format!("<trans-unit> children {names:?}"));
                }
                let rank = |n: &str| match n {
                    "source" => 0,
                    "target" => 1,
                    "note" => 2,
                    _ => 9,
                };
                if names.windows(2).any(|w| rank(w[0]) > rank(w[1]))
                    || names.iter().any(|n| rank(n) == 9)
                {
                    return Err(format!("<trans-unit> children order {names:?}"));
                }
            }
        }
    } else {
        if ns != NS_20 || root.attr("version") != Some("2.0") {
            return Err(format!("namespace/version {ns}"));
        }
        need(&root, &["srcLang"])?;
        let mut files = Vec::new();
        find_all(&root, "file", &mut files);
        let mut file_ids = HashSet::new();
        for f in files {
            need(f, &["id"])?;
            if !file_ids.insert(f.attr("id")) {
                return Err("duplicate file id".into());
            }
            let mut ids = HashSet::new();
            for u in f.elements() {
                match u.name.as_str() {
                    "notes" => continue,
                    "unit" => {}
                    n => return Err(format!("unexpected <{n}> in <file>")),
                }
                need(u, &["id"])?;
                let id = u.attr("id").unwrap_or_default();
                if id.is_empty()
                    || !id
                        .chars()
                        .all(|c| c.is_alphanumeric() || matches!(c, '.' | '-' | '_' | ':'))
                {
                    return Err(format!("unit id `{id}` is not an NMTOKEN"));
                }
                if !ids.insert(id) {
                    return Err("duplicate unit id".into());
                }
                let rank = |n: &str| match n {
                    "notes" => 0,
                    "originalData" => 1,
                    "segment" | "ignorable" => 2,
                    _ => 9,
                };
                let names: Vec<&str> = u.elements().map(|e| e.name.as_str()).collect();
                if names.windows(2).any(|w| rank(w[0]) > rank(w[1]))
                    || names.iter().any(|n| rank(n) == 9)
                {
                    return Err(format!("<unit> children order {names:?}"));
                }
                let data: HashSet<&str> = u
                    .child("originalData")
                    .map(|od| od.elements().filter_map(|d| d.attr("id")).collect())
                    .unwrap_or_default();
                for seg in u.elements().filter(|e| e.name == "segment") {
                    let src = seg.child("source").ok_or("<segment> without <source>")?;
                    if let Some(s) = seg.attr("state")
                        && !["initial", "translated", "reviewed", "final"].contains(&s)
                    {
                        return Err(format!("state `{s}`"));
                    }
                    let mut src_ids = HashSet::new();
                    collect_inline(src, &data, &mut src_ids)?;
                    if let Some(t) = seg.child("target") {
                        let mut tgt_ids = HashSet::new();
                        collect_inline(t, &data, &mut tgt_ids)?;
                    }
                }
            }
        }
    }
    Ok(())
}

#[cfg(test)]
fn collect_inline<'a>(
    el: &'a El,
    data: &std::collections::HashSet<&str>,
    ids: &mut std::collections::HashSet<&'a str>,
) -> Result<(), String> {
    for c in el.elements() {
        match c.name.as_str() {
            "pc" | "ph" => {
                let id = c.attr("id").ok_or("inline element without id")?;
                if !ids.insert(id) {
                    return Err(format!("duplicate inline id {id}"));
                }
                for r in ["dataRef", "dataRefStart", "dataRefEnd"] {
                    if let Some(d) = c.attr(r)
                        && !data.contains(d)
                    {
                        return Err(format!("{r}=`{d}` has no <data>"));
                    }
                }
                if c.attr("subType").is_some() && c.attr("type").is_none() {
                    return Err("subType without type".into());
                }
                collect_inline(c, data, ids)?;
            }
            n => return Err(format!("unexpected inline <{n}>")),
        }
    }
    Ok(())
}
