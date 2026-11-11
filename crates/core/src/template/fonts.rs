//! Fonts in templates: only fonts that declare the SIL Open Font License in
//! their name table (ID 13 licence description or ID 14 licence URL) may be
//! shipped. The licence file is kept if the project has one, else it is
//! generated from the font: the copyright line (name ID 0, plus the
//! Reserved Font Name if the font states one) and the standard OFL 1.1
//! text (user decision 2026-11-10).

use skrifa::{FontRef, MetadataProvider, string::StringId};

use super::TemplateError;

/// The bundled OFL text (`assets/fonts/OFL.txt`, Inter's licence).
const BUNDLED_OFL: &str = include_str!("../../../../assets/fonts/OFL.txt");

/// What a font's name table says about it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FontNames {
    /// Family name (typographic family, else name ID 1).
    pub family: String,
    /// Name ID 0.
    pub copyright: String,
    /// Name ID 13.
    pub license: String,
    /// Name ID 14.
    pub license_url: String,
}

impl FontNames {
    /// Does the font declare the OFL?
    pub fn declares_ofl(&self) -> bool {
        mentions_ofl(&self.license) || mentions_ofl(&self.license_url)
    }

    /// Reserved Font Names stated in the copyright or licence text
    /// (`with Reserved Font Name "Lora"` → `["Lora"]`).
    pub fn reserved_names(&self) -> Vec<String> {
        let mut out = Vec::new();
        for text in [&self.copyright, &self.license] {
            let lower = text.to_ascii_lowercase();
            let Some(i) = lower.find("reserved font name") else {
                continue;
            };
            let rest = &text[i + "reserved font name".len()..];
            let rest = rest.trim_start_matches(['s', 'S']);
            for part in rest.split(['"', '“', '”', '\'']).skip(1).step_by(2) {
                let name = part.trim();
                if !name.is_empty() && name.len() < 80 && !out.iter().any(|n| n == name) {
                    out.push(name.to_owned());
                }
            }
        }
        out
    }
}

fn mentions_ofl(s: &str) -> bool {
    let l = s.to_ascii_lowercase();
    l.contains("open font license")
        || l.contains("openfontlicense.org")
        || l.contains("scripts.sil.org/ofl")
        || l.split(|c: char| !c.is_ascii_alphanumeric())
            .any(|w| w == "ofl")
}

/// Reads the names of the first face of a font file.
pub fn font_names(data: &[u8]) -> Result<FontNames, String> {
    let font = FontRef::from_index(data, 0).map_err(|e| format!("not a font: {e}"))?;
    let get = |id: StringId| {
        font.localized_strings(id)
            .english_or_first()
            .map(|s| s.chars().collect::<String>().trim().to_owned())
            .unwrap_or_default()
    };
    let family = Some(get(StringId::TYPOGRAPHIC_FAMILY_NAME))
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| get(StringId::FAMILY_NAME));
    Ok(FontNames {
        family,
        copyright: get(StringId::COPYRIGHT_NOTICE),
        license: get(StringId::LICENSE_DESCRIPTION),
        license_url: get(StringId::LICENSE_URL),
    })
}

/// Checks that a font may go into a template; returns its names.
pub fn check_ofl(file: &str, data: &[u8]) -> Result<FontNames, TemplateError> {
    let names = font_names(data).map_err(|reason| TemplateError::FontLicense {
        file: file.to_owned(),
        reason,
    })?;
    if !names.declares_ofl() {
        let reason = if names.license.is_empty() && names.license_url.is_empty() {
            "the font states no licence, so it may not be redistributed (licence unclear)"
                .to_owned()
        } else {
            format!(
                "the font's licence is not the OFL (\"{}\"), so it may not be redistributed",
                shorten(if names.license.is_empty() {
                    &names.license_url
                } else {
                    &names.license
                })
            )
        };
        return Err(TemplateError::FontLicense {
            file: file.to_owned(),
            reason,
        });
    }
    Ok(names)
}

fn shorten(s: &str) -> String {
    let mut chars = s.chars();
    let head: String = chars.by_ref().take(80).collect();
    if chars.next().is_some() {
        format!("{head}…")
    } else {
        head
    }
}

/// The OFL 1.1 text without a copyright header (from "This Font Software
/// is licensed under …" to the end).
pub fn ofl_body() -> &'static str {
    let start = BUNDLED_OFL
        .find("This Font Software is licensed")
        .unwrap_or(0);
    &BUNDLED_OFL[start..]
}

/// Is `text` the OFL 1.1 licence (a kept or generated licence file)?
pub fn is_ofl_text(text: &str) -> bool {
    text.contains("SIL OPEN FONT LICENSE Version 1.1")
        || text.contains("SIL Open Font License, Version 1.1")
}

/// A licence file for a font: its copyright line(s), the Reserved Font
/// Name if stated, and the OFL 1.1 text.
pub fn generate_license(names: &FontNames) -> String {
    let mut head = if names.copyright.is_empty() {
        format!("Copyright (c) the authors of {}", names.family)
    } else {
        names.copyright.clone()
    };
    let reserved = names.reserved_names();
    if !reserved.is_empty()
        && !names
            .copyright
            .to_ascii_lowercase()
            .contains("reserved font name")
    {
        let list: Vec<String> = reserved.iter().map(|n| format!("\"{n}\"")).collect();
        head.push_str(", with Reserved Font Name ");
        head.push_str(&list.join(", "));
        head.push('.');
    }
    format!("{head}\n\n{}", ofl_body())
}

/// Package path of the licence file of `fonts/<stem>.<ext>`.
pub fn license_path(font_path: &str) -> String {
    let stem = font_path.rsplit_once('.').map_or(font_path, |(s, _)| s);
    format!("{stem}-OFL.txt")
}

#[cfg(test)]
mod tests {
    use super::*;

    const INTER: &[u8] = include_bytes!("../../../../assets/fonts/Inter-Regular.ttf");

    #[test]
    fn bundled_inter_declares_ofl() {
        let n = check_ofl("fonts/Inter-Regular.ttf", INTER).unwrap();
        assert_eq!(n.family, "Inter");
        assert!(n.copyright.contains("Inter"), "{}", n.copyright);
        let text = generate_license(&n);
        assert!(text.starts_with(&n.copyright));
        assert!(is_ofl_text(&text));
        assert!(text.contains("PREAMBLE"));
    }

    #[test]
    fn reserved_names_and_detection() {
        let n = FontNames {
            family: "Lora".into(),
            copyright: "Copyright 2011 The Lora Project Authors".into(),
            license: "This Font Software is licensed under the SIL Open Font License, Version 1.1, with Reserved Font Name \"Lora\".".into(),
            license_url: String::new(),
        };
        assert!(n.declares_ofl());
        assert_eq!(n.reserved_names(), ["Lora"]);
        assert!(generate_license(&n).starts_with(
            "Copyright 2011 The Lora Project Authors, with Reserved Font Name \"Lora\"."
        ));
        let commercial = FontNames {
            license: "All rights reserved. Licensed for desktop use only.".into(),
            ..n.clone()
        };
        assert!(!commercial.declares_ofl());
        let url = FontNames {
            license: String::new(),
            license_url: "https://openfontlicense.org".into(),
            ..n
        };
        assert!(url.declares_ofl());
        assert!(!mentions_ofl("Software"));
        assert!(mentions_ofl("SIL OFL 1.1"));
    }

    #[test]
    fn license_file_names() {
        assert_eq!(
            license_path("fonts/Lora-Bold.ttf"),
            "fonts/Lora-Bold-OFL.txt"
        );
        assert!(check_ofl("fonts/x.ttf", b"not a font").is_err());
    }
}
