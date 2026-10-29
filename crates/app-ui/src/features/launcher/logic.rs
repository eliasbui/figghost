//! What the Launcher pins and how the pins are kept: apps by their bundle identifier, folders and files by their
//! path, web addresses as they are written, and Shortcuts by their name, in groups that the tab and the dock flip
//! through. Nothing here launches anything or draws.

use macos_sys::launcher_ui::MAX_TILES;
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// How many things a group holds: what the tab has room for.
pub const MAX_ITEMS: usize = MAX_TILES;
/// How many groups there can be.
pub const MAX_GROUPS: usize = 8;
/// The longest name or path that is kept, in characters.
const MAX_NAME: usize = 60;
const MAX_TARGET: usize = 500;
const MAX_GROUP_NAME: usize = 24;
/// The longest address that is kept. A longer one is refused rather than cut, since a cut one is another address.
const MAX_URL: usize = 2000;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Kind {
    App,
    Folder,
    File,
    Url,
    Shortcut,
}

impl Kind {
    pub const fn code(self) -> &'static str {
        match self {
            Kind::App => "app",
            Kind::Folder => "folder",
            Kind::File => "file",
            Kind::Url => "url",
            Kind::Shortcut => "shortcut",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Item {
    pub kind: Kind,
    /// The bundle identifier of an app, the path of a folder or a file, a web address, the name of a Shortcut.
    pub target: String,
    /// What the tile says under its picture; a pin written by hand may leave it out.
    #[serde(default)]
    pub name: String,
}

/// A page of tiles, with the name its tab in the strip shows; an empty name is shown as its number.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Group {
    #[serde(default)]
    pub name: String,
    #[serde(default, deserialize_with = "lenient_items")]
    pub items: Vec<Item>,
}

fn tidy(text: &str, max: usize) -> String {
    let clean: String = text
        .chars()
        .filter(|c| !crate::features::is_hidden_char(*c))
        .collect();
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

/// The part of a web address a tile shows: the host without `www.`, or the address of a mail.
fn url_label(url: &str) -> &str {
    if let Some(rest) = url
        .get(..7)
        .filter(|scheme| scheme.eq_ignore_ascii_case("mailto:"))
        .and_then(|_| url.get(7..))
    {
        return rest.split('?').next().unwrap_or(rest);
    }
    let rest = url.split_once("://").map_or(url, |(_, rest)| rest);
    let host = rest.split(['/', '?', '#']).next().unwrap_or(rest);
    let host = host.rsplit('@').next().unwrap_or(host);
    host.strip_prefix("www.").unwrap_or(host)
}

/// Only addresses that a browser or the mail app opens are kept: any other scheme in a hand-written file could
/// start another app.
fn is_web_address(url: &str) -> bool {
    let lower = url.to_ascii_lowercase();
    let well_formed = !url.is_empty()
        && url.chars().count() <= MAX_URL
        && !url.chars().any(|c| c.is_whitespace() || c.is_control());
    let opened = match lower.split_once("://") {
        Some(("http" | "https", rest)) => !url_label(rest).is_empty() && !rest.starts_with('/'),
        _ => lower.starts_with("mailto:") && url.len() > 7,
    };
    well_formed && opened
}

/// What the page's field gave, as an address: `example.com` is read as `https://example.com`. Text that begins
/// with a scheme is left as it is, to be refused if it is not one that is opened (`localhost:3000` is a host).
pub fn url_of(text: &str) -> String {
    let text = tidy(text, MAX_URL + 1);
    let has_scheme = text.split_once(':').is_some_and(|(scheme, rest)| {
        scheme.starts_with(|c: char| c.is_ascii_alphabetic())
            && scheme
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || matches!(c, '+' | '.' | '-'))
            && !rest.starts_with(|c: char| c.is_ascii_digit())
    });
    if has_scheme {
        text
    } else if text.contains('@') && !text.contains('/') {
        format!("mailto:{text}")
    } else {
        format!("https://{text}")
    }
}

fn last_part(path: &str) -> &str {
    path.rsplit('/')
        .find(|part| !part.is_empty())
        .unwrap_or(path)
}

impl Item {
    /// An item from a file or the page can say anything: only one that can be opened is kept, with its text tidied.
    pub fn sanitized(self) -> Option<Self> {
        let target = match self.kind {
            Kind::App => self.target.trim().to_owned(),
            Kind::Folder | Kind::File => tidy(&self.target, MAX_TARGET),
            // Cut, an address would open another page, so one that is too long is refused below instead.
            Kind::Url => tidy(&self.target, MAX_URL + 1),
            // A name that begins with a hyphen would be read as an option of the command that runs it. It is kept whole,
            // since a Shortcut is run by its name and a name that was cut would run nothing, or another one.
            Kind::Shortcut => tidy(&self.target, MAX_TARGET),
        };
        let valid = match self.kind {
            Kind::App => is_bundle_id(&target),
            Kind::Folder | Kind::File => target.starts_with('/') && target.len() > 1,
            Kind::Url => is_web_address(&target),
            Kind::Shortcut => !target.is_empty() && !target.starts_with('-'),
        };
        if !valid {
            return None;
        }
        let mut name = tidy(&self.name, MAX_NAME);
        if name.is_empty() {
            name = tidy(
                match self.kind {
                    Kind::Folder | Kind::File => last_part(&target),
                    Kind::Url => url_label(&target),
                    _ => &target,
                },
                MAX_NAME,
            );
        }
        Some(Self {
            kind: self.kind,
            target,
            name,
        })
    }
}

/// The pins of a group in a file, read one by one so that a bad one leaves the others.
pub fn lenient_items<'de, D: serde::Deserializer<'de>>(
    deserializer: D,
) -> Result<Vec<Item>, D::Error> {
    let value = Value::deserialize(deserializer)?;
    Ok(match value {
        Value::Array(values) => items_of(values),
        _ => Vec::new(),
    })
}

/// The groups in a file, read one by one.
pub fn lenient_groups<'de, D: serde::Deserializer<'de>>(
    deserializer: D,
) -> Result<Vec<Group>, D::Error> {
    let value = Value::deserialize(deserializer)?;
    Ok(match value {
        Value::Array(values) => tidy_groups(
            values
                .into_iter()
                .filter_map(|value| serde_json::from_value::<Group>(value).ok()),
        ),
        _ => Vec::new(),
    })
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

/// The groups with their names and pins tidied, no more than there can be, and always at least one, so that there
/// is somewhere to pin to.
pub fn tidy_groups(groups: impl IntoIterator<Item = Group>) -> Vec<Group> {
    let mut kept: Vec<Group> = groups
        .into_iter()
        .take(MAX_GROUPS)
        .map(|group| Group {
            name: tidy(&group.name, MAX_GROUP_NAME),
            items: tidy_items(group.items),
        })
        .collect();
    if kept.is_empty() {
        kept.push(Group::default());
    }
    kept
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AddError {
    Full,
    Already,
    Unusable,
    /// There are as many groups as there can be.
    Groups,
}

/// Pins `item` at the end of group `group`.
pub fn add(groups: &mut [Group], group: usize, item: Item) -> Result<(), AddError> {
    let items = &mut groups.get_mut(group).ok_or(AddError::Unusable)?.items;
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
pub fn shift<T>(items: &mut [T], index: usize, earlier: bool) -> bool {
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

/// Adds an empty group at the end.
pub fn add_group(groups: &mut Vec<Group>) -> Result<(), AddError> {
    if groups.len() >= MAX_GROUPS {
        return Err(AddError::Groups);
    }
    groups.push(Group::default());
    Ok(())
}

/// Takes a group away with its pins; false when there is none, or it is the only one.
pub fn remove_group(groups: &mut Vec<Group>, group: usize) -> bool {
    if groups.len() > 1 && group < groups.len() {
        groups.remove(group);
        true
    } else {
        false
    }
}

/// Names a group; an empty name shows its number again. False when there is no such group.
pub fn rename_group(groups: &mut [Group], group: usize, name: &str) -> bool {
    match groups.get_mut(group) {
        Some(found) => {
            found.name = tidy(name, MAX_GROUP_NAME);
            true
        }
        None => false,
    }
}

/// The names of the entries of a folder for its menu: the visible ones, folders first, then by name as the Finder
/// sorts them roughly, no more than `most`; and whether there were more.
pub fn folder_entries(path: &std::path::Path, most: usize) -> (Vec<(String, bool)>, bool) {
    let Ok(read) = std::fs::read_dir(path) else {
        return (Vec::new(), false);
    };
    let mut entries: Vec<(String, bool)> = read
        .filter_map(Result::ok)
        .filter_map(|entry| {
            let name = entry.file_name().to_string_lossy().into_owned();
            let folder = entry.file_type().is_ok_and(|t| t.is_dir())
                && !name.ends_with(".app")
                && !name.ends_with(".bundle");
            (!name.starts_with('.')).then_some((name, folder))
        })
        .collect();
    entries.sort_by_cached_key(|(name, folder)| (!*folder, name.to_lowercase()));
    let more = entries.len() > most;
    entries.truncate(most);
    (entries, more)
}

/// The names `shortcuts list` printed, one on each line: trimmed, each once, in the order given, and no more than
/// a page of them. A name that cannot be pinned is not offered.
pub fn shortcut_names(output: &str) -> Vec<String> {
    let mut names: Vec<String> = Vec::new();
    for line in output.lines() {
        let name = tidy(line, MAX_TARGET);
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

    fn url(address: &str) -> Item {
        Item {
            kind: Kind::Url,
            target: address.into(),
            name: String::new(),
        }
    }

    fn one_group() -> Vec<Group> {
        vec![Group::default()]
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
        let file = Item {
            kind: Kind::File,
            target: "/Users/me/Plan.pdf".into(),
            name: String::new(),
        };
        assert_eq!(file.sanitized().unwrap().name, "Plan.pdf");
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
    fn only_web_and_mail_addresses_are_pinned_and_named_after_their_host() {
        for (address, name) in [
            ("https://www.github.com/me?tab=1", "github.com"),
            ("http://intranet.local:8080/", "intranet.local:8080"),
            ("HTTPS://Example.com", "Example.com"),
            ("https://user@host.vn/x", "host.vn"),
            ("mailto:me@example.com?subject=Hi", "me@example.com"),
        ] {
            assert_eq!(url(address).sanitized().expect(address).name, name);
        }
        for bad in [
            "javascript:alert(1)",
            "file:///etc/passwd",
            "x-apple.systempreferences:com.apple.preference",
            "ssh://host",
            "https://",
            "https:///path",
            "https://a b.com",
            "mailto:",
            "example.com",
        ] {
            assert!(url(bad).sanitized().is_none(), "{bad}");
        }
        let long = format!("https://example.com/{}", "a".repeat(MAX_URL));
        assert!(
            url(&long).sanitized().is_none(),
            "a long address is refused, not cut"
        );
    }

    #[test]
    fn what_is_typed_in_the_field_becomes_an_address() {
        assert_eq!(url_of(" github.com/me "), "https://github.com/me");
        assert_eq!(url_of("http://a.vn"), "http://a.vn");
        assert_eq!(url_of("me@example.com"), "mailto:me@example.com");
        assert_eq!(url_of("mailto:me@example.com"), "mailto:me@example.com");
        assert_eq!(url_of("localhost:3000/x"), "https://localhost:3000/x");
        assert!(url(&url_of("javascript:alert(1)")).sanitized().is_none());
        assert!(url(&url_of("file:///etc/passwd")).sanitized().is_none());
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
        assert_eq!(
            kept.target.chars().count(),
            300,
            "the target is what runs, so it is not cut to the tile's width"
        );
        let huge = Item {
            kind: Kind::Shortcut,
            target: "x".repeat(900),
            name: String::new(),
        };
        let kept = huge.sanitized().unwrap();
        assert_eq!(
            kept.target.chars().count(),
            MAX_TARGET,
            "but it has a limit"
        );
        assert_eq!(
            kept.name.chars().count(),
            60,
            "a name taken from the target is cut to the tile's width"
        );
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
            json!({ "kind": "url", "target": "javascript:alert(1)" }),
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
    fn there_is_always_a_group_and_never_more_than_can_be() {
        assert_eq!(tidy_groups(Vec::new()), one_group());
        let many = (0..20).map(|i| Group {
            name: format!("  Group {i}\u{202E} {}", "x".repeat(40)),
            items: vec![app("com.apple.Notes"), app("nonsense")],
        });
        let kept = tidy_groups(many);
        assert_eq!(kept.len(), MAX_GROUPS);
        assert_eq!(kept[0].name.chars().count(), MAX_GROUP_NAME);
        assert!(kept[0].name.starts_with("Group 0 x"));
        assert_eq!(kept[0].items.len(), 1, "the pins of a group are tidied too");
    }

    #[test]
    fn pinning_refuses_what_is_full_twice_or_unusable() {
        let mut groups = one_group();
        assert_eq!(add(&mut groups, 0, app("com.apple.Notes")), Ok(()));
        assert_eq!(
            add(&mut groups, 0, app("com.apple.Notes")),
            Err(AddError::Already)
        );
        assert_eq!(
            add(&mut groups, 0, app("nonsense")),
            Err(AddError::Unusable)
        );
        assert_eq!(
            add(&mut groups, 1, app("com.apple.Safari")),
            Err(AddError::Unusable),
            "there is no second group"
        );
        for i in 1..MAX_ITEMS {
            assert_eq!(
                add(&mut groups, 0, app(&format!("com.example.app{i}"))),
                Ok(())
            );
        }
        assert_eq!(groups[0].items.len(), MAX_ITEMS);
        assert_eq!(
            add(&mut groups, 0, app("com.example.one.more")),
            Err(AddError::Full)
        );
        // The same folder and app are different pins; the same name as an app is a Shortcut.
        let mut mixed = vec![Group {
            name: String::new(),
            items: vec![app("com.apple.Notes")],
        }];
        let same_text = Item {
            kind: Kind::Shortcut,
            target: "com.apple.Notes".into(),
            name: String::new(),
        };
        assert_eq!(add(&mut mixed, 0, same_text), Ok(()));
    }

    #[test]
    fn groups_are_added_named_and_removed_but_one_always_stays() {
        let mut groups = one_group();
        assert!(!remove_group(&mut groups, 0), "the only group stays");
        for _ in 1..MAX_GROUPS {
            assert_eq!(add_group(&mut groups), Ok(()));
        }
        assert_eq!(add_group(&mut groups), Err(AddError::Groups));
        assert!(rename_group(&mut groups, 2, "  Work\u{7} "));
        assert_eq!(groups[2].name, "Work");
        assert!(!rename_group(&mut groups, 99, "x"));
        assert!(shift(&mut groups, 2, true));
        assert_eq!(groups[1].name, "Work");
        assert!(remove_group(&mut groups, 1));
        assert!(groups.iter().all(|g| g.name.is_empty()));
        assert!(!remove_group(&mut groups, 99));
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
    fn a_folder_menu_lists_the_visible_entries_folders_first_up_to_a_limit() {
        let dir = std::env::temp_dir().join(format!("figghost-launcher-{}", std::process::id()));
        struct Gone(std::path::PathBuf);
        impl Drop for Gone {
            fn drop(&mut self) {
                let _ = std::fs::remove_dir_all(&self.0);
            }
        }
        let _gone = Gone(dir.clone());
        std::fs::create_dir_all(dir.join("beta")).unwrap();
        std::fs::create_dir_all(dir.join("Tool.app")).unwrap();
        for name in ["b.txt", "A.txt", ".hidden"] {
            std::fs::write(dir.join(name), "").unwrap();
        }
        let (entries, more) = folder_entries(&dir, 10);
        assert_eq!(
            entries,
            [
                ("beta".to_owned(), true),
                ("A.txt".to_owned(), false),
                ("b.txt".to_owned(), false),
                ("Tool.app".to_owned(), false),
            ]
        );
        assert!(!more);
        let (entries, more) = folder_entries(&dir, 2);
        assert_eq!(entries.len(), 2);
        assert!(more);
        assert_eq!(
            folder_entries(&dir.join("missing"), 10),
            (Vec::new(), false)
        );
    }

    #[test]
    fn a_shortcut_with_a_long_name_is_kept_whole_so_that_it_can_still_run() {
        let long = "A".repeat(80);
        let item = Item {
            kind: Kind::Shortcut,
            target: long.clone(),
            name: String::new(),
        }
        .sanitized()
        .unwrap();
        assert_eq!(
            item.target, long,
            "a name that was cut would run nothing, or another Shortcut"
        );
        assert_eq!(
            item.name.chars().count(),
            MAX_NAME,
            "only what the tile says is cut"
        );
        assert_eq!(shortcut_names(&format!("{long}\n")), [long]);
    }

    #[test]
    fn what_would_make_a_name_read_as_another_is_taken_out_of_it() {
        let item = Item {
            kind: Kind::Shortcut,
            target: "Pay\u{202E}fdp.exe".into(),
            name: "Pay\u{2066}ment".into(),
        }
        .sanitized()
        .unwrap();
        assert_eq!(
            (item.target.as_str(), item.name.as_str()),
            ("Payfdp.exe", "Payment")
        );
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
