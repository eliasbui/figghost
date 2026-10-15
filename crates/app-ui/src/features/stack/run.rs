//! Copy Stack while the app runs: a double press of the chosen modifier key starts a stack, every copy joins
//! its queue, and each Command+V puts the oldest queued item on the pasteboard just before the app reads it.
//!
//! The mechanics and the rules are in `machine`; this file connects them to the world through `Ports`, which
//! the app implements with the keyboard, the pasteboard and the pill and the tests with fakes. Nothing here
//! is kept on disk: the queue holds ids of items that are in the history anyway, so quitting loses nothing
//! and a restart never begins with a stack running.

use super::machine::{DoublePress, Held, Input, Modifier, Paste, Stack};
use macos_sys::stack::input::{Flags, Observed};
use std::{
    any::Any,
    cell::RefCell,
    rc::{Rc, Weak},
};

/// Everything the stack needs from outside. A returned guard stops what it started when it is dropped.
pub trait Ports {
    /// Whether the app may watch the keyboard and put things on the pasteboard in time.
    fn trusted(&self) -> bool;
    /// Reports modifier changes and the key presses that spoil a tap, with times in seconds.
    fn watch_keys(&self, on_input: Box<dyn Fn(Observed, f64)>) -> Option<Box<dyn Any>>;
    /// Calls `on_paste` for each Command+V, before the focused app reads the pasteboard; the key goes on.
    fn watch_paste(&self, on_paste: Box<dyn Fn()>) -> Option<Box<dyn Any>>;
    /// Tells `hook` the id of every copy that is recorded; `None` stops.
    fn listen_copies(&self, hook: Option<Box<dyn Fn(i64)>>);
    /// Puts an item on the pasteboard without recording it or moving it in the history; false if it is gone.
    fn stage(&self, id: i64) -> bool;
    /// Looks at the pasteboard more often while a stack runs.
    fn fast_poll(&self, fast: bool);
    fn show(&self, queued: usize);
    fn hide(&self);
    /// Runs `f` once the current event has been handled.
    fn defer(&self, f: Box<dyn FnOnce()>);
}

pub struct Run {
    ports: Rc<dyn Ports>,
    stack: RefCell<Stack>,
    detector: RefCell<DoublePress>,
    /// Exists only while the feature is switched on.
    keys: RefCell<Option<Box<dyn Any>>>,
    /// Exists only while a stack runs.
    paste: RefCell<Option<Box<dyn Any>>>,
    me: Weak<Run>,
}

fn held(flags: Flags) -> Held {
    Held {
        option: flags.option,
        command: flags.command,
        control: flags.control,
        shift: flags.shift,
    }
}

impl Run {
    pub fn new(ports: Rc<dyn Ports>) -> Rc<Self> {
        Rc::new_cyclic(|me| Self {
            ports,
            stack: RefCell::default(),
            detector: RefCell::new(DoublePress::new(Modifier::default())),
            keys: RefCell::new(None),
            paste: RefCell::new(None),
            me: me.clone(),
        })
    }

    pub fn is_active(&self) -> bool {
        self.stack.borrow().is_active()
    }

    pub fn queued(&self) -> usize {
        self.stack.borrow().queued()
    }

    /// Switches the feature on with the chosen key, or off (which also ends a stack that is running).
    pub fn configure(&self, enabled: bool, modifier: Modifier) {
        if !enabled {
            *self.keys.borrow_mut() = None;
            self.finish();
            return;
        }
        self.detector.borrow_mut().set_target(modifier);
        if self.keys.borrow().is_some() {
            return;
        }
        let me = self.me.clone();
        let keys = self.ports.watch_keys(Box::new(move |observed, at| {
            if let Some(run) = me.upgrade() {
                run.observe(observed, at);
            }
        }));
        if keys.is_none() {
            eprintln!("copy stack: cannot watch the keyboard");
        }
        *self.keys.borrow_mut() = keys;
    }

    pub fn observe(&self, observed: Observed, at: f64) {
        let input = match observed {
            Observed::Modifiers(flags) => Input::Modifiers(held(flags)),
            Observed::Other => Input::Other,
        };
        if self.detector.borrow_mut().feed(input, at) {
            self.toggle();
        }
    }

    /// A double press: starts a stack, or ends the one that runs.
    pub fn toggle(&self) {
        if self.is_active() {
            self.finish();
            return;
        }
        // Without Accessibility access nothing could put the next item on the pasteboard in time, so the
        // stack does not start rather than half work.
        if !self.ports.trusted() {
            eprintln!("copy stack: needs Accessibility access");
            return;
        }
        let me = self.me.clone();
        let Some(paste) = self.ports.watch_paste(Box::new(move || {
            if let Some(run) = me.upgrade() {
                run.paste();
            }
        })) else {
            eprintln!("copy stack: cannot watch the paste shortcut");
            return;
        };
        *self.paste.borrow_mut() = Some(paste);
        self.stack.borrow_mut().toggle();
        let me = self.me.clone();
        self.ports.listen_copies(Some(Box::new(move |id| {
            if let Some(run) = me.upgrade() {
                run.copied(id);
            }
        })));
        // Two copies close together must not be mistaken for one.
        self.ports.fast_poll(true);
        self.ports.show(self.queued());
    }

    fn copied(&self, id: i64) {
        if self.stack.borrow_mut().copied(id) {
            self.ports.show(self.queued());
        }
    }

    /// Command+V was pressed while a stack runs, before the focused app reads the pasteboard.
    pub fn paste(&self) {
        loop {
            let outcome = self.stack.borrow_mut().paste();
            match outcome {
                Paste::Pass => {
                    // Nothing queued: the paste goes ahead as it is, and the stack is over.
                    self.finish_soon();
                    return;
                }
                Paste::Next { id, last } => {
                    if self.ports.stage(id) {
                        if last {
                            self.finish_soon();
                        } else {
                            self.ports.show(self.queued());
                        }
                        return;
                    }
                    // The item was deleted from the history meanwhile; the next one takes its place.
                    if last {
                        self.finish_soon();
                        return;
                    }
                }
            }
        }
    }

    /// Tears the stack down after the key press that ended it has been handled: the paste watcher cannot
    /// be dropped from inside its own callback.
    fn finish_soon(&self) {
        let me = self.me.clone();
        self.ports.defer(Box::new(move || {
            if let Some(run) = me.upgrade() {
                run.finish();
            }
        }));
    }

    pub fn finish(&self) {
        self.stack.borrow_mut().end();
        if self.paste.borrow_mut().take().is_some() {
            self.ports.listen_copies(None);
            self.ports.fast_poll(false);
        }
        self.ports.hide();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::{Cell, RefCell};

    const ALT: Flags = Flags {
        option: true,
        command: false,
        control: false,
        shift: false,
    };
    const NONE: Flags = Flags {
        option: false,
        command: false,
        control: false,
        shift: false,
    };

    type KeyHook = Rc<dyn Fn(Observed, f64)>;
    type CopyHook = Rc<dyn Fn(i64)>;

    /// A world with no keyboard, pasteboard or screen: it records what the stack asked for.
    #[derive(Default)]
    struct Fake {
        trusted: Cell<bool>,
        paste_watch_fails: Cell<bool>,
        keys: RefCell<Option<KeyHook>>,
        paste: RefCell<Option<Rc<dyn Fn()>>>,
        copies: RefCell<Option<CopyHook>>,
        gone: RefCell<Vec<i64>>,
        staged: RefCell<Vec<i64>>,
        fast: Cell<bool>,
        shown: RefCell<Vec<usize>>,
        visible: Cell<bool>,
        deferred: RefCell<Vec<Box<dyn FnOnce()>>>,
        watchers: Rc<Cell<i32>>,
    }

    /// Counts how many guards are alive.
    struct Guard(Rc<Cell<i32>>);
    impl Drop for Guard {
        fn drop(&mut self) {
            self.0.set(self.0.get() - 1);
        }
    }

    impl Ports for Fake {
        fn trusted(&self) -> bool {
            self.trusted.get()
        }
        fn watch_keys(&self, on_input: Box<dyn Fn(Observed, f64)>) -> Option<Box<dyn Any>> {
            *self.keys.borrow_mut() = Some(Rc::from(on_input));
            self.watchers.set(self.watchers.get() + 1);
            Some(Box::new(Guard(Rc::clone(&self.watchers))))
        }
        fn watch_paste(&self, on_paste: Box<dyn Fn()>) -> Option<Box<dyn Any>> {
            if self.paste_watch_fails.get() {
                return None;
            }
            *self.paste.borrow_mut() = Some(Rc::from(on_paste));
            self.watchers.set(self.watchers.get() + 1);
            Some(Box::new(Guard(Rc::clone(&self.watchers))))
        }
        fn listen_copies(&self, hook: Option<Box<dyn Fn(i64)>>) {
            *self.copies.borrow_mut() = hook.map(Rc::from);
        }
        fn stage(&self, id: i64) -> bool {
            if self.gone.borrow().contains(&id) {
                return false;
            }
            self.staged.borrow_mut().push(id);
            true
        }
        fn fast_poll(&self, fast: bool) {
            self.fast.set(fast);
        }
        fn show(&self, queued: usize) {
            self.shown.borrow_mut().push(queued);
            self.visible.set(true);
        }
        fn hide(&self) {
            self.visible.set(false);
        }
        fn defer(&self, f: Box<dyn FnOnce()>) {
            self.deferred.borrow_mut().push(f);
        }
    }

    impl Fake {
        /// Two quick taps of the Option key, as the keyboard monitor would report them.
        fn double_press(&self, at: f64) {
            let keys = self.keys.borrow().clone().expect("the keyboard is watched");
            for (dt, flags) in [(0.0, ALT), (0.06, NONE), (0.2, ALT), (0.26, NONE)] {
                keys(Observed::Modifiers(flags), at + dt);
            }
        }
        fn copy(&self, id: i64) {
            if let Some(hook) = self.copies.borrow().clone() {
                hook(id);
            }
        }
        /// Command+V, then the event loop turns once.
        fn paste_key(&self) {
            if let Some(paste) = self.paste.borrow().clone() {
                paste();
            }
            self.turn();
        }
        fn turn(&self) {
            let tasks: Vec<_> = self.deferred.borrow_mut().drain(..).collect();
            for task in tasks {
                task();
            }
        }
    }

    fn running() -> (Rc<Fake>, Rc<Run>) {
        let fake = Rc::new(Fake::default());
        fake.trusted.set(true);
        let run = Run::new(fake.clone());
        run.configure(true, Modifier::Option);
        (fake, run)
    }

    #[test]
    fn flags_become_the_machine_s_view_of_the_keyboard() {
        let flags = Flags {
            option: true,
            command: false,
            control: true,
            shift: false,
        };
        assert_eq!(
            held(flags),
            Held {
                option: true,
                command: false,
                control: true,
                shift: false
            }
        );
        assert_eq!(held(Flags::default()), Held::NONE);
    }

    #[test]
    fn nothing_is_watched_until_the_feature_is_on() {
        let fake = Rc::new(Fake::default());
        let run = Run::new(fake.clone());
        assert_eq!(fake.watchers.get(), 0);
        run.configure(true, Modifier::Option);
        assert_eq!(fake.watchers.get(), 1, "the keyboard monitor, and no more");
        assert!(
            fake.paste.borrow().is_none(),
            "no paste tap until a stack runs"
        );
        assert!(!fake.fast.get());
        run.configure(false, Modifier::Option);
        assert_eq!(fake.watchers.get(), 0);
    }

    #[test]
    fn a_double_press_starts_a_stack_and_queues_copies_in_order() {
        let (fake, run) = running();
        fake.double_press(10.0);
        assert!(run.is_active());
        assert_eq!(
            fake.watchers.get(),
            2,
            "the paste tap joins the keyboard monitor"
        );
        assert!(fake.fast.get(), "the pasteboard is watched more closely");
        assert!(fake.visible.get() && fake.shown.borrow().last() == Some(&0));
        fake.copy(11);
        fake.copy(12);
        fake.copy(13);
        assert_eq!(run.queued(), 3);
        assert_eq!(fake.shown.borrow().last(), Some(&3), "the pill counts");
        fake.paste_key();
        assert_eq!(fake.staged.borrow().as_slice(), [11]);
        assert_eq!(fake.shown.borrow().last(), Some(&2));
        fake.paste_key();
        fake.paste_key();
        assert_eq!(
            fake.staged.borrow().as_slice(),
            [11, 12, 13],
            "first in, first out"
        );
        assert!(!run.is_active(), "the stack ends with its last item");
        assert!(!fake.visible.get() && !fake.fast.get());
        assert_eq!(
            fake.watchers.get(),
            1,
            "the paste tap is gone, the monitor stays"
        );
        assert!(fake.copies.borrow().is_none());
    }

    #[test]
    fn the_tap_is_not_dropped_inside_its_own_callback() {
        let (fake, run) = running();
        fake.double_press(1.0);
        fake.copy(1);
        let paste = fake.paste.borrow().clone().unwrap();
        paste();
        // The key press has been handled but the loop has not turned: the tap is still there.
        assert_eq!(fake.watchers.get(), 2);
        assert!(!run.is_active(), "the queue is spent");
        fake.turn();
        assert_eq!(fake.watchers.get(), 1);
    }

    #[test]
    fn another_double_press_ends_the_stack_early_and_forgets_the_queue() {
        let (fake, run) = running();
        fake.double_press(1.0);
        fake.copy(5);
        fake.copy(6);
        fake.double_press(5.0);
        assert!(!run.is_active() && run.queued() == 0);
        assert_eq!(fake.watchers.get(), 1);
        assert!(!fake.visible.get());
        // A paste now is nobody's business, and a copy is not queued.
        fake.paste_key();
        assert!(fake.staged.borrow().is_empty());
        fake.copy(7);
        assert_eq!(run.queued(), 0);
    }

    #[test]
    fn a_paste_with_nothing_queued_ends_the_stack_without_touching_the_clipboard() {
        let (fake, run) = running();
        fake.double_press(1.0);
        fake.paste_key();
        assert!(fake.staged.borrow().is_empty());
        assert!(!run.is_active());
        assert_eq!(fake.watchers.get(), 1);
    }

    #[test]
    fn an_item_deleted_from_the_history_is_skipped() {
        let (fake, run) = running();
        fake.double_press(1.0);
        fake.copy(1);
        fake.copy(2);
        fake.copy(3);
        fake.gone.borrow_mut().extend([1, 2]);
        fake.paste_key();
        assert_eq!(
            fake.staged.borrow().as_slice(),
            [3],
            "the first two are gone"
        );
        assert!(!run.is_active());
        // When every queued item is gone, the stack still ends.
        let (fake, run) = running();
        fake.double_press(1.0);
        fake.copy(8);
        fake.gone.borrow_mut().push(8);
        fake.paste_key();
        assert!(fake.staged.borrow().is_empty() && !run.is_active());
    }

    #[test]
    fn without_accessibility_the_stack_does_not_start() {
        let (fake, run) = running();
        fake.trusted.set(false);
        fake.double_press(1.0);
        assert!(!run.is_active());
        assert_eq!(fake.watchers.get(), 1);
        assert!(!fake.visible.get() && !fake.fast.get());
        // Nor does it when the paste tap cannot be made.
        fake.trusted.set(true);
        fake.paste_watch_fails.set(true);
        fake.double_press(10.0);
        assert!(!run.is_active());
        fake.paste_watch_fails.set(false);
        fake.double_press(20.0);
        assert!(run.is_active(), "it starts once nothing stands in the way");
    }

    #[test]
    fn switching_the_feature_off_ends_a_running_stack() {
        let (fake, run) = running();
        fake.double_press(1.0);
        fake.copy(1);
        run.configure(false, Modifier::Option);
        assert!(!run.is_active());
        assert_eq!(fake.watchers.get(), 0, "nothing is watched any more");
        assert!(!fake.visible.get() && !fake.fast.get());
    }

    #[test]
    fn the_key_can_change_while_running_and_a_modifier_with_a_key_is_no_tap() {
        let (fake, run) = running();
        run.configure(true, Modifier::Shift);
        assert_eq!(fake.watchers.get(), 1, "the same monitor keeps working");
        fake.double_press(1.0);
        assert!(!run.is_active(), "Option no longer starts it");
        let keys = fake.keys.borrow().clone().unwrap();
        let shift = Flags {
            shift: true,
            ..Flags::default()
        };
        // Shift+A twice does not count; two bare taps do.
        for (dt, input) in [
            (0.0, Observed::Modifiers(shift)),
            (0.03, Observed::Other),
            (0.06, Observed::Modifiers(NONE)),
            (0.2, Observed::Modifiers(shift)),
            (0.23, Observed::Other),
            (0.26, Observed::Modifiers(NONE)),
        ] {
            keys(input, 20.0 + dt);
        }
        assert!(!run.is_active());
        for (dt, flags) in [(0.0, shift), (0.05, NONE), (0.15, shift), (0.2, NONE)] {
            keys(Observed::Modifiers(flags), 30.0 + dt);
        }
        assert!(run.is_active());
    }

    #[test]
    fn a_stack_started_again_begins_empty() {
        let (fake, run) = running();
        fake.double_press(1.0);
        fake.copy(1);
        fake.double_press(5.0);
        fake.double_press(10.0);
        assert!(run.is_active() && run.queued() == 0);
        fake.paste_key();
        assert!(fake.staged.borrow().is_empty());
    }
}
