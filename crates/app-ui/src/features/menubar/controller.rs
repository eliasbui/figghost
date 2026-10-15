//! Hides and shows the icons: runs the rules of `logic`, measures what the system lays out, and keeps what it
//! learnt. The bar itself is a trait so that every path, including a system that drops long items from the layout,
//! can be run with a pretend one.

use super::logic::{Action, Machine, Rules, SEPARATOR_WIDTH, Search, Step, first_guess, honored};
use std::{
    cell::RefCell,
    rc::{Rc, Weak},
};

/// How long the system is given to lay a changed divider out before it is measured.
const SETTLE_SECONDS: f64 = 0.08;
/// How long after a collapse it is checked that the icons really went.
const VERIFY_SECONDS: f64 = 0.25;
/// How many times one request to hide may start over because the length stopped working.
const MAX_RETRIES: u8 = 2;

/// The two status items.
pub trait Bar {
    /// Sets the length of the divider.
    fn set_length(&self, points: f64);
    /// The distance between the right edge of the divider and the arrow, or `None` when it cannot be read.
    fn gap(&self) -> Option<f64>;
    /// Draws the arrow for the state: pointing at what a click will do.
    fn look(&self, collapsed: bool);
    fn widest_screen(&self) -> f64;
    /// Runs `f` once, soon.
    fn after(&self, seconds: f64, f: Box<dyn FnOnce()>);
}

/// What can change the situation from outside.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Event {
    /// The arrow was clicked, or the menu asked.
    Click,
    /// The pointer rested on the arrow.
    Hover,
    /// A screen came, went or changed size.
    Screens,
    /// Another app came to the front, which can change how much room the app menus take.
    AppSwitched,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Status {
    #[default]
    Showing,
    Hiding,
    /// Looking for a divider length the system lays out.
    Measuring,
    /// No length works here; the icons stay showing.
    Unavailable,
}

struct Inner {
    machine: Machine,
    status: Status,
    /// The gap between the divider and the arrow with a short divider.
    baseline: f64,
    /// The longest length known to be laid out here, until the screens or the room change.
    length: Option<f64>,
    /// Numbers a measurement, so that one that was overtaken by another does not go on.
    generation: u64,
    retries: u8,
    search: Option<Search>,
}

pub struct Controller<B> {
    bar: B,
    inner: RefCell<Inner>,
    me: Weak<Controller<B>>,
}

impl<B: Bar + 'static> Controller<B> {
    /// Starts with the icons showing. Build it with `Rc::new_cyclic` so that it can hand itself to timers.
    pub fn new(bar: B, rules: Rules, me: Weak<Self>) -> Self {
        let (machine, actions) = Machine::new(rules);
        let this = Self {
            bar,
            inner: RefCell::new(Inner {
                machine,
                status: Status::Showing,
                baseline: 0.0,
                length: None,
                generation: 0,
                retries: 0,
                search: None,
            }),
            me,
        };
        this.bar.set_length(SEPARATOR_WIDTH);
        this.bar.look(false);
        this.run(actions);
        this.after(SETTLE_SECONDS, |c| c.measure_baseline());
        this
    }

    pub fn status(&self) -> Status {
        self.inner.borrow().status
    }

    pub fn is_collapsed(&self) -> bool {
        self.inner.borrow().machine.is_collapsed()
    }

    pub fn configure(&self, rules: Rules) {
        let actions = self.inner.borrow_mut().machine.configured(rules);
        self.run(actions);
    }

    pub fn event(&self, event: Event) {
        match event {
            Event::Click => {
                self.inner.borrow_mut().retries = 0;
                let actions = self.inner.borrow_mut().machine.clicked();
                self.run(actions);
            }
            Event::Hover => {
                let actions = self.inner.borrow_mut().machine.hovered();
                self.run(actions);
            }
            Event::Screens => self.lost_length(),
            Event::AppSwitched => self.check_collapsed(),
        }
    }

    fn after(&self, seconds: f64, f: impl FnOnce(&Rc<Self>) + 'static) {
        let me = self.me.clone();
        self.bar.after(
            seconds,
            Box::new(move || {
                if let Some(me) = me.upgrade() {
                    f(&me);
                }
            }),
        );
    }

    fn run(&self, actions: Vec<Action>) {
        for action in actions {
            match action {
                Action::Expand => self.expand(),
                Action::Collapse => self.collapse(),
                Action::Arm { seconds, epoch } => self.after(f64::from(seconds), move |c| {
                    let actions = c.inner.borrow_mut().machine.timer(epoch);
                    c.run(actions);
                }),
            }
        }
    }

    fn expand(&self) {
        let mut inner = self.inner.borrow_mut();
        // A measurement in progress is overtaken: the icons are wanted now.
        inner.generation += 1;
        inner.search = None;
        inner.status = Status::Showing;
        drop(inner);
        self.bar.set_length(SEPARATOR_WIDTH);
        self.bar.look(false);
    }

    fn collapse(&self) {
        let length = self.inner.borrow().length;
        match length {
            Some(length) => {
                {
                    let mut inner = self.inner.borrow_mut();
                    inner.status = Status::Hiding;
                    inner.generation += 1;
                }
                self.bar.set_length(length);
                self.bar.look(true);
                let generation = self.inner.borrow().generation;
                self.after(VERIFY_SECONDS, move |c| c.verify(generation));
            }
            None => self.measure(),
        }
    }

    /// The short divider's gap, for everything else to be compared with.
    fn measure_baseline(&self) {
        if self.status() == Status::Showing
            && let Some(gap) = self.bar.gap()
        {
            self.inner.borrow_mut().baseline = gap;
        }
    }

    /// After a collapse: did the icons really go? A divider that was dropped from the layout pushes nothing.
    fn verify(&self, generation: u64) {
        let (current, baseline) = {
            let inner = self.inner.borrow();
            (
                inner.generation == generation && inner.status == Status::Hiding,
                inner.baseline,
            )
        };
        if !current {
            return;
        }
        if self.bar.gap().is_none_or(|gap| honored(baseline, gap)) {
            self.inner.borrow_mut().retries = 0;
            return;
        }
        self.lost_length();
    }

    /// The length that worked no longer does, or may not: find it again, and hide again.
    fn lost_length(&self) {
        let mut inner = self.inner.borrow_mut();
        inner.length = None;
        if !inner.machine.is_collapsed() {
            return;
        }
        if inner.retries >= MAX_RETRIES {
            inner.machine.failed();
            inner.status = Status::Unavailable;
            // Whatever measurement is still waiting is overtaken.
            inner.generation += 1;
            drop(inner);
            self.bar.set_length(SEPARATOR_WIDTH);
            self.bar.look(false);
            return;
        }
        inner.retries += 1;
        drop(inner);
        self.measure();
    }

    /// Another app came to the front while the icons are hidden: the room may have changed.
    fn check_collapsed(&self) {
        if self.status() != Status::Hiding {
            return;
        }
        let generation = self.inner.borrow().generation;
        self.after(VERIFY_SECONDS, move |c| c.verify(generation));
    }

    /// Searches for the longest divider the system lays out, with the icons showing while it does, then hides.
    fn measure(&self) {
        let generation = {
            let mut inner = self.inner.borrow_mut();
            inner.generation += 1;
            inner.status = Status::Measuring;
            inner.search = None;
            inner.generation
        };
        self.bar.set_length(SEPARATOR_WIDTH);
        self.after(SETTLE_SECONDS, move |c| {
            if c.inner.borrow().generation != generation {
                return;
            }
            if let Some(gap) = c.bar.gap() {
                c.inner.borrow_mut().baseline = gap;
            }
            let (search, step) = Search::start(first_guess(c.bar.widest_screen()));
            c.inner.borrow_mut().search = Some(search);
            c.probe(step, generation);
        });
    }

    fn probe(&self, step: Step, generation: u64) {
        if self.inner.borrow().generation != generation {
            return;
        }
        match step {
            Step::Probe(length) => {
                self.bar.set_length(length);
                self.after(SETTLE_SECONDS, move |c| {
                    if c.inner.borrow().generation != generation {
                        return;
                    }
                    let baseline = c.inner.borrow().baseline;
                    // A gap that cannot be read cannot say no: the longest length is used as older systems want.
                    let laid_out = c.bar.gap().is_none_or(|gap| honored(baseline, gap));
                    let next = c
                        .inner
                        .borrow_mut()
                        .search
                        .as_mut()
                        .map(|s| s.report(laid_out));
                    if let Some(next) = next {
                        c.probe(next, generation);
                    }
                });
            }
            Step::Done(found) => {
                {
                    let mut inner = self.inner.borrow_mut();
                    inner.search = None;
                    inner.length = found;
                }
                match found {
                    Some(_) => self.collapse(),
                    None => {
                        let mut inner = self.inner.borrow_mut();
                        inner.machine.failed();
                        inner.status = Status::Unavailable;
                        drop(inner);
                        self.bar.set_length(SEPARATOR_WIDTH);
                        self.bar.look(false);
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::{Cell, RefCell};

    type Timer = (f64, Box<dyn FnOnce()>);

    /// A menu bar whose system drops a divider from the layout once it is longer than `limit`.
    struct Pretend {
        length: Cell<f64>,
        limit: Cell<f64>,
        collapsed_look: Cell<bool>,
        timers: RefCell<Vec<Timer>>,
        log: RefCell<Vec<String>>,
        readable: Cell<bool>,
    }

    impl Pretend {
        fn new(limit: f64) -> Rc<Self> {
            Rc::new(Self {
                length: Cell::new(0.0),
                limit: Cell::new(limit),
                collapsed_look: Cell::new(false),
                timers: RefCell::default(),
                log: RefCell::default(),
                readable: Cell::new(true),
            })
        }
    }

    /// Both the controller and the test hold the bar.
    struct Shared(Rc<Pretend>);

    impl Bar for Shared {
        fn set_length(&self, points: f64) {
            self.0.length.set(points);
            self.0.log.borrow_mut().push(format!("length {points}"));
        }
        fn gap(&self) -> Option<f64> {
            let p = &self.0;
            if !p.readable.get() {
                return None;
            }
            // Laid out: the divider's right edge stays by the arrow (a gap of 4). Dropped: it jumps by its length.
            Some(if p.length.get() <= p.limit.get() {
                4.0
            } else {
                4.0 + p.length.get()
            })
        }
        fn look(&self, collapsed: bool) {
            self.0.collapsed_look.set(collapsed);
        }
        fn widest_screen(&self) -> f64 {
            1800.0
        }
        fn after(&self, seconds: f64, f: Box<dyn FnOnce()>) {
            self.0.timers.borrow_mut().push((seconds, f));
        }
    }

    const RULES: Rules = Rules {
        auto_seconds: 10,
        click_only: true,
    };

    fn make(limit: f64, rules: Rules) -> (Rc<Controller<Shared>>, Rc<Pretend>) {
        let bar = Pretend::new(limit);
        let shared = Shared(Rc::clone(&bar));
        let controller = Rc::new_cyclic(|me| Controller::new(shared, rules, me.clone()));
        (controller, bar)
    }

    /// Lets time pass: runs every timer that is due within `seconds`, in order, including the ones they start.
    fn advance(bar: &Pretend, seconds: f64) {
        let mut clock = 0.0;
        let mut due: Vec<Timer> = Vec::new();
        loop {
            for (delay, f) in bar.timers.borrow_mut().drain(..) {
                due.push((clock + delay, f));
            }
            let Some(index) = due
                .iter()
                .enumerate()
                .filter(|(_, (at, _))| *at <= seconds)
                .min_by(|a, b| a.1.0.total_cmp(&b.1.0))
                .map(|(i, _)| i)
            else {
                break;
            };
            let (at, f) = due.remove(index);
            clock = at;
            f();
        }
        // What has not come due yet waits for the next call.
        for (at, f) in due {
            bar.timers.borrow_mut().push((at - seconds, f));
        }
    }

    #[test]
    fn it_starts_with_the_icons_showing_and_hides_them_after_the_delay() {
        let (c, bar) = make(10_000.0, RULES);
        assert_eq!(bar.length.get(), SEPARATOR_WIDTH);
        assert_eq!(c.status(), Status::Showing);
        advance(&bar, 5.0);
        assert!(!c.is_collapsed(), "not yet");
        advance(&bar, 6.0);
        assert!(c.is_collapsed() && c.status() == Status::Hiding);
        assert_eq!(
            bar.length.get(),
            3600.0,
            "twice the widest screen where long items work"
        );
        assert!(bar.collapsed_look.get());
    }

    #[test]
    fn on_a_system_that_drops_long_items_the_longest_working_length_is_used() {
        let (c, bar) = make(629.0, RULES);
        advance(&bar, 0.5);
        c.event(Event::Click);
        advance(&bar, 5.0);
        assert_eq!(c.status(), Status::Hiding);
        let length = bar.length.get();
        assert!(length <= 629.0 && 629.0 - length <= 17.0, "{length}");
        assert!(bar.collapsed_look.get());
    }

    #[test]
    fn a_click_shows_the_icons_again_and_a_new_countdown_hides_them() {
        let (c, bar) = make(10_000.0, RULES);
        advance(&bar, 0.5);
        c.event(Event::Click);
        advance(&bar, 1.0);
        assert!(c.is_collapsed());
        c.event(Event::Click);
        assert!(!c.is_collapsed());
        assert_eq!(bar.length.get(), SEPARATOR_WIDTH);
        assert!(!bar.collapsed_look.get());
        advance(&bar, 11.0);
        assert!(c.is_collapsed(), "hidden again by itself");
    }

    #[test]
    fn where_no_length_works_the_icons_stay_showing_and_it_says_so() {
        let (c, bar) = make(50.0, RULES);
        advance(&bar, 0.5);
        c.event(Event::Click);
        advance(&bar, 5.0);
        assert_eq!(c.status(), Status::Unavailable);
        assert!(!c.is_collapsed());
        assert_eq!(
            bar.length.get(),
            SEPARATOR_WIDTH,
            "nothing is left pushed away"
        );
        assert!(!bar.collapsed_look.get());
        // And it does not try again on its own.
        advance(&bar, 60.0);
        assert_eq!(c.status(), Status::Unavailable);
    }

    #[test]
    fn when_the_room_shrinks_while_hidden_the_length_is_found_again() {
        let (c, bar) = make(900.0, RULES);
        advance(&bar, 0.5);
        c.event(Event::Click);
        advance(&bar, 5.0);
        let first = bar.length.get();
        assert!(first <= 900.0 && first > 800.0, "{first}");
        // Another app with wider menus comes to the front.
        bar.limit.set(400.0);
        c.event(Event::AppSwitched);
        advance(&bar, 5.0);
        let second = bar.length.get();
        assert!(second <= 400.0 && second > 350.0, "{second}");
        assert_eq!(c.status(), Status::Hiding);
    }

    #[test]
    fn an_app_switch_that_changes_nothing_changes_nothing() {
        let (c, bar) = make(900.0, RULES);
        advance(&bar, 0.5);
        c.event(Event::Click);
        advance(&bar, 5.0);
        let before = bar.length.get();
        bar.log.borrow_mut().clear();
        c.event(Event::AppSwitched);
        advance(&bar, 5.0);
        assert_eq!(bar.length.get(), before);
        assert!(
            bar.log.borrow().is_empty(),
            "no flicker: {:?}",
            bar.log.borrow()
        );
    }

    #[test]
    fn a_new_display_makes_it_measure_again() {
        let (c, bar) = make(10_000.0, RULES);
        advance(&bar, 0.5);
        c.event(Event::Click);
        advance(&bar, 5.0);
        assert_eq!(bar.length.get(), 3600.0);
        bar.limit.set(700.0);
        c.event(Event::Screens);
        advance(&bar, 5.0);
        assert!(bar.length.get() <= 700.0 && c.status() == Status::Hiding);
    }

    #[test]
    fn a_screen_change_while_the_icons_show_costs_nothing() {
        let (c, bar) = make(10_000.0, RULES);
        advance(&bar, 0.5);
        bar.log.borrow_mut().clear();
        c.event(Event::Screens);
        advance(&bar, 1.0);
        assert!(bar.log.borrow().is_empty());
        assert_eq!(c.status(), Status::Showing);
    }

    #[test]
    fn it_gives_up_when_the_length_is_lost_again_and_again_before_it_can_be_checked() {
        let (c, bar) = make(900.0, RULES);
        advance(&bar, 0.5);
        c.event(Event::Click);
        advance(&bar, 5.0);
        assert_eq!(c.status(), Status::Hiding);
        // Three bad pieces of news in a row, with no time for a check in between.
        for _ in 0..3 {
            c.event(Event::Screens);
        }
        advance(&bar, 5.0);
        assert_eq!(c.status(), Status::Unavailable);
        assert!(!c.is_collapsed());
        assert_eq!(
            bar.length.get(),
            SEPARATOR_WIDTH,
            "and nothing is left pushed away"
        );
    }

    #[test]
    fn a_check_that_passes_clears_the_count_of_tries() {
        let (c, bar) = make(900.0, RULES);
        advance(&bar, 0.5);
        c.event(Event::Click);
        advance(&bar, 5.0);
        for _ in 0..5 {
            c.event(Event::Screens);
            advance(&bar, 5.0);
            assert_eq!(c.status(), Status::Hiding);
        }
    }

    #[test]
    fn hovering_shows_the_icons_when_clicks_are_not_the_only_way() {
        let rules = Rules {
            click_only: false,
            ..RULES
        };
        let (c, bar) = make(10_000.0, rules);
        advance(&bar, 0.5);
        c.event(Event::Click);
        advance(&bar, 1.0);
        assert!(c.is_collapsed());
        c.event(Event::Hover);
        assert!(!c.is_collapsed() && bar.length.get() == SEPARATOR_WIDTH);
        let (c, bar) = make(10_000.0, RULES);
        advance(&bar, 0.5);
        c.event(Event::Click);
        advance(&bar, 1.0);
        c.event(Event::Hover);
        assert!(c.is_collapsed(), "click only");
    }

    #[test]
    fn a_bar_that_cannot_be_measured_uses_the_longest_length() {
        let (c, bar) = make(100.0, RULES);
        bar.readable.set(false);
        advance(&bar, 0.5);
        c.event(Event::Click);
        advance(&bar, 5.0);
        assert_eq!(c.status(), Status::Hiding);
        assert_eq!(bar.length.get(), 3600.0);
    }

    #[test]
    fn dropping_the_controller_leaves_nothing_pending() {
        let (c, bar) = make(10_000.0, RULES);
        advance(&bar, 0.5);
        c.event(Event::Click);
        drop(c);
        // The timers that were waiting find nobody and do nothing.
        bar.log.borrow_mut().clear();
        advance(&bar, 100.0);
        assert!(bar.log.borrow().is_empty());
    }
}
