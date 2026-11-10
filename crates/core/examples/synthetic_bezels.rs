//! Writes the synthetic bezel package (generated PNGs named like Apple's
//! Product Bezels, never vendor artwork) for trying out the frame import:
//!
//! ```sh
//! cargo run -p arrangia-core --example synthetic_bezels -- <dir>          # folder
//! cargo run -p arrangia-core --example synthetic_bezels -- <file.zip>     # zip
//! ```

use std::path::PathBuf;
use std::process::ExitCode;

use arrangia_core::frame_import::synthetic;

fn main() -> ExitCode {
    let Some(out) = std::env::args_os().nth(1).map(PathBuf::from) else {
        eprintln!("usage: synthetic_bezels <dir | file.zip>");
        return ExitCode::from(2);
    };
    let specs = synthetic::default_specs();
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
