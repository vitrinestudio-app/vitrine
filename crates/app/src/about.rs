//! Copyright and licence notice shown by the front ends (CLI `--help`,
//! native About panel). See `LICENSE`, `CLA.md` and ADR 0022.

/// Copyright holder of Vitrine Studio.
pub const COPYRIGHT_HOLDER: &str = "Looking4Cache UG (haftungsbeschränkt)";

/// Copyright line (About panel).
pub const COPYRIGHT: &str = "Copyright © 2026 Looking4Cache UG (haftungsbeschränkt)";

/// SPDX licence expression of Vitrine Studio's own code.
pub const LICENSE: &str = "GPL-3.0-or-later";

/// Project website (About panel, help footer).
pub const WEBSITE: &str = "https://www.vitrinestudio.app";

/// Source code repository (must match `repository` in Cargo.toml).
pub const SOURCE: &str = "https://github.com/Looking4Cache/arrangia";

/// Short notice (CLI help footer).
pub const NOTICE: &str = "Copyright © 2026 Looking4Cache UG (haftungsbeschränkt).\n\
Vitrine Studio is free software, licensed under the GNU GPL v3 or later\n\
(GPL-3.0-or-later), and comes with ABSOLUTELY NO WARRANTY.\n\
Source and licence: https://github.com/Looking4Cache/arrangia";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn notice_names_holder_licence_and_source() {
        assert!(COPYRIGHT.contains(COPYRIGHT_HOLDER));
        assert!(NOTICE.contains(COPYRIGHT_HOLDER));
        assert!(NOTICE.contains(LICENSE));
        assert!(NOTICE.contains(SOURCE));
        assert_eq!(env!("CARGO_PKG_REPOSITORY"), SOURCE);
        assert_eq!(env!("CARGO_PKG_LICENSE"), LICENSE);
    }
}
