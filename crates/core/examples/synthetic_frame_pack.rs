//! Writes the synthetic frame pack "Clay phone" (our own generated PNGs and
//! an SVG plus `frame.yaml`, never vendor artwork) for trying out own frame
//! packs (ADR 0024, format: `docs/guide/frame-packs.md`):
//!
//! ```sh
//! cargo run -p vitrine-core --example synthetic_frame_pack -- <dir>            # pack folder
//! cargo run -p vitrine-core --example synthetic_frame_pack -- <file.zip>       # zip
//! cargo run -p vitrine-core --example synthetic_frame_pack -- --full-size <dir>
//! ```
//!
//! By default the phone screens are 1/4 of the 6.3" size (301×655);
//! `--full-size` writes 1206×2622.

use std::path::PathBuf;
use std::process::ExitCode;

use vitrine_core::frame_import::synthetic;

fn main() -> ExitCode {
    let mut args: Vec<_> = std::env::args_os().skip(1).collect();
    let full = args.iter().any(|a| a == "--full-size");
    args.retain(|a| a != "--full-size");
    let Some(out) = args.first().map(PathBuf::from) else {
        eprintln!("usage: synthetic_frame_pack [--full-size] <dir | file.zip>");
        return ExitCode::from(2);
    };
    let div = if full { 1 } else { 4 };
    let result = if out
        .extension()
        .is_some_and(|e| e.eq_ignore_ascii_case("zip"))
    {
        synthetic::write_pack_zip(&out, div).map(|_| ())
    } else {
        synthetic::write_pack(&out, div)
    };
    match result {
        Ok(()) => {
            println!("wrote the frame pack `Clay phone` to {}", out.display());
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("error: {e}");
            ExitCode::FAILURE
        }
    }
}
