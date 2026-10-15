//! The AppKit side of the controller: the real screens, windows and panel.

use super::controller::{Held, Platform, Pressed};
use crate::i18n;
use core_window::{
    Layout, Rect,
    snap::{self, Display, Point},
};
use macos_sys::{MainThreadMarker, app, notch::Notch, permissions, snap_ui, window::Frame};
use std::rc::Rc;

/// How long the window is left to finish its own drag before it is placed.
const SETTLE_SECONDS: f64 = 0.06;

fn rect(f: Frame) -> Rect {
    Rect::new(f.x, f.y, f.w, f.h)
}

fn frame(r: Rect) -> Frame {
    Frame {
        x: r.x,
        y: r.y,
        w: r.w,
        h: r.h,
    }
}

struct Window(snap_ui::Held);

impl Held for Window {
    fn moved(&self) -> bool {
        self.0.moved()
    }
    fn frame(&self) -> Option<Rect> {
        self.0.frame().map(rect)
    }
    fn full_screen(&self) -> bool {
        self.0.is_full_screen()
    }
    fn place(&self, target: Rect) -> bool {
        self.0.place(frame(target))
    }
}

pub struct AppKit {
    mtm: MainThreadMarker,
    panel: snap_ui::ZonesPanel,
    notch: Option<Rc<Notch>>,
}

impl AppKit {
    pub fn new(mtm: MainThreadMarker, notch: Option<Rc<Notch>>) -> Self {
        Self {
            mtm,
            panel: snap_ui::ZonesPanel::new(mtm),
            notch,
        }
    }

    fn spec(display: &Display, layouts: &[Layout]) -> snap_ui::ZonesSpec {
        let panel = display.panel(layouts.len());
        snap_ui::ZonesSpec {
            label: text_label(layouts),
            width: panel.w,
            height: panel.h,
            cells: display
                .cells(layouts.len())
                .into_iter()
                .map(|c| Frame {
                    x: c.x - panel.x,
                    y: c.y - panel.y,
                    w: c.w,
                    h: c.h,
                })
                .collect(),
            glyphs: layouts
                .iter()
                .map(|l| {
                    let g = snap::glyph(*l);
                    [g.x, g.y, g.w, g.h]
                })
                .collect(),
        }
    }
}

fn text_label(layouts: &[Layout]) -> String {
    i18n::current().snap_zones(layouts)
}

impl Platform for AppKit {
    fn trusted(&self) -> bool {
        permissions::accessibility_trusted(false)
    }

    fn displays(&self) -> Vec<Display> {
        snap_ui::displays(self.mtm)
            .into_iter()
            .map(|d| Display {
                frame: rect(d.frame),
                area: rect(d.area),
                cutout: d.cutout,
                inset: d.inset,
            })
            .collect()
    }

    fn grab(&self, at: Point) -> Option<Pressed> {
        let held = snap_ui::grab(at.0, at.1)?;
        // Our own windows are not moved by this feature, and asking ourselves over Accessibility would only wait.
        if held.pid() == Some(std::process::id() as i32) {
            return None;
        }
        Some(Pressed {
            role: held.role().to_owned(),
            frame: rect(held.frame_at_press()),
            window: Box::new(Window(held)),
        })
    }

    fn show(&self, display: &Display, layouts: &[Layout], hover: Option<usize>) {
        let panel = display.panel(layouts.len());
        self.panel
            .show((panel.x, panel.y), &Self::spec(display, layouts), hover);
    }

    fn set_hover(&self, hover: Option<usize>) {
        self.panel.set_hover(hover);
    }

    fn hide(&self) {
        self.panel.hide();
    }

    fn hold_notch(&self, held: bool) {
        if let Some(notch) = &self.notch {
            notch.set_suppressed(held);
        }
    }

    fn later(&self, f: Box<dyn FnOnce()>) {
        app::after(SETTLE_SECONDS, f);
    }
}
