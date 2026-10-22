//! The menu bar item, a ghost with a menu. Dynamic entries are recomputed each time the menu opens.

use crate::{
    context::AppContext,
    i18n,
    settings::{SettingsFeature, base64},
};
use core_mouse::Settings;
use macos_sys::{
    app,
    login_item::{self, LoginItem},
    permissions,
    status_item::{Entry, Item, StatusMenu},
    widgets::image_png,
    workspace,
};
use std::{cell::RefCell, fs, rc::Rc, sync::Arc};

/// The panel the menu bar item opens; it lays the entries of the menu out by what they are.
const TRAY_PAGE: &str = include_str!("../web/tray.html");
const APP_ICON: &[u8] = include_bytes!("../assets/Figghost.icns");
/// The icon is drawn this many points wide, for Retina screens; the page sizes it.
const ICON_POINTS: f64 = 28.0;
const ICON_SCALE: f64 = 2.0;

/// The page of the panel with the app's own icon in it, so that the panel never shows an older one. The system
/// draws it, since the page may load nothing.
fn tray_page() -> String {
    let icon = image_png(APP_ICON, ICON_POINTS, ICON_SCALE).map_or_else(String::new, |png| {
        format!("data:image/png;base64,{}", base64(&png))
    });
    page_with_icon(&icon)
}

fn page_with_icon(icon: &str) -> String {
    TRAY_PAGE.replace("/*__ICON__*/", icon)
}

type ChangeSettings = Rc<dyn Fn(&dyn Fn(&mut Settings))>;

/// The SF Symbol of each entry of the menu, named in one place so that a test can ask the system for every
/// one of them: a name it does not know is not an error, the entry only shows without an icon. All of them
/// are in the symbol set of macOS 13 or earlier.
pub mod symbols {
    // The list the test walks is made from the same lines as the constants, so it cannot miss one.
    macro_rules! symbols {
        ($($name:ident = $symbol:literal,)*) => {
            $(pub const $name: &str = $symbol;)*
            #[cfg(test)]
            pub const ALL: &[&str] = &[$($name),*];
        };
    }

    symbols! {
        SETTINGS = "gearshape",
        PERMISSIONS = "checkmark.shield",
        ACCESSIBILITY = "hand.raised",
        SMOOTH = "computermouse",
        REVERSE = "arrow.up.arrow.down",
        SMOOTH_APP = "macwindow",
        VOLUME_HUD = "speaker.wave.2",
        OPEN_HISTORY = "doc.on.clipboard",
        CLEAR_HISTORY = "trash",
        LOGIN = "power",
        OPEN_CONFIG = "folder",
        QUIT = "xmark.rectangle",
        CAPTURE = "camera.viewfinder",
        CAPTURE_AREA = "rectangle.dashed",
        CAPTURE_WINDOW = "macwindow",
        CAPTURE_SCREEN = "display",
        CAPTURE_TIMER = "timer",
        CAPTURE_TEXT = "text.viewfinder",
        CAPTURE_SCROLL = "arrow.up.and.down.text.horizontal",
        RECORD_SCREEN = "record.circle",
        RECORD_GIF = "photo.stack",
        RECORD_STOP = "stop.circle",
        CAPTURE_FOLDER = "folder",
        SMART_SELECT = "text.magnifyingglass",
        TRANSLATE = "character.bubble",
        PIN = "pin",
        COLOR_PICKER = "eyedropper",
        MENU_BAR_ICONS = "menubar.rectangle",
        COPY_STACK = "square.stack.3d.up",
        EJECT = "eject",
        AWAKE = "cup.and.saucer",
        SNOOZE = "moon.zzz",
        POMODORO = "timer",
        MIC_MUTE = "mic.slash",
        KEYBOARD = "keyboard",
        TYPING = "character.cursor.ibeam",
    }
}

pub fn build(ctx: &AppContext, settings: &Rc<SettingsFeature>) -> StatusMenu {
    let mtm = ctx.mtm;
    let home = ctx.home.clone();
    let (config, control) = (Rc::clone(&ctx.config), Arc::clone(&ctx.control));
    let (clipboard, volume_hud) = (ctx.clipboard.clone(), Rc::clone(&ctx.volume_hud));
    let text = i18n::current();
    let open_settings = {
        let settings = Rc::clone(settings);
        Entry::Item(
            Item::new(text.menu_settings(), move || settings.show()).symbol(symbols::SETTINGS),
        )
    };
    let open_permissions = {
        let permissions_popup = Rc::clone(&ctx.permissions_popup);
        Entry::Item(
            Item::new(text.menu_permissions(), move || permissions_popup.show())
                .symbol(symbols::PERMISSIONS),
        )
    };

    // Applies one change to the live engine settings and to the saved config.
    let change: ChangeSettings = {
        let (control, config, home) = (Arc::clone(&control), Rc::clone(&config), home.clone());
        let settings = Rc::clone(settings);
        Rc::new(move |edit| {
            control.edit(|s| edit(s));
            edit(&mut config.borrow_mut().mouse);
            if let Err(e) = config.borrow().save(&home) {
                eprintln!("config: cannot save: {e}");
            }
            settings.refresh();
        })
    };

    let toggle_global =
        |name: &str, symbol: &'static str, read: fn(&Settings) -> bool, flip: fn(&mut Settings)| {
            let (control, change) = (Arc::clone(&control), Rc::clone(&change));
            Entry::Item(
                Item::new(name, move || change(&flip))
                    .symbol(symbol)
                    .checked(move || read(&control.snapshot())),
            )
        };

    let app_smooth = {
        let (control_check, change) = (Arc::clone(&control), Rc::clone(&change));
        Entry::Item(
            Item::new(text.menu_smooth_current(), move || {
                if let Some(front) = workspace::frontmost_app() {
                    change(&|s| s.toggle_smooth_for(&front.bundle_id));
                }
            })
            .symbol(symbols::SMOOTH_APP)
            .titled(move || match workspace::frontmost_app() {
                Some(front) => text.menu_smooth_for(&front.name),
                None => text.menu_smooth_current().to_owned(),
            })
            .checked(move || {
                workspace::frontmost_app()
                    .is_some_and(|front| control_check.snapshot().effective(&front.bundle_id).0)
            })
            .visible_when(|| workspace::frontmost_app().is_some()),
        )
    };

    let volume_indicator = {
        let (toggle, check) = (Rc::clone(&volume_hud), Rc::clone(&volume_hud));
        let (config, home) = (Rc::clone(&config), home.clone());
        let settings = Rc::clone(settings);
        Entry::Item(
            Item::new(text.menu_volume_hud(), move || {
                let enable = !toggle.is_enabled();
                match toggle.set_enabled(enable) {
                    Ok(()) => {
                        config.borrow_mut().volume_hud = enable;
                        if let Err(e) = config.borrow().save(&home) {
                            eprintln!("config: cannot save: {e}");
                        }
                        settings.refresh();
                    }
                    Err(e) => {
                        eprintln!(
                            "volume hud: cannot start ({e:?}); Accessibility access is needed"
                        );
                        permissions::accessibility_trusted(true);
                    }
                }
            })
            .symbol(symbols::VOLUME_HUD)
            .checked(move || check.is_enabled()),
        )
    };

    let open_history = {
        let clipboard = clipboard.clone();
        Entry::Item(
            Item::new(text.menu_open_history(), move || {
                if let Some(clipboard) = &clipboard {
                    (clipboard.open_popup)();
                }
            })
            .symbol(symbols::OPEN_HISTORY),
        )
    };
    let clear_history = {
        let clipboard = clipboard.clone();
        Entry::Item(
            Item::new(text.menu_clear_history(), move || {
                if let Some(clipboard) = &clipboard {
                    clipboard.history.clear_unpinned();
                }
            })
            .symbol(symbols::CLEAR_HISTORY),
        )
    };

    // The last failure is shown in the item's own title: an accessory app has no better place for it.
    let login_error: Rc<RefCell<Option<String>>> = Rc::default();
    let login = {
        let (on_click_error, title_error) = (Rc::clone(&login_error), Rc::clone(&login_error));
        Entry::Item(
            Item::new(text.menu_login(), move || {
                let result = match login_item::status() {
                    LoginItem::Enabled => login_item::set_enabled(false),
                    LoginItem::Disabled => login_item::set_enabled(true),
                    LoginItem::NeedsApproval => {
                        login_item::open_settings();
                        Ok(())
                    }
                    LoginItem::Unavailable => Ok(()),
                };
                *on_click_error.borrow_mut() = result.err();
            })
            .symbol(symbols::LOGIN)
            .titled(
                move || match (&*title_error.borrow(), login_item::status()) {
                    (Some(e), _) => text.menu_login_error(e),
                    (None, LoginItem::NeedsApproval) => text.menu_login_needs_approval().to_owned(),
                    (None, LoginItem::Unavailable) => text.menu_login_unavailable().to_owned(),
                    (None, _) => text.menu_login().to_owned(),
                },
            )
            .checked(|| login_item::status() == LoginItem::Enabled),
        )
    };

    let open_config = {
        let home = home.clone();
        Entry::Item(
            Item::new(text.menu_open_config(), move || {
                let _ = fs::create_dir_all(&home);
                workspace::open_path(&home);
            })
            .symbol(symbols::OPEN_CONFIG),
        )
    };

    let mut entries = vec![
        open_settings,
        open_permissions,
        Entry::Separator,
        Entry::Item(
            Item::new(text.menu_accessibility(), || {
                permissions::accessibility_trusted(true);
            })
            .symbol(symbols::ACCESSIBILITY)
            .visible_when(|| !permissions::accessibility_trusted(false)),
        ),
        toggle_global(
            text.menu_smooth(),
            symbols::SMOOTH,
            |s| s.smooth,
            |s| s.smooth = !s.smooth,
        ),
        toggle_global(
            text.menu_reverse(),
            symbols::REVERSE,
            |s| s.reverse,
            |s| s.reverse = !s.reverse,
        ),
        app_smooth,
        Entry::Separator,
        volume_indicator,
        Entry::Separator,
        open_history,
        clear_history,
        Entry::Separator,
        login,
        open_config,
    ];
    // What the features add comes after the fixed entries and before the last one.
    let extra = ctx.features.menu_entries();
    if !extra.is_empty() {
        entries.push(Entry::Separator);
        entries.extend(extra);
    }
    entries.push(Entry::Separator);
    entries.push(Entry::Item(
        Item::new(text.menu_quit(), move || app::quit(mtm)).symbol(symbols::QUIT),
    ));
    StatusMenu::new(mtm, "Figghost", entries, &tray_page())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_panel_is_one_self_contained_page_with_the_app_icon() {
        let page = page_with_icon("data:image/png;base64,AAAA");
        assert!(!page.contains("/*__ICON__*/"));
        assert!(page.contains("data:image/png;base64,AAAA"));
        // Nothing may be fetched: no external scripts, styles, images or fonts.
        for forbidden in ["src=\"http", "href=\"http", "url(http", "@import", "<link "] {
            assert!(!page.contains(forbidden), "found {forbidden}");
        }
        assert!(
            image_png(APP_ICON, ICON_POINTS, ICON_SCALE).is_some(),
            "the app icon"
        );
    }

    #[test]
    fn the_system_knows_every_symbol_of_the_menu() {
        for name in symbols::ALL {
            assert!(
                macos_sys::status_item::menu_symbol(name).is_some(),
                "no SF Symbol is named {name}"
            );
        }
    }
}
