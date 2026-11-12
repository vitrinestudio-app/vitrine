//! Scripts of a text, for automatic font fallbacks (ADR 0028).
//!
//! Two kinds of fallback depend on the script of the characters:
//!
//! - **Bundled script fonts** (Noto Sans Arabic, Hebrew, Devanagari,
//!   Bengali, Tamil, Thai; [`bundled_fallback`]): added to a text's
//!   family chain after [`super::fonts::DEFAULT_FAMILY`] when the text
//!   contains characters of that script. Deterministic, so CI and the GUI
//!   draw the same glyphs.
//! - **CJK system fonts** ([`cjk_preferences`]): Chinese, Japanese and
//!   Korean fonts are not bundled (10–20 MB each). With system fonts on,
//!   the language's preferred system family (`Hiragino Sans`, `Yu Gothic` for `ja`,
//!   `PingFang SC`, `Microsoft YaHei` for `zh-Hans` …) is loaded first, so Han
//!   characters get the glyph shapes of the language.

/// A script with special font handling.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Script {
    Arabic,
    Hebrew,
    Devanagari,
    Bengali,
    Tamil,
    Thai,
    /// Han ideographs, kana, Hangul and CJK punctuation.
    Cjk,
}

/// Bundled fallback family per script (all SIL OFL 1.1, `assets/fonts/`).
const BUNDLED: &[(Script, &str)] = &[
    (Script::Arabic, "Noto Sans Arabic"),
    (Script::Hebrew, "Noto Sans Hebrew"),
    (Script::Devanagari, "Noto Sans Devanagari"),
    (Script::Bengali, "Noto Sans Bengali"),
    (Script::Tamil, "Noto Sans Tamil"),
    (Script::Thai, "Noto Sans Thai"),
];

/// The script of `c`, for the scripts with special font handling.
pub fn script_of(c: char) -> Option<Script> {
    let u = u32::from(c);
    Some(match u {
        0x0590..=0x05FF | 0xFB1D..=0xFB4F => Script::Hebrew,
        0x0600..=0x06FF
        | 0x0750..=0x077F
        | 0x0870..=0x08FF
        | 0xFB50..=0xFDFF
        | 0xFE70..=0xFEFF
        | 0x10E60..=0x10E7F => Script::Arabic,
        0x0900..=0x097F | 0xA8E0..=0xA8FF | 0x11B00..=0x11B5F => Script::Devanagari,
        0x0980..=0x09FF => Script::Bengali,
        0x0B80..=0x0BFF | 0x11FC0..=0x11FFF => Script::Tamil,
        0x0E00..=0x0E7F => Script::Thai,
        0x1100..=0x11FF
        | 0x2E80..=0x2FDF
        | 0x3000..=0x30FF
        | 0x3130..=0x318F
        | 0x31C0..=0x31FF
        | 0x3200..=0x4DBF
        | 0x4E00..=0x9FFF
        | 0xA960..=0xA97F
        | 0xAC00..=0xD7FF
        | 0xF900..=0xFAFF
        | 0xFE30..=0xFE4F
        | 0xFF00..=0xFFEF
        | 0x1B000..=0x1B16F
        | 0x20000..=0x3134F => Script::Cjk,
        _ => return None,
    })
}

/// The scripts with special font handling that occur in `text`, sorted.
pub fn scripts(text: &str) -> Vec<Script> {
    let mut out: Vec<Script> = Vec::new();
    for c in text.chars() {
        // Fast path: ASCII and Latin/Greek/Cyrillic never match.
        if u32::from(c) < 0x0590 {
            continue;
        }
        if let Some(s) = script_of(c)
            && !out.contains(&s)
        {
            out.push(s);
        }
    }
    out.sort_unstable();
    out
}

/// The bundled fallback family for `script` (none for CJK).
pub fn bundled_fallback(script: Script) -> Option<&'static str> {
    BUNDLED.iter().find(|(s, _)| *s == script).map(|(_, f)| *f)
}

/// Bundled fallback families for the scripts of `text`.
pub fn bundled_fallbacks(text: &str) -> Vec<&'static str> {
    scripts(text)
        .into_iter()
        .filter_map(bundled_fallback)
        .collect()
}

/// Is `family` one of the bundled script fallbacks?
pub fn is_bundled_fallback(family: &str) -> bool {
    BUNDLED.iter().any(|(_, f)| f.eq_ignore_ascii_case(family))
}

/// Which CJK font flavour a language wants (Han glyph shapes differ).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CjkFlavour {
    Japanese,
    Korean,
    SimplifiedChinese,
    TraditionalChinese,
    HongKong,
}

/// The CJK flavour of a BCP 47 language tag (`ja`, `ko`, `zh-Hans`,
/// `zh-CN`, `zh-Hant`, `zh-TW`, `zh-HK` …); `None` for other languages.
pub fn cjk_flavour(lang: &str) -> Option<CjkFlavour> {
    let lower = lang.to_ascii_lowercase().replace('_', "-");
    let mut parts = lower.split('-');
    let primary = parts.next().unwrap_or_default();
    let rest: Vec<&str> = parts.collect();
    match primary {
        "ja" => Some(CjkFlavour::Japanese),
        "ko" => Some(CjkFlavour::Korean),
        "zh" | "yue" | "cmn" => {
            if rest.iter().any(|p| matches!(*p, "hk" | "mo")) {
                Some(CjkFlavour::HongKong)
            } else if rest.iter().any(|p| matches!(*p, "hant" | "tw")) || primary == "yue" {
                Some(CjkFlavour::TraditionalChinese)
            } else {
                Some(CjkFlavour::SimplifiedChinese)
            }
        }
        _ => None,
    }
}

/// System families to try first for CJK characters, by flavour: macOS,
/// Windows, then Linux/other (Noto CJK, Source Han). The first one that is
/// installed is used before the generic coverage search.
pub fn cjk_preferences(flavour: Option<CjkFlavour>) -> &'static [&'static str] {
    match flavour {
        Some(CjkFlavour::Japanese) => &[
            "Hiragino Sans",
            "Hiragino Kaku Gothic ProN",
            "Yu Gothic",
            "Yu Gothic UI",
            "Meiryo",
            "Noto Sans CJK JP",
            "Noto Sans JP",
            "Source Han Sans JP",
            "MS Gothic",
        ],
        Some(CjkFlavour::Korean) => &[
            "Apple SD Gothic Neo",
            "Malgun Gothic",
            "Noto Sans CJK KR",
            "Noto Sans KR",
            "Source Han Sans KR",
            "NanumGothic",
        ],
        Some(CjkFlavour::TraditionalChinese) => &[
            "PingFang TC",
            "Heiti TC",
            "Microsoft JhengHei",
            "Microsoft JhengHei UI",
            "Noto Sans CJK TC",
            "Noto Sans TC",
            "Source Han Sans TC",
        ],
        Some(CjkFlavour::HongKong) => &[
            "PingFang HK",
            "PingFang TC",
            "Microsoft JhengHei",
            "Noto Sans CJK HK",
            "Noto Sans CJK TC",
            "Noto Sans HK",
        ],
        Some(CjkFlavour::SimplifiedChinese) | None => &[
            "PingFang SC",
            "Hiragino Sans GB",
            "Heiti SC",
            "Microsoft YaHei",
            "Microsoft YaHei UI",
            "Noto Sans CJK SC",
            "Noto Sans SC",
            "Source Han Sans SC",
            "WenQuanYi Micro Hei",
        ],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scripts_of_text() {
        assert_eq!(scripts("Hello, Привет, Γειά"), []);
        assert_eq!(scripts("مرحبا Hello"), [Script::Arabic]);
        assert_eq!(scripts("שלום"), [Script::Hebrew]);
        assert_eq!(
            scripts("नमस्ते বাংলা தமிழ் ไทย"),
            [
                Script::Devanagari,
                Script::Bengali,
                Script::Tamil,
                Script::Thai
            ]
        );
        assert_eq!(scripts("日本語のテキスト、한국어"), [Script::Cjk]);
        assert_eq!(
            bundled_fallbacks("مرحبا 日本"),
            ["Noto Sans Arabic"],
            "no bundled CJK"
        );
        assert!(is_bundled_fallback("noto sans thai"));
    }

    #[test]
    fn cjk_flavours() {
        assert_eq!(cjk_flavour("ja"), Some(CjkFlavour::Japanese));
        assert_eq!(cjk_flavour("ja-JP"), Some(CjkFlavour::Japanese));
        assert_eq!(cjk_flavour("ko"), Some(CjkFlavour::Korean));
        assert_eq!(cjk_flavour("zh-Hans"), Some(CjkFlavour::SimplifiedChinese));
        assert_eq!(cjk_flavour("zh-CN"), Some(CjkFlavour::SimplifiedChinese));
        assert_eq!(cjk_flavour("zh"), Some(CjkFlavour::SimplifiedChinese));
        assert_eq!(cjk_flavour("zh-Hant"), Some(CjkFlavour::TraditionalChinese));
        assert_eq!(cjk_flavour("zh-TW"), Some(CjkFlavour::TraditionalChinese));
        assert_eq!(cjk_flavour("zh-Hant-HK"), Some(CjkFlavour::HongKong));
        assert_eq!(cjk_flavour("de"), None);
        assert_eq!(
            cjk_preferences(Some(CjkFlavour::Japanese))[0],
            "Hiragino Sans"
        );
    }
}
