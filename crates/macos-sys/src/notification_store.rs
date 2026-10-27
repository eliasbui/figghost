//! The notifications macOS keeps in Notification Center, as its own store has them: the app that sent each one, when,
//! and its title, subtitle and body. The store is a SQLite file of the system's notification service in a group
//! container that only an app with Full Disk Access may read (since macOS 15). This module knows where the store is and
//! how the data of one record is laid out; the app reads the store, and nothing ever writes to it.

use core_foundation::{
    base::{CFType, CFTypeRef, TCFType},
    data::CFData,
    dictionary::CFDictionary,
    number::CFNumber,
    propertylist::{CFPropertyList, create_with_data, kCFPropertyListImmutable},
    string::CFString,
};
use std::{ffi::c_void, path::PathBuf};

/// The pane of System Settings that grants Full Disk Access.
pub const FULL_DISK_PANE: &str =
    "x-apple.systempreferences:com.apple.preference.security?Privacy_AllFiles";
/// The pane of System Settings with the notification style of each app.
pub const NOTIFICATIONS_PANE: &str =
    "x-apple.systempreferences:com.apple.Notifications-Settings.extension";

/// Seconds from 1970 to 2001, where the dates of the store count from.
const FROM_2001: f64 = 978_307_200.0;

/// Where the store of this user is.
pub fn store_path() -> Option<PathBuf> {
    let home = std::env::var_os("HOME")?;
    Some(PathBuf::from(home).join("Library/Group Containers/group.com.apple.usernoted/db2/db"))
}

/// What one record says.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Fields {
    /// The bundle identifier of the app that sent it.
    pub app: String,
    /// When, in seconds since 1970.
    pub date: f64,
    pub title: String,
    pub subtitle: String,
    pub body: String,
}

fn value(dict: &CFDictionary, key: &str) -> Option<CFType> {
    let key = CFString::new(key);
    let found = dict.find(key.as_concrete_TypeRef().cast::<c_void>())?;
    // SAFETY: the values of a property list are CF objects; the get rule leaves ownership with the dictionary.
    Some(unsafe { CFType::wrap_under_get_rule(*found as CFTypeRef) })
}

fn text(dict: &CFDictionary, key: &str) -> String {
    value(dict, key)
        .and_then(|v| v.downcast::<CFString>())
        .map(|s| s.to_string())
        .unwrap_or_default()
}

/// The fields of the property list of one record; `None` for data that is not one, or that names no app.
pub fn decode(data: &[u8]) -> Option<Fields> {
    let (raw, _) = create_with_data(CFData::from_buffer(data), kCFPropertyListImmutable).ok()?;
    // SAFETY: the create rule: the property list made above is ours.
    let root: CFDictionary =
        unsafe { CFPropertyList::wrap_under_create_rule(raw.cast()) }.downcast_into()?;
    let app = text(&root, "app");
    if app.is_empty() {
        return None;
    }
    let date = value(&root, "date")
        .and_then(|v| v.downcast::<CFNumber>())
        .and_then(|n| n.to_f64())
        .map_or(0.0, |d| d + FROM_2001);
    let request: Option<CFDictionary> = value(&root, "req").and_then(|v| v.downcast());
    let field = |key: &str| request.as_ref().map(|r| text(r, key)).unwrap_or_default();
    Some(Fields {
        app,
        date,
        title: field("titl"),
        subtitle: field("subt"),
        body: field("body"),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A record as the store keeps it, in the XML form of a property list, which reads the same as the binary one.
    const RECORD: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0"><dict>
  <key>app</key><string>com.apple.ScriptEditor2</string>
  <key>date</key><real>814766740.5</real>
  <key>req</key><dict>
    <key>titl</key><string>Họp lúc 3 giờ</string>
    <key>subt</key><string>Phòng 2</string>
    <key>body</key><string>Mang theo laptop</string>
  </dict>
  <key>styl</key><integer>1</integer>
</dict></plist>"#;

    #[test]
    fn a_record_gives_its_app_its_date_and_its_words() {
        let fields = decode(RECORD.as_bytes()).unwrap();
        assert_eq!(fields.app, "com.apple.ScriptEditor2");
        assert_eq!(fields.date, 814_766_740.5 + FROM_2001);
        assert_eq!(
            (
                fields.title.as_str(),
                fields.subtitle.as_str(),
                fields.body.as_str()
            ),
            ("Họp lúc 3 giờ", "Phòng 2", "Mang theo laptop")
        );
    }

    #[test]
    fn what_is_not_a_record_is_none_and_missing_words_are_empty() {
        assert_eq!(decode(b"not a property list"), None);
        assert_eq!(decode(&[]), None);
        let no_app = RECORD.replace("<key>app</key><string>com.apple.ScriptEditor2</string>", "");
        assert_eq!(decode(no_app.as_bytes()), None, "a record names its app");
        let bare = r#"<?xml version="1.0" encoding="UTF-8"?><plist version="1.0"><dict>
            <key>app</key><string>com.example.app</string></dict></plist>"#;
        let fields = decode(bare.as_bytes()).unwrap();
        assert_eq!((fields.date, fields.title.as_str()), (0.0, ""));
    }
}
