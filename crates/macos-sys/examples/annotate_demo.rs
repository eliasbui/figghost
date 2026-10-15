//! Opens the annotation editor on a picture made in memory, with its window far outside every screen, and
//! runs it through scripted scenes: drawing with synthetic events delivered to the window, undo, delete, copy,
//! save, text with a real text editor, a blur that is read back from the saved file, a crop and a canvas that
//! grows, and a Display P3 picture whose colour space the saved file must keep. Each scene is drawn to a PNG to look at, and every claim is checked; the exit status is 1 when one
//! fails. No real input is posted and nothing is shown on a screen.
//! `cargo run -p macos-sys --example annotate_demo -- /tmp/annotate`

use core_annotate::{PALETTE, Point, Tool, WIDTHS};
use macos_sys::{
    MainThreadMarker,
    annotate::{
        Button, Editor, Options, Outcome, Phase, Placement, Request, Strings, open_with,
        render::Source,
    },
    pasteboard::{Content, Pasteboard},
};
use objc2::{AnyThread, rc::Retained};
use objc2_app_kit::{
    NSApplication, NSApplicationActivationPolicy, NSBezierPath, NSBitmapImageFileType,
    NSBitmapImageRep, NSColor, NSColorSpace, NSDeviceRGBColorSpace, NSEventModifierFlags, NSFont,
    NSGradient, NSGraphicsContext, NSStringDrawing,
};
use objc2_foundation::{NSDictionary, NSPoint, NSRect, NSSize, NSString};
use std::{
    cell::RefCell,
    env, fs,
    path::{Path, PathBuf},
    process,
    rc::Rc,
    time::Instant,
};

/// The picture is 640 by 400 points at two pixels per point.
const SCALE: f64 = 2.0;
const SIZE: (f64, f64) = (640.0, 400.0);

/// Where things are in the picture, in points from its top-left corner.
const CARD_LINE: (f64, f64, f64, f64) = (60.0, 168.0, 330.0, 24.0);

struct Checks {
    failures: usize,
    total: usize,
}

impl Checks {
    fn check(&mut self, ok: bool, what: impl AsRef<str>) {
        self.total += 1;
        if ok {
            println!("  ok    {}", what.as_ref());
        } else {
            self.failures += 1;
            println!("  FAIL  {}", what.as_ref());
        }
    }
}

fn picture() -> Vec<u8> {
    picture_at(SCALE)
}

/// The picture drawn with `scale` pixels per point of its 640 by 400.
fn picture_at(scale: f64) -> Vec<u8> {
    let (w, h) = ((SIZE.0 * scale) as isize, (SIZE.1 * scale) as isize);
    // SAFETY: a null plane pointer asks for a bitmap the representation owns.
    let rep = unsafe {
        NSBitmapImageRep::initWithBitmapDataPlanes_pixelsWide_pixelsHigh_bitsPerSample_samplesPerPixel_hasAlpha_isPlanar_colorSpaceName_bytesPerRow_bitsPerPixel(
            NSBitmapImageRep::alloc(),
            std::ptr::null_mut(),
            w,
            h,
            8,
            4,
            true,
            false,
            NSDeviceRGBColorSpace,
            0,
            32,
        )
    }
    .expect("bitmap");
    let context = NSGraphicsContext::graphicsContextWithBitmapImageRep(&rep).expect("context");
    NSGraphicsContext::saveGraphicsState_class();
    NSGraphicsContext::setCurrentContext(Some(&context));
    let rect = |x: f64, top: f64, width: f64, height: f64| {
        NSRect::new(
            NSPoint::new(x * scale, (SIZE.1 - top - height) * scale),
            NSSize::new(width * scale, height * scale),
        )
    };
    let gradient = NSGradient::initWithStartingColor_endingColor(
        NSGradient::alloc(),
        &NSColor::colorWithSRGBRed_green_blue_alpha(0.62, 0.72, 0.88, 1.0),
        &NSColor::colorWithSRGBRed_green_blue_alpha(0.86, 0.80, 0.92, 1.0),
    )
    .expect("gradient");
    gradient.drawInRect_angle(rect(0.0, 0.0, SIZE.0, SIZE.1), -90.0);
    NSColor::whiteColor().setFill();
    NSBezierPath::bezierPathWithRoundedRect_xRadius_yRadius(
        rect(30.0, 30.0, 580.0, 340.0),
        14.0,
        14.0,
    )
    .fill();
    NSColor::colorWithSRGBRed_green_blue_alpha(0.93, 0.93, 0.95, 1.0).setFill();
    NSBezierPath::bezierPathWithRect(rect(30.0, 30.0, 580.0, 34.0)).fill();
    for (i, (r, g, b)) in [(1.0, 0.37, 0.34), (1.0, 0.74, 0.18), (0.16, 0.80, 0.26)]
        .iter()
        .enumerate()
    {
        NSColor::colorWithSRGBRed_green_blue_alpha(*r, *g, *b, 1.0).setFill();
        NSBezierPath::bezierPathWithOvalInRect(rect(44.0 + i as f64 * 20.0, 41.0, 12.0, 12.0))
            .fill();
    }
    let draw = |text: &str, x: f64, top: f64, size: f64, gray: f64| {
        let font = NSFont::systemFontOfSize(size * scale);
        let colour = NSColor::colorWithWhite_alpha(gray, 1.0);
        let attrs = NSDictionary::from_slices(
            &[unsafe { objc2_app_kit::NSFontAttributeName }, unsafe {
                objc2_app_kit::NSForegroundColorAttributeName
            }],
            &[&*font as &objc2::runtime::AnyObject, &*colour],
        );
        // SAFETY: the attributes hold a font and a colour under their own keys.
        unsafe {
            NSString::from_str(text).drawAtPoint_withAttributes(
                NSPoint::new(x * scale, (SIZE.1 - top - size * 1.25) * scale),
                Some(&attrs),
            )
        };
    };
    draw("Quarterly report", 60.0, 84.0, 22.0, 0.1);
    draw("Name: Alice Nguyen", 60.0, 128.0, 16.0, 0.2);
    draw("Card: 4111 1111 1111 1111", 60.0, 170.0, 16.0, 0.2);
    draw("Email: alice@example.com", 60.0, 206.0, 16.0, 0.2);
    for (i, height) in [60.0, 95.0, 70.0, 120.0, 85.0].iter().enumerate() {
        NSColor::colorWithSRGBRed_green_blue_alpha(0.25, 0.5 + i as f64 * 0.08, 0.95, 1.0)
            .setFill();
        NSBezierPath::bezierPathWithRect(rect(
            380.0 + i as f64 * 38.0,
            340.0 - height,
            28.0,
            *height,
        ))
        .fill();
    }
    NSGraphicsContext::restoreGraphicsState_class();
    // SAFETY: an empty properties dictionary is valid for PNG output.
    let png: Retained<_> = unsafe {
        rep.representationUsingType_properties(NSBitmapImageFileType::PNG, &NSDictionary::new())
    }
    .expect("png");
    png.to_vec()
}

fn at(x: f64, y: f64) -> Point {
    Point::new(x, y)
}

fn drag(editor: &Editor, from: Point, to: Point) {
    editor.send_mouse(Phase::Down, from, false, 1);
    editor.send_mouse(
        Phase::Drag,
        Point::new((from.x + to.x) / 2.0, (from.y + to.y) / 2.0),
        false,
        1,
    );
    editor.send_mouse(Phase::Drag, to, false, 1);
    editor.send_mouse(Phase::Up, to, false, 1);
}

const COMMAND: NSEventModifierFlags = NSEventModifierFlags::Command;

/// Chooses the tool with its colour and thickness for the next drawing: first nothing is selected, so that
/// the choice is not applied to the object that was just drawn.
fn pick(editor: &Editor, tool: Tool, colour: usize, width: usize) {
    editor.click_tool(Tool::Select);
    editor.send_mouse(Phase::Down, at(-20.0, -20.0), false, 1);
    editor.send_mouse(Phase::Up, at(-20.0, -20.0), false, 1);
    editor.click(Button::Colour(colour));
    editor.click(Button::Width(width));
    editor.click_tool(tool);
}

fn object_count(editor: &Editor) -> usize {
    editor
        .with_session(|s| s.doc().objects().len())
        .unwrap_or(0)
}

fn canvas_pixels(editor: &Editor) -> (u32, u32) {
    editor
        .with_session(|s| {
            let p = s.export_plan();
            (p.width, p.height)
        })
        .unwrap_or((0, 0))
}

struct Harness {
    mtm: MainThreadMarker,
    out: PathBuf,
    save_dir: PathBuf,
    pasteboard: &'static str,
    finished: Rc<RefCell<Vec<Outcome>>>,
    sentinel: Rc<()>,
}

impl Harness {
    /// Records what a closed editor reports. The closure also holds the sentinel, which is let go of only
    /// when the editor that owns the closure is freed.
    fn on_finish(&self) -> Box<dyn Fn(Outcome)> {
        let (finished, keep) = (Rc::clone(&self.finished), Rc::clone(&self.sentinel));
        Box::new(move |outcome| {
            let _ = &keep;
            finished.borrow_mut().push(outcome);
        })
    }

    fn open(&self, save_dir: &Path) -> Editor {
        open_with(
            self.mtm,
            Request {
                png: picture(),
                scale: SCALE,
                title: "Edit capture".into(),
                save_dir: save_dir.to_owned(),
                strings: Strings::english(),
                on_finish: self.on_finish(),
            },
            Options {
                placement: Placement::Offscreen,
                pasteboard: Pasteboard::named(self.pasteboard),
            },
        )
    }

    fn snap(&self, editor: &Editor, name: &str) {
        match editor.snapshot_png() {
            Some(png) => {
                let path = self.out.join(format!("{name}.png"));
                fs::write(&path, png).expect("write snapshot");
                println!("  wrote {}", path.display());
            }
            None => println!("  no snapshot for {name}"),
        }
    }

    fn export(&self, editor: &Editor, name: &str) -> Vec<u8> {
        let png = editor.export_png().expect("export");
        fs::write(self.out.join(format!("{name}.png")), &png).expect("write export");
        png
    }
}

/// Pixels at (x, y) of a decoded picture.
fn pixel(source: &Source, x: u32, y: u32) -> [u8; 4] {
    source.pixels().at(x, y)
}

fn scene_events(h: &Harness, c: &mut Checks) {
    println!("scene: drawing with synthetic events");
    let editor = h.open(&h.save_dir);
    let frame = editor.frame().expect("frame");
    c.check(
        frame.origin.x < -20_000.0,
        format!(
            "the window is outside every screen (x = {:.0})",
            frame.origin.x
        ),
    );
    c.check(editor.is_open(), "the editor is open");
    c.check(
        editor.takes_keyboard_without_activating(),
        "the window is a non-activating panel that can become key",
    );
    objc2_foundation::NSRunLoop::currentRunLoop()
        .runUntilDate(&objc2_foundation::NSDate::dateWithTimeIntervalSinceNow(0.4));
    println!(
        "  window {:.0}x{:.0}, key window after a turn of the run loop: {}",
        frame.size.width,
        frame.size.height,
        editor.is_key()
    );

    editor.click_tool(Tool::Rectangle);
    c.check(
        editor.with_session(|s| s.tool()) == Some(Tool::Rectangle),
        "clicking the toolbar button picks the tool",
    );
    drag(&editor, at(50.0, 70.0), at(250.0, 110.0));
    c.check(
        object_count(&editor) == 1,
        "dragging with the rectangle tool draws a rectangle",
    );

    editor.send_key("z", 6, COMMAND);
    c.check(object_count(&editor) == 0, "Command+Z undoes it");
    editor.send_key("z", 6, COMMAND | NSEventModifierFlags::Shift);
    c.check(object_count(&editor) == 1, "Shift+Command+Z redoes it");
    editor.send_key("\u{7f}", 51, NSEventModifierFlags::empty());
    c.check(
        object_count(&editor) == 0,
        "Delete removes the selected object",
    );
    editor.send_key("z", 6, COMMAND);
    c.check(object_count(&editor) == 1, "and Command+Z brings it back");

    editor.click(Button::Undo);
    c.check(object_count(&editor) == 0, "the Undo button undoes");
    editor.click(Button::Redo);
    c.check(object_count(&editor) == 1, "the Redo button redoes");
    editor.click(Button::Fill);
    c.check(
        editor
            .with_session(|s| s.selected_object().map(|o| o.style.fill))
            .flatten()
            == Some(true),
        "the Fill button fills the selected shape",
    );
    editor.click(Button::Delete);
    c.check(
        object_count(&editor) == 0,
        "the Delete button removes the selected object",
    );
    editor.click(Button::Undo);
    editor.click(Button::Undo);
    editor.click(Button::Undo);
    c.check(
        object_count(&editor) == 0,
        "three undos take the rectangle and its fill back",
    );
    editor.click(Button::Redo);
    editor.click(Button::Redo);
    c.check(
        object_count(&editor) == 1
            && editor
                .with_session(|s| s.selected_object().map(|o| o.style.fill))
                .flatten()
                == Some(true),
        "and two redos bring the filled rectangle back, selected",
    );
    let before = editor
        .with_session(|s| s.selected_object().map(|o| o.frame()))
        .flatten();
    editor.send_key("\u{f703}", 124, NSEventModifierFlags::empty());
    editor.send_key("\u{f703}", 124, NSEventModifierFlags::empty());
    editor.send_key("\u{f701}", 125, NSEventModifierFlags::Shift);
    let after = editor
        .with_session(|s| s.selected_object().map(|o| o.frame()))
        .flatten();
    c.check(
        matches!((before, after), (Some(b), Some(a)) if (a.x - b.x - 2.0).abs() < 1e-9 && (a.y - b.y - 10.0).abs() < 1e-9),
        "arrow keys nudge by one point, Shift by ten",
    );

    // The arrows scene proper.
    editor.with_session(|s| s.cancel_gesture());
    pick(&editor, Tool::Arrow, 1, 2);
    drag(&editor, at(420.0, 300.0), at(330.0, 190.0));
    pick(&editor, Tool::Ellipse, 4, 1);
    drag(&editor, at(56.0, 120.0), at(260.0, 150.0));
    pick(&editor, Tool::Line, 3, 1);
    drag(&editor, at(60.0, 330.0), at(300.0, 310.0));
    pick(&editor, Tool::Pen, 5, 1);
    editor.send_mouse(Phase::Down, at(80.0, 260.0), false, 1);
    for i in 1..=30 {
        let t = f64::from(i) / 30.0;
        editor.send_mouse(
            Phase::Drag,
            at(80.0 + t * 200.0, 260.0 + (t * 14.0).sin() * 14.0),
            false,
            1,
        );
    }
    editor.send_mouse(Phase::Up, at(280.0, 260.0), false, 1);
    pick(&editor, Tool::Highlighter, 2, 2);
    drag(&editor, at(58.0, 214.0), at(300.0, 214.0));
    pick(&editor, Tool::Rectangle, 0, 1);
    editor.with_session(|s| s.set_fill(false));
    drag(&editor, at(380.0, 120.0), at(480.0, 170.0));
    c.check(
        object_count(&editor) == 6 + 1,
        format!("seven objects drawn ({})", object_count(&editor)),
    );
    h.snap(&editor, "annotate_1_arrows");
    drop(editor);
    c.check(
        h.finished.borrow().len() == 1,
        "dropping the editor closes it and reports once",
    );
    h.finished.borrow_mut().clear();
}

fn scene_text(h: &Harness, c: &mut Checks) {
    println!("scene: text with a real text editor");
    let editor = h.open(&h.save_dir);
    editor.click_tool(Tool::Text);
    editor.with_session(|s| {
        s.set_colour(PALETTE[0]);
        s.set_width(WIDTHS[2]);
    });
    editor.send_mouse(Phase::Down, at(330.0, 90.0), false, 1);
    editor.send_mouse(Phase::Up, at(330.0, 90.0), false, 1);
    c.check(
        editor.text_editor_frame().is_some(),
        "clicking with the text tool opens a text editor over the canvas",
    );
    h.snap(&editor, "annotate_2_text_empty");
    editor.type_text("Check this out, Việt Nam");
    c.check(
        editor
            .text_editor_frame()
            .is_some_and(|f| f.size.height > 20.0),
        "the editor has a height for its text",
    );
    h.snap(&editor, "annotate_2_text_typing");
    editor.send_key("\u{1b}", 53, NSEventModifierFlags::empty());
    c.check(
        editor.text_editor_frame().is_none(),
        "Escape commits the text and closes the editor",
    );
    c.check(editor.is_open(), "and does not close the window");
    let text = editor
        .with_session(|s| {
            s.doc().objects().iter().find_map(|o| match &o.kind {
                core_annotate::Kind::Text { text, .. } => Some(text.clone()),
                _ => None,
            })
        })
        .flatten();
    c.check(
        text.as_deref() == Some("Check this out, Việt Nam"),
        format!("the object holds the text ({text:?})"),
    );
    h.snap(&editor, "annotate_2_text_done");
    let png = h.export(&editor, "annotate_2_text_export");
    let exported = Source::from_png(&png).expect("export decodes");
    let changed = (0..exported.size_pixels().1).step_by(3).any(|y| {
        (0..exported.size_pixels().0).step_by(3).any(|x| {
            pixel(&exported, x, y)[0] > 200 && pixel(&exported, x, y)[1] < 80 && x > 600 && y < 300
        })
    });
    c.check(
        changed,
        "the saved picture has red text pixels where the text is",
    );

    // Edit it again with a double click, and empty it.
    editor.click_tool(Tool::Select);
    for clicks in [1, 2] {
        editor.send_mouse(Phase::Down, at(340.0, 98.0), false, clicks);
        editor.send_mouse(Phase::Up, at(340.0, 98.0), false, clicks);
    }
    c.check(
        editor.text_editor_frame().is_some(),
        "a double click on text edits it again",
    );
    editor.send_key("\u{1b}", 53, NSEventModifierFlags::empty());

    // A click elsewhere commits what was typed, too.
    editor.click_tool(Tool::Text);
    editor.send_mouse(Phase::Down, at(60.0, 330.0), false, 1);
    editor.send_mouse(Phase::Up, at(60.0, 330.0), false, 1);
    editor.type_text("typed, then a click elsewhere");
    let before = object_count(&editor);
    editor.click_tool(Tool::Select);
    c.check(
        editor.text_editor_frame().is_none() && object_count(&editor) == before,
        "choosing a tool while typing commits the text",
    );
    editor.click_tool(Tool::Text);
    editor.send_mouse(Phase::Down, at(60.0, 330.0), false, 1);
    editor.send_mouse(Phase::Up, at(60.0, 330.0), false, 1);
    c.check(
        editor.text_editor_frame().is_some(),
        "and the text tool edits it again",
    );
    editor.send_mouse(Phase::Down, at(500.0, 380.0), false, 1);
    c.check(
        editor.text_editor_frame().is_none() || object_count(&editor) >= 1,
        "a click elsewhere on the canvas commits",
    );
    editor.send_key("\u{1b}", 53, NSEventModifierFlags::empty());
    editor.send_mouse(Phase::Up, at(500.0, 380.0), false, 1);
    let texts = editor
        .with_session(|s| {
            s.doc()
                .objects()
                .iter()
                .filter(|o| matches!(o.kind, core_annotate::Kind::Text { .. }))
                .count()
        })
        .unwrap_or(0);
    c.check(texts == 2, format!("two text objects in all ({texts})"));
    drop(editor);
    h.finished.borrow_mut().clear();
}

fn scene_blur(h: &Harness, c: &mut Checks) {
    println!("scene: blur over text, read back from the saved file");
    let blur_dir = h.save_dir.join("blur");
    let editor = h.open(&blur_dir);
    let original = Source::from_png(&picture()).expect("source");
    let plain = h.export(&editor, "annotate_3_plain_export");
    let plain = Source::from_png(&plain).expect("plain export");
    c.check(
        plain.pixels() == original.pixels(),
        "with nothing drawn the saved picture is the original, pixel for pixel",
    );
    editor.click_tool(Tool::Blur);
    let (x, y, w, hgt) = CARD_LINE;
    drag(&editor, at(x - 4.0, y - 2.0), at(x + w, y + hgt));
    c.check(
        object_count(&editor) == 1,
        "the blur tool makes a blur object",
    );
    // An arrow on top of nothing, and a rectangle elsewhere that must not disturb the rest.
    editor.click_tool(Tool::Rectangle);
    drag(&editor, at(400.0, 60.0), at(600.0, 100.0));
    h.snap(&editor, "annotate_3_blur");
    editor.send_key("s", 1, COMMAND);
    let files: Vec<_> = fs::read_dir(&blur_dir)
        .map(|d| d.filter_map(Result::ok).map(|e| e.path()).collect())
        .unwrap_or_default();
    c.check(files.len() == 1, "Command+S wrote one file");
    let png = fs::read(&files[0]).expect("the saved file can be read back from disk");
    fs::write(h.out.join("annotate_3_blur_export.png"), &png).expect("copy of the saved file");
    let saved = Source::from_png(&png).expect("export decodes");
    let points = NSBitmapImageRep::imageRepWithData(&objc2_foundation::NSData::with_bytes(&png))
        .map(|rep| rep.size());
    c.check(
        points.is_some_and(|s| (s.width - SIZE.0).abs() < 0.5 && (s.height - SIZE.1).abs() < 0.5),
        format!(
            "the file says it is {} by {} points, so two pixels to the point ({points:?})",
            SIZE.0, SIZE.1
        ),
    );
    c.check(
        saved.size_pixels() == original.size_pixels(),
        format!("same size ({:?})", saved.size_pixels()),
    );

    let (px, py) = (((x - 4.0) * SCALE) as u32, ((y - 2.0) * SCALE) as u32);
    let (pw, ph) = (((w + 4.0) * SCALE) as u32, ((hgt + 2.0) * SCALE) as u32);
    // Where the line has dark strokes, the file no longer has them.
    let (mut ink, mut kept) = (0u32, 0u32);
    for yy in py + 2..py + ph - 2 {
        for xx in px + 2..px + pw - 2 {
            if pixel(&original, xx, yy)[0] < 160 {
                ink += 1;
                if pixel(&saved, xx, yy) == pixel(&original, xx, yy) {
                    kept += 1;
                }
            }
        }
    }
    c.check(
        ink > 100 && kept == 0,
        format!("none of the {ink} dark pixels of the line survive ({kept} kept)"),
    );

    // Blocks: every pixel of a 24 pixel cell on the global grid is the same colour.
    let block = 24u32;
    let (cx, cy) = ((px / block + 1) * block, (py / block + 1) * block);
    let first = pixel(&saved, cx, cy);
    let flat = (0..block).all(|dy| (0..block).all(|dx| pixel(&saved, cx + dx, cy + dy) == first));
    c.check(flat, "a block of the redaction is one colour");

    // Text strokes cannot be read back: compare the darkest pixel of the line before and after.
    let darkest = |s: &Source| {
        (py..py + ph)
            .flat_map(|yy| (px..px + pw).map(move |xx| (xx, yy)))
            .map(|(xx, yy)| u32::from(pixel(s, xx, yy)[0]))
            .min()
            .unwrap_or(255)
    };
    c.check(
        darkest(&original) < 120 && darkest(&saved) > 120,
        format!(
            "the dark text is gone ({} before, {} after)",
            darkest(&original),
            darkest(&saved)
        ),
    );

    // Everything outside the blur and the rectangle is the original.
    let rect_px = (
        (396.0 * SCALE) as u32,
        (56.0 * SCALE) as u32,
        (608.0 * SCALE) as u32,
        (104.0 * SCALE) as u32,
    );
    let mut stray = 0u32;
    let (width, height) = saved.size_pixels();
    for yy in 0..height {
        for xx in 0..width {
            let in_blur = xx + 1 >= px && xx <= px + pw + 1 && yy + 1 >= py && yy <= py + ph + 1;
            let in_rect = xx >= rect_px.0 && xx <= rect_px.2 && yy >= rect_px.1 && yy <= rect_px.3;
            if !in_blur && !in_rect && pixel(&saved, xx, yy) != pixel(&original, xx, yy) {
                stray += 1;
            }
        }
    }
    c.check(
        stray == 0,
        format!("every pixel outside the blur and the rectangle is identical ({stray} differ)"),
    );

    // Move the blur and see that the file follows: the pixels it used to cover come back.
    editor.with_session(|s| {
        s.set_tool(Tool::Select);
        s.undo();
        s.pointer_down(at(x + 10.0, y + 10.0));
        s.pointer_drag(at(x + 10.0, y + 70.0), core_annotate::Mods::default());
        s.pointer_up(at(x + 10.0, y + 70.0), core_annotate::Mods::default());
    });
    let moved = Source::from_png(&editor.export_png().expect("export")).expect("decode");
    c.check(
        pixel(&moved, px + 40, py + 20) == pixel(&original, px + 40, py + 20)
            || darkest(&moved) < 120,
        "after moving the blur the line it left is the original again",
    );
    drop(editor);
    h.finished.borrow_mut().clear();
}

fn scene_crop_and_growth(h: &Harness, c: &mut Checks) {
    println!("scene: crop, then growth");
    let editor = h.open(&h.save_dir);
    let start = editor.frame().expect("frame");
    editor.click_tool(Tool::Arrow);
    editor.with_session(|s| s.set_colour(PALETTE[0]));
    drag(&editor, at(500.0, 250.0), at(740.0, 330.0));
    let grown = editor.frame().expect("frame");
    let (w, hh) = canvas_pixels(&editor);
    c.check(
        w > 1280 && hh == 800,
        format!("dragging past the right edge grows the canvas ({w}x{hh})"),
    );
    c.check(
        (grown.size.width - start.size.width).abs() < 1.0,
        "while there is room in the window it keeps its size",
    );
    drag(&editor, at(100.0, 40.0), at(-60.0, -50.0));
    let (w1, h1) = canvas_pixels(&editor);
    c.check(
        w1 > w && h1 > hh,
        format!("and past the top left corner ({w1}x{h1})"),
    );
    editor.click_tool(Tool::Rectangle);
    let before_wide = editor.frame().expect("frame");
    drag(&editor, at(100.0, 340.0), at(1250.0, 600.0));
    let wide = editor.frame().expect("frame");
    c.check(
        wide.size.width > start.size.width + 100.0 && wide.size.height > start.size.height,
        format!(
            "past the room it has the window grows ({:.0}x{:.0} to {:.0}x{:.0})",
            start.size.width, start.size.height, wide.size.width, wide.size.height
        ),
    );
    c.check(
        (wide.origin.x - before_wide.origin.x).abs() < 1.0
            && ((wide.origin.y + wide.size.height)
                - (before_wide.origin.y + before_wide.size.height))
                .abs()
                < 1.0,
        "and it grows to the right and down, where the canvas grew, keeping its left and top edges",
    );
    editor.click_tool(Tool::Line);
    drag(&editor, at(50.0, 50.0), at(-330.0, -40.0));
    let left = editor.frame().expect("frame");
    c.check(
        left.origin.x < wide.origin.x - 100.0
            && ((left.origin.x + left.size.width) - (wide.origin.x + wide.size.width)).abs() < 1.0,
        format!("growing to the left moves the window's left edge and keeps its right edge ({:.0} to {:.0})", wide.origin.x, left.origin.x),
    );
    let (w2, h2) = canvas_pixels(&editor);
    h.snap(&editor, "annotate_4_growth");
    let export = h.export(&editor, "annotate_4_growth_export");
    let decoded = Source::from_png(&export).expect("decode");
    c.check(
        decoded.size_pixels() == (w2, h2),
        format!(
            "the exported size is the canvas times the scale ({:?})",
            decoded.size_pixels()
        ),
    );
    let original = Source::from_png(&picture()).expect("source");
    let (left, top) = editor
        .with_session(|s| {
            let canvas = s.doc().canvas();
            ((-canvas.x * SCALE) as u32, (-canvas.y * SCALE) as u32)
        })
        .unwrap_or((0, 0));
    c.check(
        pixel(&decoded, left + 300, top + 600) == pixel(&original, 300, 600),
        "the picture sits where it was inside the larger canvas",
    );
    c.check(
        pixel(&decoded, 2, 2)[3] == 0,
        "and the new area is transparent",
    );

    // Crop it back down.
    editor.click_tool(Tool::Crop);
    drag(&editor, at(40.0, 40.0), at(400.0, 300.0));
    let (cw, ch) = canvas_pixels(&editor);
    c.check(
        (cw, ch) == (720, 520),
        format!("crop leaves the region ({cw}x{ch})"),
    );
    c.check(
        editor.with_session(|s| s.tool()) == Some(Tool::Select),
        "and returns to the select tool",
    );
    h.snap(&editor, "annotate_5_crop");
    let cropped = Source::from_png(&h.export(&editor, "annotate_5_crop_export")).expect("decode");
    c.check(
        cropped.size_pixels() == (720, 520),
        "the cropped file has that size",
    );
    c.check(
        pixel(&cropped, 100, 100) == pixel(&original, 80 + 100, 80 + 100),
        "and the pixels are the ones of that region of the picture",
    );
    editor.send_key("z", 6, COMMAND);
    c.check(
        canvas_pixels(&editor) == (w2, h2),
        "undo puts the canvas back",
    );
    drop(editor);
    h.finished.borrow_mut().clear();
}

/// A capture from a wide gamut display is tagged Display P3. The saved file must keep the tag and, where
/// nothing is drawn, every value.
fn scene_wide_gamut(h: &Harness, c: &mut Checks) {
    println!("scene: a Display P3 picture");
    let plain = picture();
    let rep = NSBitmapImageRep::imageRepWithData(&objc2_foundation::NSData::with_bytes(&plain))
        .expect("rep");
    let p3 = rep
        .bitmapImageRepByRetaggingWithColorSpace(&NSColorSpace::displayP3ColorSpace())
        .expect("retag");
    // SAFETY: an empty properties dictionary is valid for PNG output.
    let png = unsafe {
        p3.representationUsingType_properties(NSBitmapImageFileType::PNG, &NSDictionary::new())
    }
    .expect("png")
    .to_vec();
    let source = Source::from_png(&png).expect("source");
    println!("  source colour space: {}", source.colour_space_name());
    let editor = open_with(
        h.mtm,
        Request {
            png,
            scale: SCALE,
            title: String::new(),
            save_dir: h.save_dir.clone(),
            strings: Strings::english(),
            on_finish: h.on_finish(),
        },
        Options {
            placement: Placement::Offscreen,
            pasteboard: Pasteboard::named(h.pasteboard),
        },
    );
    let exported = Source::from_png(&editor.export_png().expect("export")).expect("decode");
    c.check(
        source.colour_space_name().contains("P3"),
        "the source is tagged as Display P3",
    );
    c.check(
        exported.colour_space_name() == source.colour_space_name(),
        "the saved file keeps the colour space",
    );
    c.check(
        exported.pixels() == source.pixels(),
        "and, with nothing drawn, every value in it",
    );
    drop(editor);
    h.finished.borrow_mut().clear();
}

fn scene_large(h: &Harness, c: &mut Checks) {
    println!("scene: a large picture (a 5K display's worth of pixels at two per point)");
    let png = picture_at(8.0);
    let started = Instant::now();
    let editor = open_with(
        h.mtm,
        Request {
            png,
            scale: 2.0,
            title: String::new(),
            save_dir: h.save_dir.clone(),
            strings: Strings::english(),
            on_finish: h.on_finish(),
        },
        Options {
            placement: Placement::Offscreen,
            pasteboard: Pasteboard::named(h.pasteboard),
        },
    );
    println!("  opened in {:?}", started.elapsed());
    let frame = editor.frame().expect("frame");
    c.check(
        frame.size.width <= 1920.0 - 40.0 + 1.0 && frame.size.height <= 1055.0,
        format!(
            "the window stops at the size of the screen ({:.0}x{:.0}) and the canvas scrolls",
            frame.size.width, frame.size.height
        ),
    );
    c.check(
        canvas_pixels(&editor) == (5120, 3200),
        "while the canvas keeps its size",
    );
    let started = Instant::now();
    let _ = editor.snapshot_png();
    println!("  one repaint of the window: {:?}", started.elapsed());
    editor.click_tool(Tool::Blur);
    let started = Instant::now();
    editor.send_mouse(Phase::Down, at(100.0, 300.0), false, 1);
    for i in 1..=10 {
        editor.send_mouse(
            Phase::Drag,
            at(100.0 + f64::from(i) * 120.0, 300.0 + f64::from(i) * 20.0),
            false,
            1,
        );
        let _ = editor.snapshot_png();
    }
    editor.send_mouse(Phase::Up, at(1300.0, 500.0), false, 1);
    println!(
        "  a blur dragged over 1200 by 200 points with a repaint at every step: {:?}",
        started.elapsed()
    );
    let started = Instant::now();
    let png = editor.export_png().expect("export");
    println!("  export: {:?}, {} bytes", started.elapsed(), png.len());
    let decoded = Source::from_png(&png).expect("decode");
    c.check(
        decoded.size_pixels() == (5120, 3200),
        "the export has all the pixels",
    );
    drop(editor);
    h.finished.borrow_mut().clear();
}

fn scene_output(h: &Harness, c: &mut Checks) {
    println!("scene: copy, save, failure and closing");
    let pasteboard = Pasteboard::named(h.pasteboard);
    let editor = h.open(&h.save_dir);
    editor.click_tool(Tool::Ellipse);
    drag(&editor, at(200.0, 100.0), at(300.0, 160.0));
    editor.send_key("c", 8, COMMAND);
    let copied = match pasteboard.read() {
        Some(Content::Image(image)) => Some((image.width, image.height)),
        _ => None,
    };
    c.check(
        copied == Some((1280, 800)),
        format!("Command+C puts a PNG of the picture on the clipboard ({copied:?})"),
    );
    editor.send_key("s", 1, COMMAND);
    editor.send_key("s", 1, COMMAND);
    let mut files: Vec<_> = fs::read_dir(&h.save_dir)
        .map(|d| {
            d.filter_map(Result::ok)
                .map(|e| e.path())
                .filter(|p| p.extension().is_some_and(|e| e == "png"))
                .collect()
        })
        .unwrap_or_default();
    files.sort();
    c.check(
        files.len() == 2,
        format!("Command+S twice makes two files ({})", files.len()),
    );
    let sizes: Vec<_> = files
        .iter()
        .filter_map(|f| fs::read(f).ok())
        .filter_map(|b| Source::from_png(&b))
        .map(|s| s.size_pixels())
        .collect();
    c.check(
        sizes == vec![(1280, 800), (1280, 800)],
        format!("both are pictures of the right size ({sizes:?})"),
    );
    editor.send_key("\u{1b}", 53, NSEventModifierFlags::empty());
    let done = h.finished.borrow().clone();
    c.check(
        done.len() == 1 && matches!(done[0], Outcome::Saved(_)),
        format!("Escape closes the window and reports the last thing done, once ({done:?})"),
    );
    c.check(!editor.is_open(), "the window is closed");
    c.check(
        editor.export_png().is_none(),
        "and the picture is let go of",
    );
    editor.close();
    drop(editor);
    c.check(
        h.finished.borrow().len() == 1,
        "and dropping the editor afterwards reports nothing more",
    );
    h.finished.borrow_mut().clear();

    // A folder that cannot be written to: the failure is shown and the window stays.
    let blocked = h.out.join("blocked");
    fs::write(&blocked, b"a file").expect("blocker");
    let editor = h.open(&blocked.join("inside"));
    editor.send_key("s", 1, COMMAND);
    c.check(editor.is_open(), "a save that fails keeps the window open");
    h.snap(&editor, "annotate_6_save_failed");
    c.check(h.finished.borrow().is_empty(), "and reports nothing yet");
    editor.click_tool(Tool::Rectangle);
    drag(&editor, at(40.0, 40.0), at(120.0, 90.0));
    editor.send_key(
        "\u{1b}",
        53,
        NSEventModifierFlags::Shift
            .complement()
            .intersection(NSEventModifierFlags::empty()),
    );
    let done = h.finished.borrow().clone();
    c.check(
        done == vec![Outcome::Closed],
        format!("closing without saving reports Closed ({done:?})"),
    );
    h.finished.borrow_mut().clear();

    // Done copies and closes.
    let editor = h.open(&h.save_dir);
    editor.click_tool(Tool::Ellipse);
    drag(&editor, at(200.0, 100.0), at(300.0, 160.0));
    pasteboard.write(macos_sys::pasteboard::Write::Text("before"));
    editor.click(Button::Done);
    let copied = matches!(pasteboard.read(), Some(Content::Image(_)));
    let done = h.finished.borrow().clone();
    c.check(
        copied && done == vec![Outcome::Copied],
        format!("the Done button copies the picture and closes ({done:?})"),
    );
    h.finished.borrow_mut().clear();
    drop(editor);

    // The Save and Copy buttons do what the keys do.
    let editor = h.open(&h.save_dir);
    let before = fs::read_dir(&h.save_dir).map(|d| d.count()).unwrap_or(0);
    editor.click(Button::Save);
    c.check(
        fs::read_dir(&h.save_dir).map(|d| d.count()).unwrap_or(0) == before + 1,
        "the Save button saves a file",
    );
    pasteboard.write(macos_sys::pasteboard::Write::Text("again"));
    editor.click(Button::Copy);
    c.check(
        matches!(pasteboard.read(), Some(Content::Image(_))),
        "the Copy button copies",
    );
    editor.close();
    let done = h.finished.borrow().clone();
    c.check(
        matches!(done.as_slice(), [Outcome::Copied]),
        format!("the last of the two is what is reported ({done:?})"),
    );
    h.finished.borrow_mut().clear();
    drop(editor);

    // A picture that is not one opens nothing.
    let nothing = open_with(
        h.mtm,
        Request {
            png: b"not a png".to_vec(),
            scale: 2.0,
            title: String::new(),
            save_dir: h.save_dir.clone(),
            strings: Strings::english(),
            on_finish: h.on_finish(),
        },
        Options {
            placement: Placement::Offscreen,
            pasteboard: Pasteboard::named(h.pasteboard),
        },
    );
    c.check(
        !nothing.is_open() && h.finished.borrow().len() == 1,
        "bytes that are not a picture open no window and report at once",
    );
    h.finished.borrow_mut().clear();
    pasteboard.release();
}

fn main() {
    let mtm = MainThreadMarker::new().expect("main thread");
    let out = PathBuf::from(env::args().nth(1).unwrap_or_else(|| "/tmp/annotate".into()));
    fs::create_dir_all(&out).expect("output folder");
    NSApplication::sharedApplication(mtm)
        .setActivationPolicy(NSApplicationActivationPolicy::Accessory);
    let save_dir = out.join(format!("saved-{}", process::id()));
    let _ = fs::remove_dir_all(&save_dir);
    let harness = Harness {
        mtm,
        out,
        save_dir: save_dir.clone(),
        pasteboard: "figghost-annotate-demo",
        finished: Rc::default(),
        sentinel: Rc::new(()),
    };
    let mut checks = Checks {
        failures: 0,
        total: 0,
    };
    scene_events(&harness, &mut checks);
    scene_text(&harness, &mut checks);
    scene_blur(&harness, &mut checks);
    scene_crop_and_growth(&harness, &mut checks);
    scene_wide_gamut(&harness, &mut checks);
    scene_large(&harness, &mut checks);
    scene_output(&harness, &mut checks);
    checks.check(
        Rc::strong_count(&harness.sentinel) == 1,
        "every editor was freed once dropped: nothing holds a reference cycle",
    );
    let _ = fs::remove_dir_all(&save_dir);
    println!("{} checks, {} failed", checks.total, checks.failures);
    if checks.failures > 0 {
        process::exit(1);
    }
}
