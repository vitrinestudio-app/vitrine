//! Writes the synthetic bezel package (generated PNGs named like Apple's
//! Product Bezels, never vendor artwork) for trying out the frame import:
//!
//! ```sh
//! cargo run -p vitrine-core --example synthetic_bezels -- <dir>          # folder
//! cargo run -p vitrine-core --example synthetic_bezels -- <file.zip>     # zip
//! cargo run -p vitrine-core --release --example synthetic_bezels -- --full-size <dir>
//! ```
//!
//! By default the phone screens are 1/4 of the real size; `--full-size`
//! writes them at 1320×2868 ("Pro Max") and 1206×2622 ("Pro"), so the
//! automatic bezel choice per App Store target can be tried.

use std::path::PathBuf;
use std::process::ExitCode;

use vitrine_core::frame_import::synthetic;

fn main() -> ExitCode {
    let mut args: Vec<_> = std::env::args_os().skip(1).collect();
    let full = args.iter().any(|a| a == "--full-size");
    args.retain(|a| a != "--full-size");
    let Some(out) = args.first().map(PathBuf::from) else {
        eprintln!("usage: synthetic_bezels [--full-size] <dir | file.zip>");
        return ExitCode::from(2);
    };
    let specs = if full {
        synthetic::specs(1)
    } else {
        synthetic::default_specs()
    };
    let result = if out
        .extension()
        .is_some_and(|e| e.eq_ignore_ascii_case("zip"))
    {
        synthetic::write_zip(&out, &specs).map(|_| ())
    } else {
        synthetic::write_folder(&out, &specs)
    };
    match result {
        Ok(()) => {
            println!("wrote {} bezels to {}", specs.len(), out.display());
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("error: {e}");
            ExitCode::FAILURE
        }
    }
}
