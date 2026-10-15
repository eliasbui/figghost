//! Where the window and its scroll position go when the canvas changes size, so that what the person was
//! looking at stays where it was on the screen.

use crate::geom::{Point, Rect, Size};

/// What to do to the window after the canvas went from `old` to `new`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Follow {
    /// The size the scrolling area should have.
    pub visible: Size,
    /// How far the left edge of the window moves to the left (negative: to the right).
    pub left: f64,
    /// How far the top edge of the window moves up (negative: down).
    pub top: f64,
    /// The new scroll position, from the top-left of the content.
    pub scroll: Point,
}

/// `pad` is the margin around the canvas. `visible` and `scroll` are what the scrolling area is now.
/// The area fits the new canvas when it can (never smaller than `min` or larger than `max`), taking the room
/// from the side the canvas grew on; where it cannot, the scroll position takes the rest.
pub fn follow_resize(
    old: Rect,
    new: Rect,
    pad: f64,
    visible: Size,
    scroll: Point,
    min: Size,
    max: Size,
) -> Follow {
    let content = Size::new(new.w + 2.0 * pad, new.h + 2.0 * pad);
    let target = Size::new(
        content.w.max(min.w).min(max.w.max(min.w)),
        content.h.max(min.h).min(max.h.max(min.h)),
    );
    // Only a change in the canvas moves the window; a window the person made bigger stays that big.
    let resized = |old_side: f64, new_side: f64, now: f64, wanted: f64| {
        if (new_side - old_side).abs() < 1e-9 {
            now
        } else {
            wanted
        }
    };
    let visible_after = Size::new(
        resized(old.w, new.w, visible.w, target.w),
        resized(old.h, new.h, visible.h, target.h),
    );
    let (grew_left, grew_right) = (old.x - new.x, new.right() - old.right());
    let (grew_top, grew_bottom) = (old.y - new.y, new.bottom() - old.bottom());
    let share = |grew_near: f64, grew_far: f64, change: f64| {
        let total = grew_near + grew_far;
        if total.abs() < 1e-9 {
            0.0
        } else {
            change * grew_near / total
        }
    };
    let left = share(grew_left, grew_right, visible_after.w - visible.w);
    let top = share(grew_top, grew_bottom, visible_after.h - visible.h);
    let limit = |value: f64, content: f64, view: f64| value.clamp(0.0, (content - view).max(0.0));
    Follow {
        visible: visible_after,
        left,
        top,
        scroll: Point::new(
            limit(scroll.x + grew_left - left, content.w, visible_after.w),
            limit(scroll.y + grew_top - top, content.h, visible_after.h),
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const MIN: Size = Size::new(300.0, 200.0);
    const MAX: Size = Size::new(1000.0, 700.0);
    const PAD: f64 = 20.0;

    fn follow(old: Rect, new: Rect, visible: Size, scroll: Point) -> Follow {
        follow_resize(old, new, PAD, visible, scroll, MIN, MAX)
    }

    /// Where a point of the canvas is on the screen, with the window's top-left at (wx, wy) and the content
    /// scrolled to `scroll`, for a canvas whose top-left is at `canvas`.
    fn on_screen(point: Point, canvas: Rect, window: Point, scroll: Point) -> Point {
        Point::new(
            window.x + PAD + (point.x - canvas.x) - scroll.x,
            window.y + PAD + (point.y - canvas.y) - scroll.y,
        )
    }

    #[test]
    fn growing_left_and_up_moves_the_window_so_nothing_moves_on_screen() {
        let old = Rect::new(0.0, 0.0, 400.0, 300.0);
        let new = Rect::new(-100.0, -50.0, 500.0, 350.0);
        let visible = Size::new(440.0, 340.0);
        let f = follow(old, new, visible, Point::new(0.0, 0.0));
        assert_eq!((f.visible.w, f.visible.h), (540.0, 390.0));
        assert_eq!((f.left, f.top), (100.0, 50.0));
        assert_eq!((f.scroll.x, f.scroll.y), (0.0, 0.0));
        let window = Point::new(300.0, 200.0);
        let before = on_screen(Point::new(10.0, 10.0), old, window, Point::new(0.0, 0.0));
        let moved = Point::new(window.x - f.left, window.y - f.top);
        let after = on_screen(Point::new(10.0, 10.0), new, moved, f.scroll);
        assert_eq!(before, after);
    }

    #[test]
    fn growing_right_and_down_only_makes_the_window_bigger() {
        let old = Rect::new(0.0, 0.0, 400.0, 300.0);
        let new = Rect::new(0.0, 0.0, 450.0, 320.0);
        let f = follow(old, new, Size::new(440.0, 340.0), Point::new(0.0, 0.0));
        assert_eq!((f.visible.w, f.visible.h), (490.0, 360.0));
        assert_eq!((f.left, f.top), (0.0, 0.0));
    }

    #[test]
    fn at_the_largest_size_the_scroll_position_takes_over() {
        let old = Rect::new(0.0, 0.0, 940.0, 300.0);
        let new = Rect::new(-200.0, 0.0, 1140.0, 300.0);
        let visible = Size::new(980.0, 340.0);
        let f = follow(old, new, visible, Point::new(0.0, 0.0));
        assert_eq!(f.visible.w, 1000.0, "capped");
        assert_eq!(f.left, 20.0, "the window takes the room there is");
        assert_eq!(f.scroll.x, 180.0, "and the content scrolls by the rest");
        let window = Point::new(100.0, 100.0);
        let before = on_screen(Point::new(5.0, 5.0), old, window, Point::new(0.0, 0.0));
        let after = on_screen(
            Point::new(5.0, 5.0),
            new,
            Point::new(window.x - f.left, window.y),
            f.scroll,
        );
        assert_eq!(before, after);
    }

    #[test]
    fn a_crop_shrinks_the_window_from_the_cut_sides() {
        let old = Rect::new(0.0, 0.0, 800.0, 600.0);
        let new = Rect::new(100.0, 50.0, 500.0, 400.0);
        let f = follow(old, new, Size::new(840.0, 640.0), Point::new(0.0, 0.0));
        assert_eq!((f.visible.w, f.visible.h), (540.0, 440.0));
        assert_eq!(
            (f.left, f.top),
            (-100.0, -50.0),
            "the window gives up what was cut off its left and top"
        );
    }

    #[test]
    fn a_window_made_bigger_by_hand_is_left_alone_when_the_size_does_not_change() {
        let rect = Rect::new(0.0, 0.0, 400.0, 300.0);
        let f = follow(rect, rect, Size::new(900.0, 650.0), Point::new(0.0, 0.0));
        assert_eq!((f.visible.w, f.visible.h), (900.0, 650.0));
        assert_eq!((f.left, f.top), (0.0, 0.0));
    }

    #[test]
    fn the_window_never_gets_smaller_than_the_minimum() {
        let old = Rect::new(0.0, 0.0, 400.0, 300.0);
        let new = Rect::new(0.0, 0.0, 50.0, 40.0);
        let f = follow(old, new, Size::new(440.0, 340.0), Point::new(0.0, 0.0));
        assert_eq!((f.visible.w, f.visible.h), (MIN.w, MIN.h));
    }

    #[test]
    fn the_scroll_position_stays_inside_the_content() {
        let old = Rect::new(0.0, 0.0, 2000.0, 1500.0);
        let new = Rect::new(0.0, 0.0, 1500.0, 1500.0);
        let f = follow(old, new, MAX, Point::new(1900.0, 400.0));
        assert!(f.scroll.x <= 1540.0 - f.visible.w + 1e-9, "{:?}", f.scroll);
        assert!(f.scroll.y >= 0.0);
    }
}
