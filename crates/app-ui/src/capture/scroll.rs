//! Scrolling capture: the content of an area scrolls down by itself one step at a time, and each new
//! frame is stitched under the last, until the content stops moving, the picture is full, or Done.

use super::{CaptureFeature, Taken};
use crate::{i18n, trace};
use core_capture::{FrameRef, Step, Stitcher};
use macos_sys::{
    NSPoint, NSRect, app,
    capture_ui::{self, ScrollSession, ScrollTexts},
    screen_capture::{self, AreaCapture, Shot},
    synth::{self, ScrollPhase},
};
use std::{
    cell::{Cell, RefCell},
    rc::{Rc, Weak},
};

/// Share of the area's height scrolled per step; the rest overlaps the last frame, for matching.
const STEP_SHARE: f64 = 0.6;
/// Time for the app to draw after a scroll before the next frame is taken.
const SETTLE_SECONDS: f64 = 0.25;
const MAX_ROWS: usize = 12_000;
const MAX_STEPS: usize = 80;
/// Overlay scroll bars appear in this many points at the right edge while scrolling.
const SCROLL_BAR_POINTS: f64 = 18.0;
/// Frames in a row without movement that mean the end of the content.
const STILL_LIMIT: u32 = 2;

pub(super) struct Scrolling {
    me: Weak<Scrolling>,
    feature: Weak<CaptureFeature>,
    capture: AreaCapture,
    stitcher: RefCell<Option<Stitcher>>,
    /// The first frame, whose pixel format the stitched picture keeps.
    reference: RefCell<Option<Shot>>,
    session: RefCell<Option<Rc<ScrollSession>>>,
    still: Cell<u32>,
    steps: Cell<usize>,
    /// Done was pressed: finish after the frame in flight.
    done: Cell<bool>,
    cancelled: Cell<bool>,
    finished: Cell<bool>,
    center: NSPoint,
    step_points: f64,
    screen: NSRect,
    place: NSRect,
}

impl Scrolling {
    /// `area` is in AppKit screen coordinates; `screen` is the display it is on.
    pub(super) fn start(
        feature: &Rc<CaptureFeature>,
        capture: AreaCapture,
        area: NSRect,
        screen: NSRect,
    ) -> Rc<Self> {
        let scrolling = Rc::new_cyclic(|me| Self {
            me: me.clone(),
            feature: Rc::downgrade(feature),
            capture,
            stitcher: RefCell::new(None),
            reference: RefCell::new(None),
            session: RefCell::new(None),
            still: Cell::new(0),
            steps: Cell::new(0),
            done: Cell::new(false),
            cancelled: Cell::new(false),
            finished: Cell::new(false),
            center: NSPoint::new(
                area.origin.x + area.size.width / 2.0,
                area.origin.y + area.size.height / 2.0,
            ),
            step_points: (area.size.height * STEP_SHARE).round().max(20.0),
            screen,
            place: area,
        });
        let text = i18n::current();
        let texts = ScrollTexts {
            done: text.capture_scroll_done().into(),
            cancel: text.capture_scroll_cancel().into(),
        };
        let (on_done, on_cancel) = (scrolling.me.clone(), scrolling.me.clone());
        let session = ScrollSession::show(
            feature.mtm,
            area,
            &texts,
            move || {
                if let Some(s) = on_done.upgrade() {
                    s.done.set(true);
                }
            },
            move || {
                if let Some(s) = on_cancel.upgrade() {
                    s.cancel();
                }
            },
        );
        *scrolling.session.borrow_mut() = Some(session);
        scrolling.take_frame();
        scrolling
    }

    fn take_frame(&self) {
        let me = self.me.clone();
        self.capture.capture(move |result| {
            let Some(scrolling) = me.upgrade() else {
                return;
            };
            match result {
                Ok(shot) => scrolling.frame(shot),
                Err(e) => {
                    eprintln!("capture: scrolling frame failed: {e}");
                    scrolling.finish();
                }
            }
        });
    }

    fn frame(&self, shot: Shot) {
        if self.cancelled.get() || self.finished.get() {
            return;
        }
        let Some(pixels) = shot.pixels() else {
            eprintln!("capture: unexpected pixel format for a scrolling capture");
            return self.finish();
        };
        let view = FrameRef {
            data: &pixels.data,
            width: pixels.width,
            height: pixels.height,
            stride: pixels.stride,
        };
        let scale = shot.scale();
        let step = {
            let mut stitcher = self.stitcher.borrow_mut();
            match stitcher.as_mut() {
                None => {
                    let margin = (SCROLL_BAR_POINTS * scale) as usize;
                    *stitcher = Some(Stitcher::new(view, margin, MAX_ROWS));
                    *self.reference.borrow_mut() = Some(shot.clone());
                    Step::Moved(0)
                }
                Some(stitcher) => {
                    let expected = (self.step_points * scale).round() as usize;
                    stitcher.push(view, Some(expected))
                }
            }
        };
        trace::log(&format!("capture: scroll step {step:?}"));
        let rows = self.stitcher.borrow().as_ref().map_or(0, Stitcher::rows);
        if let Some(session) = self.session.borrow().as_ref() {
            session.set_status(&i18n::current().capture_scroll_status(rows));
        }
        match step {
            Step::Moved(_) => self.still.set(0),
            Step::Still => self.still.set(self.still.get() + 1),
            Step::Lost | Step::Full => return self.finish(),
        }
        self.steps.set(self.steps.get() + 1);
        if self.done.get() || self.still.get() >= STILL_LIMIT || self.steps.get() >= MAX_STEPS {
            return self.finish();
        }
        self.scroll();
        let me = self.me.clone();
        app::after(SETTLE_SECONDS, move || {
            if let Some(scrolling) = me.upgrade() {
                scrolling.take_frame();
            }
        });
    }

    /// One scroll gesture down, with the pointer over the area so the right view receives it.
    fn scroll(&self) {
        screen_capture::warp_pointer(self.center);
        let quarter = -((self.step_points / 4.0).round() as i32);
        synth::post_scroll(0, quarter, ScrollPhase::Began);
        for _ in 0..3 {
            synth::post_scroll(0, quarter, ScrollPhase::Changed);
        }
        synth::post_scroll(0, 0, ScrollPhase::Ended);
    }

    fn cancel(&self) {
        self.cancelled.set(true);
        self.close();
        if let Some(feature) = self.feature.upgrade() {
            feature.scrolling.borrow_mut().take();
        }
    }

    fn close(&self) {
        if let Some(session) = self.session.borrow_mut().take() {
            session.close();
        }
    }

    fn finish(&self) {
        if self.finished.replace(true) {
            return;
        }
        self.close();
        let (Some(stitcher), Some(reference)) = (
            self.stitcher.borrow_mut().take(),
            self.reference.borrow_mut().take(),
        ) else {
            return;
        };
        let (data, width, height) = stitcher.finish();
        let pixels = screen_capture::Pixels {
            data,
            width,
            height,
            stride: width * 4,
        };
        let Some(feature) = self.feature.upgrade() else {
            return;
        };
        feature.scrolling.borrow_mut().take();
        match reference.with_pixels(&pixels) {
            Some(shot) => feature.deliver(Taken {
                shot,
                screen: self.screen,
                place: self.place,
            }),
            None => capture_ui::toast(feature.mtm, &i18n::current().capture_failed("stitching")),
        }
    }
}
