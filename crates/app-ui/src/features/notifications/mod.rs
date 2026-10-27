//! The Notifications tab of the notch: the notifications other apps send, read from the store of Notification Center
//! and listed newest first, a click opening the app that sent one. The person chooses how they arrive: through the
//! system, as macOS's own banners, with the tab keeping the list; or through the notch, each one shown on the closed
//! notch with the app's icon, its title and its words. macOS's banners are the system's to show: for the apps whose
//! notifications should come through the notch the person turns them off in System Settings, which the settings page
//! opens. Reading the store needs Full Disk Access. Nothing is written to it, nothing is kept on disk and nothing leaves
//! the Mac; the store is looked into only when its files change. It is off until the person turns it on.
//!
//! This folder belongs to one task. Shared code reaches it only through `features/mod.rs`.

pub mod settings;
mod store;
mod text;

use super::AttachEnv;
use crate::{
    history::now_ms,
    i18n::{self, Language},
    settings::model::Fault,
    trace,
};
use macos_sys::{
    app,
    notch::{Activity, Detail, Ear, Notch, URGENT},
    notification_store::{self, FULL_DISK_PANE, NOTIFICATIONS_PANE},
    notifications_ui::{
        NoticeRow, NotificationsCallbacks, NotificationsDisplay, NotificationsView, PAGE_SIZE,
    },
    style::Tone,
    workspace,
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{
    cell::{Cell, RefCell},
    collections::{HashMap, hash_map::DefaultHasher},
    hash::{Hash, Hasher},
    path::PathBuf,
    rc::{Rc, Weak},
    sync::{
        Arc,
        mpsc::{self, RecvTimeoutError, Sender},
    },
    time::Duration,
};

const SLOT: &str = "notifications";
/// How often the times of the store's files are looked at; the store itself only when they changed.
const POLL: Duration = Duration::from_millis(500);
/// Without access, how often the store is tried again.
const RETRY: Duration = Duration::from_secs(10);
/// How many notifications the tab keeps, and how many are read at the start.
const KEEP: usize = 50;
/// The seconds a notification may stay on the closed notch.
pub const SECONDS: [u32; 4] = [3, 5, 8, 12];
/// How a notification arrives: as macOS's banner, or on the notch.
pub const MODES: [&str; 2] = ["system", "notch"];

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    pub enabled: bool,
    /// `system`: macOS's banners announce them and the tab keeps the list. `notch`: the closed notch shows each one.
    pub mode: String,
    /// How long one stays on the closed notch.
    pub seconds: u32,
    /// The words of a notification are shown, not only its app and title.
    pub preview: bool,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            enabled: false,
            mode: "notch".into(),
            seconds: 5,
            preview: true,
        }
    }
}

impl Config {
    pub fn sanitized(mut self) -> Self {
        if !MODES.contains(&self.mode.as_str()) {
            self.mode = Config::default().mode;
        }
        if !SECONDS.contains(&self.seconds) {
            self.seconds = Config::default().seconds;
        }
        self
    }

    fn on_notch(&self) -> bool {
        self.mode == "notch"
    }
}

/// One notification, as the tab and the notch show it. The Outlook tab reads the same notifications.
#[derive(Debug, Clone, PartialEq)]
pub struct Notice {
    /// Its number in the store.
    pub(crate) id: i64,
    /// The bundle identifier of the app that sent it.
    pub(crate) app: String,
    /// When, in seconds since 1970.
    pub(crate) date: f64,
    pub(crate) title: String,
    pub(crate) subtitle: String,
    pub(crate) body: String,
    /// The category the app gave it.
    pub(crate) category: String,
}

impl Notice {
    fn read(raw: &store::Raw) -> Option<Notice> {
        let fields = notification_store::decode(&raw.data)?;
        let notice = Notice {
            id: raw.id,
            app: fields.app,
            date: fields.date,
            title: fields.title.trim().to_owned(),
            subtitle: fields.subtitle.trim().to_owned(),
            body: fields.body.trim().to_owned(),
            category: fields.category,
        };
        // One that says nothing (a silent update of a badge) is nothing to show.
        let says =
            !(notice.title.is_empty() && notice.subtitle.is_empty() && notice.body.is_empty());
        says.then_some(notice)
    }

    /// The words under the title: the body, else the subtitle.
    pub(crate) fn words(&self) -> &str {
        if self.body.is_empty() {
            &self.subtitle
        } else {
            &self.body
        }
    }
}

/// What the watcher found, for the main thread.
pub(crate) enum Event {
    /// The store was looked into, or could not be: what is in it at the start.
    Opened { readable: bool, recent: Vec<Notice> },
    /// Notifications that came since the last look, oldest first.
    Arrived(Vec<Notice>),
}

/// Watches the store on a thread of its own until `stop` speaks or goes away. The store is looked into only when the
/// times of its files changed; without access it is tried again now and then, as access may have been given.
pub(crate) fn watch(path: PathBuf, stop: mpsc::Receiver<()>, deliver: impl Fn(Event)) {
    let read = |raws: &[store::Raw]| raws.iter().filter_map(Notice::read).collect::<Vec<_>>();
    let mut last: Option<i64> = None;
    let mut stamp = None;
    let mut readable: Option<bool> = None;
    let mut wait = Duration::ZERO;
    loop {
        match stop.recv_timeout(wait) {
            Err(RecvTimeoutError::Timeout) => {}
            _ => return,
        }
        let Some(after) = last else {
            match store::newest(&path, KEEP) {
                Ok((id, raws)) => {
                    last = Some(id);
                    stamp = store::stamp(&path);
                    readable = Some(true);
                    deliver(Event::Opened {
                        readable: true,
                        recent: read(&raws),
                    });
                    wait = POLL;
                }
                Err(e) => {
                    if readable != Some(false) {
                        trace::log(&format!("notifications: the store cannot be read ({e})"));
                        deliver(Event::Opened {
                            readable: false,
                            recent: Vec::new(),
                        });
                    }
                    readable = Some(false);
                    wait = RETRY;
                }
            }
            continue;
        };
        wait = POLL;
        let now = store::stamp(&path);
        if now == stamp {
            continue;
        }
        stamp = now;
        match store::after(&path, after, KEEP) {
            Ok(raws) => {
                if let Some(newest) = raws.last() {
                    last = Some(newest.id);
                }
                let arrived = read(&raws);
                if !arrived.is_empty() {
                    deliver(Event::Arrived(arrived));
                }
            }
            // Access was taken away, or the store was made anew: start over.
            Err(_) => last = None,
        }
    }
}

thread_local! {
    /// What the watcher finds hops to the main thread, which finds the feature here.
    static FEATURE: RefCell<Weak<Feature>> = const { RefCell::new(Weak::new()) };
}

/// The name, the place and the icon of an app, looked up once.
struct AppLook {
    name: String,
    path: Option<PathBuf>,
    png: Option<Arc<Vec<u8>>>,
}

pub struct Feature {
    notch: Weak<Notch>,
    /// The number of this tab in the notch.
    page: usize,
    view: Rc<NotificationsView>,
    config: RefCell<Config>,
    /// Newest first.
    notices: RefCell<Vec<Notice>>,
    /// Whether the store could be read at the last try; `None` before the first.
    readable: Cell<Option<bool>>,
    /// Tells the watcher to stop; it stops by itself when this is dropped.
    watcher: RefCell<Option<Sender<()>>>,
    /// Which watcher speaks for the feature now, so that one that was stopped is not listened to.
    generation: Cell<u64>,
    apps: RefCell<HashMap<String, Rc<AppLook>>>,
    /// Which notification the closed notch shows, so that the end of an older one does not take a newer one away.
    on_notch: Cell<u64>,
    showing: Cell<bool>,
    me: Weak<Feature>,
}

/// Adds the tab. `None` without a notch, which is where the tab lives.
pub fn attach(env: &AttachEnv) -> Option<Rc<Feature>> {
    let notch = env.notch?;
    let feature = Rc::new_cyclic(|me: &Weak<Feature>| {
        let (open, press) = (me.clone(), me.clone());
        let view = NotificationsView::new(
            env.mtm,
            notch.page_area(PAGE_SIZE.width, PAGE_SIZE.height),
            NotificationsCallbacks {
                on_open: Box::new(move |index| {
                    if let Some(f) = open.upgrade() {
                        f.open(index);
                    }
                }),
                on_button: Box::new(move || {
                    if let Some(f) = press.upgrade() {
                        f.button();
                    }
                }),
            },
        );
        Feature {
            notch: Rc::downgrade(notch),
            page: notch.page_count(),
            view,
            config: RefCell::new(env.config.notifications.clone().sanitized()),
            notices: RefCell::default(),
            readable: Cell::new(None),
            watcher: RefCell::default(),
            generation: Cell::new(0),
            apps: RefCell::default(),
            on_notch: Cell::new(0),
            showing: Cell::new(false),
            me: me.clone(),
        }
    });
    FEATURE.with(|slot| *slot.borrow_mut() = Rc::downgrade(&feature));
    let (shown, hidden) = (Rc::downgrade(&feature), Rc::downgrade(&feature));
    notch.add_page(feature.view.page(
        "notifications",
        "bell.badge.fill",
        i18n::current().notifications_tab().to_owned(),
        move || {
            if let Some(f) = shown.upgrade() {
                f.showing.set(true);
                f.render();
            }
        },
        move || {
            if let Some(f) = hidden.upgrade() {
                f.showing.set(false);
            }
        },
    ));
    if feature.config.borrow().enabled {
        feature.start();
    }
    Some(feature)
}

impl Feature {
    /// Applies the settings: the watcher runs exactly while the feature is on.
    pub fn configure(&self, config: &Config) -> Result<(), Fault> {
        let config = config.clone().sanitized();
        let was = self.config.replace(config.clone());
        match (was.enabled, config.enabled) {
            (false, true) => self.start(),
            (true, false) => self.shut(),
            _ => {}
        }
        if !config.on_notch() {
            self.clear_notch();
        }
        self.render();
        Ok(())
    }

    /// Facts only the running app knows, for the settings page: whether the store could be read (null until the
    /// feature is on, as the store is not touched before), and how many are listed.
    pub fn runtime(&self) -> Value {
        json!({ "access": self.readable.get(), "count": self.notices.borrow().len() })
    }

    /// The buttons of the settings page. `None` when the name is not this feature's.
    pub fn action(&self, name: &str) -> Option<Result<(), Fault>> {
        match name {
            "notifications_access" => workspace::open_url(FULL_DISK_PANE),
            "notifications_banners" => workspace::open_url(NOTIFICATIONS_PANE),
            _ => return None,
        }
        Some(Ok(()))
    }

    // -- the watcher ----------------------------------------------------------------------------------------------

    fn start(&self) {
        let Some(path) = notification_store::store_path() else {
            return;
        };
        let generation = self.generation.get() + 1;
        self.generation.set(generation);
        let (stop, stopped) = mpsc::channel();
        let spawned = std::thread::Builder::new()
            .name("fig-notifications".into())
            .spawn(move || {
                watch(path, stopped, |event| {
                    app::on_main(move || {
                        FEATURE.with(|slot| {
                            if let Some(f) = slot.borrow().upgrade() {
                                f.event(generation, event);
                            }
                        });
                    });
                });
            });
        match spawned {
            Ok(_) => *self.watcher.borrow_mut() = Some(stop),
            Err(e) => eprintln!("notifications: cannot start watching ({e})"),
        }
        trace::log("notifications: watching");
    }

    /// The feature is off: the watcher stops, the list is let go and nothing shows on the notch.
    fn shut(&self) {
        if let Some(stop) = self.watcher.borrow_mut().take() {
            let _ = stop.send(());
        }
        self.generation.set(self.generation.get() + 1);
        self.notices.borrow_mut().clear();
        self.readable.set(None);
        self.clear_notch();
        trace::log("notifications: stopped");
    }

    fn event(&self, generation: u64, event: Event) {
        if generation != self.generation.get() || !self.config.borrow().enabled {
            return;
        }
        match event {
            Event::Opened { readable, recent } => {
                self.readable.set(Some(readable));
                if readable {
                    *self.notices.borrow_mut() = recent.into_iter().rev().collect();
                }
            }
            Event::Arrived(arrived) => {
                trace::log(&format!("notifications: {} arrived", arrived.len()));
                let newest = arrived.last().cloned();
                let more = arrived.len().saturating_sub(1);
                {
                    let mut notices = self.notices.borrow_mut();
                    for notice in arrived {
                        notices.insert(0, notice);
                    }
                    notices.truncate(KEEP);
                }
                if let Some(newest) = newest
                    && self.config.borrow().on_notch()
                {
                    self.announce(&newest, more);
                }
            }
        }
        self.render();
    }

    // -- what shows -----------------------------------------------------------------------------------------------

    fn look_of(&self, app: &str) -> Rc<AppLook> {
        if let Some(look) = self.apps.borrow().get(app) {
            return Rc::clone(look);
        }
        let look = Rc::new(AppLook {
            name: workspace::app_name(app).unwrap_or_else(|| app.to_owned()),
            path: workspace::app_path(app),
            png: workspace::app_icon_png(app, 64.0).map(Arc::new),
        });
        self.apps
            .borrow_mut()
            .insert(app.to_owned(), Rc::clone(&look));
        look
    }

    /// The newest notification on the closed notch for a few seconds; `more` came with it.
    fn announce(&self, notice: &Notice, more: usize) {
        let Some(notch) = self.notch.upgrade() else {
            return;
        };
        let config = self.config.borrow().clone();
        let look = self.look_of(&notice.app);
        let (title, subtitle) = lines(i18n::current(), notice, &look.name, config.preview);
        let serial = self.on_notch.get() + 1;
        self.on_notch.set(serial);
        let left = match &look.png {
            Some(bytes) => {
                let mut hasher = DefaultHasher::new();
                notice.app.hash(&mut hasher);
                Ear::Artwork {
                    id: hasher.finish(),
                    bytes: Arc::clone(bytes),
                }
            }
            None => Ear::Symbol {
                name: "bell.fill",
                tone: Tone::Blue,
                pulse: false,
            },
        };
        let right = if more > 0 {
            Ear::Text {
                text: format!("+{more}"),
                tone: Tone::Gray,
            }
        } else {
            Ear::Empty
        };
        notch.set_activity(
            SLOT,
            Some(Activity {
                left,
                right,
                detail: Some(Detail { title, subtitle }),
                page: Some(self.page),
                // Before everything else on the closed notch, the music and the date and time too, for its few
                // seconds.
                priority: URGENT,
            }),
        );
        let weak = self.me.clone();
        app::after(f64::from(config.seconds), move || {
            if let Some(f) = weak.upgrade()
                && f.on_notch.get() == serial
            {
                f.clear_notch();
            }
        });
    }

    fn clear_notch(&self) {
        if let Some(notch) = self.notch.upgrade() {
            notch.set_activity(SLOT, None);
        }
    }

    fn render(&self) {
        if !self.showing.get() {
            return;
        }
        let language = i18n::current();
        let now = now_ms() as f64 / 1000.0;
        let notices = self.notices.borrow();
        let looks: Vec<Rc<AppLook>> = notices.iter().map(|n| self.look_of(&n.app)).collect();
        let names: Vec<&str> = looks.iter().map(|l| l.name.as_str()).collect();
        let mut display = display(
            language,
            self.config.borrow().enabled,
            self.readable.get(),
            &notices,
            &names,
            now,
        );
        for (row, look) in display.rows.iter_mut().zip(&looks) {
            row.app.clone_from(&look.path);
        }
        drop(notices);
        self.view.show(&display);
    }

    /// A row was pressed: the app that sent it comes to the front.
    fn open(&self, index: usize) {
        let app = self.notices.borrow().get(index).map(|n| n.app.clone());
        let Some(path) = app.and_then(|app| self.look_of(&app).path.clone()) else {
            return;
        };
        workspace::open_path(&path);
        if let Some(notch) = self.notch.upgrade() {
            notch.close();
        }
    }

    /// The button beside the heading: the settings that give access, or clearing the list.
    fn button(&self) {
        if self.readable.get() == Some(false) {
            workspace::open_url(FULL_DISK_PANE);
            return;
        }
        self.notices.borrow_mut().clear();
        self.clear_notch();
        self.render();
    }
}

/// The two lines on the closed notch: the title, else the app; and the words, or the app when the words are kept off
/// the notch.
fn lines(language: Language, notice: &Notice, app: &str, preview: bool) -> (String, String) {
    if !preview {
        return (app.to_owned(), language.notifications_hidden().to_owned());
    }
    let title = if notice.title.is_empty() {
        app.to_owned()
    } else {
        notice.title.clone()
    };
    let words = notice.words();
    (
        title,
        if words.is_empty() {
            app.to_owned()
        } else {
            words.to_owned()
        },
    )
}

/// The tab for what is known: a note when it is off, cannot read the store or has nothing; otherwise a row for each
/// notification with the app, how long ago and what it says. Pure, so that the tests drive it without the store.
fn display(
    language: Language,
    enabled: bool,
    readable: Option<bool>,
    notices: &[Notice],
    names: &[&str],
    now: f64,
) -> NotificationsDisplay {
    let heading = language.notifications_tab().to_owned();
    let note = |symbol: &'static str, title: &str, hint: &str| {
        Some((symbol, title.to_owned(), hint.to_owned()))
    };
    if !enabled {
        return NotificationsDisplay {
            heading,
            note: note(
                "bell.slash",
                language.notifications_off_title(),
                language.notifications_off_hint(),
            ),
            ..NotificationsDisplay::default()
        };
    }
    if readable == Some(false) {
        return NotificationsDisplay {
            heading,
            button: (
                "lock.open.fill",
                language.notifications_open_settings().to_owned(),
            ),
            note: note(
                "lock.fill",
                language.notifications_access_title(),
                language.notifications_access_hint(),
            ),
            ..NotificationsDisplay::default()
        };
    }
    if notices.is_empty() {
        return NotificationsDisplay {
            heading,
            note: note(
                "bell",
                language.notifications_empty_title(),
                language.notifications_empty_hint(),
            ),
            ..NotificationsDisplay::default()
        };
    }
    let rows = notices
        .iter()
        .zip(names)
        .map(|(notice, app)| {
            let title = if notice.title.is_empty() {
                (*app).to_owned()
            } else {
                notice.title.clone()
            };
            let ago = language.notifications_ago(now - notice.date);
            let words = notice.words();
            NoticeRow {
                title,
                detail: if words.is_empty() {
                    format!("{app}, {ago}")
                } else {
                    format!("{app}, {ago}: {words}")
                },
                app: None,
            }
        })
        .collect();
    NotificationsDisplay {
        heading,
        status: language.notifications_count(notices.len()),
        rows,
        button: ("trash", language.notifications_clear().to_owned()),
        note: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn notice(id: i64, title: &str, subtitle: &str, body: &str) -> Notice {
        Notice {
            id,
            app: "com.example.chat".into(),
            date: 1_000_000.0,
            title: title.into(),
            subtitle: subtitle.into(),
            body: body.into(),
            category: String::new(),
        }
    }

    #[test]
    fn the_feature_is_off_by_default_and_a_hostile_file_is_tamed() {
        let config: Config = serde_json::from_str("{}").unwrap();
        assert_eq!(config, Config::default());
        assert!(!config.enabled && config.on_notch() && config.preview);
        let hostile: Config =
            serde_json::from_str(r#"{ "mode": "everywhere", "seconds": 999 }"#).unwrap();
        let tamed = hostile.sanitized();
        assert_eq!((tamed.mode.as_str(), tamed.seconds), ("notch", 5));
    }

    #[test]
    fn the_notch_says_the_title_and_the_words_or_only_the_app_when_they_are_kept_off() {
        let full = notice(1, "Lan", "", "Đi ăn trưa không?");
        assert_eq!(
            lines(Language::Vietnamese, &full, "Messages", true),
            ("Lan".into(), "Đi ăn trưa không?".into())
        );
        assert_eq!(
            lines(Language::Vietnamese, &full, "Messages", false),
            ("Messages".into(), "Thông báo mới".into()),
            "nothing of what it says"
        );
        let bare = notice(2, "", "Build failed", "");
        assert_eq!(
            lines(Language::English, &bare, "Xcode", true),
            ("Xcode".into(), "Build failed".into()),
            "no title: the app; no body: the subtitle"
        );
    }

    #[test]
    fn the_tab_says_why_it_is_empty_and_lists_what_came_newest_first() {
        let off = display(Language::English, false, Some(true), &[], &[], 0.0);
        assert!(off.note.is_some() && off.button.1.is_empty());
        let locked = display(Language::English, true, Some(false), &[], &[], 0.0);
        assert_eq!(locked.note.as_ref().map(|n| n.0), Some("lock.fill"));
        assert!(!locked.button.1.is_empty(), "the button opens the settings");
        let empty = display(Language::Vietnamese, true, Some(true), &[], &[], 0.0);
        assert_eq!(
            empty.note.as_ref().map(|n| n.1.as_str()),
            Some("Chưa có thông báo nào")
        );
        let notices = [
            notice(9, "Lan", "", "Đi ăn trưa không?"),
            notice(7, "", "", "Đã xong"),
        ];
        let list = display(
            Language::Vietnamese,
            true,
            Some(true),
            &notices,
            &["Tin nhắn", "Xcode"],
            1_000_000.0 + 300.0,
        );
        assert!(list.note.is_none());
        assert_eq!(list.status, "2 thông báo");
        assert_eq!(list.rows[0].title, "Lan");
        assert_eq!(
            list.rows[0].detail,
            "Tin nhắn, 5 phút trước: Đi ăn trưa không?"
        );
        assert_eq!(list.rows[1].title, "Xcode", "no title: the app's name");
    }

    #[test]
    fn a_notification_that_says_nothing_is_not_listed() {
        let raw = |xml: &str| store::Raw {
            id: 4,
            data: format!(
                r#"<?xml version="1.0" encoding="UTF-8"?><plist version="1.0"><dict>
                <key>app</key><string>com.example.app</string>{xml}</dict></plist>"#
            )
            .into_bytes(),
        };
        assert_eq!(Notice::read(&raw("")), None, "a badge that changed");
        let said = Notice::read(&raw(
            "<key>req</key><dict><key>titl</key><string>  Hi  </string></dict>",
        ))
        .unwrap();
        assert_eq!((said.id, said.title.as_str()), (4, "Hi"));
    }

    #[test]
    fn how_long_ago_is_said_in_words() {
        let en = Language::English;
        assert_eq!(en.notifications_ago(20.0), "just now");
        assert_eq!(
            en.notifications_ago(-5.0),
            "just now",
            "a clock that went back"
        );
        assert_eq!(en.notifications_ago(5.0 * 60.0), "5 min ago");
        assert_eq!(en.notifications_ago(3.0 * 3600.0), "3 h ago");
        assert_eq!(en.notifications_ago(30.0 * 3600.0), "yesterday");
        assert_eq!(
            Language::Vietnamese.notifications_ago(3.0 * 86_400.0),
            "3 ngày trước"
        );
    }

    /// Watches the real store until the next notification of any app arrives, and checks that it is read whole; only
    /// the app is printed, never what it says. It needs Full Disk Access for the terminal, and something to arrive within
    /// `FIGGHOST_LIVE_SECONDS` (60 by default): `cargo test -p app-ui -- --ignored --nocapture notifications_live`.
    #[test]
    #[ignore]
    fn notifications_live_the_next_notification_is_heard() {
        let seconds: u64 = std::env::var("FIGGHOST_LIVE_SECONDS")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(60);
        let path = notification_store::store_path().unwrap();
        let (stop, stopped) = mpsc::channel();
        let (opened, open) = mpsc::channel::<bool>();
        let (arrived, heard) = mpsc::channel::<Vec<Notice>>();
        let thread = std::thread::spawn(move || {
            watch(path, stopped, move |event| match event {
                Event::Opened { readable, .. } => {
                    let _ = opened.send(readable);
                }
                Event::Arrived(notices) => {
                    let _ = arrived.send(notices);
                }
            });
        });
        assert_eq!(
            open.recv_timeout(Duration::from_secs(10)),
            Ok(true),
            "the store can be read"
        );
        println!("the store is read; waiting up to {seconds} s for a notification");
        let started = std::time::Instant::now();
        let batch = heard.recv_timeout(Duration::from_secs(seconds));
        stop.send(()).unwrap();
        thread.join().unwrap();
        let batch = batch.expect("a notification arrived and was heard");
        assert!(batch.iter().all(|n| !n.app.is_empty() && n.id > 0));
        let apps: Vec<&str> = batch.iter().map(|n| n.app.as_str()).collect();
        println!(
            "heard {} after {:.1} s, from {apps:?}",
            batch.len(),
            started.elapsed().as_secs_f64()
        );
    }

    #[test]
    fn the_watcher_reports_a_store_it_cannot_read_once_and_stops_when_told() {
        let (stop, stopped) = mpsc::channel();
        let (events, heard) = mpsc::channel::<bool>();
        let missing =
            std::env::temp_dir().join(format!("figghost-no-store-{}/db", std::process::id()));
        let thread = std::thread::spawn(move || {
            watch(missing, stopped, move |event| {
                if let Event::Opened { readable, .. } = event {
                    let _ = events.send(readable);
                }
            });
        });
        assert_eq!(heard.recv_timeout(Duration::from_secs(5)), Ok(false));
        stop.send(()).unwrap();
        thread.join().unwrap();
        assert!(heard.try_recv().is_err(), "said once");
    }
}
