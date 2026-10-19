//! Keeping a window above the others. macOS lets a program raise only its own windows, so a pinned window of
//! another app is kept on top by showing a live copy of it over whatever covers it. This decides from the list of
//! windows on screen, front to back, whether a pinned window is clear, covered, away or gone; nothing here touches
//! the system, so every rule is covered by plain tests.

use crate::Rect;

/// One window on screen as the system lists them, front to back.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Listed {
    pub id: u32,
    pub pid: i32,
    /// The level the window sits at: 0 for an ordinary one, more for floating panels, the Dock and the menu bar.
    pub layer: i32,
    pub frame: Rect,
    pub alpha: f64,
}

/// The highest level that counts as covering: ordinary windows and the floating panels above them. The Dock, the
/// menu bar, menus and the system's own windows are higher and never count.
pub const MAX_LAYER: i32 = 8;
/// A window narrower or shorter than this covers nothing that matters: a status item, a tooltip, a handle.
pub const MIN_SIDE: f64 = 40.0;
/// A window less opaque than this is not in the way.
pub const MIN_ALPHA: f64 = 0.1;
/// The part of a window, as a fraction of its area, that must lie under others for it to count as covered: a sliver
/// of a neighbour's border does not.
pub const MIN_COVERED: f64 = 0.02;

/// Where a pinned window stands.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum State {
    /// Not in the system's list at all: the window was closed.
    Gone,
    /// There, but not on screen: minimised, its app hidden, or on another desktop.
    Away,
    /// On screen with nothing over it, at this frame.
    Clear(Rect),
    /// On screen under another window, at this frame.
    Covered(Rect),
}

/// Whether another window lies over `frame` enough to count.
fn covers(above: &Listed, frame: Rect, own_pid: i32) -> bool {
    above.pid != own_pid
        && above.layer <= MAX_LAYER
        && above.alpha >= MIN_ALPHA
        && above.frame.w >= MIN_SIDE
        && above.frame.h >= MIN_SIDE
        && above.frame.overlap(&frame) >= MIN_COVERED * frame.w * frame.h
}

/// The state of window `id`. `on_screen` is the list of windows on screen, front to back; `exists` says whether the
/// window is in the system's list of every window when it is not in that one. The windows of this app (`own_pid`)
/// never cover it: the copy shown over a pinned window is one.
pub fn state(on_screen: &[Listed], exists: bool, id: u32, own_pid: i32) -> State {
    let Some(at) = on_screen.iter().position(|w| w.id == id) else {
        return if exists { State::Away } else { State::Gone };
    };
    let frame = on_screen[at].frame;
    if on_screen[..at].iter().any(|w| covers(w, frame, own_pid)) {
        State::Covered(frame)
    } else {
        State::Clear(frame)
    }
}

/// Whether two frames are the same place, to a point: what a window that has not moved reports.
pub fn same_place(a: Rect, b: Rect) -> bool {
    (a.x - b.x).abs() < 1.0
        && (a.y - b.y).abs() < 1.0
        && (a.w - b.w).abs() < 1.0
        && (a.h - b.h).abs() < 1.0
}

#[cfg(test)]
mod tests {
    use super::*;

    const OWN: i32 = 100;

    fn window(id: u32, pid: i32, frame: Rect) -> Listed {
        Listed {
            id,
            pid,
            layer: 0,
            frame,
            alpha: 1.0,
        }
    }

    fn pinned() -> Listed {
        window(7, 1, Rect::new(100.0, 100.0, 400.0, 300.0))
    }

    #[test]
    fn a_window_with_nothing_over_it_is_clear() {
        let list = [window(9, 2, Rect::new(900.0, 100.0, 400.0, 300.0)), pinned()];
        assert_eq!(state(&list, true, 7, OWN), State::Clear(pinned().frame));
        // The first in the list has nothing before it.
        assert_eq!(state(&[pinned()], true, 7, OWN), State::Clear(pinned().frame));
    }

    #[test]
    fn a_window_in_front_that_overlaps_it_covers_it_and_one_behind_does_not() {
        let over = window(9, 2, Rect::new(300.0, 200.0, 400.0, 300.0));
        let behind = window(8, 3, Rect::new(100.0, 100.0, 800.0, 800.0));
        assert_eq!(
            state(&[over, pinned(), behind], true, 7, OWN),
            State::Covered(pinned().frame)
        );
        assert_eq!(state(&[pinned(), over, behind], true, 7, OWN), State::Clear(pinned().frame));
    }

    #[test]
    fn a_sliver_or_a_corner_is_not_covering() {
        // 5 points of overlap across a 300 high window: 1500 of 120000 square points.
        let sliver = window(9, 2, Rect::new(495.0, 100.0, 400.0, 300.0));
        assert_eq!(state(&[sliver, pinned()], true, 7, OWN), State::Clear(pinned().frame));
        // A tenth of the window is covered: that counts.
        let corner = window(9, 2, Rect::new(460.0, 70.0, 400.0, 100.0));
        assert!(matches!(state(&[corner, pinned()], true, 7, OWN), State::Covered(_)));
        // A window that only touches the edge overlaps nothing.
        let beside = window(9, 2, Rect::new(500.0, 100.0, 400.0, 300.0));
        assert_eq!(state(&[beside, pinned()], true, 7, OWN), State::Clear(pinned().frame));
    }

    #[test]
    fn what_is_not_in_the_way_does_not_cover() {
        let over = Rect::new(300.0, 200.0, 400.0, 300.0);
        let clear = |w: Listed| state(&[w, pinned()], true, 7, OWN) == State::Clear(pinned().frame);
        // The windows of this app: the copy and the border that are shown over a pinned window.
        assert!(clear(window(9, OWN, over)));
        // The Dock and the menu bar sit at higher levels.
        assert!(clear(Listed { layer: 20, ..window(9, 2, over) }));
        assert!(clear(Listed { layer: 24, ..window(9, 2, over) }));
        // A floating panel does cover, as an ordinary window.
        assert!(!clear(Listed { layer: 3, ..window(9, 2, over) }));
        // A status item, a transparent window.
        assert!(clear(window(9, 2, Rect::new(300.0, 200.0, 30.0, 300.0))));
        assert!(clear(window(9, 2, Rect::new(300.0, 200.0, 300.0, 30.0))));
        assert!(clear(Listed { alpha: 0.0, ..window(9, 2, over) }));
        assert!(!clear(Listed { alpha: 0.5, ..window(9, 2, over) }));
    }

    #[test]
    fn a_window_that_is_not_on_screen_is_away_or_gone() {
        let list = [window(9, 2, Rect::new(0.0, 0.0, 400.0, 300.0))];
        assert_eq!(state(&list, true, 7, OWN), State::Away, "minimised, hidden or on another desktop");
        assert_eq!(state(&list, false, 7, OWN), State::Gone, "closed");
        assert_eq!(state(&[], false, 7, OWN), State::Gone);
    }

    #[test]
    fn the_frame_comes_from_the_list_wherever_the_window_is() {
        let moved = window(7, 1, Rect::new(-1500.0, -900.0, 640.0, 480.0));
        assert_eq!(state(&[moved], true, 7, OWN), State::Clear(moved.frame));
        let over = window(9, 2, Rect::new(-1400.0, -800.0, 500.0, 400.0));
        assert_eq!(state(&[over, moved], true, 7, OWN), State::Covered(moved.frame));
    }

    #[test]
    fn two_frames_are_the_same_place_to_a_point() {
        let a = Rect::new(10.0, 20.0, 300.0, 200.0);
        assert!(same_place(a, Rect::new(10.4, 20.4, 300.4, 199.6)));
        assert!(!same_place(a, Rect::new(12.0, 20.0, 300.0, 200.0)));
        assert!(!same_place(a, Rect::new(10.0, 20.0, 300.0, 203.0)));
    }
}
