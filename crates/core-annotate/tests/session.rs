use core_annotate::{
    Colour, Effect, Kind, Mods, Point, Rect, Session, Tool, WIDTHS,
    shape::{Handle, PALETTE},
};

const NO: Mods = Mods { shift: false };
const SHIFT: Mods = Mods { shift: true };

fn pt(x: f64, y: f64) -> Point {
    Point::new(x, y)
}

/// A press at `from`, a drag through `via` and a release at `to`.
fn drag(s: &mut Session, from: Point, to: Point) -> Effect {
    s.pointer_down(from);
    s.pointer_drag(pt((from.x + to.x) / 2.0, (from.y + to.y) / 2.0), NO);
    s.pointer_up(to, NO)
}

fn draw(s: &mut Session, tool: Tool, from: Point, to: Point) -> u64 {
    s.set_tool(tool);
    drag(s, from, to);
    s.selection().expect("something was drawn")
}

fn session() -> Session {
    Session::new(400, 300, 1.0)
}

#[test]
fn every_drawing_tool_makes_its_kind_and_selects_it() {
    for (tool, check) in [
        (
            Tool::Arrow,
            (|k: &Kind| matches!(k, Kind::Arrow { .. })) as fn(&Kind) -> bool,
        ),
        (Tool::Line, |k| matches!(k, Kind::Line { .. })),
        (Tool::Rectangle, |k| matches!(k, Kind::Rectangle(_))),
        (Tool::Ellipse, |k| matches!(k, Kind::Ellipse(_))),
        (Tool::Pen, |k| matches!(k, Kind::Pen(_))),
        (Tool::Highlighter, |k| matches!(k, Kind::Highlight(_))),
        (Tool::Blur, |k| matches!(k, Kind::Blur(_))),
    ] {
        let mut s = session();
        let id = draw(&mut s, tool, pt(40.0, 40.0), pt(160.0, 120.0));
        assert_eq!(s.doc().objects().len(), 1, "{tool:?}");
        assert!(check(&s.doc().object(id).unwrap().kind), "{tool:?}");
        assert_eq!(s.doc().object(id).unwrap().style, s.style());
        assert!(s.can_undo());
        assert!(s.undo());
        assert!(s.doc().objects().is_empty(), "{tool:?}");
        assert!(s.redo());
        assert_eq!(s.doc().objects().len(), 1);
    }
}

#[test]
fn a_click_without_a_drag_draws_no_shape() {
    for tool in [
        Tool::Arrow,
        Tool::Line,
        Tool::Rectangle,
        Tool::Ellipse,
        Tool::Pen,
        Tool::Highlighter,
        Tool::Blur,
    ] {
        let mut s = session();
        s.set_tool(tool);
        s.pointer_down(pt(50.0, 50.0));
        s.pointer_up(pt(50.0, 50.0), NO);
        assert!(s.doc().objects().is_empty(), "{tool:?}");
        assert!(!s.can_undo(), "{tool:?}");
        assert_eq!(s.selection(), None);
    }
}

#[test]
fn a_shape_dragged_backwards_is_the_same_shape() {
    let mut s = session();
    let id = draw(&mut s, Tool::Rectangle, pt(160.0, 120.0), pt(40.0, 40.0));
    assert_eq!(
        s.doc().object(id).unwrap().kind,
        Kind::Rectangle(Rect::new(40.0, 40.0, 120.0, 80.0))
    );
}

#[test]
fn shift_keeps_lines_on_the_eighths_and_boxes_square() {
    let mut s = session();
    s.set_tool(Tool::Line);
    s.pointer_down(pt(100.0, 100.0));
    s.pointer_drag(pt(200.0, 108.0), SHIFT);
    s.pointer_up(pt(200.0, 108.0), SHIFT);
    let id = s.selection().unwrap();
    let Kind::Line { to, .. } = s.doc().object(id).unwrap().kind else {
        panic!()
    };
    assert!((to.y - 100.0).abs() < 1e-9, "level: {to:?}");

    s.set_tool(Tool::Ellipse);
    s.pointer_down(pt(10.0, 10.0));
    s.pointer_drag(pt(90.0, 40.0), SHIFT);
    s.pointer_up(pt(90.0, 40.0), SHIFT);
    let id = s.selection().unwrap();
    let Kind::Ellipse(r) = s.doc().object(id).unwrap().kind else {
        panic!()
    };
    assert_eq!((r.w, r.h), (80.0, 80.0));
}

#[test]
fn the_pen_records_a_path_and_ignores_points_too_close_together() {
    let mut s = session();
    s.set_tool(Tool::Pen);
    s.pointer_down(pt(10.0, 10.0));
    for i in 1..=30 {
        s.pointer_drag(pt(10.0 + f64::from(i) * 0.4, 10.0), NO);
    }
    s.pointer_up(pt(40.0, 10.0), NO);
    let Kind::Pen(points) = &s.doc().object(s.selection().unwrap()).unwrap().kind else {
        panic!()
    };
    assert!(points.len() < 15, "decimated: {}", points.len());
    assert_eq!(points[0], pt(10.0, 10.0));
    assert_eq!(
        *points.last().unwrap(),
        pt(40.0, 10.0),
        "the release point is kept"
    );
}

#[test]
fn dragging_past_the_edge_grows_the_canvas_instead_of_clipping() {
    let mut s = session();
    let before = s.doc().canvas();
    s.set_tool(Tool::Rectangle);
    s.pointer_down(pt(300.0, 200.0));
    s.pointer_drag(pt(520.0, 340.0), NO);
    let live = s.doc().canvas();
    assert!(live.right() >= 520.0 && live.bottom() >= 340.0, "{live:?}");
    assert_eq!(
        (live.x, live.y),
        (before.x, before.y),
        "the top-left did not move"
    );
    s.pointer_up(pt(520.0, 340.0), NO);
    let id = s.selection().unwrap();
    let object = s.doc().object(id).unwrap();
    assert!(s.doc().canvas().contains_rect(&object.bounds()));
    assert!(s.undo());
    assert_eq!(
        s.doc().canvas(),
        before,
        "one undo takes the growth back too"
    );
}

#[test]
fn dragging_back_inside_shrinks_the_canvas_back_during_the_drag() {
    let mut s = session();
    let before = s.doc().canvas();
    s.set_tool(Tool::Line);
    s.pointer_down(pt(100.0, 100.0));
    s.pointer_drag(pt(800.0, 100.0), NO);
    assert!(s.doc().canvas().w > 700.0);
    s.pointer_drag(pt(300.0, 100.0), NO);
    assert_eq!(s.doc().canvas(), before);
    s.pointer_up(pt(300.0, 100.0), NO);
    assert_eq!(s.doc().canvas(), before);
}

#[test]
fn growing_left_and_up_keeps_every_object_where_it_is() {
    let mut s = session();
    let a = draw(&mut s, Tool::Rectangle, pt(50.0, 50.0), pt(150.0, 150.0));
    let b = draw(&mut s, Tool::Ellipse, pt(200.0, 60.0), pt(300.0, 160.0));
    let (ka, kb) = (
        s.doc().object(a).unwrap().clone(),
        s.doc().object(b).unwrap().clone(),
    );
    let before = s.doc().canvas();
    draw(&mut s, Tool::Arrow, pt(20.0, 20.0), pt(-120.0, -80.0));
    let after = s.doc().canvas();
    assert!(after.x <= -120.0 && after.y <= -80.0, "{after:?}");
    assert!(after.contains_rect(&before));
    assert_eq!(s.doc().object(a).unwrap(), &ka);
    assert_eq!(s.doc().object(b).unwrap(), &kb);
    assert_eq!(
        s.doc().image_rect(),
        Rect::new(0.0, 0.0, 400.0, 300.0),
        "and so is the picture"
    );
}

#[test]
fn clicking_selects_the_top_object_and_empty_space_clears_the_selection() {
    let mut s = session();
    s.set_tool(Tool::Rectangle);
    s.set_fill(true);
    let low = draw(&mut s, Tool::Rectangle, pt(10.0, 10.0), pt(110.0, 110.0));
    let high = draw(&mut s, Tool::Rectangle, pt(60.0, 60.0), pt(160.0, 160.0));
    s.set_tool(Tool::Select);
    s.pointer_down(pt(80.0, 80.0));
    s.pointer_up(pt(80.0, 80.0), NO);
    assert_eq!(s.selection(), Some(high));
    s.pointer_down(pt(20.0, 20.0));
    s.pointer_up(pt(20.0, 20.0), NO);
    assert_eq!(s.selection(), Some(low));
    s.pointer_down(pt(380.0, 280.0));
    s.pointer_up(pt(380.0, 280.0), NO);
    assert_eq!(s.selection(), None);
}

#[test]
fn moving_an_object_is_one_step_and_a_click_on_it_is_none() {
    let mut s = session();
    let id = draw(&mut s, Tool::Rectangle, pt(10.0, 10.0), pt(110.0, 110.0));
    s.set_fill(true);
    s.undo();
    s.redo();
    let original = s.doc().object(id).unwrap().clone();
    s.set_tool(Tool::Select);
    s.pointer_down(pt(50.0, 50.0));
    s.pointer_up(pt(50.0, 50.0), NO);
    assert_eq!(s.doc().object(id).unwrap(), &original);
    drag(&mut s, pt(50.0, 50.0), pt(80.0, 90.0));
    assert_eq!(
        s.doc().object(id).unwrap().kind,
        Kind::Rectangle(Rect::new(40.0, 50.0, 100.0, 100.0))
    );
    assert!(s.undo());
    assert_eq!(s.doc().object(id).unwrap(), &original, "back in one undo");
    assert!(s.redo());
    assert_eq!(
        s.doc().object(id).unwrap().kind,
        Kind::Rectangle(Rect::new(40.0, 50.0, 100.0, 100.0))
    );
}

#[test]
fn moving_an_object_off_the_edge_grows_the_canvas_and_undo_takes_it_back() {
    let mut s = session();
    let id = draw(&mut s, Tool::Rectangle, pt(300.0, 200.0), pt(390.0, 290.0));
    s.set_fill(true);
    s.set_tool(Tool::Select);
    let canvas = s.doc().canvas();
    drag(&mut s, pt(340.0, 240.0), pt(500.0, 400.0));
    assert!(s.doc().canvas().right() > canvas.right());
    assert!(
        s.doc()
            .canvas()
            .contains_rect(&s.doc().object(id).unwrap().bounds())
    );
    s.undo();
    assert_eq!(s.doc().canvas(), canvas);
}

#[test]
fn dragging_a_handle_resizes_and_a_flip_is_allowed() {
    let mut s = session();
    let id = draw(&mut s, Tool::Rectangle, pt(100.0, 100.0), pt(200.0, 160.0));
    s.set_tool(Tool::Select);
    let handle = s
        .selected_object()
        .unwrap()
        .handles()
        .into_iter()
        .find(|(h, _)| *h == Handle::SouthEast)
        .unwrap()
        .1;
    drag(&mut s, handle, pt(260.0, 220.0));
    assert_eq!(
        s.doc().object(id).unwrap().kind,
        Kind::Rectangle(Rect::new(100.0, 100.0, 160.0, 120.0))
    );
    let nw = s
        .selected_object()
        .unwrap()
        .handles()
        .into_iter()
        .find(|(h, _)| *h == Handle::NorthWest)
        .unwrap()
        .1;
    drag(&mut s, nw, pt(300.0, 260.0));
    assert_eq!(
        s.doc().object(id).unwrap().kind,
        Kind::Rectangle(Rect::new(260.0, 220.0, 40.0, 40.0))
    );
}

#[test]
fn the_handle_of_the_selected_object_wins_over_drawing_with_a_tool() {
    let mut s = session();
    let id = draw(&mut s, Tool::Line, pt(50.0, 50.0), pt(150.0, 50.0));
    // The tool is still the line tool; a press on the end grip moves the end, it does not start a new line.
    drag(&mut s, pt(152.0, 52.0), pt(150.0, 120.0));
    assert_eq!(s.doc().objects().len(), 1);
    let Kind::Line { to, .. } = s.doc().object(id).unwrap().kind else {
        panic!()
    };
    assert_eq!(to, pt(150.0, 120.0));
}

#[test]
fn cancelling_a_drag_puts_everything_back() {
    let mut s = session();
    let id = draw(&mut s, Tool::Rectangle, pt(10.0, 10.0), pt(110.0, 110.0));
    s.set_fill(true);
    s.set_tool(Tool::Select);
    let (object, canvas) = (s.doc().object(id).unwrap().clone(), s.doc().canvas());
    s.pointer_down(pt(50.0, 50.0));
    s.pointer_drag(pt(700.0, 500.0), NO);
    s.cancel_gesture();
    assert_eq!(s.doc().object(id).unwrap(), &object);
    assert_eq!(s.doc().canvas(), canvas);
    s.set_tool(Tool::Arrow);
    s.pointer_down(pt(1.0, 1.0));
    s.pointer_drag(pt(300.0, 300.0), NO);
    s.cancel_gesture();
    assert_eq!(s.doc().objects().len(), 1);
    assert!(!s.gesture_active());
}

#[test]
fn undo_during_a_drag_drops_the_drag_and_then_undoes() {
    let mut s = session();
    draw(&mut s, Tool::Rectangle, pt(10.0, 10.0), pt(110.0, 110.0));
    s.set_tool(Tool::Arrow);
    s.pointer_down(pt(5.0, 5.0));
    s.pointer_drag(pt(200.0, 200.0), NO);
    assert!(s.undo());
    assert!(s.doc().objects().is_empty());
    assert!(!s.gesture_active());
}

#[test]
fn delete_and_duplicate() {
    let mut s = session();
    let id = draw(&mut s, Tool::Rectangle, pt(10.0, 10.0), pt(110.0, 110.0));
    assert!(s.duplicate_selected());
    assert_eq!(s.doc().objects().len(), 2);
    let copy = s.selection().unwrap();
    assert_ne!(copy, id);
    let (a, b) = (
        s.doc().object(id).unwrap().frame(),
        s.doc().object(copy).unwrap().frame(),
    );
    assert_eq!((b.x - a.x, b.y - a.y), (12.0, 12.0));
    assert_eq!(s.doc().objects()[1].id, copy, "on top");
    assert!(s.delete_selected());
    assert_eq!(s.doc().objects().len(), 1);
    assert_eq!(s.selection(), None);
    assert!(!s.delete_selected(), "nothing selected");
    assert!(s.undo());
    assert_eq!(s.doc().objects().len(), 2);
    assert!(s.undo());
    assert_eq!(s.doc().objects().len(), 1);
}

#[test]
fn undo_and_redo_select_the_object_they_touch_and_drop_one_that_is_gone() {
    let mut s = session();
    let id = draw(&mut s, Tool::Rectangle, pt(10.0, 10.0), pt(110.0, 110.0));
    s.undo();
    assert_eq!(s.selection(), None, "the object is gone");
    s.redo();
    assert_eq!(
        s.selection(),
        Some(id),
        "redo brings it back selected, ready to delete or move"
    );
    s.delete_selected();
    s.undo();
    assert_eq!(s.selection(), Some(id), "so does undoing a delete");
    let other = draw(&mut s, Tool::Ellipse, pt(200.0, 100.0), pt(300.0, 200.0));
    s.set_tool(Tool::Select);
    s.pointer_down(pt(390.0, 290.0));
    s.pointer_up(pt(390.0, 290.0), NO);
    assert_eq!(s.selection(), None);
    s.crop(Rect::new(150.0, 50.0, 200.0, 200.0));
    assert!(
        s.doc().object(id).is_none(),
        "the first rectangle is cropped away"
    );
    assert_eq!(s.selection(), None);
    s.undo();
    assert_eq!(
        s.selection(),
        Some(id),
        "undoing a crop that dropped one object selects it again"
    );
    assert!(s.doc().object(id).is_some() && s.doc().object(other).is_some());
    s.redo();
    s.undo();
    s.crop(Rect::new(300.0, 250.0, 90.0, 40.0));
    s.undo();
    assert_eq!(
        s.selection(),
        None,
        "and with two objects back nothing is picked"
    );
}

#[test]
fn a_run_of_nudges_is_one_step_and_a_nudge_back_leaves_none() {
    let mut s = session();
    let id = draw(&mut s, Tool::Rectangle, pt(100.0, 100.0), pt(200.0, 200.0));
    s.end_run();
    let before = s.doc().clone();
    for _ in 0..5 {
        assert!(s.nudge(1.0, 0.0));
    }
    assert!(s.nudge(0.0, 10.0));
    assert_eq!(
        s.doc().object(id).unwrap().kind,
        Kind::Rectangle(Rect::new(105.0, 110.0, 100.0, 100.0))
    );
    assert!(s.undo());
    assert_eq!(s.doc(), &before, "all six were one step");
    assert!(s.redo());
    s.undo();
    s.redo();
    s.end_run();
    s.nudge(-5.0, -10.0);
    assert_eq!(
        s.doc().object(id).unwrap().kind,
        Kind::Rectangle(Rect::new(100.0, 100.0, 100.0, 100.0))
    );
    let nudges_back = s.doc().clone();
    s.end_run();
    s.nudge(3.0, 0.0);
    s.nudge(-3.0, 0.0);
    assert_eq!(s.doc(), &nudges_back);
    assert!(s.undo(), "the step before is still there");
}

#[test]
fn nudging_past_the_edge_grows_the_canvas_and_a_nudge_back_takes_it_in() {
    let mut s = session();
    draw(&mut s, Tool::Rectangle, pt(300.0, 100.0), pt(396.0, 200.0));
    s.end_run();
    let canvas = s.doc().canvas();
    for _ in 0..10 {
        s.nudge(1.0, 0.0);
    }
    assert!(s.doc().canvas().right() > canvas.right());
    s.undo();
    assert_eq!(s.doc().canvas(), canvas);
}

#[test]
fn style_changes_apply_to_what_is_selected_and_to_the_next_drawing() {
    let mut s = session();
    let id = draw(&mut s, Tool::Rectangle, pt(10.0, 10.0), pt(110.0, 110.0));
    s.end_run();
    s.set_colour(PALETTE[4]);
    s.set_colour(PALETTE[5]);
    s.set_width(WIDTHS[3]);
    s.set_fill(true);
    let styled = s.doc().object(id).unwrap().style;
    assert_eq!(
        (styled.colour, styled.width, styled.fill),
        (PALETTE[5], WIDTHS[3], true)
    );
    assert_eq!(s.style(), styled);
    assert!(s.undo());
    assert_eq!(
        s.doc().object(id).unwrap().style,
        Default::default(),
        "four changes, one step"
    );
    s.redo();
    s.set_tool(Tool::Select);
    s.pointer_down(pt(390.0, 290.0));
    s.pointer_up(pt(390.0, 290.0), NO);
    s.set_colour(PALETTE[1]);
    let next = draw(&mut s, Tool::Ellipse, pt(200.0, 200.0), pt(300.0, 280.0));
    assert_eq!(s.doc().object(next).unwrap().style.colour, PALETTE[1]);
    assert_eq!(
        s.doc().object(id).unwrap().style.colour,
        PALETTE[5],
        "an unselected drawing keeps its colour"
    );
}

#[test]
fn crop_keeps_a_region_drops_what_is_outside_and_undo_brings_it_back() {
    let mut s = session();
    let inside = draw(&mut s, Tool::Rectangle, pt(120.0, 100.0), pt(180.0, 160.0));
    let outside = draw(&mut s, Tool::Ellipse, pt(10.0, 10.0), pt(60.0, 60.0));
    let straddling = draw(&mut s, Tool::Line, pt(90.0, 130.0), pt(140.0, 130.0));
    let before = s.doc().clone();
    s.set_tool(Tool::Crop);
    drag(&mut s, pt(100.0, 80.0), pt(300.0, 250.0));
    assert_eq!(s.doc().canvas(), Rect::new(100.0, 80.0, 200.0, 170.0));
    assert!(s.doc().object(inside).is_some() && s.doc().object(straddling).is_some());
    assert!(
        s.doc().object(outside).is_none(),
        "wholly outside is dropped"
    );
    assert_eq!(s.tool(), Tool::Select, "back to the pointer after a crop");
    let plan = s.export_plan();
    assert_eq!((plan.width, plan.height), (200, 170));
    assert!(s.undo());
    assert_eq!(s.doc(), &before);
    assert!(s.redo());
    assert_eq!(s.doc().canvas(), Rect::new(100.0, 80.0, 200.0, 170.0));
}

#[test]
fn crop_is_clamped_to_the_canvas_and_ignores_slivers() {
    let mut s = session();
    s.set_tool(Tool::Crop);
    drag(&mut s, pt(-50.0, -50.0), pt(200.0, 150.0));
    assert_eq!(s.doc().canvas(), Rect::new(0.0, 0.0, 200.0, 150.0));
    s.set_tool(Tool::Crop);
    drag(&mut s, pt(10.0, 10.0), pt(14.0, 100.0));
    assert_eq!(
        s.doc().canvas(),
        Rect::new(0.0, 0.0, 200.0, 150.0),
        "four points wide is a slip of the hand"
    );
    assert_eq!(s.tool(), Tool::Crop, "and the tool stays");
    drag(&mut s, pt(500.0, 500.0), pt(600.0, 600.0));
    assert_eq!(
        s.doc().canvas(),
        Rect::new(0.0, 0.0, 200.0, 150.0),
        "away from the canvas"
    );
    assert!(
        !s.crop(Rect::new(0.0, 0.0, 200.0, 150.0)),
        "the whole canvas is no crop"
    );
}

#[test]
fn a_crop_can_be_grown_again_by_drawing_past_its_edge() {
    let mut s = session();
    s.set_tool(Tool::Crop);
    drag(&mut s, pt(100.0, 100.0), pt(300.0, 250.0));
    draw(&mut s, Tool::Arrow, pt(150.0, 150.0), pt(350.0, 150.0));
    assert_eq!(s.doc().canvas().x, 100.0);
    assert!(s.doc().canvas().right() > 350.0);
    assert_eq!(
        s.doc().image_rect(),
        Rect::new(0.0, 0.0, 400.0, 300.0),
        "the picture never moves"
    );
}

#[test]
fn the_crop_preview_follows_the_drag() {
    let mut s = session();
    s.set_tool(Tool::Crop);
    s.pointer_down(pt(50.0, 60.0));
    s.pointer_drag(pt(150.0, 160.0), NO);
    assert_eq!(s.crop_preview(), Some(Rect::new(50.0, 60.0, 100.0, 100.0)));
    s.pointer_up(pt(150.0, 160.0), NO);
    assert_eq!(s.crop_preview(), None);
}

fn lines_of_text(text: &str, size: f64, _width: f64) -> f64 {
    text.lines().count().max(1) as f64 * size * 2.0
}

#[test]
fn text_is_made_by_a_click_edited_and_added_when_it_has_words() {
    let mut s = session();
    s.set_text_measure(Box::new(lines_of_text));
    s.set_tool(Tool::Text);
    s.pointer_down(pt(30.0, 40.0));
    let effect = s.pointer_up(pt(30.0, 40.0), NO);
    let Effect::EditText(id) = effect else {
        panic!("{effect:?}")
    };
    assert_eq!(s.editing(), Some(id));
    assert!(!s.can_undo(), "nothing is added until there are words");
    let Kind::Text { rect, .. } = s.doc().object(id).unwrap().kind.clone() else {
        panic!()
    };
    assert_eq!((rect.x, rect.y, rect.w), (30.0, 40.0, 200.0));
    s.finish_text("Hello\nworld");
    assert_eq!(s.editing(), None);
    let Kind::Text { rect, text } = &s.doc().object(id).unwrap().kind else {
        panic!()
    };
    assert_eq!(text, "Hello\nworld");
    assert_eq!(
        rect.h,
        2.0 * 20.0 * 2.0,
        "two lines at the measured height of the default size"
    );
    assert!(s.undo());
    assert!(s.doc().objects().is_empty());
}

#[test]
fn text_with_nothing_typed_leaves_nothing_behind() {
    let mut s = session();
    s.set_tool(Tool::Text);
    s.pointer_down(pt(30.0, 40.0));
    s.pointer_up(pt(30.0, 40.0), NO);
    s.finish_text("   ");
    assert!(s.doc().objects().is_empty());
    assert!(!s.can_undo());
    assert_eq!(s.selection(), None);
}

#[test]
fn a_text_box_dragged_out_has_the_width_it_was_given() {
    let mut s = session();
    s.set_tool(Tool::Text);
    s.pointer_down(pt(30.0, 40.0));
    s.pointer_drag(pt(180.0, 100.0), NO);
    s.pointer_up(pt(330.0, 100.0), NO);
    let id = s.editing().unwrap();
    let Kind::Text { rect, .. } = s.doc().object(id).unwrap().kind else {
        panic!()
    };
    assert_eq!((rect.x, rect.w), (30.0, 300.0));
}

#[test]
fn text_past_the_edge_grows_the_canvas_and_a_cancelled_text_gives_it_back() {
    let mut s = session();
    let canvas = s.doc().canvas();
    s.set_tool(Tool::Text);
    s.pointer_down(pt(350.0, 100.0));
    s.pointer_up(pt(350.0, 100.0), NO);
    assert!(s.doc().canvas().right() >= 550.0, "a 200 wide box from 350");
    s.finish_text("");
    assert_eq!(s.doc().canvas(), canvas);
    s.set_tool(Tool::Text);
    s.pointer_down(pt(350.0, 100.0));
    s.pointer_up(pt(350.0, 100.0), NO);
    s.finish_text("kept");
    assert!(s.doc().canvas().right() >= 550.0);
    s.undo();
    assert_eq!(s.doc().canvas(), canvas);
}

#[test]
fn editing_existing_text_is_one_step_and_clearing_it_deletes_it() {
    let mut s = session();
    s.set_tool(Tool::Text);
    s.pointer_down(pt(30.0, 40.0));
    s.pointer_up(pt(30.0, 40.0), NO);
    let id = s.editing().unwrap();
    s.finish_text("one");
    s.set_tool(Tool::Select);
    let effect = s.double_click(pt(40.0, 45.0));
    assert_eq!(effect, Effect::EditText(id));
    s.finish_text("one");
    assert!(s.undo(), "an edit that changed nothing added no step");
    assert!(s.doc().object(id).is_none());
    s.redo();
    s.double_click(pt(40.0, 45.0));
    s.finish_text("two");
    let Kind::Text { text, .. } = &s.doc().object(id).unwrap().kind else {
        panic!()
    };
    assert_eq!(text, "two");
    s.undo();
    let Kind::Text { text, .. } = &s.doc().object(id).unwrap().kind else {
        panic!()
    };
    assert_eq!(text, "one");
    s.redo();
    s.double_click(pt(40.0, 45.0));
    s.finish_text("");
    assert!(s.doc().object(id).is_none());
    s.undo();
    assert!(s.doc().object(id).is_some(), "and it comes back");
}

#[test]
fn the_text_tool_edits_text_it_is_clicked_on_instead_of_stacking_a_new_box() {
    let mut s = session();
    s.set_tool(Tool::Text);
    s.pointer_down(pt(30.0, 40.0));
    s.pointer_up(pt(30.0, 40.0), NO);
    let id = s.editing().unwrap();
    s.finish_text("one");
    s.set_tool(Tool::Text);
    let effect = s.pointer_down(pt(40.0, 45.0));
    assert_eq!(effect, Effect::EditText(id));
    s.finish_text("one");
    assert_eq!(s.doc().objects().len(), 1);
}

#[test]
fn while_text_is_edited_the_document_does_not_change_under_it() {
    let mut s = session();
    let id = draw(&mut s, Tool::Rectangle, pt(10.0, 10.0), pt(110.0, 110.0));
    s.set_tool(Tool::Text);
    s.pointer_down(pt(200.0, 200.0));
    s.pointer_up(pt(200.0, 200.0), NO);
    let before = s.doc().clone();
    assert!(!s.undo() && !s.redo() && !s.crop(Rect::new(0.0, 0.0, 100.0, 100.0)));
    assert!(!s.nudge(1.0, 1.0) && !s.duplicate_selected() && !s.delete_selected());
    assert_eq!(s.pointer_down(pt(50.0, 50.0)), Effect::None);
    assert_eq!(s.doc(), &before);
    s.finish_text("x");
    assert!(s.doc().object(id).is_some());
}

#[test]
fn changing_the_size_of_selected_text_refits_its_height() {
    let mut s = session();
    s.set_text_measure(Box::new(|_, size, _| size * 3.0));
    s.set_tool(Tool::Text);
    s.pointer_down(pt(30.0, 40.0));
    s.pointer_up(pt(30.0, 40.0), NO);
    let id = s.editing().unwrap();
    s.finish_text("abc");
    s.set_width(WIDTHS[4]);
    let Kind::Text { rect, .. } = s.doc().object(id).unwrap().kind else {
        panic!()
    };
    assert_eq!(rect.h, 50.0 * 3.0);
}

#[test]
fn export_size_is_the_canvas_times_the_scale_at_any_scale() {
    for scale in [1.0, 1.5, 2.0, 3.0] {
        let mut s = Session::new(600, 400, scale);
        draw(&mut s, Tool::Rectangle, pt(10.0, 10.0), pt(500.0, 400.0));
        draw(&mut s, Tool::Line, pt(5.3, 5.7), pt(-33.3, -41.9));
        let plan = s.export_plan();
        let canvas = s.doc().canvas();
        assert_eq!(
            f64::from(plan.width),
            (canvas.w * scale).round(),
            "scale {scale}"
        );
        assert_eq!(f64::from(plan.height), (canvas.h * scale).round());
        for edge in [canvas.x, canvas.y, canvas.right(), canvas.bottom()] {
            let px = edge * scale;
            assert!(
                (px - px.round()).abs() < 1e-6,
                "{edge} at {scale} is not on a pixel"
            );
        }
        assert!(plan.width >= 600 && plan.height >= 400, "growth only adds");
    }
}

// Property checks over random sessions.

struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }

    fn below(&mut self, n: u64) -> u64 {
        self.next() % n
    }

    fn range(&mut self, lo: f64, hi: f64) -> f64 {
        lo + (self.next() >> 11) as f64 / (1u64 << 53) as f64 * (hi - lo)
    }

    fn point(&mut self) -> Point {
        pt(self.range(-150.0, 550.0), self.range(-150.0, 450.0))
    }
}

const TOOLS: [Tool; 10] = [
    Tool::Select,
    Tool::Arrow,
    Tool::Line,
    Tool::Rectangle,
    Tool::Ellipse,
    Tool::Text,
    Tool::Pen,
    Tool::Highlighter,
    Tool::Blur,
    Tool::Crop,
];

/// One random thing a person might do. Returns whether it was a crop.
fn random_op(s: &mut Session, rng: &mut Rng) -> bool {
    let mut cropped = false;
    match rng.below(12) {
        0..=4 => {
            let tool = TOOLS[rng.below(TOOLS.len() as u64) as usize];
            cropped = tool == Tool::Crop;
            s.set_tool(tool);
            let (a, b) = (rng.point(), rng.point());
            let mut effect = s.pointer_down(a);
            for _ in 0..rng.below(4) {
                let mid = rng.point();
                s.pointer_drag(
                    mid,
                    Mods {
                        shift: rng.below(2) == 0,
                    },
                );
            }
            if effect == Effect::None {
                effect = s.pointer_up(b, NO);
            }
            if let Effect::EditText(_) = effect {
                let words = ["", "word", "two\nlines", "  "][rng.below(4) as usize];
                s.finish_text(words);
            }
        }
        5 => {
            s.delete_selected();
        }
        6 => {
            s.duplicate_selected();
        }
        7 => {
            s.nudge(
                rng.range(-12.0, 12.0).round(),
                rng.range(-12.0, 12.0).round(),
            );
        }
        8 => s.set_colour(PALETTE[rng.below(8) as usize]),
        9 => s.set_width(WIDTHS[rng.below(5) as usize]),
        10 => s.set_fill(rng.below(2) == 0),
        _ => {
            s.set_tool(Tool::Select);
            let p = rng.point();
            s.pointer_down(p);
            s.pointer_up(p, NO);
        }
    }
    cropped
}

#[test]
fn undoing_everything_restores_the_first_document_and_redoing_the_last() {
    for seed in 1..=60u64 {
        for scale in [1.0, 2.0] {
            let mut rng = Rng(seed.wrapping_mul(0x9E3779B97F4A7C15) | 1);
            let mut s = Session::new(200, 150, scale);
            let initial = s.doc().clone();
            for _ in 0..40 {
                random_op(&mut s, &mut rng);
            }
            let last = s.doc().clone();
            while s.undo() {}
            assert_eq!(s.doc(), &initial, "seed {seed} scale {scale}: undo all");
            while s.redo() {}
            assert_eq!(s.doc(), &last, "seed {seed} scale {scale}: redo all");
        }
    }
}

#[test]
fn every_state_the_person_left_things_in_can_be_undone_to() {
    for seed in 100..140u64 {
        let mut rng = Rng(seed.wrapping_mul(0xD6E8FEB86659FD93) | 1);
        let mut s = Session::new(200, 150, 1.0);
        let mut snapshots = vec![s.doc().clone()];
        for _ in 0..30 {
            s.end_run();
            snapshots.push(s.doc().clone());
            for _ in 0..1 + rng.below(3) {
                random_op(&mut s, &mut rng);
            }
        }
        // Walking back through the history passes through every state recorded at a pause.
        let mut wanted = snapshots.into_iter().rev().peekable();
        if wanted.peek() == Some(s.doc()) {
            wanted.next();
        }
        loop {
            while let Some(next) = wanted.peek() {
                if next == s.doc() {
                    wanted.next();
                } else {
                    break;
                }
            }
            if !s.undo() {
                break;
            }
        }
        assert!(wanted.next().is_none(), "seed {seed}: a state was skipped");
    }
}

#[test]
fn drawing_never_moves_other_objects_and_the_canvas_only_grows_unless_cropped() {
    for seed in 200..260u64 {
        let mut rng = Rng(seed.wrapping_mul(0xA24BAED4963EE407) | 1);
        let mut s = Session::new(200, 150, 2.0);
        for step in 0..40 {
            // A nudge or a style change right after another one extends its step, and may take growth back.
            s.end_run();
            let (canvas, objects) = (s.doc().canvas(), s.doc().objects().to_vec());
            let cropped = random_op(&mut s, &mut rng);
            let now = s.doc().canvas();
            if !cropped {
                assert!(
                    now.contains_rect(&canvas),
                    "seed {seed} step {step}: {canvas:?} -> {now:?}"
                );
            }
            // The picture itself is where it always was, and the pixel grid holds.
            assert_eq!(s.doc().image_rect(), Rect::new(0.0, 0.0, 100.0, 75.0));
            for edge in [now.x, now.y, now.w, now.h] {
                let px = edge * 2.0;
                assert!(
                    (px - px.round()).abs() < 1e-6,
                    "seed {seed} step {step}: {edge}"
                );
            }
            let plan = s.export_plan();
            assert_eq!(f64::from(plan.width), now.w * 2.0);
            assert_eq!(f64::from(plan.height), now.h * 2.0);
            // Any object that was there before and is still there only changed if the op was about it.
            let changed = objects
                .iter()
                .filter(|before| {
                    s.doc()
                        .object(before.id)
                        .is_some_and(|after| after != *before)
                })
                .count();
            assert!(
                changed <= 1,
                "seed {seed} step {step}: {changed} objects changed at once"
            );
        }
    }
}

#[test]
fn what_is_drawn_inside_the_limits_is_inside_the_canvas() {
    for seed in 300..340u64 {
        let mut rng = Rng(seed.wrapping_mul(0x8CB92BA72F3D8DD7) | 1);
        let mut s = Session::new(200, 150, 1.0);
        for _ in 0..30 {
            let cropped = random_op(&mut s, &mut rng);
            if cropped {
                continue;
            }
            if let Some(object) = s.selected_object() {
                if s.doc()
                    .objects()
                    .last()
                    .is_some_and(|last| last.id == object.id)
                {
                    let canvas = s.doc().canvas();
                    assert!(
                        canvas.inflate(1.0).contains_rect(&object.bounds()),
                        "seed {seed}: {:?} not in {canvas:?}",
                        object.bounds()
                    );
                }
            }
        }
    }
}

#[test]
fn colour_is_plain_data() {
    assert_eq!(Colour::rgb(1, 2, 3).a, 255);
    assert_eq!(Colour::CLEAR.a, 0);
}
