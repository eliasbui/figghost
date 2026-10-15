//! The rules of the menu bar hider that need no menu bar: when to collapse and expand, and how to find a length
//! the system really lays out. Pure, so the timing and the search are covered by plain tests.

/// The width of the divider while the icons are showing.
pub const SEPARATOR_WIDTH: f64 = 12.0;
/// The shortest collapse length worth trying; below it nothing useful hides.
pub const MIN_LENGTH: f64 = 120.0;
/// The largest length macOS accepts for a status item.
pub const MAX_LENGTH: f64 = 10_000.0;
/// The search stops when the best length and the first failing one are this close.
const RESOLUTION: f64 = 16.0;
/// How far the divider's edge next to the arrow may move and still count as laid out.
const TOLERANCE: f64 = 24.0;

/// The length to try first: twice the widest screen, as far as macOS allows. Every display shows the menu bar,
/// so the widest decides.
pub fn first_guess(widest_screen: f64) -> f64 {
    (widest_screen * 2.0).clamp(500.0, MAX_LENGTH)
}

/// Whether a divider of some length was laid out, which is when the icons to its left are pushed away. The
/// divider sits left of the arrow, so a laid-out divider grows away from it and the gap between its right edge and
/// the arrow stays what it was with a short divider; one that macOS leaves out of the layout instead jumps by about
/// its length. `baseline` is the gap with the short divider and `now` the gap with the long one.
pub fn honored(baseline: f64, now: f64) -> bool {
    (now - baseline).abs() <= TOLERANCE
}

/// What the search wants next.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Step {
    /// Try this length and report whether it was laid out.
    Probe(f64),
    /// The search is over: the longest length that works, or `None` when not even a short one does.
    Done(Option<f64>),
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum Phase {
    First,
    Shortest,
    Between,
}

/// Finds the longest divider length that is laid out. The first guess is almost always right on older systems
/// (one step). Where an over-long item is dropped from the layout (macOS 27, by reports of other apps), the
/// limit depends on the display and on how much room the status icons have, so it is searched for.
#[derive(Debug, Clone, PartialEq)]
pub struct Search {
    phase: Phase,
    /// The longest length known to work and the shortest known not to.
    good: f64,
    bad: f64,
    asked: f64,
}

impl Search {
    pub fn start(first: f64) -> (Self, Step) {
        let first = first.clamp(MIN_LENGTH * 2.0, MAX_LENGTH);
        (
            Self {
                phase: Phase::First,
                good: 0.0,
                bad: first,
                asked: first,
            },
            Step::Probe(first),
        )
    }

    pub fn report(&mut self, laid_out: bool) -> Step {
        match self.phase {
            Phase::First if laid_out => Step::Done(Some(self.asked)),
            Phase::First => {
                self.phase = Phase::Shortest;
                self.asked = MIN_LENGTH;
                Step::Probe(MIN_LENGTH)
            }
            Phase::Shortest if !laid_out => Step::Done(None),
            Phase::Shortest => {
                self.phase = Phase::Between;
                self.good = MIN_LENGTH;
                self.next()
            }
            Phase::Between => {
                if laid_out {
                    self.good = self.asked;
                } else {
                    self.bad = self.asked;
                }
                self.next()
            }
        }
    }

    fn next(&mut self) -> Step {
        if self.bad - self.good <= RESOLUTION {
            return Step::Done(Some(self.good));
        }
        self.asked = ((self.good + self.bad) / 2.0).round();
        Step::Probe(self.asked)
    }
}

/// What the machine asks the outside to do.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    Collapse,
    Expand,
    /// Collapse after `seconds`, if nothing changed in between: the number says which request this is, and a timer
    /// whose number is no longer the current one does nothing when it fires.
    Arm {
        seconds: u32,
        epoch: u64,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rules {
    /// Collapse by itself this long after the icons were shown; 0 never does.
    pub auto_seconds: u32,
    /// Show the icons only when the arrow is clicked. Off, resting the pointer on the arrow shows them too.
    pub click_only: bool,
}

/// Whether the icons are showing, and when they should hide again.
#[derive(Debug)]
pub struct Machine {
    rules: Rules,
    collapsed: bool,
    epoch: u64,
}

impl Machine {
    /// The icons are showing when the feature starts: nothing is hidden before the person has arranged it.
    pub fn new(rules: Rules) -> (Self, Vec<Action>) {
        let mut machine = Self {
            rules,
            collapsed: false,
            epoch: 0,
        };
        let actions = machine.arm();
        (machine, actions)
    }

    pub fn is_collapsed(&self) -> bool {
        self.collapsed
    }

    fn arm(&mut self) -> Vec<Action> {
        self.epoch += 1;
        match self.rules.auto_seconds {
            0 => Vec::new(),
            seconds => vec![Action::Arm {
                seconds,
                epoch: self.epoch,
            }],
        }
    }

    fn show(&mut self) -> Vec<Action> {
        self.collapsed = false;
        let mut actions = vec![Action::Expand];
        actions.extend(self.arm());
        actions
    }

    fn hide(&mut self) -> Vec<Action> {
        self.collapsed = true;
        // Whatever was armed no longer matters.
        self.epoch += 1;
        vec![Action::Collapse]
    }

    /// The arrow was clicked.
    pub fn clicked(&mut self) -> Vec<Action> {
        if self.collapsed {
            self.show()
        } else {
            self.hide()
        }
    }

    /// The pointer rested on the arrow.
    pub fn hovered(&mut self) -> Vec<Action> {
        if self.collapsed && !self.rules.click_only {
            self.show()
        } else {
            Vec::new()
        }
    }

    /// A timer from `Action::Arm` fired.
    pub fn timer(&mut self, epoch: u64) -> Vec<Action> {
        if !self.collapsed && epoch == self.epoch && self.rules.auto_seconds > 0 {
            self.hide()
        } else {
            Vec::new()
        }
    }

    /// The icons could not be hidden; they stay showing and nothing is armed.
    pub fn failed(&mut self) {
        self.collapsed = false;
        self.epoch += 1;
    }

    /// New settings. While the icons show, the countdown starts over with the new delay.
    pub fn configured(&mut self, rules: Rules) -> Vec<Action> {
        let delay_changed = rules.auto_seconds != self.rules.auto_seconds;
        self.rules = rules;
        if !self.collapsed && delay_changed {
            self.arm()
        } else {
            Vec::new()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const RULES: Rules = Rules {
        auto_seconds: 10,
        click_only: true,
    };

    /// A pretend menu bar that lays a divider out up to `limit` points and drops a longer one from the layout.
    fn run(first: f64, limit: f64) -> (Option<f64>, usize) {
        let (mut search, mut step) = Search::start(first);
        let mut probes = 0;
        loop {
            match step {
                Step::Probe(length) => {
                    probes += 1;
                    step = search.report(length <= limit);
                }
                Step::Done(found) => return (found, probes),
            }
        }
    }

    #[test]
    fn the_first_guess_is_twice_the_widest_screen_within_what_macos_allows() {
        assert_eq!(first_guess(1512.0), 3024.0);
        assert_eq!(first_guess(5120.0), MAX_LENGTH);
        assert_eq!(first_guess(100.0), 500.0, "never absurdly short");
    }

    #[test]
    fn a_divider_is_laid_out_when_the_gap_to_the_arrow_stays_put() {
        // Measured by other apps on macOS 27: 8 points of movement is laid out, 999 is not.
        assert!(honored(4.0, 12.0));
        assert!(honored(4.0, 4.0));
        assert!(!honored(4.0, 1003.0));
        assert!(!honored(4.0, 3000.0));
        assert!(!honored(1200.0, 4.0), "and a jump back is as bad");
    }

    #[test]
    fn where_the_first_guess_works_it_is_one_step() {
        assert_eq!(run(3024.0, 10_000.0), (Some(3024.0), 1));
    }

    #[test]
    fn where_long_items_are_dropped_the_longest_working_length_is_found() {
        for limit in [160.0, 300.0, 629.0, 1000.0, 2999.0] {
            let (found, probes) = run(3024.0, limit);
            let found = found.expect("a length works");
            assert!(found <= limit, "{found} is laid out for a limit of {limit}");
            assert!(
                limit - found <= RESOLUTION + 1.0,
                "{found} is close to {limit}"
            );
            assert!(probes <= 14, "{probes} probes for {limit}");
        }
    }

    #[test]
    fn when_not_even_a_short_divider_works_there_is_no_length() {
        assert_eq!(run(3024.0, 100.0).0, None);
        assert_eq!(run(3024.0, 0.0).0, None);
    }

    #[test]
    fn the_search_never_asks_for_more_than_macos_allows_or_less_than_it_can_use() {
        let (_, step) = Search::start(50_000.0);
        assert_eq!(step, Step::Probe(MAX_LENGTH));
        let (_, step) = Search::start(1.0);
        assert_eq!(step, Step::Probe(MIN_LENGTH * 2.0));
    }

    #[test]
    fn the_icons_show_at_the_start_and_hide_after_the_delay() {
        let (mut m, actions) = Machine::new(RULES);
        assert!(!m.is_collapsed());
        let Action::Arm { seconds: 10, epoch } = actions[0] else {
            panic!("{actions:?}");
        };
        assert_eq!(m.timer(epoch), [Action::Collapse]);
        assert!(m.is_collapsed());
    }

    #[test]
    fn a_stale_timer_does_nothing() {
        let (mut m, actions) = Machine::new(RULES);
        let Action::Arm { epoch: first, .. } = actions[0] else {
            panic!()
        };
        // Hidden by hand, shown again: a new countdown starts, and the old timer must not cut it short.
        assert_eq!(m.clicked(), [Action::Collapse]);
        let shown = m.clicked();
        let Action::Arm { epoch: second, .. } = shown[1] else {
            panic!("{shown:?}")
        };
        assert_eq!(shown[0], Action::Expand);
        assert_ne!(first, second);
        assert!(m.timer(first).is_empty());
        assert!(!m.is_collapsed());
        assert_eq!(m.timer(second), [Action::Collapse]);
    }

    #[test]
    fn hiding_by_hand_cancels_the_countdown() {
        let (mut m, actions) = Machine::new(RULES);
        let Action::Arm { epoch, .. } = actions[0] else {
            panic!()
        };
        m.clicked();
        assert!(m.timer(epoch).is_empty());
        assert!(m.is_collapsed(), "and it stays hidden");
    }

    #[test]
    fn with_no_delay_nothing_is_ever_armed() {
        let rules = Rules {
            auto_seconds: 0,
            click_only: true,
        };
        let (mut m, actions) = Machine::new(rules);
        assert!(actions.is_empty());
        m.clicked();
        assert_eq!(m.clicked(), [Action::Expand]);
        assert!(m.timer(1).is_empty() && m.timer(2).is_empty() && m.timer(3).is_empty());
    }

    #[test]
    fn hovering_shows_the_icons_only_when_clicks_are_not_the_only_way() {
        let (mut m, _) = Machine::new(RULES);
        m.clicked();
        assert!(m.hovered().is_empty(), "click only");
        assert!(m.is_collapsed());
        m.configured(Rules {
            click_only: false,
            ..RULES
        });
        let shown = m.hovered();
        assert_eq!(shown[0], Action::Expand);
        assert!(!m.is_collapsed());
        assert!(m.hovered().is_empty(), "already showing");
    }

    #[test]
    fn changing_the_delay_while_showing_starts_the_countdown_again() {
        let (mut m, actions) = Machine::new(RULES);
        let Action::Arm { epoch: old, .. } = actions[0] else {
            panic!()
        };
        let again = m.configured(Rules {
            auto_seconds: 30,
            ..RULES
        });
        let Action::Arm {
            seconds: 30,
            epoch: new,
        } = again[0]
        else {
            panic!("{again:?}")
        };
        assert!(m.timer(old).is_empty());
        assert_eq!(m.timer(new), [Action::Collapse]);
        // The same settings again change nothing.
        let (mut m, _) = Machine::new(RULES);
        assert!(m.configured(RULES).is_empty());
        // And while hidden, a new delay waits for the next time the icons show.
        m.clicked();
        assert!(
            m.configured(Rules {
                auto_seconds: 5,
                ..RULES
            })
            .is_empty()
        );
        let shown = m.clicked();
        assert!(matches!(shown[1], Action::Arm { seconds: 5, .. }));
    }

    #[test]
    fn a_failed_hide_leaves_the_icons_showing_and_quiet() {
        let (mut m, actions) = Machine::new(RULES);
        let Action::Arm { epoch, .. } = actions[0] else {
            panic!()
        };
        m.clicked();
        m.failed();
        assert!(!m.is_collapsed());
        assert!(m.timer(epoch).is_empty());
    }
}
