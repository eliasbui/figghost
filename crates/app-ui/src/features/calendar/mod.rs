//! Calendar, reminders and meeting alerts on the notch.
//!
//! EventKit is read only while the feature is on and access was given; nothing polls. One timer is armed for
//! the earliest of the next alert and midnight, and the database change notification refreshes the page.

use super::AttachEnv;
use crate::{i18n, settings::model::Fault, trace};
use core_calendar::{
    format,
    model::{Event, Reminder},
    schedule::{MAX_LEAD, MIN_LEAD, Scheduler},
};
use macos_sys::{
    MainThreadMarker, app,
    calendar_ui::{CalendarCallbacks, CalendarView},
    eventkit::{self, Access, CalendarInfo, Kind, Observer, Store},
    notch::Notch,
    status_item::Entry,
    workspace,
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{
    cell::{Cell, RefCell},
    rc::{Rc, Weak},
};

mod display;
pub mod settings;
mod strings;

/// How many calendars may be switched off in the file; more would be hostile.
pub const MAX_EXCLUDED: usize = 200;
/// How long the notch stays open for an alert, unless the pointer is on it.
const PEEK_SECONDS: f64 = 8.0;
/// Events are read this far ahead, so alerts near midnight see tomorrow's first meetings.
const LOOKAHEAD: i64 = 3 * 24 * 3600;
const ALERT_SOUND: &str = "Ping";

/// A calendar identifier is short printable text.
pub fn valid_id(id: &str) -> bool {
    !id.is_empty() && id.len() <= 200 && !id.chars().any(char::is_control)
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    pub enabled: bool,
    /// Minutes before an event that the alert comes.
    pub lead_minutes: u32,
    pub sound: bool,
    pub reminders: bool,
    pub all_day: bool,
    /// Identifiers of the calendars that are left out; a new calendar is in until it is switched off.
    pub excluded: Vec<String>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            enabled: false,
            lead_minutes: 5,
            sound: true,
            reminders: true,
            all_day: true,
            excluded: Vec::new(),
        }
    }
}

impl Config {
    /// What a hand-edited file may say is brought into range.
    pub fn sanitized(mut self) -> Self {
        self.lead_minutes = self.lead_minutes.clamp(MIN_LEAD, MAX_LEAD);
        self.excluded.retain(|id| valid_id(id));
        self.excluded.sort();
        self.excluded.dedup();
        self.excluded.truncate(MAX_EXCLUDED);
        self
    }
}

#[derive(Default)]
struct Data {
    events: Vec<Event>,
    reminders: Vec<Reminder>,
    calendars: Vec<CalendarInfo>,
    lists: Vec<CalendarInfo>,
}

/// The running feature. Created by `attach`, kept alive by the app context.
pub struct Feature {
    mtm: MainThreadMarker,
    notch: Weak<Notch>,
    page: usize,
    view: Rc<CalendarView>,
    config: RefCell<Config>,
    /// Present while the feature is on.
    store: RefCell<Option<Store>>,
    /// Asks for access while the feature is off, when the person presses a button on the settings page.
    asker: RefCell<Option<Store>>,
    watchers: RefCell<Vec<Observer>>,
    data: RefCell<Data>,
    scheduler: RefCell<Scheduler>,
    /// Bumped whenever the timer is re-armed; a timer that finds another number does nothing.
    epoch: Cell<u64>,
    refresh_queued: Cell<bool>,
    visible: Cell<bool>,
    /// The notch was closed when an alert opened it, so it may be closed again.
    opened_by_us: Cell<bool>,
    me: Weak<Feature>,
}

/// Starts the feature; `None` when it cannot run (no notch to put a page in, for example).
pub fn attach(env: &AttachEnv) -> Option<Rc<Feature>> {
    let notch = env.notch?;
    let config = env.config.calendar.clone().sanitized();
    let language = i18n::current();
    let feature = Rc::new_cyclic(|me: &Weak<Feature>| {
        let weak = |me: &Weak<Feature>| me.clone();
        let (join, dismiss, complete, action) = (weak(me), weak(me), weak(me), weak(me));
        let view = CalendarView::new(
            env.mtm,
            notch.page_frame(),
            language.calendar_hint(),
            CalendarCallbacks {
                on_join: Box::new(move |key| {
                    if let Some(f) = join.upgrade() {
                        f.join(key);
                    }
                }),
                on_dismiss: Box::new(move |key| {
                    if let Some(f) = dismiss.upgrade() {
                        f.dismiss(key);
                    }
                }),
                on_complete: Box::new(move |id| {
                    if let Some(f) = complete.upgrade() {
                        f.complete(id);
                    }
                }),
                on_action: Box::new(move || {
                    if let Some(f) = action.upgrade() {
                        f.page_action();
                    }
                }),
            },
        );
        Feature {
            mtm: env.mtm,
            notch: Rc::downgrade(notch),
            page: notch.page_count(),
            view,
            scheduler: RefCell::new(Scheduler::new(config.lead_minutes)),
            config: RefCell::new(config),
            store: RefCell::default(),
            asker: RefCell::default(),
            watchers: RefCell::default(),
            data: RefCell::default(),
            epoch: Cell::new(0),
            refresh_queued: Cell::new(false),
            visible: Cell::new(false),
            opened_by_us: Cell::new(false),
            me: me.clone(),
        }
    });
    let (shown, hidden) = (Rc::downgrade(&feature), Rc::downgrade(&feature));
    notch.add_page(feature.view.page(
        language.calendar_tab(),
        move || {
            if let Some(f) = shown.upgrade() {
                f.visible.set(true);
                f.refresh(false);
            }
        },
        move || {
            if let Some(f) = hidden.upgrade() {
                f.visible.set(false);
            }
        },
    ));
    // Never asks for access here: a launch must not show a permission prompt. The page offers a button.
    if feature.config.borrow().enabled {
        feature.start();
        feature.refresh(true);
    }
    Some(feature)
}

impl Feature {
    /// Applies the settings after a change. An error is shown on the settings page and the change is undone.
    pub fn configure(&self, config: &Config) -> Result<(), Fault> {
        let config = config.clone().sanitized();
        let was = self.config.replace(config.clone());
        self.scheduler.borrow_mut().set_lead(config.lead_minutes);
        match (was.enabled, config.enabled) {
            (_, false) => {
                if was.enabled {
                    self.stop();
                }
                self.render();
            }
            (false, true) => {
                self.start();
                self.ask_for_missing();
                // A setting that is changed does not make the notch jump open with a sound.
                self.refresh(false);
            }
            (true, true) => {
                if config.reminders && !was.reminders {
                    self.ask_for_missing();
                }
                self.refresh(false);
            }
        }
        Ok(())
    }

    /// Releases the global shortcuts while the settings page records a new one.
    pub fn suspend_hotkeys(&self) {}

    /// Registers the global shortcuts again from the config.
    pub fn resume_hotkeys(&self, _config: &Config) {}

    /// Facts only the running app knows: the permissions and the calendars of the system.
    pub fn runtime(&self) -> Value {
        let data = self.data.borrow();
        let listing = |list: &[CalendarInfo]| -> Vec<Value> {
            list.iter()
                .map(|c| {
                    json!({
                        "id": c.id,
                        "title": c.title,
                        "source": c.source,
                        "color": format!("#{:06x}", c.color & 0xff_ffff),
                    })
                })
                .collect()
        };
        json!({
            "events_access": eventkit::access(Kind::Events).code(),
            "reminders_access": eventkit::access(Kind::Reminders).code(),
            "can_prompt": eventkit::can_prompt(Kind::Events) && eventkit::can_prompt(Kind::Reminders),
            "calendars": listing(&data.calendars),
            "lists": listing(&data.lists),
        })
    }

    /// A button of the settings page. `None` when the name is not this feature's.
    pub fn action(&self, name: &str, _config: &mut Config) -> Option<Result<(), Fault>> {
        let kind = match name {
            "calendar_access_events" => Kind::Events,
            "calendar_access_reminders" => Kind::Reminders,
            _ => return None,
        };
        self.open_access(kind);
        Some(Ok(()))
    }

    /// Entries for the menu bar item, between "Open clipboard" and the login item.
    pub fn menu_entries(_this: &Rc<Self>) -> Vec<Entry> {
        Vec::new()
    }

    fn start(&self) {
        if self.store.borrow().is_some() {
            return;
        }
        let store = Store::new();
        let (changed, clock) = (self.me.clone(), self.me.clone());
        let mut watchers = vec![
            store.observe_changes(move || {
                if let Some(f) = changed.upgrade() {
                    f.queue_refresh();
                }
            }),
            eventkit::observe_clock(move || {
                if let Some(f) = clock.upgrade() {
                    f.queue_refresh();
                }
            }),
        ];
        self.watchers.borrow_mut().append(&mut watchers);
        *self.store.borrow_mut() = Some(store);
    }

    fn stop(&self) {
        self.store.borrow_mut().take();
        self.watchers.borrow_mut().clear();
        *self.data.borrow_mut() = Data::default();
        *self.scheduler.borrow_mut() = Scheduler::new(self.config.borrow().lead_minutes);
        // A timer that is still pending finds a newer number and does nothing.
        self.epoch.set(self.epoch.get() + 1);
        self.update_badge();
    }

    /// The database changes in bursts while an account syncs; one refresh covers a burst.
    fn queue_refresh(&self) {
        if self.refresh_queued.replace(true) {
            return;
        }
        let weak = self.me.clone();
        app::after(0.5, move || {
            if let Some(f) = weak.upgrade() {
                f.refresh_queued.set(false);
                f.refresh(true);
            }
        });
    }

    /// Reads the events again, fires the alerts that are due and arms the timer for the next one. `loud` lets a
    /// due alert open the notch and play its sound.
    fn refresh(&self, loud: bool) {
        let config = self.config.borrow().clone();
        if !config.enabled || self.store.borrow().is_none() {
            self.render();
            return;
        }
        let now = eventkit::now();
        let events_ok = eventkit::access(Kind::Events) == Access::Granted;
        let reminders_ok = config.reminders && eventkit::access(Kind::Reminders) == Access::Granted;
        let fired = {
            let store = self.store.borrow();
            let Some(store) = store.as_ref() else {
                return;
            };
            let mut data = self.data.borrow_mut();
            let mut events = if events_ok {
                store.events(now, now + LOOKAHEAD, &config.excluded)
            } else {
                Vec::new()
            };
            if !config.all_day {
                events.retain(|e| !e.all_day);
            }
            data.calendars = if events_ok {
                store.calendars(Kind::Events)
            } else {
                Vec::new()
            };
            data.lists = if reminders_ok || eventkit::access(Kind::Reminders) == Access::Granted {
                store.calendars(Kind::Reminders)
            } else {
                Vec::new()
            };
            if !reminders_ok {
                data.reminders.clear();
            }
            data.events = events;
            self.scheduler.borrow_mut().tick(&data.events, now)
        };
        if reminders_ok {
            self.fetch_reminders(now, &config);
        }
        if loud && !fired.is_empty() {
            if config.sound {
                workspace::play_sound(ALERT_SOUND);
            }
            self.peek();
        }
        trace::log(&format!(
            "calendar: events {} reminders {}: {} events, {} pending alerts, {} fired{}",
            eventkit::access(Kind::Events).code(),
            eventkit::access(Kind::Reminders).code(),
            self.data.borrow().events.len(),
            self.pending().len(),
            fired.len(),
            if loud { "" } else { " (quiet)" }
        ));
        self.update_badge();
        self.render();
        self.arm(now);
    }

    fn fetch_reminders(&self, now: i64, config: &Config) {
        let store = self.store.borrow();
        let Some(store) = store.as_ref() else {
            return;
        };
        let today = format::day_index(now, &eventkit::utc_offset);
        // Everything up to the end of tomorrow, and everything overdue before that.
        let before = format::day_start(today + 2, &eventkit::utc_offset);
        let weak = self.me.clone();
        store.reminders(before, &config.excluded, move |list| {
            if let Some(f) = weak.upgrade()
                && f.config.borrow().enabled
                && f.store.borrow().is_some()
            {
                f.data.borrow_mut().reminders = list;
                f.render();
            }
        });
    }

    /// One timer, for the earliest of the next alert and midnight, when the days change.
    fn arm(&self, now: i64) {
        let epoch = self.epoch.get() + 1;
        self.epoch.set(epoch);
        let today = format::day_index(now, &eventkit::utc_offset);
        let midnight = format::day_start(today + 1, &eventkit::utc_offset);
        let next = self
            .scheduler
            .borrow()
            .next_wake(&self.data.borrow().events, now)
            .map_or(midnight, |t| t.min(midnight));
        let weak = self.me.clone();
        app::after((next - now).max(1) as f64, move || {
            if let Some(f) = weak.upgrade()
                && f.epoch.get() == epoch
            {
                f.refresh(true);
            }
        });
    }

    fn pending(&self) -> Vec<usize> {
        self.scheduler
            .borrow()
            .pending(&self.data.borrow().events, eventkit::now())
    }

    fn update_badge(&self) {
        if let Some(notch) = self.notch.upgrade() {
            notch.set_page_badge(self.page, !self.pending().is_empty());
        }
    }

    /// Redraws the page, if it is the one on screen.
    fn render(&self) {
        if !self.visible.get() {
            return;
        }
        let (config, data) = (self.config.borrow(), self.data.borrow());
        let pending = self.pending();
        let display = display::build(&display::Input {
            language: i18n::current(),
            now: eventkit::now(),
            offset: &eventkit::utc_offset,
            config: &config,
            events: &data.events,
            reminders: &data.reminders,
            pending: &pending,
            events_access: eventkit::access(Kind::Events),
            reminders_access: eventkit::access(Kind::Reminders),
            can_prompt: eventkit::can_prompt(Kind::Events),
        });
        self.view.show(self.mtm, &display);
    }

    /// Opens the notch on this page for a few seconds, remembering whether it was closed before.
    fn peek(&self) {
        let Some(notch) = self.notch.upgrade() else {
            return;
        };
        if !notch.is_expanded() {
            self.opened_by_us.set(true);
        }
        notch.open_page(self.page);
        self.close_later(PEEK_SECONDS);
    }

    fn close_later(&self, seconds: f64) {
        let weak = self.me.clone();
        app::after(seconds, move || {
            if let Some(f) = weak.upgrade() {
                f.close_if_ours();
            }
        });
    }

    /// Closes the notch if an alert opened it and the pointer is elsewhere; with the pointer on it, the notch
    /// closes by itself when the pointer leaves.
    fn close_if_ours(&self) {
        let Some(notch) = self.notch.upgrade() else {
            return;
        };
        if self.opened_by_us.get() && notch.is_expanded() && !notch.pointer_inside() {
            notch.close();
        }
        self.opened_by_us.set(false);
    }

    fn event(&self, key: &str) -> Option<Event> {
        self.data
            .borrow()
            .events
            .iter()
            .find(|e| e.key() == key)
            .cloned()
    }

    fn dismiss(&self, key: &str) {
        if let Some(event) = self.event(key) {
            self.scheduler.borrow_mut().dismiss(&event);
        }
        self.update_badge();
        self.render();
        if self.pending().is_empty() {
            self.close_later(0.6);
        }
    }

    /// Opens the meeting link in the default browser. The link is found again here from the event, never
    /// taken from the page, so only a link the parser accepted can be opened.
    fn join(&self, key: &str) {
        if let Some(event) = self.event(key)
            && let Some(link) = event.link()
        {
            workspace::open_url(&link.url);
        }
        self.dismiss(key);
    }

    fn complete(&self, id: &str) {
        let done = self
            .store
            .borrow()
            .as_ref()
            .map(|store| store.complete_reminder(id));
        match done {
            Some(Ok(())) => {
                self.data.borrow_mut().reminders.retain(|r| r.id != id);
                self.render();
            }
            Some(Err(e)) => eprintln!("calendar: cannot complete the reminder: {e}"),
            None => {}
        }
    }

    /// The button of the page's note: ask the first time, afterwards open the privacy pane.
    fn page_action(&self) {
        self.open_access(Kind::Events);
    }

    /// Asks for access when the system has not been asked yet, and otherwise opens the pane where the person
    /// can change it.
    fn open_access(&self, kind: Kind) {
        if eventkit::access(kind) == Access::NotDetermined {
            self.request(kind);
        } else {
            workspace::open_url(match kind {
                Kind::Events => {
                    "x-apple.systempreferences:com.apple.preference.security?Privacy_Calendars"
                }
                Kind::Reminders => {
                    "x-apple.systempreferences:com.apple.preference.security?Privacy_Reminders"
                }
            });
        }
    }

    /// Turning the feature on is the moment to ask; reminders follow once events were answered.
    fn ask_for_missing(&self) {
        if eventkit::access(Kind::Events) == Access::NotDetermined {
            self.request(Kind::Events);
        } else if self.config.borrow().reminders
            && eventkit::access(Kind::Reminders) == Access::NotDetermined
        {
            self.request(Kind::Reminders);
        }
    }

    fn request(&self, kind: Kind) {
        trace::log(&format!("calendar: asking for {kind:?} access"));
        let weak = self.me.clone();
        let store = self.store.borrow();
        let mut asker = self.asker.borrow_mut();
        // The store that asks stays alive until the answer comes, or the system may drop the prompt.
        let asking = store
            .as_ref()
            .unwrap_or_else(|| asker.get_or_insert_with(Store::new));
        let result = asking.request(kind, move |_granted| {
            if let Some(f) = weak.upgrade() {
                f.access_changed();
            }
        });
        if let Err(e) = result {
            eprintln!("calendar: cannot ask for access: {e}");
        }
    }

    /// An answer to a permission prompt: read with a fresh store, and ask for the next permission if there is
    /// one.
    fn access_changed(&self) {
        self.asker.borrow_mut().take();
        if self.config.borrow().enabled {
            self.watchers.borrow_mut().clear();
            self.store.borrow_mut().take();
            self.start();
            self.ask_for_missing();
            self.refresh(false);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_feature_is_off_by_default_and_old_files_load() {
        let config: Config = serde_json::from_str("{}").unwrap();
        assert_eq!(config, Config::default());
        assert!(!config.enabled);
        assert_eq!(config.lead_minutes, 5);
        let partial: Config =
            serde_json::from_str(r#"{ "enabled": true, "future_key": 1 }"#).unwrap();
        assert!(partial.enabled && partial.sound && partial.reminders && partial.all_day);
    }

    #[test]
    fn a_hostile_file_is_brought_into_range() {
        let text = format!(
            r#"{{ "lead_minutes": 99999, "excluded": ["b", "a", "a", "", "x\ny", "{}"] }}"#,
            "z".repeat(500)
        );
        let config: Config = serde_json::from_str(&text).unwrap();
        let config = config.sanitized();
        assert_eq!(config.lead_minutes, 60);
        assert_eq!(config.excluded, ["a", "b"]);
        let many = Config {
            excluded: (0..1000).map(|i| format!("id-{i}")).collect(),
            ..Config::default()
        };
        assert_eq!(many.sanitized().excluded.len(), MAX_EXCLUDED);
        let zero = Config {
            lead_minutes: 0,
            ..Config::default()
        };
        assert_eq!(zero.sanitized().lead_minutes, 1);
    }

    #[test]
    fn identifiers_are_short_printable_text() {
        assert!(valid_id("3F2A9C1E-7B44-4E0B-9A55-0C6D2B1E8F77"));
        assert!(!valid_id(""));
        assert!(!valid_id("tab\there"));
        assert!(!valid_id(&"x".repeat(201)));
    }
}
