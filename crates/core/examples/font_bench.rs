//! Start-up cost of the bundled fonts (lazy loading,
//! `vitrine_core::layout::bundled`): building the library from the
//! manifest vs. parsing every file, a new layouter, and the first layout
//! of a family (registration) vs. later ones.
//!
//! `cargo run -p vitrine-core --release --example font_bench`

use std::time::{Duration, Instant};

use vitrine_core::layout::bundled::{self, FILES};
use vitrine_core::layout::{FontLibrary, FontSource, ResolvedTextStyle, TextInput, TextLayouter};

fn avg(n: u32, mut f: impl FnMut()) -> Duration {
    let t = Instant::now();
    for _ in 0..n {
        f();
    }
    t.elapsed() / n
}

fn style(font: &str) -> ResolvedTextStyle {
    ResolvedTextStyle {
        font: font.to_owned(),
        weight: 700,
        size: 80.0,
        ..ResolvedTextStyle::default()
    }
}

fn main() {
    println!("bundled files: {}", FILES.len());
    let lazy = avg(200, || {
        std::hint::black_box(FontLibrary::bundled());
    });
    println!("FontLibrary::bundled() (manifest, lazy):     {lazy:>10.1?}");
    let eager = avg(20, || {
        let mut lib = FontLibrary::empty();
        for f in &FILES {
            lib.add_font(f.data().to_vec(), &FontSource::Memory)
                .unwrap();
        }
        std::hint::black_box(lib);
    });
    println!("parsing every file (eager, for comparison):  {eager:>10.1?}");
    let new = avg(200, || {
        std::hint::black_box(TextLayouter::new(FontLibrary::bundled()));
    });
    println!("TextLayouter::new (nothing registered):      {new:>10.1?}");

    let text = "Plane deine Beete in Minuten";
    for fam in ["Inter", "Montserrat", "Playfair Display", "JetBrains Mono"] {
        let first = avg(20, || {
            let mut l = TextLayouter::new(FontLibrary::bundled());
            std::hint::black_box(l.layout(&TextInput::new(text, &style(fam), 900.0)));
        });
        let mut l = TextLayouter::new(FontLibrary::bundled());
        l.layout(&TextInput::new(text, &style(fam), 900.0));
        let again = avg(200, || {
            std::hint::black_box(l.layout(&TextInput::new(text, &style(fam), 900.0)));
        });
        println!(
            "{fam:<17} new layouter + first layout {first:>9.1?}, later layouts {again:>9.1?}"
        );
    }
    println!(
        "bundled blobs created in this process: {}",
        bundled::loaded_files()
    );
}
