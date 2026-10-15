//! The "Fig" menu bar item. Dynamic entries are recomputed each time the menu opens.

use crate::{
    capture::{CaptureFeature, Kind},
    context::AppContext,
    i18n,
    settings::SettingsFeature,
};
use core_mouse::Settings;
use macos_sys::{
    app,
    login_item::{self, LoginItem},
    permissions,
    status_item::{Entry, Item, StatusMenu},
    workspace,
};
use std::{cell::RefCell, fs, rc::Rc, sync::Arc};

type ChangeSettings = Rc<dyn Fn(&dyn Fn(&mut Settings))>;

pub fn build(
    ctx: &AppContext,
    settings: &Rc<SettingsFeature>,
    capture: &Rc<CaptureFeature>,
) -> StatusMenu {
    let mtm = ctx.mtm;
    let home = ctx.home.clone();
    let (config, control) = (Rc::clone(&ctx.config), Arc::clone(&ctx.control));
    let (clipboard, volume_hud) = (ctx.clipboard.clone(), Rc::clone(&ctx.volume_hud));
    let text = i18n::current();
    let open_settings = {
        let settings = Rc::clone(settings);
        Entry::Item(Item::new(text.menu_settings(), move || settings.show()))
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

    let toggle_global = |name: &str, read: fn(&Settings) -> bool, flip: fn(&mut Settings)| {
        let (control, change) = (Arc::clone(&control), Rc::clone(&change));
        Entry::Item(
            Item::new(name, move || change(&flip)).checked(move || read(&control.snapshot())),
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
            .checked(move || check.is_enabled()),
        )
    };

    let open_history = {
        let clipboard = clipboard.clone();
        Entry::Item(Item::new(text.menu_open_history(), move || {
            if let Some(clipboard) = &clipboard {
                (clipboard.open_popup)();
            }
        }))
    };
    let clear_history = {
        let clipboard = clipboard.clone();
        Entry::Item(Item::new(text.menu_clear_history(), move || {
            if let Some(clipboard) = &clipboard {
                clipboard.history.clear_unpinned();
            }
        }))
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
        Entry::Item(Item::new(text.menu_open_config(), move || {
            let _ = fs::create_dir_all(&home);
            workspace::open_path(&home);
        }))
    };

    // Menu items run after the menu has closed, so the capture never shows the menu itself.
    let take = |title: &str, kind: Kind| {
        let capture = Rc::clone(capture);
        Entry::Item(Item::new(title, move || {
            let capture = Rc::clone(&capture);
            app::after(0.25, move || capture.take(kind));
        }))
    };
    let timer_seconds = config.borrow().capture.timer_seconds;
    let entries = vec![
        open_settings,
        Entry::Separator,
        take(text.menu_capture_area(), Kind::Area),
        take(text.menu_capture_window(), Kind::Window),
        take(text.menu_capture_screen(), Kind::Screen),
        take(&text.menu_capture_timer(timer_seconds), Kind::Timer),
        take(text.menu_capture_text(), Kind::Text),
        take(text.menu_capture_scroll(), Kind::Scroll),
        {
            let capture = Rc::clone(capture);
            Entry::Item(Item::new(text.menu_capture_folder(), move || {
                capture.open_folder()
            }))
        },
        Entry::Separator,
        Entry::Item(
            Item::new(text.menu_accessibility(), || {
                permissions::accessibility_trusted(true);
            })
            .visible_when(|| !permissions::accessibility_trusted(false)),
        ),
        toggle_global(text.menu_smooth(), |s| s.smooth, |s| s.smooth = !s.smooth),
        toggle_global(
            text.menu_reverse(),
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
        Entry::Item(Item::new(text.menu_quit(), move || app::quit(mtm))),
    ];
    StatusMenu::new(mtm, "Fig", entries)
}
