//! What the Launcher tab pins and how a list of pins is kept: apps by their bundle identifier, folders by their path,
//! and Shortcuts by their name. Nothing here launches anything or draws.

use macos_sys::launcher_ui::MAX_TILES;
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// How many things can be pinned: what the tab has room for.
pub const MAX_ITEMS: usize = MAX_TILES;
/// The longest name or path that is kept, in characters.
const MAX_NAME: usize = 60;
const MAX_TARGET: usize = 500;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Kind {
    App,
    Folder,
    Shortcut,
}

impl Kind {
    pub const fn code(self) -> &'static str {
        match self {
            Kind::App => "app",
            Kind::Folder => "folder",
            Kind::Shortcut => "shortcut",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Item {
    pub kind: Kind,
    /// The bundle identifier of an app, the path of a folder, the name of a Shortcut.
    pub target: String,
    /// What the tile says under its picture; a pin written by hand may leave it out.
    #[serde(default)]
    pub name: String,
}

fn tidy(text: &str, max: usize) -> String {
    let clean: String = text.chars().filter(|c| !c.is_control()).collect();
    clean.trim().chars().take(max).collect()
}

/// A bundle identifier is dotted words of letters, digits, hyphens and underscores.
fn is_bundle_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 200
        && id.contains('.')
        && id
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'-' | b'_'))
}

impl Item {
    /// An item from a file or the page can say anything: only one that can be opened is kept, with its text tidied.
    pub fn sanitized(self) -> Option<Self> {
        let target = match self.kind {
            Kind::App => self.target.trim().to_owned(),
            Kind::Folder => tidy(&self.target, MAX_TARGET),
            // A name that begins with a hyphen would be read as an option of the command that runs it.
            Kind::Shortcut => tidy(&self.target, MAX_NAME),
        };
        let valid = match self.kind {
            Kind::App => is_bundle_id(&target),
            Kind::Folder => target.starts_with('/') && target.len() > 1,
            Kind::Shortcut => !target.is_empty() && !target.starts_with('-'),
        };
        if !valid {
            return None;
        }
        let mut name = tidy(&self.name, MAX_NAME);
        if name.is_empty() {
            name = match self.kind {
                Kind::Folder => target
                    .rsplit('/')
                    .find(|part| !part.is_empty())
                    .unwrap_or(&target)
                    .to_owned(),
                _ => target.clone(),
            };
        }
        Some(Self {
            kind: self.kind,
            target,
            name,
        })
    }
}

/// The pins in a file: each one that is usable, none twice, no more than the tab has room for. One that is not
/// usable, or not even an object, is left out and the rest stay.
pub fn items_of(values: Vec<Value>) -> Vec<Item> {
    tidy_items(
        values
            .into_iter()
            .filter_map(|value| serde_json::from_value::<Item>(value).ok()),
    )
}

/// The pins that are usable, none twice, no more than the tab has room for.
pub fn tidy_items(items: impl IntoIterator<Item = Item>) -> Vec<Item> {
    let mut kept: Vec<Item> = Vec::new();
    for item in items.into_iter().filter_map(Item::sanitized) {
        if !kept
            .iter()
            .any(|i| i.kind == item.kind && i.target == item.target)
        {
            kept.push(item);
        }
    }
    kept.truncate(MAX_ITEMS);
    kept
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AddError {
    Full,
    Already,
    Unusable,
}

/// Pins `item` at the end of the list.
pub fn add(items: &mut Vec<Item>, item: Item) -> Result<(), AddError> {
    let item = item.sanitized().ok_or(AddError::Unusable)?;
    if items
        .iter()
        .any(|i| i.kind == item.kind && i.target == item.target)
    {
        return Err(AddError::Already);
    }
    if items.len() >= MAX_ITEMS {
        return Err(AddError::Full);
    }
    items.push(item);
    Ok(())
}

/// Takes the pin at `index` away; false when there is none.
pub fn remove(items: &mut Vec<Item>, index: usize) -> bool {
    if index < items.len() {
        items.remove(index);
        true
    } else {
        false
    }
}

/// Moves the pin at `index` one place earlier (`earlier`) or later; false when it cannot move.
pub fn shift(items: &mut [Item], index: usize, earlier: bool) -> bool {
    let to = if earlier {
        match index.checked_sub(1) {
            Some(to) => to,
            None => return false,
        }
    } else {
        index + 1
    };
    if index >= items.len() || to >= items.len() {
        return false;
    }
    items.swap(index, to);
    true
}

/// The names `shortcuts list` printed, one on each line: trimmed, each once, in the order given, and no more than
/// a page of them. A name that cannot be pinned is not offered.
pub fn shortcut_names(output: &str) -> Vec<String> {
    let mut names: Vec<String> = Vec::new();
    for line in output.lines() {
        let name = tidy(line, MAX_NAME);
        if !name.is_empty() && !name.starts_with('-') && !names.contains(&name) {
            names.push(name);
        }
        if names.len() >= 200 {
            break;
        }
    }
    names
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn app(id: &str) -> Item {
        Item {
            kind: Kind::App,
            target: id.into(),
            name: String::new(),
        }
    }

    #[test]
    fn what_can_be_opened_is_kept_and_the_rest_is_not() {
        assert!(app("com.apple.Safari").sanitized().is_some());
        assert!(app("com.apple.Safari").sanitized().unwrap().name == "com.apple.Safari");
        for bad in [
            "",
            "Safari",
            "com apple",
            "com.apple/Safari",
            &"a.".repeat(200),
        ] {
            assert!(app(bad).sanitized().is_none(), "{bad:?}");
        }
        assert_eq!(
            app("com.apple.Safari\n").sanitized().unwrap().target,
            "com.apple.Safari",
            "a trailing newline is trimmed"
        );
        let folder = |path: &str| Item {
            kind: Kind::Folder,
            target: path.into(),
            name: String::new(),
        };
        assert_eq!(
            folder("/Users/me/Documents").sanitized().unwrap().name,
            "Documents"
        );
        assert_eq!(
            folder("/Users/me/Documents/").sanitized().unwrap().name,
            "Documents"
        );
        assert!(
            folder("Documents").sanitized().is_none(),
            "a folder is a full path"
        );
        assert!(folder("/").sanitized().is_none());
        let shortcut = |name: &str| Item {
            kind: Kind::Shortcut,
            target: name.into(),
            name: String::new(),
        };
        assert!(shortcut("Morning routine").sanitized().is_some());
        assert!(
            shortcut("-h").sanitized().is_none(),
            "a name is never read as an option"
        );
        assert!(shortcut("  ").sanitized().is_none());
        assert_eq!(
            shortcut("Bad\u{7}Name").sanitized().unwrap().target,
            "BadName"
        );
    }

    #[test]
    fn a_name_is_cut_and_a_missing_one_is_taken_from_the_target() {
        let long = Item {
            kind: Kind::Shortcut,
            target: "x".repeat(300),
            name: "y".repeat(300),
        };
        let kept = long.sanitized().unwrap();
        assert_eq!(kept.name.chars().count(), 60);
        assert_eq!(kept.target.chars().count(), 60);
    }

    #[test]
    fn a_file_with_some_bad_pins_keeps_the_good_ones_once_each() {
        let items = items_of(vec![
            json!({ "kind": "app", "target": "com.apple.Notes", "name": "Notes" }),
            json!({ "kind": "app", "target": "com.apple.Notes", "name": "Again" }),
            json!({ "kind": "robot", "target": "x" }),
            json!("a string"),
            json!(null),
            json!({ "kind": "folder", "target": "relative/path" }),
            json!({ "kind": "folder", "target": "/Users/me", "name": "" }),
            json!({ "kind": "shortcut", "target": "Morning routine" }),
            json!({ "kind": "app" }),
        ]);
        assert_eq!(
            items
                .iter()
                .map(|i| (i.kind.code(), i.name.as_str()))
                .collect::<Vec<_>>(),
            [
                ("app", "Notes"),
                ("folder", "me"),
                ("shortcut", "Morning routine")
            ]
        );
        let many: Vec<Value> = (0..40)
            .map(|i| json!({ "kind": "app", "target": format!("com.example.app{i}") }))
            .collect();
        assert_eq!(items_of(many).len(), MAX_ITEMS);
    }

    #[test]
    fn pinning_refuses_what_is_full_twice_or_unusable() {
        let mut items = Vec::new();
        assert_eq!(add(&mut items, app("com.apple.Notes")), Ok(()));
        assert_eq!(
            add(&mut items, app("com.apple.Notes")),
            Err(AddError::Already)
        );
        assert_eq!(add(&mut items, app("nonsense")), Err(AddError::Unusable));
        for i in 1..MAX_ITEMS {
            assert_eq!(add(&mut items, app(&format!("com.example.app{i}"))), Ok(()));
        }
        assert_eq!(items.len(), MAX_ITEMS);
        assert_eq!(
            add(&mut items, app("com.example.one.more")),
            Err(AddError::Full)
        );
        // The same folder and app are different pins; the same name as an app is a Shortcut.
        let mut mixed = vec![app("com.apple.Notes")];
        let same_text = Item {
            kind: Kind::Shortcut,
            target: "com.apple.Notes".into(),
            name: String::new(),
        };
        assert_eq!(add(&mut mixed, same_text), Ok(()));
    }

    #[test]
    fn a_pin_is_removed_and_moved_by_its_number() {
        let mut items: Vec<Item> = (0..3).map(|i| app(&format!("com.example.a{i}"))).collect();
        assert!(shift(&mut items, 2, true));
        assert_eq!(items[1].target, "com.example.a2");
        assert!(!shift(&mut items, 0, true), "the first cannot go earlier");
        assert!(!shift(&mut items, 2, false), "the last cannot go later");
        assert!(!shift(&mut items, 9, true) && !shift(&mut items, 9, false));
        assert!(shift(&mut items, 0, false));
        assert_eq!(items[0].target, "com.example.a2");
        assert!(remove(&mut items, 0));
        assert_eq!(items.len(), 2);
        assert!(!remove(&mut items, 5));
    }

    #[test]
    fn the_names_of_the_shortcuts_are_read_from_the_lines_of_the_output() {
        let output = "Morning routine\n  Resize images  \n\nMorning routine\n-help\nBad\u{7}Bell\n";
        assert_eq!(
            shortcut_names(output),
            ["Morning routine", "Resize images", "BadBell"]
        );
        assert!(shortcut_names("").is_empty());
        let many: String = (0..500).map(|i| format!("Shortcut {i}\n")).collect();
        assert_eq!(shortcut_names(&many).len(), 200);
    }
}
