//! Identifier newtypes.
//!
//! All ids are plain strings in the YAML file. They are validated when parsed:
//! ASCII letters, digits, `-`, `_` and `.`, starting with a letter or digit.

use std::fmt;

use super::fmt::string_serde;

/// Is `s` a valid identifier (canvas, element, target, variant, style name …)?
pub fn is_valid_id(s: &str) -> bool {
    let mut chars = s.chars();
    matches!(chars.next(), Some(c) if c.is_ascii_alphanumeric())
        && chars.all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'))
}

macro_rules! id_type {
    ($(#[$meta:meta])* $name:ident, $what:literal) => {
        $(#[$meta])*
        #[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
        pub struct $name(String);

        impl $name {
            /// Parse and validate an id.
            pub fn parse(s: &str) -> Result<Self, String> {
                if is_valid_id(s) {
                    Ok(Self(s.to_owned()))
                } else {
                    Err(format!(
                        concat!("invalid ", $what, " `{}`: use letters, digits, `-`, `_`, `.`; start with a letter or digit"),
                        s
                    ))
                }
            }

            /// Create an id, panicking if it is invalid. For literals in code and tests.
            pub fn new(s: &str) -> Self {
                Self::parse(s).unwrap_or_else(|e| panic!("{e}"))
            }

            pub fn as_str(&self) -> &str {
                &self.0
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str(&self.0)
            }
        }

        impl AsRef<str> for $name {
            fn as_ref(&self) -> &str {
                &self.0
            }
        }

        impl std::borrow::Borrow<str> for $name {
            fn borrow(&self) -> &str {
                &self.0
            }
        }

        string_serde!($name, concat!("a ", $what));
    };
}

id_type!(
    /// Id of a canvas.
    CanvasId,
    "canvas id"
);
id_type!(
    /// Id of an element; unique across the whole project.
    ElementId,
    "element id"
);
id_type!(
    /// Id of an output target.
    TargetId,
    "target id"
);
id_type!(
    /// Name of a variant of a canvas.
    VariantId,
    "variant id"
);
id_type!(
    /// Language code (`en`, `de`, `pt-BR` …).
    Lang,
    "language code"
);

/// Translation key (`hero.title`). Keys are flat: dots are part of the key,
/// not nesting.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct StringKey(String);

impl StringKey {
    pub fn parse(s: &str) -> Result<Self, String> {
        if s.is_empty() || s.chars().any(char::is_whitespace) {
            Err(format!(
                "invalid translation key `{s}`: must be non-empty without whitespace"
            ))
        } else {
            Ok(Self(s.to_owned()))
        }
    }

    pub fn new(s: &str) -> Self {
        Self::parse(s).unwrap_or_else(|e| panic!("{e}"))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for StringKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::borrow::Borrow<str> for StringKey {
    fn borrow(&self) -> &str {
        &self.0
    }
}

string_serde!(StringKey, "a translation key");

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn id_validation() {
        assert!(is_valid_id("hero-phone"));
        assert!(is_valid_id("pt-BR"));
        assert!(is_valid_id("ios_phone.69"));
        assert!(!is_valid_id(""));
        assert!(!is_valid_id("-x"));
        assert!(!is_valid_id("a b"));
        assert!(!is_valid_id("@x"));
        assert!(CanvasId::parse("a/b").is_err());
    }

    #[test]
    fn keys_may_contain_dots() {
        assert_eq!(StringKey::new("hero.title").as_str(), "hero.title");
        assert!(StringKey::parse("a b").is_err());
    }
}
