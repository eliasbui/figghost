//! Draws the outline of the notch at regular steps of its opening and closing into one PNG film strip, without
//! showing anything on screen: the black drop that grows out of the closed notch and fills out into the open shape,
//! and the way back. The spring and the openness curves are the ones `notch.rs` uses.
//! `cargo run -p macos-sys --example render_drip -- /tmp/drip.png [width height]`

use macos_sys::silhouette::{Cmd, Outline, openness, smooth};
use objc2::AnyThread;
use objc2_app_kit::{
    NSBezierPath, NSBitmapImageFileType, NSBitmapImageRep, NSColor, NSGradient, NSGraphicsContext,
};
use objc2_foundation::{NSDictionary, NSPoint, NSRect, NSSize};
use std::{env, fs, process};

const CLOSED: NSSize = NSSize::new(200.0, 32.0);
const FLARE: f64 = 20.0;
const OPEN_REFERENCE: f64 = 150.0;
const COLUMNS: usize = 8;
const SCALE: f64 = 0.46;

/// A damped spring from 0 to 1: `period` seconds for one turn and a damping ratio `zeta`.
fn spring(t: f64, period: f64, zeta: f64) -> f64 {
    let w = std::f64::consts::TAU / period;
    let wd = w * (1.0 - zeta * zeta).sqrt();
    let decay = (-zeta * w * t).exp();
    1.0 - decay * ((wd * t).cos() + zeta * w / wd * (wd * t).sin())
}

/// A critically damped approach from 1 to 0 over about `seconds`, as in notch.rs.
fn relax(t: f64, seconds: f64) -> f64 {
    let w = std::f64::consts::TAU / seconds;
    (1.0 + w * t) * (-w * t).exp()
}

fn outline_at(progress: f64, pinch: f64, open: NSSize, radius_open: f64) -> Outline {
    let mix = |a: f64, b: f64| (a + (b - a) * progress).max(0.0);
    let size = NSSize::new(
        mix(CLOSED.width, open.width),
        mix(CLOSED.height, open.height),
    );
    let o = openness(size.height, CLOSED.height, OPEN_REFERENCE);
    Outline {
        width: size.width,
        height: size.height,
        radius: mix(10.0, radius_open),
        flare: FLARE * smooth(0.5, 1.0, o) * (1.0 - pinch.clamp(0.0, 1.0)),
        pinch,
        neck_half: CLOSED.width / 2.0,
    }
}

fn path_of(outline: &Outline, origin: NSPoint) -> objc2::rc::Retained<NSBezierPath> {
    let path = NSBezierPath::bezierPath();
    // The outline is y up with the bottom at 0; the cell is drawn with its top at `origin.y`, so flip y.
    let at = |x: f64, y: f64| {
        NSPoint::new(
            origin.x + x * SCALE,
            origin.y - (outline.height - y) * SCALE,
        )
    };
    for cmd in outline.commands(FLARE) {
        match cmd {
            Cmd::Move(x, y) => path.moveToPoint(at(x, y)),
            Cmd::Line(x, y) => path.lineToPoint(at(x, y)),
            Cmd::Cubic(a, b, c, d, x, y) => {
                path.curveToPoint_controlPoint1_controlPoint2(at(x, y), at(a, b), at(c, d))
            }
            Cmd::Close => path.closePath(),
        }
    }
    path
}

fn main() {
    let out = env::args().nth(1).unwrap_or_else(|| "/tmp/drip.png".into());
    let width: f64 = env::args()
        .nth(2)
        .and_then(|v| v.parse().ok())
        .unwrap_or(500.0);
    let height: f64 = env::args()
        .nth(3)
        .and_then(|v| v.parse().ok())
        .unwrap_or(440.0);
    let open = NSSize::new(width, height);
    // The opening spring of notch.rs (0.42 s, bounce 0.22) and the closing one (0.26 s, no bounce).
    let opening = |t: f64| spring(t, 0.42, 0.78);
    let closing = |t: f64| spring(t, 0.26, 0.999);
    let steps: Vec<(String, Outline)> = (0..16)
        .map(|i| {
            let t = f64::from(i) * 0.03;
            (
                format!("open {t:.2}s"),
                outline_at(opening(t), relax(t, 0.52), open, 28.0),
            )
        })
        .chain((0..16).map(|i| {
            let t = f64::from(i) * 0.02;
            (
                format!("close {t:.2}s"),
                outline_at(
                    1.0 - closing(t.max(1e-6)).min(1.0),
                    0.55 * (1.0 - relax(t, 0.30)),
                    open,
                    28.0,
                ),
            )
        }))
        .collect();
    let (cell_w, cell_h) = (
        (open.width + 2.0 * FLARE) * SCALE + 12.0,
        open.height * SCALE * 1.06 + 12.0,
    );
    let rows = steps.len().div_ceil(COLUMNS);
    let (w, h) = (cell_w * COLUMNS as f64, cell_h * rows as f64);
    // SAFETY: an 8-bit RGBA bitmap with no planes passed in; AppKit allocates its own buffer.
    let rep = unsafe {
        NSBitmapImageRep::initWithBitmapDataPlanes_pixelsWide_pixelsHigh_bitsPerSample_samplesPerPixel_hasAlpha_isPlanar_colorSpaceName_bytesPerRow_bitsPerPixel(
            NSBitmapImageRep::alloc(),
            std::ptr::null_mut(),
            w as isize,
            h as isize,
            8,
            4,
            true,
            false,
            objc2_app_kit::NSCalibratedRGBColorSpace,
            0,
            0,
        )
    }
    .expect("bitmap");
    let context = NSGraphicsContext::graphicsContextWithBitmapImageRep(&rep).expect("context");
    NSGraphicsContext::saveGraphicsState_class();
    NSGraphicsContext::setCurrentContext(Some(&context));
    let sky = NSGradient::initWithStartingColor_endingColor(
        NSGradient::alloc(),
        &NSColor::colorWithSRGBRed_green_blue_alpha(0.55, 0.76, 0.95, 1.0),
        &NSColor::colorWithSRGBRed_green_blue_alpha(0.93, 0.80, 0.62, 1.0),
    )
    .expect("gradient");
    sky.drawInRect_angle(NSRect::new(NSPoint::ZERO, NSSize::new(w, h)), 90.0);
    NSColor::blackColor().setFill();
    for (i, (_label, outline)) in steps.iter().enumerate() {
        let (col, row) = (i % COLUMNS, i / COLUMNS);
        // Cell origin: its top, centred; drawing is y up so the top of row 0 is at the top of the image.
        let top = h - row as f64 * cell_h - 6.0;
        let origin = NSPoint::new(col as f64 * cell_w + 6.0, top);
        let path = path_of(outline, origin);
        path.fill();
    }
    NSGraphicsContext::restoreGraphicsState_class();
    // SAFETY: an empty properties dictionary is valid for PNG output.
    let data = unsafe {
        rep.representationUsingType_properties(NSBitmapImageFileType::PNG, &NSDictionary::new())
    }
    .expect("png");
    if let Err(error) = fs::write(&out, data.to_vec()) {
        eprintln!("{out}: {error}");
        process::exit(1);
    }
    println!("wrote {out}");
}
