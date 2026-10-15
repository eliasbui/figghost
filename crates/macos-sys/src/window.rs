//! Moving and resizing other apps' windows through the Accessibility API.

use crate::ax::Element;
use objc2::MainThreadMarker;
use objc2_app_kit::NSScreen;

/// Global screen coordinates with the origin at the top-left of the primary screen, y growing downwards.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Frame {
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
}

pub struct Window(Element);

impl Window {
    /// The window an element belongs to: the element itself when it is one.
    pub fn containing(element: &Element) -> Option<Self> {
        if element.string("AXRole").as_deref() == Some("AXWindow") {
            return Some(Self(element.duplicate()));
        }
        element.attribute("AXWindow").map(Self)
    }

    pub fn set_timeout(&self, seconds: f32) {
        self.0.set_timeout(seconds);
    }

    pub fn pid(&self) -> Option<i32> {
        self.0.pid()
    }

    /// The focused window of the frontmost app.
    pub fn focused() -> Option<Self> {
        let app = Element::system_wide()?.attribute("AXFocusedApplication")?;
        app.attribute("AXFocusedWindow").map(Self)
    }

    /// The first window of a process, for tests that must not touch anyone else's windows.
    pub fn first_of(pid: i32) -> Option<Self> {
        Element::application(pid)?
            .elements("AXWindows")
            .into_iter()
            .next()
            .map(Self)
    }

    pub fn frame(&self) -> Option<Frame> {
        let (x, y) = self.0.point("AXPosition")?;
        let (w, h) = self.0.size("AXSize")?;
        Some(Frame { x, y, w, h })
    }

    pub fn is_full_screen(&self) -> bool {
        self.0.flag("AXFullScreen").unwrap_or(false)
    }

    /// Size, then position, then size again: moving a window to another screen can clamp the first size,
    /// and some apps only accept the final size once the window sits where it will live.
    pub fn set_frame(&self, frame: Frame) -> bool {
        let size = (frame.w, frame.h);
        let first = self.0.set_size("AXSize", size);
        let moved = self.0.set_point("AXPosition", (frame.x, frame.y));
        let second = self.0.set_size("AXSize", size);
        moved && (first || second)
    }
}

/// Usable area (below the menu bar, beside the Dock) of every screen, in `Frame` coordinates.
pub fn screen_areas(mtm: MainThreadMarker) -> Vec<Frame> {
    let screens = NSScreen::screens(mtm);
    // The first screen is the primary one; Cocoa measures y upwards from its bottom edge.
    let Some(primary) = screens.firstObject() else {
        return Vec::new();
    };
    let primary_height = primary.frame().size.height;
    screens
        .iter()
        .map(|screen| {
            let visible = screen.visibleFrame();
            Frame {
                x: visible.origin.x,
                y: primary_height - (visible.origin.y + visible.size.height),
                w: visible.size.width,
                h: visible.size.height,
            }
        })
        .collect()
}
