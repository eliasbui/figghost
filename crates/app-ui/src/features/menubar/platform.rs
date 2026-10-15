//! The AppKit side of the controller: the real status items.

use super::controller::Bar;
use macos_sys::{MainThreadMarker, app, menubar};

pub struct AppKitBar {
    mtm: MainThreadMarker,
    hider: menubar::Hider,
}

impl AppKitBar {
    pub fn new(mtm: MainThreadMarker, hider: menubar::Hider) -> Self {
        Self { mtm, hider }
    }
}

impl Bar for AppKitBar {
    fn set_length(&self, points: f64) {
        self.hider.set_length(points);
    }

    fn gap(&self) -> Option<f64> {
        self.hider.gap(self.mtm)
    }

    fn look(&self, collapsed: bool) {
        self.hider.look(self.mtm, collapsed);
    }

    fn widest_screen(&self) -> f64 {
        menubar::widest_screen(self.mtm)
    }

    fn after(&self, seconds: f64, f: Box<dyn FnOnce()>) {
        app::after(seconds, f);
    }
}
