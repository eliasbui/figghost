//! The camera of a presentation: which part of the presented picture fills the audience's screen. A click zooms in
//! where it lands, the view follows the pointer while it is zoomed, and after a quiet while it goes back to the whole
//! picture, the way the automatic zoom of screen recorders and the live zoom of ZoomIt do; keys zoom in and out by
//! steps around the pointer. Positions are fractions of the picture, 0 to 1 from its bottom-left, so the camera knows
//! nothing of pixels, screens or time other than the seconds it is told have passed.

use crate::geometry::{Point, Rect};

/// The zooms the keys step through.
pub const STEPS: [f64; 8] = [1.0, 1.5, 2.0, 2.5, 3.0, 4.0, 6.0, 8.0];
/// The pointer may roam this share of the view from its middle before the view follows it.
const ROAM: f64 = 0.5;
/// How fast the camera closes in on where it is going: the seconds in which it covers about two thirds of the way.
const ZOOM_TIME: f64 = 0.22;
const PAN_TIME: f64 = 0.14;
/// Closer than this to where it is going, it is there.
const EPSILON: f64 = 0.0005;

#[derive(Debug, Clone, PartialEq)]
pub struct Camera {
    /// The zoom now (1 is the whole picture) and the middle of the view.
    zoom: f64,
    center: Point,
    /// Where it is going.
    goal_zoom: f64,
    goal_center: Point,
    /// What a click zooms to.
    level: f64,
    /// Seconds without a click or a key before an automatic zoom goes back out; `None` keeps it.
    hold: Option<f64>,
    /// Clicks zoom in and the view goes back out by itself.
    auto: bool,
    /// The zoom was chosen with the keys, which the quiet does not undo.
    manual: bool,
    quiet: f64,
}

impl Camera {
    pub fn new(level: f64, hold: Option<f64>, auto: bool) -> Self {
        let middle = Point::new(0.5, 0.5);
        Self {
            zoom: 1.0,
            center: middle,
            goal_zoom: 1.0,
            goal_center: middle,
            level: level.clamp(1.0, STEPS[STEPS.len() - 1]),
            hold: hold.filter(|s| s.is_finite() && *s > 0.0),
            auto,
            manual: false,
            quiet: 0.0,
        }
    }

    pub fn set_level(&mut self, level: f64) {
        self.level = level.clamp(1.0, STEPS[STEPS.len() - 1]);
    }

    pub fn set_hold(&mut self, hold: Option<f64>) {
        self.hold = hold.filter(|s| s.is_finite() && *s > 0.0);
    }

    pub fn is_auto(&self) -> bool {
        self.auto
    }

    /// Turns the automatic zoom on or off; off goes back to the whole picture unless the keys chose a zoom.
    pub fn set_auto(&mut self, auto: bool) {
        self.auto = auto;
        if !auto && !self.manual {
            self.goal_zoom = 1.0;
        }
    }

    /// A click at `at`: with the automatic zoom it zooms in there, or keeps zooming and moves there.
    pub fn click(&mut self, at: Point) {
        self.quiet = 0.0;
        if !self.auto {
            return;
        }
        if self.goal_zoom < self.level {
            self.goal_zoom = self.level;
        }
        self.goal_center = clamp_center(clamp_unit(at), self.goal_zoom);
    }

    /// The pointer is at `at`: while zoomed, the view moves just enough to keep it in the middle part.
    pub fn pointer(&mut self, at: Point) {
        if self.goal_zoom <= 1.0 {
            return;
        }
        let at = clamp_unit(at);
        let half = 0.5 / self.goal_zoom;
        let roam = half * ROAM;
        let follow = |center: f64, p: f64| {
            if p > center + roam {
                p - roam
            } else if p < center - roam {
                p + roam
            } else {
                center
            }
        };
        let moved = Point::new(
            follow(self.goal_center.x, at.x),
            follow(self.goal_center.y, at.y),
        );
        self.goal_center = clamp_center(moved, self.goal_zoom);
    }

    /// One step of the keys, in or out, around the pointer. Back at the whole picture, the keys let go.
    pub fn step(&mut self, inwards: bool, at: Point) {
        self.quiet = 0.0;
        let next = if inwards {
            STEPS.iter().copied().find(|s| *s > self.goal_zoom + 1e-9)
        } else {
            STEPS
                .iter()
                .rev()
                .copied()
                .find(|s| *s < self.goal_zoom - 1e-9)
        };
        let Some(next) = next else {
            return;
        };
        self.goal_zoom = next;
        self.manual = next > 1.0;
        if inwards {
            self.goal_center = clamp_center(clamp_unit(at), next);
        } else {
            self.goal_center = clamp_center(self.goal_center, next);
        }
    }

    /// Back to the whole picture, and the keys let go.
    pub fn reset(&mut self) {
        self.goal_zoom = 1.0;
        self.manual = false;
    }

    /// Moves the camera on by `seconds`. Returns whether it is still on its way, so the caller can stop asking once it
    /// is still.
    pub fn advance(&mut self, seconds: f64) -> bool {
        let seconds = seconds.clamp(0.0, 0.25);
        self.quiet += seconds;
        if self.auto
            && !self.manual
            && self.goal_zoom > 1.0
            && self.hold.is_some_and(|hold| self.quiet >= hold)
        {
            self.goal_zoom = 1.0;
        }
        if self.goal_zoom <= 1.0 {
            self.goal_center = Point::new(0.5, 0.5);
        }
        // The zoom closes in on a log scale, so going from 1 to 2 takes as long as from 2 to 4.
        let towards =
            |now: f64, goal: f64, time: f64| now + (goal - now) * (1.0 - (-seconds / time).exp());
        let log = towards(self.zoom.ln(), self.goal_zoom.ln(), ZOOM_TIME);
        self.zoom = log.exp();
        self.center = Point::new(
            towards(self.center.x, self.goal_center.x, PAN_TIME),
            towards(self.center.y, self.goal_center.y, PAN_TIME),
        );
        let near = (self.zoom - self.goal_zoom).abs() < EPSILON
            && (self.center.x - self.goal_center.x).abs() < EPSILON
            && (self.center.y - self.goal_center.y).abs() < EPSILON;
        if near {
            self.zoom = self.goal_zoom;
            self.center = self.goal_center;
        }
        !near || self.waits()
    }

    /// Goes where it is going at once, for a person who asked for less motion.
    pub fn settle(&mut self) {
        self.zoom = self.goal_zoom;
        self.center = self.goal_center;
    }

    /// An automatic zoom that will go back out by itself needs the time to keep coming.
    fn waits(&self) -> bool {
        self.auto && !self.manual && self.goal_zoom > 1.0 && self.hold.is_some()
    }

    /// Zoomed in now, or about to be.
    pub fn zoomed(&self) -> bool {
        self.zoom > 1.0 + EPSILON || self.goal_zoom > 1.0
    }

    pub fn zoom(&self) -> f64 {
        self.zoom
    }

    /// The part of the picture on the audience's screen, never beyond its edges.
    pub fn view(&self) -> Rect {
        let side = 1.0 / self.zoom.max(1.0);
        let center = clamp_center(self.center, self.zoom);
        Rect::new(center.x - side / 2.0, center.y - side / 2.0, side, side)
    }
}

fn clamp_unit(p: Point) -> Point {
    let unit = |v: f64| {
        if v.is_finite() {
            v.clamp(0.0, 1.0)
        } else {
            0.5
        }
    };
    Point::new(unit(p.x), unit(p.y))
}

/// The middle of a view at `zoom` that keeps the whole view on the picture.
fn clamp_center(c: Point, zoom: f64) -> Point {
    let half = 0.5 / zoom.max(1.0);
    Point::new(c.x.clamp(half, 1.0 - half), c.y.clamp(half, 1.0 - half))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Runs the camera for `seconds` in steps of a sixtieth.
    fn run(camera: &mut Camera, seconds: f64) {
        for _ in 0..(seconds * 60.0) as usize {
            camera.advance(1.0 / 60.0);
        }
    }

    fn close(a: f64, b: f64) -> bool {
        (a - b).abs() < 1e-3
    }

    #[test]
    fn a_click_zooms_in_where_it_lands_and_the_quiet_takes_it_back_out() {
        let mut camera = Camera::new(2.0, Some(3.0), true);
        assert_eq!(
            camera.view(),
            Rect::new(0.0, 0.0, 1.0, 1.0),
            "the whole picture at first"
        );
        camera.click(Point::new(0.3, 0.6));
        run(&mut camera, 2.0);
        let view = camera.view();
        assert!(
            close(view.w, 0.5)
                && close(view.x + view.w / 2.0, 0.3)
                && close(view.y + view.h / 2.0, 0.6)
        );
        run(&mut camera, 3.5);
        assert!(
            close(camera.view().w, 1.0),
            "three quiet seconds go back to the whole picture"
        );
        assert!(!camera.advance(1.0 / 60.0), "and then it is still");
    }

    #[test]
    fn with_less_motion_the_camera_is_there_at_once() {
        let mut camera = Camera::new(2.0, None, true);
        camera.click(Point::new(0.25, 0.75));
        camera.advance(1.0 / 60.0);
        camera.settle();
        let view = camera.view();
        assert_eq!((view.x, view.y, view.w), (0.0, 0.5, 0.5));
    }

    #[test]
    fn the_view_never_leaves_the_picture() {
        let mut camera = Camera::new(4.0, None, true);
        for corner in [
            Point::new(0.0, 0.0),
            Point::new(1.0, 1.0),
            Point::new(0.99, 0.01),
        ] {
            camera.click(corner);
            for _ in 0..240 {
                camera.advance(1.0 / 60.0);
                let v = camera.view();
                assert!(
                    v.x >= -1e-9
                        && v.y >= -1e-9
                        && v.max_x() <= 1.0 + 1e-9
                        && v.max_y() <= 1.0 + 1e-9,
                    "{v:?}"
                );
            }
        }
        camera.click(Point::new(f64::NAN, f64::INFINITY));
        run(&mut camera, 1.0);
        assert!(
            camera.view().x.is_finite(),
            "nonsense from the pointer is not taken"
        );
    }

    #[test]
    fn the_view_follows_the_pointer_only_once_it_leaves_the_middle() {
        let mut camera = Camera::new(2.0, None, true);
        camera.click(Point::new(0.5, 0.5));
        run(&mut camera, 2.0);
        camera.pointer(Point::new(0.55, 0.45));
        run(&mut camera, 1.0);
        assert!(
            close(camera.view().x, 0.25),
            "a small move stays inside the middle part"
        );
        camera.pointer(Point::new(0.9, 0.5));
        run(&mut camera, 1.0);
        let v = camera.view();
        assert!(
            v.x > 0.25 && 0.9 <= v.max_x(),
            "the pointer is brought back into view: {v:?}"
        );
        let mut whole = Camera::new(2.0, None, true);
        whole.pointer(Point::new(0.9, 0.9));
        run(&mut whole, 1.0);
        assert!(
            close(whole.view().w, 1.0),
            "not zoomed, the pointer moves nothing"
        );
    }

    #[test]
    fn keys_step_through_the_zooms_and_the_quiet_leaves_them_alone() {
        let mut camera = Camera::new(2.0, Some(1.0), true);
        camera.step(true, Point::new(0.5, 0.5));
        camera.step(true, Point::new(0.5, 0.5));
        run(&mut camera, 3.0);
        assert!(
            close(camera.zoom(), 2.0),
            "1 to 1.5 to 2, kept through the quiet"
        );
        camera.step(false, Point::new(0.5, 0.5));
        camera.step(false, Point::new(0.5, 0.5));
        camera.step(false, Point::new(0.5, 0.5));
        run(&mut camera, 3.0);
        assert!(
            close(camera.zoom(), 1.0),
            "and out again, no further than the whole picture"
        );
        for _ in 0..20 {
            camera.step(true, Point::new(0.5, 0.5));
        }
        run(&mut camera, 3.0);
        assert!(
            close(camera.zoom(), 8.0),
            "no further in than the last step"
        );
        camera.reset();
        run(&mut camera, 3.0);
        assert!(close(camera.zoom(), 1.0));
    }

    #[test]
    fn without_the_automatic_zoom_a_click_does_nothing_and_turning_it_off_zooms_out() {
        let mut camera = Camera::new(2.0, None, false);
        camera.click(Point::new(0.2, 0.2));
        run(&mut camera, 1.0);
        assert!(close(camera.zoom(), 1.0));
        camera.set_auto(true);
        camera.click(Point::new(0.2, 0.2));
        run(&mut camera, 3.0);
        assert!(close(camera.zoom(), 2.0) && camera.zoomed());
        camera.set_auto(false);
        run(&mut camera, 3.0);
        assert!(close(camera.zoom(), 1.0) && !camera.zoomed());
    }

    #[test]
    fn clicks_keep_an_automatic_zoom_and_a_long_frame_is_taken_as_a_short_one() {
        let mut camera = Camera::new(2.5, Some(2.0), true);
        camera.click(Point::new(0.5, 0.5));
        for _ in 0..4 {
            run(&mut camera, 1.5);
            camera.click(Point::new(0.5, 0.5));
        }
        assert!(
            close(camera.zoom(), 2.5),
            "six seconds of clicks, still zoomed"
        );
        camera.advance(10.0);
        assert!(
            camera.zoom() > 1.5,
            "a stall of the clock does not jump the zoom out at once"
        );
    }
}
