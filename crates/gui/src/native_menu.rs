//! Native menu bar (macOS only; see `ui/README.md`, "Native menu bar").
//!
//! The web view owns the menu's content: `ui/js/menu.js` sends a
//! [`MenuSpec`] (structure, localized labels, shortcuts, enabled state)
//! with the GUI-only command `menu_set` at start, after a UI language
//! switch (the page reloads) and whenever a project is opened or closed.
//! Clicking one of our items emits the event `menu-action` with the item
//! id; the UI runs the same action as its in-app menu. Predefined items
//! (Hide, Quit, Cut/Copy/Paste, Minimize …) are handled natively.
//!
//! Windows and Linux keep only the in-app menu: Tauri would put a native
//! menu bar inside the window there, right above our own top bar.

use serde::Deserialize;

/// Event emitted to the web view when one of our items is clicked.
pub const MENU_EVENT: &str = "menu-action";

/// The whole menu bar: submenus from left to right. On macOS the first
/// one is the application menu (its title is replaced by the app name).
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MenuSpec {
    pub menus: Vec<SubmenuSpec>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SubmenuSpec {
    pub label: String,
    /// `window` / `help`: registered as the macOS Window / Help menu
    /// (window list, help search field).
    #[serde(default)]
    pub role: Option<SubmenuRole>,
    pub items: Vec<ItemSpec>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SubmenuRole {
    Window,
    Help,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum ItemSpec {
    /// One of our actions: clicking it emits [`MENU_EVENT`] with `id`.
    Item {
        id: String,
        label: String,
        /// Shortcut in the UI's notation (`mod+shift+z`, `mod+,`).
        #[serde(default)]
        key: Option<String>,
        #[serde(default = "yes")]
        #[cfg_attr(
            not(target_os = "macos"),
            allow(dead_code, reason = "only the macOS menu bar is built")
        )]
        enabled: bool,
    },
    Separator,
    /// A native item handled by the OS.
    Predefined {
        item: Predefined,
        label: String,
    },
}

fn yes() -> bool {
    true
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Predefined {
    About,
    Services,
    Hide,
    HideOthers,
    ShowAll,
    Quit,
    Cut,
    Copy,
    Paste,
    SelectAll,
    Minimize,
    Maximize,
    Fullscreen,
    CloseWindow,
}

/// Converts the UI's shortcut notation (`mod+shift+z`, `mod+,`) to a
/// Tauri accelerator (`CmdOrCtrl+Shift+Z`, `CmdOrCtrl+Comma`).
pub fn accelerator(key: &str) -> Result<String, String> {
    let parts: Vec<&str> = key.split('+').collect();
    let (last, mods) = parts
        .split_last()
        .ok_or_else(|| "empty shortcut".to_owned())?;
    let mut out: Vec<String> = Vec::new();
    for m in mods {
        out.push(
            match *m {
                "mod" => "CmdOrCtrl",
                "shift" => "Shift",
                "alt" => "Alt",
                other => return Err(format!("unknown modifier `{other}` in `{key}`")),
            }
            .to_owned(),
        );
    }
    let code = match *last {
        "," => "Comma".to_owned(),
        "." => "Period".to_owned(),
        k if k.len() == 1 && k.chars().all(|c| c.is_ascii_alphanumeric()) => k.to_ascii_uppercase(),
        other => return Err(format!("unsupported key `{other}` in `{key}`")),
    };
    out.push(code);
    Ok(out.join("+"))
}

impl MenuSpec {
    /// Checks labels, ids and shortcuts (on every OS, so a broken spec
    /// from the UI shows up in the dev bridge tests and on Windows too).
    pub fn validate(&self) -> Result<(), String> {
        let mut ids = std::collections::BTreeSet::new();
        let mut roles = std::collections::BTreeSet::new();
        for menu in &self.menus {
            if menu.label.trim().is_empty() {
                return Err("submenu without a label".to_owned());
            }
            if let Some(role) = menu.role
                && !roles.insert(role)
            {
                return Err(format!("two submenus with the role {role:?}"));
            }
            for item in &menu.items {
                match item {
                    ItemSpec::Item { id, label, key, .. } => {
                        if id.is_empty() || label.trim().is_empty() {
                            return Err(format!("item `{id}` needs an id and a label"));
                        }
                        if !ids.insert(id.as_str()) {
                            return Err(format!("duplicate menu item id `{id}`"));
                        }
                        if let Some(key) = key {
                            accelerator(key)?;
                        }
                    }
                    ItemSpec::Predefined { label, item } => {
                        if label.trim().is_empty() {
                            return Err(format!("predefined item {item:?} without a label"));
                        }
                    }
                    ItemSpec::Separator => {}
                }
            }
        }
        Ok(())
    }
}

/// Builds the menu and installs it as the app menu bar.
#[cfg(target_os = "macos")]
pub fn install(app: &tauri::AppHandle, spec: &MenuSpec) -> tauri::Result<()> {
    use tauri::menu::{
        AboutMetadata, IsMenuItem, Menu, MenuItem, MenuItemKind, PredefinedMenuItem, Submenu,
    };

    let bar = Menu::new(app)?;
    let mut window_menu = None;
    let mut help_menu = None;
    for menu in &spec.menus {
        let sub = Submenu::new(app, &menu.label, true)?;
        for item in &menu.items {
            let built: MenuItemKind<tauri::Wry> = match item {
                ItemSpec::Item {
                    id,
                    label,
                    key,
                    enabled,
                } => {
                    let accel = key.as_deref().map(accelerator).transpose().map_err(|e| {
                        tauri::Error::Io(std::io::Error::new(std::io::ErrorKind::InvalidInput, e))
                    })?;
                    MenuItemKind::MenuItem(MenuItem::with_id(
                        app,
                        id.as_str(),
                        label,
                        *enabled,
                        accel.as_deref(),
                    )?)
                }
                ItemSpec::Separator => {
                    MenuItemKind::Predefined(PredefinedMenuItem::separator(app)?)
                }
                ItemSpec::Predefined { item, label } => {
                    let text = Some(label.as_str());
                    MenuItemKind::Predefined(match item {
                        Predefined::About => {
                            let info = app.package_info();
                            PredefinedMenuItem::about(
                                app,
                                text,
                                Some(AboutMetadata {
                                    name: Some(info.name.clone()),
                                    version: Some(info.version.to_string()),
                                    ..AboutMetadata::default()
                                }),
                            )?
                        }
                        Predefined::Services => PredefinedMenuItem::services(app, text)?,
                        Predefined::Hide => PredefinedMenuItem::hide(app, text)?,
                        Predefined::HideOthers => PredefinedMenuItem::hide_others(app, text)?,
                        Predefined::ShowAll => PredefinedMenuItem::show_all(app, text)?,
                        Predefined::Quit => PredefinedMenuItem::quit(app, text)?,
                        Predefined::Cut => PredefinedMenuItem::cut(app, text)?,
                        Predefined::Copy => PredefinedMenuItem::copy(app, text)?,
                        Predefined::Paste => PredefinedMenuItem::paste(app, text)?,
                        Predefined::SelectAll => PredefinedMenuItem::select_all(app, text)?,
                        Predefined::Minimize => PredefinedMenuItem::minimize(app, text)?,
                        Predefined::Maximize => PredefinedMenuItem::maximize(app, text)?,
                        Predefined::Fullscreen => PredefinedMenuItem::fullscreen(app, text)?,
                        Predefined::CloseWindow => PredefinedMenuItem::close_window(app, text)?,
                    })
                }
            };
            let item: &dyn IsMenuItem<tauri::Wry> = &built;
            sub.append(item)?;
        }
        bar.append(&sub)?;
        match menu.role {
            Some(SubmenuRole::Window) => window_menu = Some(sub),
            Some(SubmenuRole::Help) => help_menu = Some(sub),
            None => {}
        }
    }
    app.set_menu(bar)?;
    if let Some(sub) = window_menu {
        sub.set_as_windows_menu_for_nsapp()?;
    }
    if let Some(sub) = help_menu {
        sub.set_as_help_menu_for_nsapp()?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn converts_shortcuts() {
        assert_eq!(accelerator("mod+n").unwrap(), "CmdOrCtrl+N");
        assert_eq!(accelerator("mod+shift+z").unwrap(), "CmdOrCtrl+Shift+Z");
        assert_eq!(accelerator("mod+,").unwrap(), "CmdOrCtrl+Comma");
        assert_eq!(accelerator("alt+mod+1").unwrap(), "Alt+CmdOrCtrl+1");
        assert!(accelerator("hyper+n").is_err());
        assert!(accelerator("mod+F13").is_err());
        assert!(accelerator("").is_err());
    }

    #[test]
    fn parses_and_validates_the_ui_spec() {
        let spec: MenuSpec = serde_json::from_value(serde_json::json!({
            "menus": [
                {"label": "Arrangia", "items": [
                    {"type": "predefined", "item": "about", "label": "About Arrangia"},
                    {"type": "separator"},
                    {"type": "item", "id": "appSettings", "label": "Settings…", "key": "mod+,"},
                    {"type": "predefined", "item": "hide_others", "label": "Hide Others"},
                ]},
                {"label": "File", "items": [
                    {"type": "item", "id": "exportDialog", "label": "Export…", "key": "mod+e", "enabled": false},
                ]},
                {"label": "Window", "role": "window", "items": [
                    {"type": "predefined", "item": "minimize", "label": "Minimize"},
                ]},
            ]
        }))
        .unwrap();
        spec.validate().unwrap();
        assert_eq!(spec.menus[2].role, Some(SubmenuRole::Window));
        assert!(matches!(
            spec.menus[1].items[0],
            ItemSpec::Item { enabled: false, .. }
        ));

        let dup: MenuSpec = serde_json::from_value(serde_json::json!({"menus": [
            {"label": "File", "items": [
                {"type": "item", "id": "a", "label": "A"},
                {"type": "item", "id": "a", "label": "B"},
            ]},
        ]}))
        .unwrap();
        assert!(dup.validate().unwrap_err().contains("duplicate"));
        let bad_key: MenuSpec = serde_json::from_value(serde_json::json!({"menus": [
            {"label": "File", "items": [{"type": "item", "id": "a", "label": "A", "key": "mod+?"}]},
        ]}))
        .unwrap();
        assert!(bad_key.validate().is_err());
        assert!(
            serde_json::from_value::<MenuSpec>(serde_json::json!({"menus": [
                {"label": "File", "items": [{"type": "predefined", "item": "reload", "label": "R"}]},
            ]}))
            .is_err()
        );
    }
}
