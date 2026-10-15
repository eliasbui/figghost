//! The mechanics of Copy Stack with no AppKit in them, so every rule can be tested with a fake clock:
//! recognising a double press of a bare modifier key, and the first-in first-out queue of copied items.

use std::collections::VecDeque;

/// Longest a modifier may be held and still count as a tap.
pub const TAP_MAX_SECONDS: f64 = 0.30;
/// Longest between letting go of the first tap and pressing for the second.
pub const DOUBLE_GAP_SECONDS: f64 = 0.40;
/// Items one stack holds; the rest are in the history anyway.
pub const MAX_QUEUED: usize = 200;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Modifier {
    #[default]
    Option,
    Command,
    Control,
    Shift,
}

impl Modifier {
    pub const ALL: [Modifier; 4] = [
        Modifier::Option,
        Modifier::Command,
        Modifier::Control,
        Modifier::Shift,
    ];

    pub const fn as_str(self) -> &'static str {
        match self {
            Modifier::Option => "option",
            Modifier::Command => "command",
            Modifier::Control => "control",
            Modifier::Shift => "shift",
        }
    }

    pub fn parse(text: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|m| m.as_str() == text)
    }
}

/// The four modifier keys that matter here. Caps Lock, Fn and the rest are not part of a tap.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Held {
    pub option: bool,
    pub command: bool,
    pub control: bool,
    pub shift: bool,
}

impl Held {
    pub const NONE: Held = Held {
        option: false,
        command: false,
        control: false,
        shift: false,
    };

    pub fn only(modifier: Modifier) -> Self {
        Self {
            option: modifier == Modifier::Option,
            command: modifier == Modifier::Command,
            control: modifier == Modifier::Control,
            shift: modifier == Modifier::Shift,
        }
    }
}

/// What the keyboard and pointer did.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Input {
    /// The set of held modifier keys changed.
    Modifiers(Held),
    /// A key went down or a button was pressed: whatever the modifier was held for, it is not a tap.
    Other,
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum Phase {
    Idle,
    /// The chosen modifier alone went down at this time.
    First {
        since: f64,
    },
    /// It came up quickly: one tap is done.
    Between {
        at: f64,
    },
    Second {
        since: f64,
    },
    /// Something else was involved; nothing counts until every modifier is up again.
    Spoiled,
}

/// Recognises two quick taps of one modifier key, with nothing else pressed in between.
#[derive(Debug)]
pub struct DoublePress {
    target: Modifier,
    phase: Phase,
}

impl DoublePress {
    pub fn new(target: Modifier) -> Self {
        Self {
            target,
            phase: Phase::Idle,
        }
    }

    pub fn set_target(&mut self, target: Modifier) {
        self.target = target;
        self.phase = Phase::Idle;
    }

    /// Feeds one event with its time in seconds; true on the release that completes the second tap.
    pub fn feed(&mut self, input: Input, now: f64) -> bool {
        let alone = Held::only(self.target);
        self.phase = match (self.phase, input) {
            (Phase::Spoiled, Input::Modifiers(Held::NONE)) => Phase::Idle,
            (Phase::Spoiled, _) => Phase::Spoiled,

            (Phase::First { .. } | Phase::Second { .. }, Input::Other) => Phase::Spoiled,
            (_, Input::Other) => Phase::Idle,

            (Phase::Idle, Input::Modifiers(Held::NONE)) => Phase::Idle,
            (Phase::Idle, Input::Modifiers(held)) if held == alone => Phase::First { since: now },
            (Phase::Idle, Input::Modifiers(_)) => Phase::Spoiled,

            (Phase::First { since }, Input::Modifiers(held)) if held == alone => {
                Phase::First { since }
            }
            (Phase::First { since }, Input::Modifiers(Held::NONE)) => {
                if now - since <= TAP_MAX_SECONDS {
                    Phase::Between { at: now }
                } else {
                    Phase::Idle
                }
            }
            (Phase::First { .. }, Input::Modifiers(_)) => Phase::Spoiled,

            (Phase::Between { at }, Input::Modifiers(held)) if held == alone => {
                if now - at <= DOUBLE_GAP_SECONDS {
                    Phase::Second { since: now }
                } else {
                    // Too slow to be the second tap, but it can be the first of a new pair.
                    Phase::First { since: now }
                }
            }
            (Phase::Between { at }, Input::Modifiers(Held::NONE)) => Phase::Between { at },
            (Phase::Between { .. }, Input::Modifiers(_)) => Phase::Spoiled,

            (Phase::Second { since }, Input::Modifiers(held)) if held == alone => {
                Phase::Second { since }
            }
            (Phase::Second { since }, Input::Modifiers(Held::NONE)) => {
                if now - since <= TAP_MAX_SECONDS {
                    self.phase = Phase::Idle;
                    return true;
                }
                Phase::Idle
            }
            (Phase::Second { .. }, Input::Modifiers(_)) => Phase::Spoiled,
        };
        false
    }
}

/// What a paste key press should do.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Paste {
    /// Not ours: the paste goes ahead with whatever the clipboard holds.
    Pass,
    /// Put this item on the clipboard first. `last` is true when nothing else is queued, and the stack
    /// has ended with it.
    Next { id: i64, last: bool },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Toggled {
    Started,
    Ended,
}

/// The queue of one stack session. Nothing here is saved: a restart always begins with no stack.
#[derive(Debug, Default)]
pub struct Stack {
    active: bool,
    queue: VecDeque<i64>,
}

impl Stack {
    pub fn is_active(&self) -> bool {
        self.active
    }

    pub fn queued(&self) -> usize {
        self.queue.len()
    }

    /// A double press: starts an empty stack, or ends the one that is running.
    pub fn toggle(&mut self) -> Toggled {
        if self.active {
            self.end();
            Toggled::Ended
        } else {
            self.active = true;
            self.queue.clear();
            Toggled::Started
        }
    }

    pub fn end(&mut self) {
        self.active = false;
        self.queue.clear();
    }

    /// A copy while the stack runs joins the end of the queue; true when it was queued.
    pub fn copied(&mut self, id: i64) -> bool {
        if !self.active || self.queue.len() >= MAX_QUEUED {
            return false;
        }
        self.queue.push_back(id);
        true
    }

    /// A paste key press. The oldest queued item goes first; an empty queue ends the stack.
    pub fn paste(&mut self) -> Paste {
        if !self.active {
            return Paste::Pass;
        }
        match self.queue.pop_front() {
            Some(id) => {
                let last = self.queue.is_empty();
                if last {
                    self.active = false;
                }
                Paste::Next { id, last }
            }
            None => {
                self.active = false;
                Paste::Pass
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const ALT: Held = Held {
        option: true,
        command: false,
        control: false,
        shift: false,
    };
    const CMD: Held = Held {
        option: false,
        command: true,
        control: false,
        shift: false,
    };
    const ALT_CMD: Held = Held {
        option: true,
        command: true,
        control: false,
        shift: false,
    };

    /// Plays (time, input) pairs and returns the times at which a double press was reported.
    fn play(target: Modifier, events: &[(f64, Input)]) -> Vec<f64> {
        let mut detector = DoublePress::new(target);
        events
            .iter()
            .filter(|(t, input)| detector.feed(*input, *t))
            .map(|(t, _)| *t)
            .collect()
    }

    fn down(held: Held) -> Input {
        Input::Modifiers(held)
    }
    fn up() -> Input {
        Input::Modifiers(Held::NONE)
    }

    #[test]
    fn two_quick_taps_are_a_double_press() {
        let events = [
            (1.00, down(ALT)),
            (1.08, up()),
            (1.25, down(ALT)),
            (1.31, up()),
        ];
        assert_eq!(play(Modifier::Option, &events), [1.31]);
    }

    #[test]
    fn the_chosen_modifier_is_the_only_one_that_counts() {
        let events = [
            (1.00, down(ALT)),
            (1.08, up()),
            (1.20, down(ALT)),
            (1.26, up()),
        ];
        assert!(play(Modifier::Command, &events).is_empty());
        let cmd = [
            (1.00, down(CMD)),
            (1.08, up()),
            (1.20, down(CMD)),
            (1.26, up()),
        ];
        assert_eq!(play(Modifier::Command, &cmd), [1.26]);
        for m in Modifier::ALL {
            let held = Held::only(m);
            let both = [
                (0.0, down(held)),
                (0.05, up()),
                (0.10, down(held)),
                (0.15, up()),
            ];
            assert_eq!(play(m, &both).len(), 1, "{m:?}");
        }
    }

    #[test]
    fn a_slow_gap_or_a_long_hold_is_not_a_tap() {
        // The pause between the taps is too long.
        let slow_gap = [
            (1.00, down(ALT)),
            (1.05, up()),
            (1.50, down(ALT)),
            (1.55, up()),
        ];
        assert!(play(Modifier::Option, &slow_gap).is_empty());
        // The first press is a hold, as when Option is used to reveal something.
        let hold = [
            (1.00, down(ALT)),
            (1.60, up()),
            (1.70, down(ALT)),
            (1.75, up()),
        ];
        assert!(play(Modifier::Option, &hold).is_empty());
        // The second press is held too long.
        let long_second = [
            (1.00, down(ALT)),
            (1.05, up()),
            (1.15, down(ALT)),
            (1.80, up()),
        ];
        assert!(play(Modifier::Option, &long_second).is_empty());
        // Just inside the limits still counts.
        let edge = [
            (0.00, down(ALT)),
            (0.29, up()),
            (0.68, down(ALT)),
            (0.97, up()),
        ];
        assert_eq!(play(Modifier::Option, &edge).len(), 1);
    }

    #[test]
    fn a_slow_second_tap_starts_a_new_pair() {
        // A late press is not the second tap, but it is the first of the next pair.
        let events = [
            (1.00, down(ALT)),
            (1.05, up()),
            (2.00, down(ALT)),
            (2.05, up()),
            (2.20, down(ALT)),
            (2.25, up()),
        ];
        assert_eq!(play(Modifier::Option, &events), [2.25]);
    }

    #[test]
    fn a_modifier_used_with_another_key_is_not_a_tap() {
        // Option+A, twice: the key in between spoils both.
        let shortcut = [
            (1.00, down(ALT)),
            (1.03, Input::Other),
            (1.06, up()),
            (1.15, down(ALT)),
            (1.18, Input::Other),
            (1.21, up()),
        ];
        assert!(play(Modifier::Option, &shortcut).is_empty());
        // A key typed between the taps breaks the pair.
        let typed = [
            (1.00, down(ALT)),
            (1.05, up()),
            (1.10, Input::Other),
            (1.20, down(ALT)),
            (1.25, up()),
        ];
        assert!(play(Modifier::Option, &typed).is_empty());
        // So does a click, which arrives as the same kind of event.
        let clicked = [
            (1.00, down(ALT)),
            (1.02, Input::Other),
            (1.05, up()),
            (1.10, down(ALT)),
            (1.15, up()),
        ];
        assert!(play(Modifier::Option, &clicked).is_empty());
    }

    #[test]
    fn another_modifier_joining_spoils_the_tap() {
        let chord = [
            (1.00, down(ALT)),
            (1.02, down(ALT_CMD)),
            (1.04, down(ALT)),
            (1.06, up()),
            (1.10, down(ALT)),
            (1.14, up()),
        ];
        assert!(play(Modifier::Option, &chord).is_empty());
        // Starting with the other modifier held first is no tap either.
        let other_first = [(1.00, down(CMD)), (1.02, down(ALT_CMD)), (1.05, up())];
        assert!(play(Modifier::Option, &other_first).is_empty());
        // A third press that brings in a second modifier after the first tap.
        let third = [
            (1.00, down(ALT)),
            (1.05, up()),
            (1.10, down(CMD)),
            (1.15, up()),
            (1.20, down(ALT)),
            (1.25, up()),
        ];
        assert!(play(Modifier::Option, &third).is_empty());
    }

    #[test]
    fn a_spoiled_press_recovers_once_every_key_is_up() {
        let events = [
            (1.00, down(ALT)),
            (1.01, Input::Other),
            (1.05, up()),
            (2.00, down(ALT)),
            (2.05, up()),
            (2.15, down(ALT)),
            (2.20, up()),
        ];
        assert_eq!(play(Modifier::Option, &events), [2.20]);
    }

    #[test]
    fn three_taps_report_once_and_a_fourth_starts_over() {
        let mut d = DoublePress::new(Modifier::Option);
        let mut reports = 0;
        let mut t = 0.0;
        for _ in 0..4 {
            for input in [down(ALT), up()] {
                reports += d.feed(input, t) as usize;
                t += 0.06;
            }
            t += 0.05;
        }
        assert_eq!(reports, 2, "taps 1+2 and taps 3+4");
    }

    #[test]
    fn repeated_events_and_changing_the_target_are_harmless() {
        let mut d = DoublePress::new(Modifier::Option);
        // The system may repeat a flags event; the press time stays the first one.
        assert!(!d.feed(down(ALT), 0.00));
        assert!(!d.feed(down(ALT), 0.20));
        assert!(!d.feed(up(), 0.35), "held for 0.35 s is a hold");
        d.set_target(Modifier::Shift);
        assert!(!d.feed(down(Held::only(Modifier::Shift)), 1.0));
        assert!(!d.feed(up(), 1.05));
        assert!(!d.feed(up(), 1.06), "a repeated release changes nothing");
        assert!(!d.feed(down(Held::only(Modifier::Shift)), 1.10));
        assert!(d.feed(up(), 1.15));
        // Changing the target forgets a half-finished pair.
        assert!(!d.feed(down(Held::only(Modifier::Shift)), 2.0));
        assert!(!d.feed(up(), 2.05));
        d.set_target(Modifier::Option);
        assert!(!d.feed(down(ALT), 2.10));
        assert!(!d.feed(up(), 2.15));
    }

    #[test]
    fn the_stack_hands_items_out_first_in_first_out_and_ends_when_empty() {
        let mut s = Stack::default();
        assert_eq!(s.paste(), Paste::Pass, "no stack, no change");
        assert!(!s.copied(1), "copies are ignored while no stack runs");
        assert_eq!(s.toggle(), Toggled::Started);
        assert!(s.is_active() && s.queued() == 0);
        assert!(s.copied(10) && s.copied(11) && s.copied(12));
        assert_eq!(s.queued(), 3);
        assert_eq!(
            s.paste(),
            Paste::Next {
                id: 10,
                last: false
            }
        );
        assert!(s.copied(13), "copying goes on while pasting");
        assert_eq!(
            s.paste(),
            Paste::Next {
                id: 11,
                last: false
            }
        );
        assert_eq!(
            s.paste(),
            Paste::Next {
                id: 12,
                last: false
            }
        );
        assert_eq!(s.paste(), Paste::Next { id: 13, last: true });
        assert!(!s.is_active(), "the stack ends with its last item");
        assert_eq!(s.paste(), Paste::Pass);
        assert!(!s.copied(14));
    }

    #[test]
    fn a_paste_with_nothing_queued_ends_the_stack_and_changes_nothing() {
        let mut s = Stack::default();
        s.toggle();
        assert_eq!(s.paste(), Paste::Pass);
        assert!(!s.is_active());
        assert_eq!(
            s.toggle(),
            Toggled::Started,
            "a fresh stack can start again"
        );
    }

    #[test]
    fn a_second_double_press_ends_the_stack_early_and_forgets_the_queue() {
        let mut s = Stack::default();
        s.toggle();
        s.copied(1);
        s.copied(2);
        assert_eq!(s.toggle(), Toggled::Ended);
        assert!(!s.is_active() && s.queued() == 0);
        assert_eq!(s.paste(), Paste::Pass);
        // Starting again begins empty, never with the old queue.
        s.toggle();
        assert_eq!(s.queued(), 0);
    }

    #[test]
    fn the_same_item_copied_twice_is_pasted_twice() {
        let mut s = Stack::default();
        s.toggle();
        s.copied(5);
        s.copied(6);
        s.copied(5);
        let order: Vec<i64> = std::iter::from_fn(|| match s.paste() {
            Paste::Next { id, .. } => Some(id),
            Paste::Pass => None,
        })
        .collect();
        assert_eq!(order, [5, 6, 5]);
    }

    #[test]
    fn the_queue_has_a_limit() {
        let mut s = Stack::default();
        s.toggle();
        for id in 0..MAX_QUEUED as i64 {
            assert!(s.copied(id));
        }
        assert!(!s.copied(9999));
        assert_eq!(s.queued(), MAX_QUEUED);
        assert_eq!(s.paste(), Paste::Next { id: 0, last: false });
        assert!(s.copied(9999), "room again after a paste");
    }

    #[test]
    fn modifier_names_round_trip() {
        for m in Modifier::ALL {
            assert_eq!(Modifier::parse(m.as_str()), Some(m));
        }
        assert_eq!(Modifier::parse("alt"), None);
        assert_eq!(Modifier::default(), Modifier::Option);
    }
}
