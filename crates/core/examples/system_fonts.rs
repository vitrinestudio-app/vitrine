//! Scans the installed system fonts like the app does (ADR 0028) and
//! prints the time, the number of families and – for a few languages –
//! which system family the CJK fallback picks.
//!
//! `cargo run -p vitrine-core --release --example system_fonts [-- --list]`

use std::time::Instant;

use vitrine_core::layout::FontLibrary;
use vitrine_core::layout::fonts::system_font_dirs;
use vitrine_core::layout::system::SystemFontIndex;

fn main() {
    let dirs = system_font_dirs();
    let t = Instant::now();
    let index = SystemFontIndex::scan(&dirs);
    println!(
        "scanned {} folders: {} families in {:.1?}",
        dirs.len(),
        index.len(),
        t.elapsed()
    );
    if std::env::args().any(|a| a == "--list") {
        for f in index.infos() {
            println!(
                "  {} ({}–{}{})",
                f.family,
                f.weights[0],
                f.weights[1],
                if f.italic { ", italic" } else { "" }
            );
        }
    }
    for (lang, text) in [
        ("ja", "数分で庭を計画しましょう"),
        ("zh-Hans", "几分钟内规划你的花园"),
        ("zh-Hant", "幾分鐘內規劃你的花園"),
        ("ko", "몇 분 만에 정원을 계획하세요"),
    ] {
        let mut lib = FontLibrary::bundled();
        let t = Instant::now();
        let added = lib.add_system_fallbacks_from(&index, Some(lang), text);
        println!(
            "{lang}: {added:?} in {:.1?}, still missing: {:?}",
            t.elapsed(),
            lib.uncovered_for(text, Some(lang))
        );
    }
}
