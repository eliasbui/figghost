//! How each app wants a replacement typed. A replacement is a few deletions and then the new text, posted as key
//! events; most apps take them as fast as they come, but address bars select a suggestion that a deletion then
//! removes instead of a letter, editors built on web views drop events that come too close together, and some apps
//! read only the first character of an event that carries several. These rules are the ones Gõ Nhanh settled on
//! for its macOS app, by bundle identifier and by the Accessibility role of the focused element.

/// The way the deletions and the text are posted.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Delivery {
    /// Backspaces, then the text in events of up to 20 UTF-16 units.
    Keys,
    /// The same with one character per event, for apps that read only the first one.
    CharByChar,
    /// Shift+Left over what is replaced, then the text typed over the selection: combo boxes and search fields,
    /// whose suggestion would otherwise take the first backspace.
    Selection,
    /// A narrow no-break space first, which dismisses the suggestion of an address bar, and one backspace more
    /// to take it away again.
    EmptyCharPrefix,
    /// Forward Delete first, which clears the suggestion Spotlight selects after the cursor.
    ClearSuggestion,
    /// The field's own value is rewritten through Accessibility, so no key can overtake another or meet the
    /// suggestion. When the field does not take it, the same as `ClearSuggestion`.
    AxDirect,
    /// Nothing is replaced: the keys go to another device (iPhone Mirroring, a remote desktop), which types for
    /// itself and never sees posted events.
    Passthrough,
}

/// Pauses, in microseconds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Delays {
    /// After each deletion (or each Shift+Left).
    pub delete: u32,
    /// After the last deletion, before the text.
    pub after_delete: u32,
    /// After each event of text.
    pub text: u32,
    /// After the whole replacement, before the next key is let through.
    pub settle: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Plan {
    pub delivery: Delivery,
    pub delays: Delays,
}

const fn plan(delivery: Delivery, delete: u32, after_delete: u32, text: u32, settle: u32) -> Plan {
    Plan {
        delivery,
        delays: Delays {
            delete,
            after_delete,
            text,
            settle,
        },
    }
}

const SETTLE: u32 = 5_000;
/// Editors on web views and terminals need longer before the next key arrives.
const SETTLE_SLOW: u32 = 20_000;

const PASSTHROUGH: Plan = plan(Delivery::Passthrough, 0, 0, 0, 0);
const SELECTION: Plan = plan(Delivery::Selection, 1_000, 3_000, 2_000, SETTLE);
const BROWSER: Plan = plan(Delivery::EmptyCharPrefix, 3_000, 8_000, 3_000, SETTLE);
const DEFAULT: Plan = plan(Delivery::Keys, 1_000, 3_000, 1_500, SETTLE);
/// When the app could not be told at all.
const UNKNOWN: Plan = plan(Delivery::Keys, 200, 800, 500, SETTLE);

/// Remote desktop clients send the physical keys to the other machine and never what is posted here.
const REMOTE_DESKTOPS: &[&str] = &[
    "com.carriez.rustdesk",
    "com.philandro.anydesk",
    "com.teamviewer.TeamViewer",
];

const BROWSERS: &[&str] = &[
    "company.thebrowser.Browser",
    "company.thebrowser.Arc",
    "company.thebrowser.dia",
    "org.mozilla.firefox",
    "org.mozilla.firefoxdeveloperedition",
    "org.mozilla.nightly",
    "org.waterfoxproject.waterfox",
    "io.gitlab.librewolf-community.librewolf",
    "one.ablaze.floorp",
    "org.torproject.torbrowser",
    "net.mullvad.mullvadbrowser",
    "app.zen-browser.zen",
    "com.google.Chrome",
    "com.google.Chrome.canary",
    "com.google.Chrome.beta",
    "org.chromium.Chromium",
    "com.brave.Browser",
    "com.brave.Browser.beta",
    "com.brave.Browser.nightly",
    "com.microsoft.edgemac",
    "com.microsoft.edgemac.Beta",
    "com.microsoft.edgemac.Dev",
    "com.microsoft.edgemac.Canary",
    "com.vivaldi.Vivaldi",
    "com.vivaldi.Vivaldi.snapshot",
    "ru.yandex.desktop.yandex-browser",
    "net.imput.helium",
    "com.opera.Opera",
    "com.operasoftware.Opera",
    "com.operasoftware.OperaGX",
    "com.operasoftware.OperaAir",
    "com.opera.OperaNext",
    "com.kagi.kagimacOS",
    "com.sigmaos.sigmaos.macos",
    "com.pushplaylabs.sidekick",
    "com.firstversionist.polypane",
    "ai.perplexity.comet",
    "com.duckduckgo.macos.browser",
    "com.openai.atlas",
];

/// Code editors and terminals, many of them on web views.
const CODE_APPS: &[&str] = &[
    "com.microsoft.VSCode",
    "com.google.antigravity-ide",
    "com.todesktop.cursor",
    "com.visualstudio.code.oss",
    "com.vscodium",
    "dev.warp.Warp-Stable",
    "com.mitchellh.ghostty",
    "net.kovidgoyal.kitty",
    "com.apple.Terminal",
    "com.googlecode.iterm2",
    "io.alacritty",
    "com.github.wez.wezterm",
    "co.zeit.hyper",
    "org.tabby",
    "com.raphaelamorim.rio",
    "com.termius-dmg.mac",
    "dev.zed.Zed",
    "com.sublimetext.4",
    "com.sublimetext.3",
    "com.panic.Nova",
];

/// The plan for the focused element of `app` (a bundle identifier) with the Accessibility `role`; either may be
/// unknown.
pub fn plan_for(app: Option<&str>, role: Option<&str>) -> Plan {
    let Some(app) = app else {
        return UNKNOWN;
    };
    if app == "com.apple.ScreenContinuity" || REMOTE_DESKTOPS.contains(&app) {
        return PASSTHROUGH;
    }
    // Spotlight's box is a search field too, but it selects its suggestion after the cursor, which a selection
    // typed over would meet; so it comes before the rule of the role.
    if app.starts_with("com.apple.Spotlight") {
        return plan(Delivery::AxDirect, 1_000, 5_000, 0, SETTLE);
    }
    if matches!(role, Some("AXComboBox" | "AXSearchField")) {
        return SELECTION;
    }
    if app.starts_with("com.apple.campo") || app == "com.apple.systemuiserver" {
        return plan(Delivery::ClearSuggestion, 1_000, 5_000, 0, SETTLE);
    }
    if app == "com.apple.Safari" || app == "com.apple.SafariTechnologyPreview" {
        // The address bar is a text field; the pages (Google Docs among them) take one character at a time.
        return if role == Some("AXTextField") {
            BROWSER
        } else {
            plan(Delivery::CharByChar, 3_000, 8_000, 3_000, SETTLE)
        };
    }
    if BROWSERS.contains(&app) {
        return BROWSER;
    }
    let jetbrains = app.starts_with("com.jetbrains");
    if jetbrains && role == Some("AXTextField") {
        return SELECTION;
    }
    match app {
        "com.microsoft.Excel" | "com.microsoft.Word" => {
            return plan(Delivery::Keys, 3_000, 8_000, 3_000, SETTLE_SLOW);
        }
        // Outlook and the Claude app.
        "com.microsoft.Outlook" | "com.todesktop.230313mzl4w4u92" => {
            return plan(Delivery::Keys, 8_000, 15_000, 8_000, SETTLE_SLOW);
        }
        "notion.id" => return plan(Delivery::Keys, 12_000, 25_000, 12_000, SETTLE_SLOW),
        "texstudio" => return plan(Delivery::CharByChar, 3_000, 8_000, 3_000, SETTLE),
        "com.caudex.dev" => return plan(Delivery::CharByChar, 5_000, 15_000, 5_000, SETTLE),
        "com.foxit-software.Foxit.PDF.Reader" => {
            return plan(Delivery::CharByChar, 0, 0, 0, SETTLE);
        }
        _ => {}
    }
    if CODE_APPS.contains(&app) || jetbrains {
        return plan(Delivery::Keys, 8_000, 25_000, 8_000, SETTLE_SLOW);
    }
    if app.starts_with("com.adobe.") {
        // Their text engine reads only the first character of an event that carries several.
        return plan(Delivery::CharByChar, 3_000, 8_000, 3_000, SETTLE);
    }
    if app.starts_with("com.riotgames") {
        // Games read keys as they come; any pause shows.
        return plan(Delivery::Keys, 0, 0, 0, SETTLE);
    }
    DEFAULT
}

#[cfg(test)]
mod tests {
    use super::*;

    fn delivery(app: &str, role: Option<&str>) -> Delivery {
        plan_for(Some(app), role).delivery
    }

    #[test]
    fn keys_for_another_device_are_left_alone() {
        for app in [
            "com.apple.ScreenContinuity",
            "com.carriez.rustdesk",
            "com.philandro.anydesk",
            "com.teamviewer.TeamViewer",
        ] {
            assert_eq!(
                delivery(app, Some("AXTextArea")),
                Delivery::Passthrough,
                "{app}"
            );
            assert_eq!(
                delivery(app, Some("AXSearchField")),
                Delivery::Passthrough,
                "{app}: before any rule of the role"
            );
        }
    }

    #[test]
    fn fields_with_suggestions_are_typed_over_a_selection_in_any_app() {
        for role in ["AXComboBox", "AXSearchField"] {
            for app in [
                "com.apple.finder",
                "com.google.Chrome",
                "com.tinyspeck.slackmacgap",
            ] {
                assert_eq!(
                    delivery(app, Some(role)),
                    Delivery::Selection,
                    "{app} {role}"
                );
            }
        }
        assert_eq!(
            delivery("com.jetbrains.intellij", Some("AXTextField")),
            Delivery::Selection
        );
        assert_eq!(
            plan_for(Some("com.apple.finder"), Some("AXSearchField")).delays,
            Delays {
                delete: 1_000,
                after_delete: 3_000,
                text: 2_000,
                settle: SETTLE
            }
        );
    }

    #[test]
    fn spotlight_is_written_through_accessibility_whatever_its_role() {
        for role in [
            Some("AXTextField"),
            Some("AXSearchField"),
            Some("AXComboBox"),
            None,
        ] {
            assert_eq!(
                delivery("com.apple.Spotlight", role),
                Delivery::AxDirect,
                "{role:?}"
            );
        }
    }

    #[test]
    fn the_other_system_searches_clear_their_suggestion_first() {
        for app in ["com.apple.campo", "com.apple.systemuiserver"] {
            assert_eq!(
                delivery(app, Some("AXTextField")),
                Delivery::ClearSuggestion,
                "{app}"
            );
        }
    }

    #[test]
    fn browsers_dismiss_the_suggestion_of_the_address_bar() {
        for app in [
            "com.google.Chrome",
            "company.thebrowser.Arc",
            "org.mozilla.firefox",
            "com.openai.atlas",
        ] {
            assert_eq!(
                delivery(app, Some("AXTextField")),
                Delivery::EmptyCharPrefix,
                "{app}"
            );
            assert_eq!(
                delivery(app, Some("AXWebArea")),
                Delivery::EmptyCharPrefix,
                "{app}"
            );
        }
        // Safari's pages take one character at a time; only its address bar gets the narrow space.
        assert_eq!(
            delivery("com.apple.Safari", Some("AXTextField")),
            Delivery::EmptyCharPrefix
        );
        assert_eq!(
            delivery("com.apple.Safari", Some("AXTextArea")),
            Delivery::CharByChar
        );
        assert_eq!(delivery("com.apple.Safari", None), Delivery::CharByChar);
    }

    #[test]
    fn editors_on_web_views_and_terminals_get_longer_pauses() {
        for app in [
            "com.microsoft.VSCode",
            "com.todesktop.cursor",
            "com.mitchellh.ghostty",
            "com.apple.Terminal",
            "com.googlecode.iterm2",
            "dev.zed.Zed",
            "com.jetbrains.goland",
        ] {
            let p = plan_for(Some(app), Some("AXTextArea"));
            assert_eq!(p.delivery, Delivery::Keys, "{app}");
            assert_eq!(p.delays.after_delete, 25_000, "{app}");
            assert_eq!(p.delays.settle, SETTLE_SLOW, "{app}");
        }
        assert_eq!(plan_for(Some("notion.id"), None).delays.delete, 12_000);
        assert_eq!(
            plan_for(Some("com.microsoft.Outlook"), None)
                .delays
                .after_delete,
            15_000
        );
        assert_eq!(
            plan_for(Some("com.microsoft.Word"), None).delays.settle,
            SETTLE_SLOW
        );
    }

    #[test]
    fn apps_that_read_one_character_per_event_get_one() {
        for app in [
            "com.adobe.illustrator",
            "com.adobe.Photoshop",
            "texstudio",
            "com.caudex.dev",
            "com.foxit-software.Foxit.PDF.Reader",
        ] {
            assert_eq!(
                delivery(app, Some("AXTextArea")),
                Delivery::CharByChar,
                "{app}"
            );
        }
    }

    #[test]
    fn everything_else_is_typed_with_short_pauses() {
        for app in [
            "com.apple.TextEdit",
            "com.apple.Notes",
            "com.tinyspeck.slackmacgap",
            "com.example.unknown",
        ] {
            assert_eq!(plan_for(Some(app), Some("AXTextArea")), DEFAULT, "{app}");
        }
        assert_eq!(
            plan_for(None, None),
            UNKNOWN,
            "an app that could not be told"
        );
        assert_eq!(plan_for(None, Some("AXSearchField")), UNKNOWN);
        assert_eq!(
            plan_for(Some("com.riotgames.LeagueofLegends"), None)
                .delays
                .delete,
            0
        );
    }
}
