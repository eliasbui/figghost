//! The AppKit side of the controller: the real screens, windows and panels.

use super::controller::{Held, Platform, Pressed};
use crate::i18n;
use core_window::{
    Rect,
    snap::{Display, Point, View},
    templates::Template,
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
    cells: snap_ui::CellsOverlay,
    footprint: snap_ui::Footprint,
    notch: Option<Rc<Notch>>,
}

/// The name a layout goes by in the row: the one a person gave it, or the built-in one in the language of the app.
pub fn name_of(template: &Template) -> String {
    template
        .name
        .clone()
        .unwrap_or_else(|| i18n::current().snap_template(&template.id).to_owned())
}

/// The parts of a small screen the cells of a layout fill.
fn parts(template: &Template) -> Vec<[f64; 4]> {
    template.cells.iter().map(|c| c.part()).collect()
}

impl AppKit {
    pub fn new(mtm: MainThreadMarker, notch: Option<Rc<Notch>>) -> Self {
        Self {
            mtm,
            panel: snap_ui::ZonesPanel::new(mtm),
            cells: snap_ui::CellsOverlay::new(mtm),
            footprint: snap_ui::Footprint::new(mtm),
            notch,
        }
    }

    /// The row of `templates` as the panel draws it, and where it hangs from.
    pub fn row_spec(display: &Display, templates: &[Template]) -> (snap_ui::ZonesSpec, (f64, f64)) {
        let panel = display.panel(templates.len());
        let names: Vec<String> = templates.iter().map(name_of).collect();
        let spec = snap_ui::ZonesSpec {
            label: i18n::current().snap_row(&names),
            width: panel.w,
            height: panel.h,
            cells: display
                .cells(templates.len())
                .into_iter()
                .map(|c| Frame {
                    x: c.x - panel.x,
                    y: c.y - panel.y,
                    w: c.w,
                    h: c.h,
                })
                .collect(),
            glyphs: templates.iter().map(parts).collect(),
            names,
            chip: None,
        };
        (spec, (panel.x, panel.y))
    }

    /// The chip the row shrinks to once `template` is chosen, and where it hangs from.
    pub fn chip_spec(display: &Display, template: &Template) -> (snap_ui::ZonesSpec, (f64, f64)) {
        let chip = display.chip();
        let name = name_of(template);
        let spec = snap_ui::ZonesSpec {
            label: i18n::current().snap_chosen(&name),
            width: chip.w,
            height: chip.h,
            cells: Vec::new(),
            glyphs: Vec::new(),
            names: Vec::new(),
            chip: Some(snap_ui::ChipSpec {
                parts: parts(template),
                name,
                top: display.inset,
            }),
        };
        (spec, (chip.x, chip.y))
    }
}

impl Platform for AppKit {
    fn trusted(&self) -> bool {
        permissions::accessibility_trusted(false)
    }

    fn displays(&self) -> Vec<(Display, String)> {
        snap_ui::displays(self.mtm)
            .into_iter()
            .map(|d| {
                (
                    Display {
                        frame: rect(d.frame),
                        area: rect(d.area),
                        cutout: d.cutout,
                        inset: d.inset,
                    },
                    d.name,
                )
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

    fn show(&self, display: &Display, templates: &[Template], view: View) {
        let chosen = view.chosen.and_then(|i| templates.get(i));
        let (spec, at) = match chosen {
            Some(template) if view.compact => Self::chip_spec(display, template),
            _ => Self::row_spec(display, templates),
        };
        self.panel.show(at, &spec, view.hover, view.chosen);
    }

    fn hide(&self) {
        self.panel.hide();
    }

    fn footprint(&self, target: Option<Rect>) {
        match target {
            Some(r) => self.footprint.show(frame(r)),
            None => self.footprint.hide(),
        }
    }

    fn cells(&self, shown: Option<(Rect, Vec<Rect>)>) {
        match shown {
            Some((area, cells)) => {
                let cells: Vec<Frame> = cells.into_iter().map(frame).collect();
                self.cells.show(frame(area), &cells);
            }
            None => self.cells.hide(),
        }
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
