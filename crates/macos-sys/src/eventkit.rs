//! Calendar events and reminders through EventKit. Everything here runs on the main thread. Nothing is read
//! until the feature asks, and a permission prompt only ever follows a call to `Store::request`.

use crate::app;
use block2::RcBlock;
use core_calendar::model::{Event, Reminder};
use objc2::{
    AnyThread,
    rc::Retained,
    runtime::{AnyObject, Bool, NSObjectProtocol, ProtocolObject},
    sel,
};
use objc2_event_kit::{
    EKAuthorizationStatus, EKCalendar, EKCalendarItem, EKEntityType, EKEventStatus, EKEventStore,
    EKEventStoreChangedNotification, EKParticipantStatus, EKReminder,
};
use objc2_foundation::{
    NSArray, NSBundle, NSCalendar, NSDate, NSDateComponentUndefined, NSDateComponents, NSError,
    NSNotification, NSNotificationCenter, NSNotificationName, NSOperationQueue, NSString,
    NSSystemClockDidChangeNotification, NSSystemTimeZoneDidChangeNotification, NSTimeZone,
};
use std::{cell::RefCell, collections::HashMap, ptr::NonNull, rc::Rc};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Events,
    Reminders,
}

impl Kind {
    fn entity(self) -> EKEntityType {
        match self {
            Kind::Events => EKEntityType::Event,
            Kind::Reminders => EKEntityType::Reminder,
        }
    }

    /// The Info.plist key without which macOS ends the process when access is requested.
    fn usage_key(self) -> &'static str {
        match self {
            Kind::Events => "NSCalendarsFullAccessUsageDescription",
            Kind::Reminders => "NSRemindersFullAccessUsageDescription",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Access {
    NotDetermined,
    Restricted,
    Denied,
    /// May add events but not read them, which is no use here.
    WriteOnly,
    Granted,
}

impl Access {
    pub const fn code(self) -> &'static str {
        match self {
            Access::NotDetermined => "not_determined",
            Access::Restricted => "restricted",
            Access::Denied => "denied",
            Access::WriteOnly => "write_only",
            Access::Granted => "granted",
        }
    }
}

pub fn access(kind: Kind) -> Access {
    // SAFETY: a class method that only reads the privacy database.
    let status = unsafe { EKEventStore::authorizationStatusForEntityType(kind.entity()) };
    match status {
        EKAuthorizationStatus::NotDetermined => Access::NotDetermined,
        EKAuthorizationStatus::Restricted => Access::Restricted,
        EKAuthorizationStatus::Denied => Access::Denied,
        EKAuthorizationStatus::WriteOnly => Access::WriteOnly,
        EKAuthorizationStatus::FullAccess => Access::Granted,
        _ => Access::Denied,
    }
}

/// Whether this build carries the usage text that the system prompt needs. A program run from a build folder
/// has no Info.plist; asking then would end the process, so the request is refused instead.
pub fn can_prompt(kind: Kind) -> bool {
    NSBundle::mainBundle()
        .objectForInfoDictionaryKey(&NSString::from_str(kind.usage_key()))
        .is_some()
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CalendarInfo {
    pub id: String,
    pub title: String,
    pub source: String,
    /// 0xRRGGBB.
    pub color: u32,
}

type AccessDone = Box<dyn FnOnce(bool)>;
type RemindersDone = Box<dyn FnOnce(Vec<Reminder>)>;

thread_local! {
    static NEXT: RefCell<u64> = const { RefCell::new(0) };
    /// The completion handlers of EventKit run on any thread; what they find here runs on the main one.
    static ACCESS: RefCell<HashMap<u64, AccessDone>> = RefCell::new(HashMap::new());
    static FETCHES: RefCell<HashMap<u64, RemindersDone>> = RefCell::new(HashMap::new());
}

fn next_id() -> u64 {
    NEXT.with(|n| {
        let mut n = n.borrow_mut();
        *n += 1;
        *n
    })
}

fn date(t: i64) -> Retained<NSDate> {
    NSDate::dateWithTimeIntervalSince1970(t as f64)
}

fn seconds(date: &NSDate) -> i64 {
    date.timeIntervalSince1970().floor() as i64
}

/// The offset of the local time zone from UTC at an instant, in seconds.
pub fn utc_offset(t: i64) -> i32 {
    NSTimeZone::localTimeZone().secondsFromGMTForDate(&date(t)) as i32
}

pub fn now() -> i64 {
    seconds(&NSDate::now())
}

fn text(s: Option<Retained<NSString>>) -> String {
    s.map(|s| s.to_string()).unwrap_or_default()
}

fn rgb(calendar: &EKCalendar) -> u32 {
    // SAFETY: a plain property read.
    let color = unsafe { calendar.color() };
    let srgb = color
        .colorUsingColorSpace(&objc2_app_kit::NSColorSpace::sRGBColorSpace())
        .unwrap_or(color);
    let byte = |c: f64| (c.clamp(0.0, 1.0) * 255.0).round() as u32;
    byte(srgb.redComponent()) << 16 | byte(srgb.greenComponent()) << 8 | byte(srgb.blueComponent())
}

fn calendar_info(calendar: &EKCalendar) -> CalendarInfo {
    // SAFETY: plain property reads on a calendar of the live store.
    unsafe {
        CalendarInfo {
            id: calendar.calendarIdentifier().to_string(),
            title: calendar.title().to_string(),
            source: calendar
                .source()
                .map(|s| s.title().to_string())
                .unwrap_or_default(),
            color: rgb(calendar),
        }
    }
}

/// The moment a reminder is due, read from its date components, and whether it has no time of day.
fn due_of(components: &NSDateComponents) -> Option<(i64, bool)> {
    let calendar = components
        .calendar()
        .unwrap_or_else(NSCalendar::currentCalendar);
    let date_only = components.hour() == NSDateComponentUndefined;
    calendar
        .dateFromComponents(components)
        .map(|d| (seconds(&d), date_only))
}

fn reminder_of(item: &EKReminder) -> Reminder {
    // SAFETY: plain property reads on a reminder of the live store.
    unsafe {
        let calendar = item.calendar();
        let due = item.dueDateComponents().and_then(|c| due_of(&c));
        Reminder {
            id: item.calendarItemIdentifier().to_string(),
            title: item.title().to_string(),
            due: due.map(|(t, _)| t),
            date_only: due.is_some_and(|(_, d)| d),
            calendar: calendar
                .as_ref()
                .map(|c| c.title().to_string())
                .unwrap_or_default(),
            color: calendar.as_deref().map(rgb).unwrap_or(0x8e8e93),
        }
    }
}

pub struct Store {
    store: Retained<EKEventStore>,
}

impl Store {
    pub fn new() -> Self {
        // SAFETY: `init` is the designated initializer; it does not ask for access.
        let store = unsafe { EKEventStore::init(EKEventStore::alloc()) };
        Self { store }
    }

    /// Asks the system for access, which shows its prompt the first time. `done` runs on the main thread with
    /// whether access was given. `Err` when this build cannot ask without ending the process.
    pub fn request(&self, kind: Kind, done: impl FnOnce(bool) + 'static) -> Result<(), String> {
        if !can_prompt(kind) {
            return Err(format!("the app has no {} text", kind.usage_key()));
        }
        let id = next_id();
        ACCESS.with(|map| map.borrow_mut().insert(id, Box::new(done)));
        let block = RcBlock::new(move |granted: Bool, _error: *mut NSError| {
            let granted = granted.as_bool();
            app::on_main(move || {
                if let Some(done) = ACCESS.with(|map| map.borrow_mut().remove(&id)) {
                    done(granted);
                }
            });
        });
        let handler = RcBlock::as_ptr(&block);
        // SAFETY: the handler is a valid block that EventKit copies if it keeps it.
        unsafe {
            let store = &self.store;
            match kind {
                Kind::Events
                    if store.respondsToSelector(sel!(requestFullAccessToEventsWithCompletion:)) =>
                {
                    store.requestFullAccessToEventsWithCompletion(handler)
                }
                Kind::Reminders
                    if store
                        .respondsToSelector(sel!(requestFullAccessToRemindersWithCompletion:)) =>
                {
                    store.requestFullAccessToRemindersWithCompletion(handler)
                }
                #[allow(deprecated)]
                _ => store.requestAccessToEntityType_completion(kind.entity(), handler),
            }
        }
        Ok(())
    }

    fn calendars_of(&self, kind: Kind) -> Vec<Retained<EKCalendar>> {
        // SAFETY: a plain query of the live store.
        unsafe { self.store.calendarsForEntityType(kind.entity()) }
            .iter()
            .collect()
    }

    /// The calendars of the system for events or reminders, in the order the system lists them.
    pub fn calendars(&self, kind: Kind) -> Vec<CalendarInfo> {
        self.calendars_of(kind)
            .iter()
            .map(|c| calendar_info(c))
            .collect()
    }

    fn included(&self, kind: Kind, excluded: &[String]) -> Retained<NSArray<EKCalendar>> {
        let list: Vec<Retained<EKCalendar>> = self
            .calendars_of(kind)
            .into_iter()
            // SAFETY: a plain property read.
            .filter(|c| !excluded.contains(&unsafe { c.calendarIdentifier() }.to_string()))
            .collect();
        NSArray::from_retained_slice(&list)
    }

    /// The occurrences of events that overlap `from..to`, recurring events expanded by EventKit and each
    /// starting at its own start. Cancelled events and those the person declined are left out.
    pub fn events(&self, from: i64, to: i64, excluded: &[String]) -> Vec<Event> {
        let calendars = self.included(Kind::Events, excluded);
        if calendars.is_empty() {
            return Vec::new();
        }
        // SAFETY: the predicate comes from this store and is only used with it; the events are read at once.
        let found = unsafe {
            let predicate = self
                .store
                .predicateForEventsWithStartDate_endDate_calendars(
                    &date(from),
                    &date(to),
                    Some(&calendars),
                );
            self.store.eventsMatchingPredicate(&predicate)
        };
        found
            .iter()
            .filter_map(|e| {
                // SAFETY: plain property reads on an event of the live store.
                unsafe {
                    if e.status() == EKEventStatus::Canceled {
                        return None;
                    }
                    let declined = e.attendees().is_some_and(|list| {
                        list.iter().any(|p| {
                            p.isCurrentUser()
                                && p.participantStatus() == EKParticipantStatus::Declined
                        })
                    });
                    if declined {
                        return None;
                    }
                    let calendar = e.calendar();
                    let (start, end) = (seconds(&e.startDate()), seconds(&e.endDate()));
                    Some(Event {
                        id: e
                            .eventIdentifier()
                            .unwrap_or_else(|| e.calendarItemIdentifier())
                            .to_string(),
                        title: e.title().to_string(),
                        start,
                        end: end.max(start),
                        all_day: e.isAllDay(),
                        calendar_id: calendar
                            .as_ref()
                            .map(|c| c.calendarIdentifier().to_string())
                            .unwrap_or_default(),
                        calendar: calendar
                            .as_ref()
                            .map(|c| c.title().to_string())
                            .unwrap_or_default(),
                        color: calendar.as_deref().map(rgb).unwrap_or(0x8e8e93),
                        location: text(e.location()),
                        notes: text(e.notes()),
                        url: e
                            .URL()
                            .and_then(|u| u.absoluteString())
                            .map(|s| s.to_string())
                            .unwrap_or_default(),
                    })
                }
            })
            .collect()
    }

    /// Reminders that are not completed and are due before `before`, which includes those long overdue.
    /// `done` runs on the main thread. Reminders without a date are not included.
    pub fn reminders(
        &self,
        before: i64,
        excluded: &[String],
        done: impl FnOnce(Vec<Reminder>) + 'static,
    ) {
        let calendars = self.included(Kind::Reminders, excluded);
        if calendars.is_empty() {
            done(Vec::new());
            return;
        }
        let id = next_id();
        FETCHES.with(|map| map.borrow_mut().insert(id, Box::new(done)));
        let block = RcBlock::new(move |list: *mut NSArray<EKReminder>| {
            // SAFETY: EventKit passes null or an array that is valid for the duration of this call.
            let found: Vec<Reminder> = unsafe { list.as_ref() }
                .map(|list| list.iter().map(|r| reminder_of(&r)).collect())
                .unwrap_or_default();
            app::on_main(move || {
                if let Some(done) = FETCHES.with(|map| map.borrow_mut().remove(&id)) {
                    done(found);
                }
            });
        });
        // SAFETY: the predicate comes from this store; the block is copied by EventKit.
        unsafe {
            let predicate = self
                .store
                .predicateForIncompleteRemindersWithDueDateStarting_ending_calendars(
                    None,
                    Some(&date(before)),
                    Some(&calendars),
                );
            self.store
                .fetchRemindersMatchingPredicate_completion(&predicate, &block);
        }
    }

    /// Marks one reminder completed. This is the only thing this module ever writes.
    pub fn complete_reminder(&self, id: &str) -> Result<(), String> {
        // SAFETY: the item is looked up in, changed through and saved to this one store.
        unsafe {
            let item = self
                .store
                .calendarItemWithIdentifier(&NSString::from_str(id))
                .and_then(|item| Retained::<EKCalendarItem>::downcast::<EKReminder>(item).ok())
                .ok_or("reminder not found")?;
            item.setCompleted(true);
            self.store
                .saveReminder_commit_error(&item, true)
                .map_err(|e| e.localizedDescription().to_string())
        }
    }

    /// Calls `changed` on the main thread when the calendar database changes or access is given or taken away.
    pub fn observe_changes(&self, changed: impl Fn() + 'static) -> Observer {
        // SAFETY: the name is a process-lifetime constant.
        observe(
            Some(&self.store),
            &[unsafe { EKEventStoreChangedNotification }],
            changed,
        )
    }
}

impl Default for Store {
    fn default() -> Self {
        Self::new()
    }
}

/// Unregisters a notification observer when dropped.
pub struct Observer {
    center: Retained<NSNotificationCenter>,
    tokens: Vec<Retained<ProtocolObject<dyn NSObjectProtocol>>>,
}

impl Drop for Observer {
    fn drop(&mut self) {
        for token in &self.tokens {
            // SAFETY: each token was returned by this center's addObserverForName.
            unsafe {
                self.center
                    .removeObserver(AsRef::<AnyObject>::as_ref(&**token))
            };
        }
    }
}

fn observe(
    object: Option<&EKEventStore>,
    names: &[&NSNotificationName],
    f: impl Fn() + 'static,
) -> Observer {
    let center = NSNotificationCenter::defaultCenter();
    let f = Rc::new(f);
    let sender = object.map(|o| o as &AnyObject);
    let tokens = names
        .iter()
        .map(|name| {
            let f = Rc::clone(&f);
            let block = RcBlock::new(move |_: NonNull<NSNotification>| f());
            // SAFETY: the block is 'static and runs on the main queue.
            unsafe {
                center.addObserverForName_object_queue_usingBlock(
                    Some(name),
                    sender,
                    Some(&NSOperationQueue::mainQueue()),
                    &block,
                )
            }
        })
        .collect();
    Observer { center, tokens }
}

/// Calls `changed` when the clock is set or the time zone changes, which move every alert and midnight.
pub fn observe_clock(changed: impl Fn() + 'static) -> Observer {
    // SAFETY: the names are process-lifetime constants.
    let names = unsafe {
        [
            NSSystemClockDidChangeNotification,
            NSSystemTimeZoneDidChangeNotification,
        ]
    };
    observe(None, &names, changed)
}
