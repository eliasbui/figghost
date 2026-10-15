//! Renders an annotated sample picture without opening a window, and the editor's layout, to PNGs.
//!
//!     cargo run -p macos-sys --example editor_check -- /tmp/figghost-editor

use core_capture::{Background, Document, Mark, PALETTE, Point, Rect, Shape};
use macos_sys::{
    editor_ui::{self, EditorActions, EditorTexts, EditorWindow},
    screen_capture::Shot,
};
use objc2::{AnyThread, MainThreadMarker};
use objc2_app_kit::{
    NSApplication, NSBezierPath, NSBitmapImageRep, NSColor, NSDeviceRGBColorSpace, NSFont,
    NSGraphicsContext, NSStringDrawing,
};
use objc2_foundation::{NSDictionary, NSPoint, NSRect, NSSize, NSString};
use std::{env, fs, path::PathBuf};

/// A 600 x 360 point sample at 2x: a white page with some text on a blue band.
fn sample() -> Shot {
    // SAFETY: a plain RGBA bitmap with valid dimensions.
    let rep = unsafe {
        NSBitmapImageRep::initWithBitmapDataPlanes_pixelsWide_pixelsHigh_bitsPerSample_samplesPerPixel_hasAlpha_isPlanar_colorSpaceName_bytesPerRow_bitsPerPixel(
            NSBitmapImageRep::alloc(), std::ptr::null_mut(), 1200, 720, 8, 4, true, false, NSDeviceRGBColorSpace, 0, 0,
        )
    }
    .unwrap();
    rep.setSize(NSSize::new(600.0, 360.0));
    let context = NSGraphicsContext::graphicsContextWithBitmapImageRep(&rep).unwrap();
    NSGraphicsContext::saveGraphicsState_class();
    NSGraphicsContext::setCurrentContext(Some(&context));
    NSColor::whiteColor().setFill();
    NSBezierPath::fillRect(NSRect::new(NSPoint::ZERO, NSSize::new(600.0, 360.0)));
    NSColor::systemBlueColor().setFill();
    NSBezierPath::fillRect(NSRect::new(
        NSPoint::new(0.0, 300.0),
        NSSize::new(600.0, 60.0),
    ));
    let font = NSFont::systemFontOfSize(22.0);
    let attrs = NSDictionary::from_retained_objects(
        &[unsafe { objc2_app_kit::NSFontAttributeName }],
        &[objc2::rc::Retained::into_super(
            objc2::rc::Retained::into_super(font),
        )],
    );
    for (i, line) in [
        "Mật khẩu: hunter2",
        "Số tài khoản 0123 4567 89",
        "Bấm nút Lưu để hoàn tất",
    ]
    .iter()
    .enumerate()
    {
        // SAFETY: drawing into the current bitmap context with valid attributes.
        unsafe {
            NSString::from_str(line).drawAtPoint_withAttributes(
                NSPoint::new(40.0, 230.0 - i as f64 * 60.0),
                Some(&attrs),
            )
        };
    }
    NSGraphicsContext::restoreGraphicsState_class();
    Shot::from_bitmap(&rep, 2.0).unwrap()
}

fn main() {
    let mtm = MainThreadMarker::new().expect("main thread");
    let _app = NSApplication::sharedApplication(mtm);
    let dir = PathBuf::from(
        env::args()
            .nth(1)
            .unwrap_or_else(|| "/tmp/figghost-editor".into()),
    );
    fs::create_dir_all(&dir).unwrap();
    let shot = sample();
    let mut doc = Document::new(600.0, 360.0);
    let mark = |shape, color: usize, width: f64| Mark {
        shape,
        color: PALETTE[color],
        width,
    };
    doc.add(mark(
        Shape::Pixelate(Rect::new(30.0, 225.0, 230.0, 34.0)),
        0,
        4.0,
    ));
    doc.add(mark(
        Shape::Highlight(Rect::new(30.0, 165.0, 300.0, 34.0)),
        2,
        4.0,
    ));
    doc.add(mark(
        Shape::Arrow {
            from: Point::new(470.0, 60.0),
            to: Point::new(330.0, 115.0),
        },
        0,
        4.0,
    ));
    doc.add(mark(
        Shape::Rectangle(Rect::new(30.0, 100.0, 300.0, 40.0)),
        4,
        4.0,
    ));
    doc.add(mark(
        Shape::Ellipse(Rect::new(400.0, 150.0, 150.0, 90.0)),
        3,
        6.0,
    ));
    let size = 22.0;
    doc.add(mark(
        Shape::Text {
            at: Point::new(360.0, 30.0),
            text: "Bấm vào đây".into(),
            size,
            bounds: Rect::new(360.0, 30.0, 140.0, 28.0),
        },
        0,
        4.0,
    ));
    for (i, x) in [60.0, 120.0, 180.0].iter().enumerate() {
        doc.add(mark(
            Shape::Counter {
                center: Point::new(*x, 40.0),
                number: i as u32 + 1,
            },
            4,
            2.0,
        ));
    }
    doc.set_background(Some(Background::default()));
    let rendered = editor_ui::render_document(&shot, doc).expect("render");
    let path = dir.join("annotated.png");
    fs::write(&path, rendered.png().unwrap()).unwrap();
    println!(
        "annotated: {}x{} px -> {}",
        rendered.width(),
        rendered.height(),
        path.display()
    );
    let expected = ((600.0 + 96.0) * 2.0, (360.0 + 96.0) * 2.0);
    let ok = (rendered.width() as f64, rendered.height() as f64) == expected;

    let tools = [
        "Chọn",
        "Mũi tên",
        "Đường",
        "Khung",
        "Elip",
        "Tô sáng",
        "Chữ",
        "Số",
        "Che",
        "Cắt",
    ];
    let texts = EditorTexts {
        title: "Sửa ảnh chụp".into(),
        tools: tools.iter().map(|t| (*t).to_owned()).collect(),
        undo: "Hoàn tác".into(),
        redo: "Làm lại".into(),
        background: "Nền".into(),
        copy: "Sao chép".into(),
        save: "Lưu...".into(),
        pin: "Ghim".into(),
        done: "Xong".into(),
    };
    let none = || Box::new(|_: Shot| {});
    let window = EditorWindow::hidden(
        mtm,
        &shot,
        &texts,
        EditorActions {
            on_copy: none(),
            on_save: none(),
            on_pin: none(),
            on_done: none(),
            on_closed: Box::new(|| {}),
        },
    );
    let layout = dir.join("editor.png");
    fs::write(&layout, window.snapshot_png().unwrap()).unwrap();
    println!("editor layout -> {}", layout.display());
    println!("{}", if ok { "OK" } else { "FAIL: unexpected size" });
}
