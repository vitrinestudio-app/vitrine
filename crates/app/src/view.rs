//! What the user is looking at in the GUI (ADR 0013): strip, target,
//! language, variant and selection. The GUI pushes it into the session
//! (`view_set`, debounced) whenever its view context or selection changes;
//! other front ends (MCP: `view_get`, `selection_get`) read it, so an
//! assistant can work on what the user sees ("make this bigger").

use serde::{Deserialize, Deserializer, Serialize};

/// The GUI's current view. All fields are optional; the session only
/// stores what the GUI sends (no validation against the project).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
#[allow(clippy::option_option)]
pub struct ViewState {
    /// Current strip id.
    #[serde(default)]
    pub strip: Option<String>,
    /// Current target id.
    #[serde(default)]
    pub target: Option<String>,
    /// Current content language.
    #[serde(default)]
    pub lang: Option<String>,
    /// As in the view context of the edit commands: missing = the
    /// target's own variant, `null` = the base elements, a name = that
    /// variant of the strip.
    #[serde(
        default,
        deserialize_with = "double_option",
        skip_serializing_if = "Option::is_none"
    )]
    pub variant: Option<Option<String>>,
    /// Selected element ids (groups by their id).
    #[serde(default)]
    pub selection: Vec<String>,
}

/// Tells a missing field (`None`) from an explicit `null` (`Some(None)`).
#[allow(clippy::option_option)]
fn double_option<'de, D, T>(d: D) -> Result<Option<Option<T>>, D::Error>
where
    D: Deserializer<'de>,
    T: Deserialize<'de>,
{
    Option::<T>::deserialize(d).map(Some)
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::ViewState;

    #[test]
    fn variant_keeps_missing_and_null_apart() {
        let own: ViewState = serde_json::from_value(json!({"target": "t"})).unwrap();
        assert_eq!(own.variant, None);
        let base: ViewState = serde_json::from_value(json!({"variant": null})).unwrap();
        assert_eq!(base.variant, Some(None));
        assert_eq!(serde_json::to_value(&base).unwrap()["variant"], json!(null));
        assert!(serde_json::to_value(&own).unwrap().get("variant").is_none());
    }
}
