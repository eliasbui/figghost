//! What the page sends, as the settings page of the current app sends it (crates/app-ui/src/settings/model.rs, `Message`):
//! JSON with a `t` field that names the kind. The TypeScript side of it is src/bridge/messages.ts.
use serde::Deserialize;
use serde_json::Value;

#[derive(Debug, Deserialize, PartialEq)]
#[serde(tag = "t", rename_all = "snake_case")]
pub enum Message {
    Ready,
    Set {
        path: String,
        value: Value,
    },
    AppRule {
        id: String,
        field: RuleField,
        value: Option<bool>,
    },
    AppAdd {
        id: String,
    },
    AppRemove {
        id: String,
    },
    IgnoredAdd {
        id: String,
    },
    IgnoredRemove {
        id: String,
    },
    Hotkey {
        target: String,
        value: String,
    },
    /// The page is capturing a key combination: global shortcuts must not fire meanwhile.
    Recording {
        on: bool,
    },
    Action {
        name: String,
    },
    PickApp {
        purpose: Purpose,
    },
    /// The apps that are open now, to offer in the "add app" menu.
    Running {
        purpose: Purpose,
    },
    Icons {
        ids: Vec<String>,
    },
    Page {
        id: String,
    },
    JsError {
        message: String,
        #[serde(default)]
        line: u32,
    },
}

#[derive(Debug, Deserialize, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum RuleField {
    Smooth,
    Reverse,
}

/// Which list an app picked in the page is destined for.
#[derive(Debug, Deserialize, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Purpose {
    Scroll,
    Ignore,
    Typing,
}

impl Purpose {
    pub fn as_str(self) -> &'static str {
        match self {
            Purpose::Scroll => "scroll",
            Purpose::Ignore => "ignore",
            Purpose::Typing => "typing",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn the_messages_of_the_page_are_read_by_their_kind() {
        let set: Message =
            serde_json::from_value(json!({ "t": "set", "path": "mouse.smooth", "value": true }))
                .unwrap();
        assert_eq!(
            set,
            Message::Set {
                path: "mouse.smooth".into(),
                value: json!(true)
            }
        );
        let rule: Message = serde_json::from_value(
            json!({ "t": "app_rule", "id": "a", "field": "smooth", "value": null }),
        )
        .unwrap();
        assert_eq!(
            rule,
            Message::AppRule {
                id: "a".into(),
                field: RuleField::Smooth,
                value: None
            }
        );
        let error: Message =
            serde_json::from_value(json!({ "t": "js_error", "message": "x" })).unwrap();
        assert_eq!(
            error,
            Message::JsError {
                message: "x".into(),
                line: 0
            }
        );
        assert!(serde_json::from_value::<Message>(json!({ "t": "nonsense" })).is_err());
    }
}
