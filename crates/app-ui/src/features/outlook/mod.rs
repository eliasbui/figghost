//! The Outlook tab of the notch: the new mail and the calendar reminders of the Outlook app, read from the
//! notifications it sends to Notification Center, so no account is signed in and nothing is asked of Microsoft. They
//! are listed newest first; a click on one opens a card in the middle of the screen to read it, with a button that
//! brings Outlook to the front. A new one can show on the closed notch, and can fall from the notch as a drop that
//! opens into that card. What a notification holds is all there is: the sender, the subject and the first lines.
//! Reading the store needs Full Disk Access, as for the Notifications tab, whose watcher this uses.
//!
//! This folder belongs to one task. Shared code reaches it only through `features/mod.rs`.

pub mod settings;
mod text;

use super::{
    AttachEnv,
    notifications::{Event, Notice, watch},
};
use crate::{
    history::now_ms,
    i18n::{self, Language},
    settings::model::Fault,
    trace,
};
use macos_sys::{
    MainThreadMarker, NSSize, app,
    notch::{Activity, Detail, Ear, Notch, URGENT},
    notification_store::{self, FULL_DISK_PANE, NOTIFICATIONS_PANE},
    notifications_ui::{
        NoticeRow, NotificationsCallbacks, NotificationsDisplay, NotificationsView, PAGE_SIZE,
    },
    style::Tone,
    webview::DropPanel,
    widgets::base64,
    workspace,
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{
    cell::{Cell, RefCell},
    path::PathBuf,
    rc::{Rc, Weak},
    sync::{
        Arc,
        mpsc::{self, Sender},
    },
};

const SLOT: &str = "outlook";
/// The app whose notifications this tab lists.
pub const OUTLOOK: &str = "com.microsoft.Outlook";
/// How many the tab keeps.
const KEEP: usize = 50;
/// The seconds a new one may stay on the closed notch.
pub const SECONDS: [u32; 4] = [3, 5, 8, 12];
const PAGE: &str = include_str!("../../../web/outlook.html");
/// The card and the clear margin around it for its shadow, as the page lays them out.
const CARD: NSSize = NSSize::new(420.0 + 48.0, 236.0 + 48.0);

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    pub enabled: bool,
    /// A new one shows on the closed notch for a few seconds.
    pub notch: bool,
    pub seconds: u32,
    /// What it says shows on the closed notch, not only that one came.
    pub preview: bool,
    /// A new one falls from the notch as a drop that opens into the card in the middle of the screen.
    pub drop: bool,
    /// The reminders of the calendar are listed and announced too, not only mail.
    pub events: bool,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            enabled: false,
            notch: true,
            seconds: 5,
            preview: true,
            drop: false,
            events: true,
        }
    }
}

impl Config {
    pub fn sanitized(mut self) -> Self {
        if !SECONDS.contains(&self.seconds) {
            self.seconds = Config::default().seconds;
        }
        self
    }
}

/// Whether Outlook sent it as a reminder of the calendar rather than for a mail.
// shortcut: told apart by the category Outlook gives the notification, guessed from its name; a reminder under a
// category without these words is listed as mail. Read the real categories from the store when one is mislabelled.
fn is_event(notice: &Notice) -> bool {
    let category = notice.category.to_ascii_lowercase();
    ["remind", "calendar", "event", "meeting", "alarm"]
        .iter()
        .any(|word| category.contains(word))
}

/// The notifications of Outlook that this tab shows, of those that came.
fn mine(notices: Vec<Notice>, events: bool) -> Vec<Notice> {
    notices
        .into_iter()
        .filter(|n| n.app.eq_ignore_ascii_case(OUTLOOK) && (events || !is_event(n)))
        .collect()
}

/// The three lines of one: who sent it (or the event), the subject, and its first lines. Outlook puts the sender in
/// the title, the subject in the subtitle and the start of the mail in the body; a reminder has its event as the title.
fn parts(notice: &Notice) -> (&str, &str, &str) {
    let first = if notice.title.is_empty() {
        notice.subtitle.as_str()
    } else {
        notice.title.as_str()
    };
    let subject = if notice.title.is_empty() {
        ""
    } else {
        notice.subtitle.as_str()
    };
    (first, subject, notice.body.as_str())
}

thread_local! {
    static FEATURE: RefCell<Weak<Feature>> = const { RefCell::new(Weak::new()) };
}

pub struct Feature {
    mtm: MainThreadMarker,
    notch: Weak<Notch>,
    page: usize,
    view: Rc<NotificationsView>,
    config: RefCell<Config>,
    /// Newest first.
    notices: RefCell<Vec<Notice>>,
    readable: Cell<Option<bool>>,
    watcher: RefCell<Option<Sender<()>>>,
    generation: Cell<u64>,
    /// Where Outlook is and its icon, looked up once.
    app: Option<PathBuf>,
    icon: Option<Arc<Vec<u8>>>,
    on_notch: Cell<u64>,
    showing: Cell<bool>,
    /// The card, made the first time it is needed; what it shows, and a message waiting for its page to load.
    card: RefCell<Option<Rc<DropPanel>>>,
    card_ready: Cell<bool>,
    card_waiting: RefCell<Option<String>>,
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
                        f.read(index, false);
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
            mtm: env.mtm,
            notch: Rc::downgrade(notch),
            page: notch.page_count(),
            view,
            config: RefCell::new(env.config.outlook.clone().sanitized()),
            notices: RefCell::default(),
            readable: Cell::new(None),
            watcher: RefCell::default(),
            generation: Cell::new(0),
            app: workspace::app_path(OUTLOOK),
            icon: workspace::app_icon_png(OUTLOOK, 64.0).map(Arc::new),
            on_notch: Cell::new(0),
            showing: Cell::new(false),
            card: RefCell::default(),
            card_ready: Cell::new(false),
            card_waiting: RefCell::default(),
            me: me.clone(),
        }
    });
    FEATURE.with(|slot| *slot.borrow_mut() = Rc::downgrade(&feature));
    let (shown, hidden) = (Rc::downgrade(&feature), Rc::downgrade(&feature));
    notch.add_page(feature.view.page(
        "outlook",
        "envelope.fill",
        i18n::current().outlook_tab().to_owned(),
        move || {
            if let Some(f) = shown.upgrade() {
                f.showing.set(true);
                if let Some(notch) = f.notch.upgrade() {
                    notch.set_page_badge(f.page, false);
                }
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
    pub fn configure(&self, config: &Config) -> Result<(), Fault> {
        let config = config.clone().sanitized();
        let was = self.config.replace(config.clone());
        match (was.enabled, config.enabled) {
            (false, true) => self.start(),
            (true, false) => self.shut(),
            _ => {}
        }
        if !config.notch {
            self.clear_notch();
        }
        if !config.events {
            self.notices.borrow_mut().retain(|n| !is_event(n));
        }
        self.render();
        Ok(())
    }

    pub fn runtime(&self) -> Value {
        json!({
            "access": self.readable.get(),
            "count": self.notices.borrow().len(),
            "installed": self.app.is_some(),
        })
    }

    pub fn action(&self, name: &str) -> Option<Result<(), Fault>> {
        match name {
            "outlook_access" => workspace::open_url(FULL_DISK_PANE),
            "outlook_banners" => workspace::open_url(NOTIFICATIONS_PANE),
            // Shows the card as a new mail would, so the drop can be seen before one comes.
            "outlook_try" => self.card(&self.sample(), self.config.borrow().drop),
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
            .name("fig-outlook".into())
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
            Err(e) => eprintln!("outlook: cannot start watching ({e})"),
        }
        trace::log("outlook: watching");
    }

    fn shut(&self) {
        if let Some(stop) = self.watcher.borrow_mut().take() {
            let _ = stop.send(());
        }
        self.generation.set(self.generation.get() + 1);
        self.notices.borrow_mut().clear();
        self.readable.set(None);
        self.clear_notch();
        if let Some(card) = self.card.borrow().as_ref() {
            card.hide();
        }
        trace::log("outlook: stopped");
    }

    fn event(&self, generation: u64, event: Event) {
        let config = self.config.borrow().clone();
        if generation != self.generation.get() || !config.enabled {
            return;
        }
        match event {
            Event::Opened { readable, recent } => {
                self.readable.set(Some(readable));
                if readable {
                    *self.notices.borrow_mut() =
                        mine(recent, config.events).into_iter().rev().collect();
                }
            }
            Event::Arrived(arrived) => {
                let arrived = mine(arrived, config.events);
                let Some(newest) = arrived.last().cloned() else {
                    return;
                };
                trace::log(&format!("outlook: {} arrived", arrived.len()));
                let more = arrived.len() - 1;
                {
                    let mut notices = self.notices.borrow_mut();
                    for notice in arrived {
                        notices.insert(0, notice);
                    }
                    notices.truncate(KEEP);
                }
                if !self.showing.get()
                    && let Some(notch) = self.notch.upgrade()
                {
                    notch.set_page_badge(self.page, true);
                }
                if config.notch {
                    self.announce(&newest, more);
                }
                if config.drop {
                    self.card(&newest, true);
                }
            }
        }
        self.render();
    }

    // -- the notch ------------------------------------------------------------------------------------------------

    fn announce(&self, notice: &Notice, more: usize) {
        let Some(notch) = self.notch.upgrade() else {
            return;
        };
        let config = self.config.borrow().clone();
        let event = is_event(notice);
        let (first, subject, body) = parts(notice);
        let language = i18n::current();
        let (title, subtitle) = if config.preview {
            let second = [subject, body].into_iter().find(|s| !s.is_empty());
            (
                first.to_owned(),
                second.unwrap_or(language.outlook_hidden(event)).to_owned(),
            )
        } else {
            (
                "Outlook".to_owned(),
                language.outlook_hidden(event).to_owned(),
            )
        };
        let serial = self.on_notch.get() + 1;
        self.on_notch.set(serial);
        let left = match &self.icon {
            Some(bytes) => Ear::Artwork {
                // A fixed id: this tab has one icon.
                id: 0x006f_7574_6c6f_6f6b,
                bytes: Arc::clone(bytes),
            },
            None => Ear::Symbol {
                name: if event { "calendar" } else { "envelope.fill" },
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

    // -- the card -------------------------------------------------------------------------------------------------

    /// Shows one in the card in the middle of the screen of the notch, falling from the notch as a drop with `fall`.
    fn card(&self, notice: &Notice, fall: bool) {
        let Some(notch) = self.notch.upgrade() else {
            return;
        };
        let language = i18n::current();
        let now = now_ms() as f64 / 1000.0;
        let message = card_message(
            language,
            notice,
            self.icon.as_deref().map(Vec::as_slice),
            fall,
            now,
        );
        let panel = self.card.borrow().clone().unwrap_or_else(|| {
            let weak = self.me.clone();
            let panel = DropPanel::new(self.mtm, PAGE, move |text| {
                if let Some(f) = weak.upgrade() {
                    f.card_said(&text);
                }
            });
            *self.card.borrow_mut() = Some(Rc::clone(&panel));
            panel
        });
        *self.card_waiting.borrow_mut() = Some(message.to_string());
        if self.card_ready.get() {
            self.flush_card(&panel);
        }
        panel.show(notch.geometry().screen, CARD, fall);
    }

    fn flush_card(&self, panel: &DropPanel) {
        if let Some(message) = self.card_waiting.borrow_mut().take() {
            panel.host.post(&message);
        }
    }

    fn card_said(&self, text: &str) {
        let Ok(message) = serde_json::from_str::<Value>(text) else {
            return;
        };
        let Some(panel) = self.card.borrow().clone() else {
            return;
        };
        match message["t"].as_str() {
            Some("ready") => {
                self.card_ready.set(true);
                self.flush_card(&panel);
            }
            Some("landed") => panel.landed(),
            Some("close") => panel.hide(),
            Some("open") => {
                panel.hide();
                if let Some(path) = &self.app {
                    workspace::open_path(path);
                }
            }
            Some("js_error") => trace::log(&format!("outlook: card script error {text}")),
            _ => {}
        }
    }

    /// A made-up mail for the button that tries the card.
    fn sample(&self) -> Notice {
        let vietnamese = i18n::current() == Language::Vietnamese;
        Notice {
            id: 0,
            app: OUTLOOK.into(),
            date: now_ms() as f64 / 1000.0,
            title: "Figghost".into(),
            subtitle: if vietnamese {
                "Thư thử"
            } else {
                "A test mail"
            }
            .into(),
            body: if vietnamese {
                "Thư mới sẽ hiện như thế này: người gửi, tiêu đề và vài dòng đầu."
            } else {
                "New mail shows like this: the sender, the subject and its first lines."
            }
            .into(),
            category: String::new(),
        }
    }

    // -- the tab --------------------------------------------------------------------------------------------------

    fn render(&self) {
        if !self.showing.get() {
            return;
        }
        let now = now_ms() as f64 / 1000.0;
        let mut display = display(
            i18n::current(),
            self.config.borrow().enabled,
            self.readable.get(),
            &self.notices.borrow(),
            now,
        );
        for row in &mut display.rows {
            row.app.clone_from(&self.app);
        }
        self.view.show(&display);
    }

    /// A row was pressed: it opens in the card.
    fn read(&self, index: usize, fall: bool) {
        let Some(notice) = self.notices.borrow().get(index).cloned() else {
            return;
        };
        if let Some(notch) = self.notch.upgrade() {
            notch.close();
        }
        self.card(&notice, fall);
    }

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

/// What the card's page is told to show.
fn card_message(
    language: Language,
    notice: &Notice,
    icon: Option<&[u8]>,
    fall: bool,
    now: f64,
) -> Value {
    let event = is_event(notice);
    let (from, subject, body) = parts(notice);
    let ago = language.notifications_ago(now - notice.date);
    json!({
        "t": "show",
        "fall": fall,
        "icon": icon.map(|png| format!("data:image/png;base64,{}", base64(png))),
        "where": format!("Outlook, {}, {ago}", language.outlook_kind(event)),
        "from": from,
        "subject": subject,
        "body": body,
        "open": language.outlook_open(),
        "close": language.outlook_close(),
    })
}

/// The tab for what is known. Pure, so the tests drive it without the store.
fn display(
    language: Language,
    enabled: bool,
    readable: Option<bool>,
    notices: &[Notice],
    now: f64,
) -> NotificationsDisplay {
    let heading = language.outlook_tab().to_owned();
    let note = |symbol: &'static str, title: &str, hint: &str| {
        Some((symbol, title.to_owned(), hint.to_owned()))
    };
    if !enabled {
        return NotificationsDisplay {
            heading,
            note: note(
                "envelope",
                language.outlook_off_title(),
                language.outlook_off_hint(),
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
                "envelope.open",
                language.outlook_empty_title(),
                language.outlook_empty_hint(),
            ),
            ..NotificationsDisplay::default()
        };
    }
    let events = notices.iter().filter(|n| is_event(n)).count();
    let rows = notices
        .iter()
        .map(|notice| {
            let (first, subject, body) = parts(notice);
            let words = if subject.is_empty() { body } else { subject };
            let lead = format!(
                "{}, {}",
                language.outlook_kind(is_event(notice)),
                language.notifications_ago(now - notice.date)
            );
            NoticeRow {
                title: if first.is_empty() {
                    "Outlook".to_owned()
                } else {
                    first.to_owned()
                },
                detail: if words.is_empty() {
                    lead
                } else {
                    format!("{lead}: {words}")
                },
                app: None,
            }
        })
        .collect();
    NotificationsDisplay {
        heading,
        status: language.outlook_count(notices.len() - events, events),
        rows,
        button: ("trash", language.notifications_clear().to_owned()),
        note: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn notice(app: &str, title: &str, subtitle: &str, body: &str, category: &str) -> Notice {
        Notice {
            id: 1,
            app: app.into(),
            date: 1_000_000.0,
            title: title.into(),
            subtitle: subtitle.into(),
            body: body.into(),
            category: category.into(),
        }
    }

    #[test]
    fn it_is_off_by_default_without_the_drop_and_a_hostile_file_is_tamed() {
        let config: Config = serde_json::from_str("{}").unwrap();
        assert_eq!(config, Config::default());
        assert!(!config.enabled && !config.drop && config.notch && config.events);
        let tamed = serde_json::from_str::<Config>(r#"{ "seconds": 999 }"#)
            .unwrap()
            .sanitized();
        assert_eq!(tamed.seconds, 5);
    }

    #[test]
    fn only_outlook_is_kept_and_its_reminders_only_when_asked() {
        let came = vec![
            notice("com.microsoft.teams2", "Lan", "", "hi", ""),
            notice(OUTLOOK, "Lan", "Báo cáo tuần", "Gửi anh", "MailCategory"),
            notice(
                "com.microsoft.outlook",
                "Họp giao ban",
                "",
                "10:00",
                "ReminderCategory",
            ),
        ];
        assert_eq!(
            mine(came.clone(), true).len(),
            2,
            "the case of the id does not matter"
        );
        let mails = mine(came, false);
        assert_eq!(mails.len(), 1);
        assert_eq!(mails[0].subtitle, "Báo cáo tuần");
        assert!(is_event(&notice(OUTLOOK, "", "", "", "CALENDAR_EVENT")));
        assert!(!is_event(&notice(OUTLOOK, "", "", "", "")));
    }

    #[test]
    fn a_mail_reads_as_sender_subject_and_first_lines() {
        let mail = notice(OUTLOOK, "Lan", "Báo cáo tuần", "Gửi anh bản nháp", "");
        assert_eq!(parts(&mail), ("Lan", "Báo cáo tuần", "Gửi anh bản nháp"));
        let bare = notice(OUTLOOK, "", "Chỉ có tiêu đề", "", "");
        assert_eq!(
            parts(&bare),
            ("Chỉ có tiêu đề", "", ""),
            "no title: the subtitle leads"
        );
        let card = card_message(Language::Vietnamese, &mail, None, true, 1_000_000.0 + 120.0);
        assert_eq!(card["where"], "Outlook, Thư, 2 phút trước");
        assert_eq!(
            (card["from"].as_str(), card["fall"].as_bool()),
            (Some("Lan"), Some(true))
        );
        assert!(card["icon"].is_null());
    }

    #[test]
    fn the_tab_counts_mail_and_reminders_and_says_why_it_is_empty() {
        let off = display(Language::English, false, Some(true), &[], 0.0);
        assert!(off.note.is_some());
        let locked = display(Language::English, true, Some(false), &[], 0.0);
        assert_eq!(locked.note.as_ref().map(|n| n.0), Some("lock.fill"));
        let empty = display(Language::Vietnamese, true, Some(true), &[], 0.0);
        assert_eq!(
            empty.note.as_ref().map(|n| n.1.as_str()),
            Some("Chưa có thư mới")
        );
        let list = display(
            Language::Vietnamese,
            true,
            Some(true),
            &[
                notice(OUTLOOK, "Lan", "Báo cáo tuần", "Gửi anh", ""),
                notice(OUTLOOK, "Họp giao ban", "", "10:00, phòng 2", "reminder"),
            ],
            1_000_000.0 + 300.0,
        );
        assert_eq!(list.status, "1 thư, 1 lịch");
        assert_eq!(list.rows[0].detail, "Thư, 5 phút trước: Báo cáo tuần");
        assert_eq!(list.rows[1].detail, "Lịch, 5 phút trước: 10:00, phòng 2");
        assert_eq!(Language::English.outlook_count(2, 0), "2 mails");
    }
}
