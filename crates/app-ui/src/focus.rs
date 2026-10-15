//! The focus tab of the notch: a countdown that opens the notch and plays a sound when time is up.
//! The timer only ticks while it is running, so an idle app does no work for it.

use crate::{
    focus_timer::{FocusTimer, PRESET_MINUTES, Phase, adjusted_minutes, clock_text},
    history::now_ms,
};
use macos_sys::{
    MainThreadMarker, app,
    focus_ui::{FocusCallbacks, FocusDisplay, FocusView},
    notch::Notch,
    workspace,
};
use std::{
    cell::RefCell,
    rc::{Rc, Weak},
};

const TICK_SECONDS: f64 = 0.25;
const END_SOUND: &str = "Glass";

struct State {
    timer: RefCell<FocusTimer>,
    view: RefCell<Option<Rc<FocusView>>>,
    ticker: RefCell<Option<app::Timer>>,
    notch: Weak<Notch>,
    /// The number of this tab in the notch.
    page: usize,
    me: Weak<State>,
}

impl State {
    fn render(&self) {
        let snapshot = self.timer.borrow().snapshot(now_ms());
        let clock = clock_text(snapshot.remaining_secs);
        let progress = match snapshot.phase {
            Phase::Ready => 0.0,
            Phase::Finished => 1.0,
            _ if snapshot.total_secs == 0 => 0.0,
            _ => 1.0 - snapshot.remaining_secs as f64 / snapshot.total_secs as f64,
        };
        if let Some(view) = self.view.borrow().as_ref() {
            view.show(&FocusDisplay {
                clock: &clock,
                progress,
                running: snapshot.phase == Phase::Running,
                paused: snapshot.phase == Phase::Paused,
                finished: snapshot.phase == Phase::Finished,
                minutes: self.timer.borrow().minutes(),
            });
        }
    }

    /// Runs the ticker exactly while the countdown runs.
    fn sync_ticker(&self) {
        let running = self.timer.borrow().is_running();
        let mut ticker = self.ticker.borrow_mut();
        match (running, ticker.is_some()) {
            (true, false) => {
                let weak = self.me.clone();
                *ticker = Some(app::every(TICK_SECONDS, move || {
                    if let Some(state) = weak.upgrade() {
                        state.tick();
                    }
                }));
            }
            (false, true) => {
                ticker.take();
            }
            _ => {}
        }
    }

    fn tick(&self) {
        let finished = self.timer.borrow_mut().tick(now_ms());
        if finished {
            self.sync_ticker();
            workspace::play_sound(END_SOUND);
            if let Some(notch) = self.notch.upgrade() {
                notch.open_page(self.page);
            }
        }
        self.render();
    }

    fn toggle(&self) {
        self.timer.borrow_mut().toggle(now_ms());
        self.sync_ticker();
        self.render();
    }

    fn reset(&self) {
        self.timer.borrow_mut().reset();
        self.sync_ticker();
        self.render();
    }

    /// Minus and plus: only while nothing is counting, since a new length starts the countdown over.
    fn adjust(&self, delta: i32) {
        if self.timer.borrow().is_running() {
            return;
        }
        let minutes = adjusted_minutes(self.timer.borrow().minutes(), delta);
        self.preset(minutes);
    }

    fn preset(&self, minutes: u32) {
        self.timer.borrow_mut().set_minutes(minutes);
        self.sync_ticker();
        self.render();
    }
}

/// Keeps the focus tab and its timer alive.
pub struct FocusFeature {
    _state: Rc<State>,
    _view: Rc<FocusView>,
}

impl FocusFeature {
    pub fn attach(mtm: MainThreadMarker, notch: &Rc<Notch>) -> Self {
        let state = Rc::new_cyclic(|me| State {
            timer: RefCell::default(),
            view: RefCell::default(),
            ticker: RefCell::default(),
            notch: Rc::downgrade(notch),
            page: notch.page_count(),
            me: me.clone(),
        });
        // The view owns its callbacks, so they hold the state weakly.
        let weak = Rc::downgrade(&state);
        let act = |f: fn(&State)| {
            let weak = weak.clone();
            Box::new(move || {
                if let Some(state) = weak.upgrade() {
                    f(&state);
                }
            }) as Box<dyn Fn()>
        };
        let on_preset = {
            let weak = weak.clone();
            Box::new(move |minutes| {
                if let Some(state) = weak.upgrade() {
                    state.preset(minutes);
                }
            }) as Box<dyn Fn(u32)>
        };
        let on_adjust = {
            let weak = weak.clone();
            Box::new(move |delta| {
                if let Some(state) = weak.upgrade() {
                    state.adjust(delta);
                }
            }) as Box<dyn Fn(i32)>
        };
        let view = FocusView::new(
            mtm,
            notch.page_frame(),
            &PRESET_MINUTES,
            FocusCallbacks {
                on_toggle: act(State::toggle),
                on_reset: act(State::reset),
                on_preset,
                on_adjust,
            },
        );
        *state.view.borrow_mut() = Some(Rc::clone(&view));
        let shown = Rc::downgrade(&state);
        notch.add_page(view.page(
            move || {
                if let Some(state) = shown.upgrade() {
                    state.render();
                }
            },
            || {},
        ));
        state.render();
        Self {
            _state: state,
            _view: view,
        }
    }
}
