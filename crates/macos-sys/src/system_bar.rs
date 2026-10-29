//! The figures of the System tab that stay in the menu bar, drawn as one picture of cells: a label and a value on
//! one line, a small label over the value, what is sent over what is received, a line of the last readings, or a
//! bar for each core. A cell is as wide as the widest value it can show, so a figure that changes every second moves
//! nothing beside it. Plain, the picture is a template the menu bar paints for a light or a dark bar; in colour the
//! lines and bars take the accent (red past `HOT`) and the words the colour of the menu bar's text.

use crate::{
    style::{self, Tone, Weight},
    widgets::{Fit, draw_text, text_attributes},
};
use block2::RcBlock;
use objc2::{rc::Retained, runtime::Bool};
use objc2_app_kit::{NSBezierPath, NSColor, NSFont, NSImage, NSStringDrawing};
use objc2_foundation::{NSPoint, NSRect, NSSize, NSString};

/// The height of the picture: the menu bar's own.
pub const HEIGHT: f64 = 22.0;
/// How many readings a graph cell holds; fewer are drawn from the right.
pub const GRAPH_POINTS: usize = 30;
/// A core busier than this is drawn red when the cells are in colour.
pub const HOT: f64 = 0.85;

const GAP: f64 = 6.0;
const CHART: f64 = 16.0;
const GRAPH_WIDTH: f64 = 30.0;
// Whole points, so that each bar is two pixels wide with one between on a Retina screen.
const BAR: f64 = 2.0;
const BAR_GAP: f64 = 1.0;
const GROUP_GAP: f64 = 3.0;
const ARROW: f64 = 6.0;
const LABEL_SIZE: f64 = 8.0;
const VALUE_SIZE: f64 = 11.0;
const RATE_SIZE: f64 = 9.5;

#[derive(Debug, Clone, PartialEq)]
pub enum Cell {
    /// `CPU 23%` on one line, in the menu bar's own type.
    Text {
        label: String,
        value: String,
        widest: String,
    },
    /// A small label over the value.
    Stacked {
        label: String,
        value: String,
        widest: String,
    },
    /// What is sent over what is received, each after its arrow.
    Rates {
        up: String,
        down: String,
        widest: String,
    },
    /// The last readings, 0 to 1, oldest first.
    Graph { history: Vec<f64> },
    /// Each core's load, 0 to 1; the first `slow` are the frugal ones, with a gap after them.
    Cores { loads: Vec<f64>, slow: usize },
}

/// What the cells are drawn in.
struct Ink {
    text: Retained<NSColor>,
    label: Retained<NSColor>,
    line: Retained<NSColor>,
    area: Retained<NSColor>,
    frugal: Retained<NSColor>,
    hot: Retained<NSColor>,
    track: Retained<NSColor>,
}

impl Ink {
    fn new(color: bool) -> Self {
        if color {
            // Read while the picture is drawn, so the words take the menu bar's own colour for a light or a dark bar.
            let text = NSColor::labelColor();
            let accent = NSColor::controlAccentColor();
            Self {
                label: text.colorWithAlphaComponent(0.8),
                track: text.colorWithAlphaComponent(0.18),
                area: accent.colorWithAlphaComponent(0.35),
                frugal: accent.colorWithAlphaComponent(0.6),
                hot: Tone::Red.color(),
                line: accent,
                text,
            }
        } else {
            // A template keeps only how opaque each point is.
            Self {
                text: style::black(1.0),
                label: style::black(0.8),
                line: style::black(1.0),
                area: style::black(0.3),
                frugal: style::black(1.0),
                hot: style::black(1.0),
                track: style::black(0.18),
            }
        }
    }
}

fn text_font() -> Retained<NSFont> {
    style::digits(NSFont::menuBarFontOfSize(0.0).pointSize(), Weight::Regular)
}

fn width_of(text: &str, font: &NSFont) -> f64 {
    let attributes = text_attributes(font, &style::black(1.0), Fit::Clip, false);
    // SAFETY: the attributes dictionary holds valid attribute objects.
    unsafe { NSString::from_str(text).sizeWithAttributes(Some(&attributes)) }.width
}

/// Text with its top `top` points down the picture and its right edge at `right`, or centred on `centre`.
fn put(text: &str, font: &NSFont, color: &NSColor, top: f64, x: Place) {
    let attributes = text_attributes(font, color, Fit::Clip, false);
    // SAFETY: the attributes dictionary holds valid attribute objects.
    let size = unsafe { NSString::from_str(text).sizeWithAttributes(Some(&attributes)) };
    let left = match x {
        Place::Right(right) => right - size.width,
        Place::Centre(centre) => centre - size.width / 2.0,
    };
    draw_text(
        text,
        NSRect::new(NSPoint::new(left, top), size),
        &attributes,
    );
}

#[derive(Clone, Copy)]
enum Place {
    Right(f64),
    Centre(f64),
}

fn width(cell: &Cell) -> f64 {
    let w = match cell {
        Cell::Text { label, widest, .. } => width_of(&format!("{label} {widest}"), &text_font()),
        Cell::Stacked { label, widest, .. } => {
            width_of(label, &style::font(LABEL_SIZE, Weight::Semibold)).max(width_of(
                widest,
                &style::digits(VALUE_SIZE, Weight::Semibold),
            ))
        }
        Cell::Rates { widest, .. } => {
            ARROW + 2.0 + width_of(widest, &style::digits(RATE_SIZE, Weight::Semibold))
        }
        Cell::Graph { .. } => GRAPH_WIDTH,
        Cell::Cores { loads, slow } => {
            let n = loads.len() as f64;
            let split = if *slow > 0 && *slow < loads.len() {
                GROUP_GAP - BAR_GAP
            } else {
                0.0
            };
            (n * BAR + (n - 1.0) * BAR_GAP + split).max(BAR)
        }
    };
    w.ceil()
}

/// A small arrow, pointing up or down, in a box `ARROW` wide and as tall as a capital, whose top is `top`.
fn arrow(x: f64, top: f64, up: bool, color: &NSColor) {
    let half = 3.5;
    let (tip, base) = if up {
        (top, top + half)
    } else {
        (top + 2.0 * half, top + half)
    };
    let stem = if up {
        (top + half, top + 2.0 * half)
    } else {
        (top, top + half)
    };
    let path = NSBezierPath::bezierPath();
    path.moveToPoint(NSPoint::new(x + ARROW / 2.0, tip));
    path.lineToPoint(NSPoint::new(x + ARROW, base));
    path.lineToPoint(NSPoint::new(x, base));
    path.closePath();
    path.appendBezierPathWithRect(NSRect::new(
        NSPoint::new(x + ARROW / 2.0 - 1.0, stem.0),
        NSSize::new(2.0, stem.1 - stem.0),
    ));
    color.setFill();
    path.fill();
}

fn draw_cell(cell: &Cell, x: f64, w: f64, ink: &Ink) {
    let chart_top = (HEIGHT - CHART) / 2.0;
    match cell {
        Cell::Text { label, value, .. } => {
            let font = text_font();
            let top = (HEIGHT - (font.ascender() - font.descender())) / 2.0;
            put(
                &format!("{label} {value}"),
                &font,
                &ink.text,
                top,
                Place::Right(x + w),
            );
        }
        Cell::Stacked { label, value, .. } => {
            let centre = x + w / 2.0;
            put(
                label,
                &style::font(LABEL_SIZE, Weight::Semibold),
                &ink.label,
                0.5,
                Place::Centre(centre),
            );
            put(
                value,
                &style::digits(VALUE_SIZE, Weight::Semibold),
                &ink.text,
                8.0,
                Place::Centre(centre),
            );
        }
        Cell::Rates { up, down, .. } => {
            let font = style::digits(RATE_SIZE, Weight::Semibold);
            for (text, top, is_up) in [(up, 0.5, true), (down, 10.5, false)] {
                arrow(x, top + 2.2, is_up, &ink.text);
                put(text, &font, &ink.text, top, Place::Right(x + w));
            }
        }
        Cell::Graph { history } => {
            let bottom = chart_top + CHART;
            let step = w / (GRAPH_POINTS - 1) as f64;
            let n = history.len().min(GRAPH_POINTS);
            let points: Vec<NSPoint> = history[history.len() - n..]
                .iter()
                .enumerate()
                .map(|(i, v)| {
                    NSPoint::new(
                        x + w - (n - 1 - i) as f64 * step,
                        bottom - v.clamp(0.0, 1.0) * CHART,
                    )
                })
                .collect();
            ink.track.setFill();
            NSBezierPath::fillRect(NSRect::new(
                NSPoint::new(x, bottom - 1.0),
                NSSize::new(w, 1.0),
            ));
            let (Some(first), Some(last)) = (points.first(), points.last()) else {
                return;
            };
            let through = || {
                let path = NSBezierPath::bezierPath();
                path.moveToPoint(*first);
                for p in &points[1..] {
                    path.lineToPoint(*p);
                }
                path
            };
            let area = through();
            area.lineToPoint(NSPoint::new(last.x, bottom));
            area.lineToPoint(NSPoint::new(first.x, bottom));
            area.closePath();
            let line = through();
            ink.area.setFill();
            area.fill();
            line.setLineWidth(1.2);
            ink.line.setStroke();
            line.stroke();
        }
        Cell::Cores { loads, slow } => {
            let mut left = x;
            for (i, load) in loads.iter().enumerate() {
                if i > 0 && i == *slow {
                    left += GROUP_GAP - BAR_GAP;
                }
                let load = load.clamp(0.0, 1.0);
                style::fill_rounded(
                    NSRect::new(NSPoint::new(left, chart_top), NSSize::new(BAR, CHART)),
                    0.5,
                    &ink.track,
                );
                let height = (load * CHART).max(1.0);
                let color = if load > HOT {
                    &ink.hot
                } else if i < *slow {
                    &ink.frugal
                } else {
                    &ink.line
                };
                style::fill_rounded(
                    NSRect::new(
                        NSPoint::new(left, chart_top + CHART - height),
                        NSSize::new(BAR, height),
                    ),
                    0.5,
                    color,
                );
                left += BAR + BAR_GAP;
            }
        }
    }
}

/// The cells side by side as the picture of one item of the menu bar. A template unless `color`.
pub fn image(cells: &[Cell], color: bool) -> Retained<NSImage> {
    let widths: Vec<f64> = cells.iter().map(width).collect();
    let total = widths.iter().sum::<f64>() + GAP * widths.len().saturating_sub(1) as f64;
    let cells = cells.to_vec();
    let draw = RcBlock::new(move |_: NSRect| {
        let ink = Ink::new(color);
        let mut x = 0.0;
        for (cell, w) in cells.iter().zip(&widths) {
            draw_cell(cell, x, *w, &ink);
            x += w + GAP;
        }
        Bool::YES
    });
    // Flipped, so that a top is measured down from the top as the menu bar is read.
    let image = NSImage::imageWithSize_flipped_drawingHandler(
        NSSize::new(total.max(1.0), HEIGHT),
        true,
        &draw,
    );
    image.setTemplate(!color);
    image
}

#[cfg(test)]
mod tests {
    use super::*;

    fn stacked(value: &str) -> Cell {
        Cell::Stacked {
            label: "CPU".into(),
            value: value.into(),
            widest: "100%".into(),
        }
    }

    #[test]
    fn a_cell_is_as_wide_whatever_its_value_says() {
        assert_eq!(
            image(&[stacked("5%")], false).size(),
            image(&[stacked("100%")], false).size()
        );
        let rates = |down: &str| Cell::Rates {
            up: "0 B/s".into(),
            down: down.into(),
            widest: "88.8 MB/s".into(),
        };
        assert_eq!(
            image(&[rates("1 B/s")], false).size(),
            image(&[rates("48.2 MB/s")], false).size()
        );
        let graph = |n: usize| Cell::Graph {
            history: vec![0.5; n],
        };
        assert_eq!(
            image(&[graph(0)], false).size(),
            image(&[graph(90)], false).size()
        );
    }

    #[test]
    fn cells_stand_side_by_side_at_the_height_of_the_menu_bar() {
        let one = image(&[stacked("5%")], false).size();
        let cores = Cell::Cores {
            loads: vec![0.2; 10],
            slow: 4,
        };
        let two = image(&[stacked("5%"), cores.clone()], false).size();
        assert_eq!(one.height, HEIGHT);
        assert_eq!(two.height, HEIGHT);
        // Ten bars two points wide with one between, and two more between the frugal and the fast ones.
        assert_eq!(width(&cores), 31.0);
        assert_eq!(two.width, one.width + GAP + 31.0);
        assert_eq!(
            width(&Cell::Cores {
                loads: vec![0.2; 8],
                slow: 0
            }),
            23.0
        );
    }

    #[test]
    fn plain_cells_are_a_template_and_coloured_ones_are_not() {
        assert!(image(&[stacked("5%")], false).isTemplate());
        assert!(!image(&[stacked("5%")], true).isTemplate());
    }
}
