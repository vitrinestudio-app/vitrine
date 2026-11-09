//! Colours: literal `#RRGGBB[AA]` values and `@name` references to the
//! project palette (`styles.colors`).

use std::fmt;

use super::fmt::string_serde;
use super::ids::is_valid_id;

/// A literal sRGB colour with alpha. Written as `#RRGGBB` (opaque) or
/// `#RRGGBBAA` (upper-case hex).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Rgba {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}

impl Rgba {
    pub const fn rgb(r: u8, g: u8, b: u8) -> Self {
        Self { r, g, b, a: 255 }
    }

    /// Parse `#RGB`, `#RRGGBB` or `#RRGGBBAA` (case-insensitive).
    pub fn parse(s: &str) -> Result<Self, String> {
        let err = || format!("invalid colour `{s}`: expected `#RRGGBB`, `#RRGGBBAA` or `@name`");
        let hex = s.strip_prefix('#').ok_or_else(err)?;
        if !hex.chars().all(|c| c.is_ascii_hexdigit()) {
            return Err(err());
        }
        let byte = |i: usize| u8::from_str_radix(&hex[i..i + 2], 16).map_err(|_| err());
        match hex.len() {
            3 => {
                let n = |i: usize| {
                    u8::from_str_radix(&hex[i..=i], 16)
                        .map(|v| v * 17)
                        .map_err(|_| err())
                };
                Ok(Self::rgb(n(0)?, n(1)?, n(2)?))
            }
            6 => Ok(Self::rgb(byte(0)?, byte(2)?, byte(4)?)),
            8 => Ok(Self {
                r: byte(0)?,
                g: byte(2)?,
                b: byte(4)?,
                a: byte(6)?,
            }),
            _ => Err(err()),
        }
    }
}

impl fmt::Display for Rgba {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "#{:02X}{:02X}{:02X}", self.r, self.g, self.b)?;
        if self.a != 255 {
            write!(f, "{:02X}", self.a)?;
        }
        Ok(())
    }
}

string_serde!(Rgba, "a colour `#RRGGBB` or `#RRGGBBAA`");

/// A colour value in the model: literal or a reference to a named palette
/// colour (`@primary`).
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum ColorRef {
    Rgba(Rgba),
    Named(String),
}

impl ColorRef {
    pub fn parse(s: &str) -> Result<Self, String> {
        if let Some(name) = s.strip_prefix('@') {
            if is_valid_id(name) {
                Ok(Self::Named(name.to_owned()))
            } else {
                Err(format!("invalid colour reference `{s}`"))
            }
        } else {
            Rgba::parse(s).map(Self::Rgba)
        }
    }

    /// Name of the referenced palette colour, if any.
    pub fn name(&self) -> Option<&str> {
        match self {
            Self::Named(n) => Some(n),
            Self::Rgba(_) => None,
        }
    }
}

impl fmt::Display for ColorRef {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Rgba(c) => c.fmt(f),
            Self::Named(n) => write!(f, "@{n}"),
        }
    }
}

string_serde!(ColorRef, "a colour `#RRGGBB`, `#RRGGBBAA` or `@name`");

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_and_format() {
        assert_eq!(Rgba::parse("#2f6b3a").unwrap().to_string(), "#2F6B3A");
        assert_eq!(Rgba::parse("#fff").unwrap(), Rgba::rgb(255, 255, 255));
        assert_eq!(Rgba::parse("#00000080").unwrap().to_string(), "#00000080");
        assert!(Rgba::parse("red").is_err());
        assert!(Rgba::parse("#12345").is_err());
        assert_eq!(
            ColorRef::parse("@accent").unwrap(),
            ColorRef::Named("accent".into())
        );
        assert!(ColorRef::parse("@").is_err());
    }
}
