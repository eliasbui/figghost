//! Draws window snapping over a pretend desktop into PNG files, without showing anything on screen: the row of
//! layouts, or the chip it shrinks to, is drawn from its own view, then composed over a backdrop with a menu bar, a
//! Dock, the window being dragged, the cutout on a notched screen, the cells of the chosen layout and, while one is
//! lit, the footprint where the window will land. One whole screen per step of the drag (`step-*.png`), then
//! close-ups of the shape at two pixels per point (`close-*.png`). The names mirror `app-ui/src/features/snap/text.rs`;
//! `app-ui/snap_check` draws the live panel with the real ones.
//! `cargo run -p macos-sys --example render_snap -- /tmp/snap`

use block2::RcBlock;
use core_window::{
    Rect,
    snap::Display,
    templates::{self, Template},
};
use macos_sys::{
    MainThreadMarker,
    snap_ui::{self, CellsOverlay, ChipSpec, ZonesPanel, ZonesSpec},
    style,
    window::Frame,
};
use objc2::runtime::Bool;
use objc2_app_kit::{
    NSAffineTransformNSAppKitAdditions, NSApplication, NSApplicationActivationPolicy, NSBezierPath,
    NSBitmapImageFileType, NSBitmapImageRep, NSColor, NSGradient, NSImage,
};
use objc2_foundation::{NSAffineTransform, NSDictionary, NSPoint, NSRect, NSSize};
use std::{env, fs, path::PathBuf};

const SCREEN: (f64, f64) = (1440.0, 900.0);
const MENU_BAR: f64 = 24.0;
const DOCK: f64 = 76.0;

/// The short names of `text.rs`, English then Vietnamese.
fn caption(id: &str, vietnamese: bool) -> String {
    let (en, vi) = match id {
        "full" => ("Full screen", "Tối đa"),
        "halves" => ("Halves", "Chia đôi"),
        "rows" => ("Rows", "Hai hàng"),
        "thirds" => ("Thirds", "Chia ba"),
        "quarters" => ("Quarters", "Bốn góc"),
        "main_left" => ("Wide left", "Rộng trái"),
        "main_right" => ("Wide right", "Rộng phải"),
        "left_stack" => ("1 + 2", "1 + 2"),
        "right_stack" => ("2 + 1", "2 + 1"),
        "grid_six" => ("Six", "Sáu ô"),
        _ => ("", ""),
    };
    if vietnamese { vi } else { en }.to_owned()
}

fn name(template: &Template, vietnamese: bool) -> String {
    template
        .name
        .clone()
        .unwrap_or_else(|| caption(&template.id, vietnamese))
}

fn parts(template: &Template) -> Vec<[f64; 4]> {
    template.cells.iter().map(|c| c.part()).collect()
}

/// What the shape shows: the row of layouts, or the chip of the one chosen when `compact`.
fn spec(display: &Display, scene: &Scene) -> (ZonesSpec, Rect) {
    let chosen = scene.chosen.map(|i| &scene.templates[i]);
    if let (true, Some(template)) = (scene.compact, chosen) {
        let chip = display.chip();
        let spec = ZonesSpec {
            label: "Chosen layout".into(),
            width: chip.w,
            height: chip.h,
            cells: Vec::new(),
            glyphs: Vec::new(),
            names: Vec::new(),
            chip: Some(ChipSpec {
                parts: parts(template),
                name: name(template, scene.vietnamese),
                top: display.inset,
            }),
        };
        return (spec, chip);
    }
    let panel = display.panel(scene.templates.len());
    let spec = ZonesSpec {
        label: "Snap layouts".into(),
        width: panel.w,
        height: panel.h,
        cells: display
            .cells(scene.templates.len())
            .into_iter()
            .map(|c| Frame {
                x: c.x - panel.x,
                y: c.y - panel.y,
                w: c.w,
                h: c.h,
            })
            .collect(),
        glyphs: scene.templates.iter().map(parts).collect(),
        names: scene
            .templates
            .iter()
            .map(|t| name(t, scene.vietnamese))
            .collect(),
        chip: None,
    };
    (spec, panel)
}

/// The pretend screen, with the usable area between the menu bar and the Dock.
fn display(notched: bool) -> Display {
    let frame = Rect::new(0.0, 0.0, SCREEN.0, SCREEN.1);
    Display {
        frame,
        area: Rect::new(0.0, MENU_BAR, SCREEN.0, SCREEN.1 - MENU_BAR - DOCK),
        cutout: if notched { 185.0 } else { 200.0 },
        inset: if notched { 38.0 } else { 10.0 },
    }
}

struct Scene {
    templates: Vec<Template>,
    /// The layout under the pointer in the row, the one whose cells show, the lit cell, and whether the row is a chip.
    hover: Option<usize>,
    chosen: Option<usize>,
    cell: Option<usize>,
    compact: bool,
    notched: bool,
    dark: bool,
    vietnamese: bool,
}

/// A rect in top-left coordinates of the pretend screen, in the bottom-left ones of drawing.
fn cocoa(r: Rect) -> NSRect {
    NSRect::new(
        NSPoint::new(r.x, SCREEN.1 - r.y - r.h),
        NSSize::new(r.w, r.h),
    )
}

fn color(r: f64, g: f64, b: f64, a: f64) -> objc2::rc::Retained<NSColor> {
    NSColor::colorWithSRGBRed_green_blue_alpha(r, g, b, a)
}

fn image(png: &[u8]) -> objc2::rc::Retained<NSImage> {
    let data = objc2_foundation::NSData::with_bytes(png);
    NSImage::initWithData(objc2::AnyThread::alloc(), &data).expect("image")
}

fn frame_of(r: Rect) -> Frame {
    Frame {
        x: r.x,
        y: r.y,
        w: r.w,
        h: r.h,
    }
}

/// Draws `region` of the scene (top-left coordinates of the screen) at `scale` pixels per point.
fn render(
    shape_png: &[u8],
    shape: Rect,
    cells_png: Option<&[u8]>,
    scene: &Scene,
    region: Rect,
    scale: f64,
) -> Vec<u8> {
    let d = display(scene.notched);
    let landing = scene
        .chosen
        .zip(scene.cell)
        .map(|(t, c)| d.place(scene.templates[t].cells[c]));
    let shape_image = image(shape_png);
    let cells_image = cells_png.map(image);
    let (dark, notched, cutout) = (scene.dark, scene.notched, d.cutout);
    let area = d.area;
    let size = NSSize::new(region.w * scale, region.h * scale);
    let draw = RcBlock::new(move |_: NSRect| {
        let place = NSAffineTransform::transform();
        place.scaleBy(scale);
        place.translateXBy_yBy(-region.x, -(SCREEN.1 - region.y - region.h));
        place.concat();
        // A desktop.
        let (a, b) = if dark {
            ((0.05, 0.08, 0.2), (0.3, 0.12, 0.42))
        } else {
            ((0.72, 0.8, 0.98), (0.98, 0.84, 0.9))
        };
        if let Some(g) = NSGradient::initWithStartingColor_endingColor(
            objc2::AnyThread::alloc(),
            &color(a.0, a.1, a.2, 1.0),
            &color(b.0, b.1, b.2, 1.0),
        ) {
            g.drawInRect_angle(
                NSRect::new(NSPoint::ZERO, NSSize::new(SCREEN.0, SCREEN.1)),
                -45.0,
            );
        }
        // Another app's window, left where it was.
        let other = cocoa(Rect::new(90.0, 420.0, 520.0, 330.0));
        style::fill_rounded(other, 12.0, &style::white(if dark { 0.16 } else { 0.9 }));
        // The window being dragged: its title bar under the pointer, just below the menu bar.
        let window = Rect::new(470.0, MENU_BAR + 6.0, 500.0, 330.0);
        style::fill_rounded(
            cocoa(window),
            12.0,
            &style::white(if dark { 0.22 } else { 0.97 }),
        );
        style::fill_rounded(
            cocoa(Rect::new(window.x, window.y, window.w, 34.0)),
            12.0,
            &style::black(if dark { 0.3 } else { 0.07 }),
        );
        // A menu bar along the top and a Dock at the bottom.
        style::white(if dark { 0.14 } else { 0.55 }).setFill();
        NSBezierPath::fillRect(cocoa(Rect::new(0.0, 0.0, SCREEN.0, MENU_BAR)));
        style::fill_rounded(
            cocoa(Rect::new(420.0, SCREEN.1 - DOCK + 8.0, 600.0, DOCK - 16.0)),
            18.0,
            &style::white(if dark { 0.2 } else { 0.45 }),
        );
        // The cells of the chosen layout, over every window; then the lit one, under the shape.
        if let Some(cells) = &cells_image {
            cells.drawInRect(cocoa(area));
        }
        if let Some(r) = landing {
            snap_ui::paint_footprint(cocoa(r));
        }
        // The hardware cutout.
        if notched {
            style::black(1.0).setFill();
            NSBezierPath::fillRect(cocoa(Rect::new(
                (SCREEN.0 - cutout) / 2.0,
                0.0,
                cutout,
                32.0,
            )));
        }
        // The shape hangs from the top edge.
        shape_image.drawInRect(cocoa(shape));
        Bool::YES
    });
    let image = NSImage::imageWithSize_flipped_drawingHandler(size, false, &draw);
    let tiff = image.TIFFRepresentation().expect("tiff");
    let rep = NSBitmapImageRep::imageRepWithData(&tiff).expect("bitmap");
    // SAFETY: an empty properties dictionary is valid for PNG output.
    unsafe {
        rep.representationUsingType_properties(NSBitmapImageFileType::PNG, &NSDictionary::new())
    }
    .expect("png")
    .to_vec()
}

/// How much of the pretend screen a picture holds.
#[derive(Clone, Copy)]
enum Shot {
    Screen,
    Close,
    Top,
}

fn main() {
    let mtm = MainThreadMarker::new().expect("main thread");
    NSApplication::sharedApplication(mtm)
        .setActivationPolicy(NSApplicationActivationPolicy::Accessory);
    let dir = PathBuf::from(env::args().nth(1).unwrap_or_else(|| "/tmp/snap".into()));
    fs::create_dir_all(&dir).expect("output directory");
    let row_view = ZonesPanel::new(mtm);
    let cells_view = CellsOverlay::new(mtm);
    let write = |file: String, scene: &Scene, shot: Shot| {
        let d = display(scene.notched);
        let (spec, shape) = spec(&d, scene);
        let png = row_view
            .snapshot_png(&spec, scene.hover, scene.chosen)
            .expect("shape png");
        let cells_png = scene.chosen.and_then(|t| {
            let cells: Vec<Frame> = scene.templates[t]
                .cells
                .iter()
                .map(|c| frame_of(d.place(*c)))
                .collect();
            cells_view.snapshot_png(frame_of(d.area), &cells)
        });
        let out = match shot {
            // The shape with a margin, cut from the top of the screen.
            Shot::Close => {
                let region = Rect::new(shape.x - 60.0, 0.0, shape.w + 120.0, shape.h + 50.0);
                render(&png, shape, cells_png.as_deref(), scene, region, 2.0)
            }
            // The top of the screen, wide: the shape and the cells under it.
            Shot::Top => {
                let region = Rect::new(0.0, 0.0, SCREEN.0, 520.0);
                render(&png, shape, cells_png.as_deref(), scene, region, 1.0)
            }
            Shot::Screen => render(&png, shape, cells_png.as_deref(), scene, d.frame, 1.0),
        };
        let path = dir.join(format!("{file}.png"));
        fs::write(&path, out).expect("write png");
        println!("{}", path.display());
    };
    let seven = templates::choose::<&str>(&[], &[]);
    let nine = templates::choose(
        &[
            "full",
            "halves",
            "rows",
            "thirds",
            "quarters",
            "main_left",
            "main_right",
            "left_stack",
            "right_stack",
        ],
        &[],
    );
    let scene = |templates: &[Template],
                 hover: Option<usize>,
                 chosen: Option<usize>,
                 cell: Option<usize>,
                 compact: bool,
                 notched: bool,
                 dark: bool,
                 vietnamese: bool| Scene {
        templates: templates.to_vec(),
        hover,
        chosen,
        cell,
        compact,
        notched,
        dark,
        vietnamese,
    };
    // The drag, step by step, on a notched screen: the row, a layout under the pointer with its cells on the screen,
    // then the pointer on the screen with the row shrunk to a chip and one cell lit.
    write("step-1-row".into(), &scene(&seven, None, None, None, false, true, false, false), Shot::Screen);
    write("step-2-hover-thirds".into(), &scene(&seven, Some(2), Some(2), None, false, true, false, false), Shot::Screen);
    write("step-3-cell".into(), &scene(&seven, None, Some(2), Some(1), true, true, false, false), Shot::Screen);
    write("step-3-cell-dark".into(), &scene(&seven, None, Some(3), Some(3), true, true, true, false), Shot::Screen);
    write("step-2-hover-quarters-dark".into(), &scene(&seven, Some(3), Some(3), None, false, true, true, false), Shot::Screen);
    write("step-2-hover-full".into(), &scene(&seven, Some(0), Some(0), None, false, true, false, false), Shot::Screen);
    let close_ups = [
        ("close-rest", scene(&seven, None, None, None, false, true, false, false)),
        ("close-hover-halves", scene(&seven, Some(1), Some(1), None, false, true, true, false)),
        ("close-outlined", scene(&seven, None, Some(2), None, false, true, true, false)),
        ("close-chip", scene(&seven, None, Some(2), Some(0), true, true, true, false)),
        ("close-chip-notchless", scene(&seven, None, Some(4), Some(0), true, false, false, false)),
        ("close-vietnamese", scene(&seven, Some(4), Some(4), None, false, true, true, true)),
        ("close-notchless", scene(&seven, Some(0), Some(0), None, false, false, false, false)),
        ("close-nine-two-rows", scene(&nine, Some(7), Some(7), None, false, true, true, false)),
        ("close-one", scene(&seven[..1], Some(0), Some(0), None, false, true, false, false)),
    ];
    for (file, scene) in close_ups {
        write(file.into(), &scene, Shot::Close);
    }
    // The pictures of the README: the row with a layout under the pointer, and the cells with the chip.
    write("readme-row".into(), &scene(&seven, None, Some(2), None, false, false, false, false), Shot::Close);
    write("readme-row-hover".into(), &scene(&seven, Some(2), Some(2), None, false, false, false, false), Shot::Close);
    write("readme-cells".into(), &scene(&seven, None, Some(2), Some(1), true, true, false, false), Shot::Top);
}
