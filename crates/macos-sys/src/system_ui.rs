//! The System tab of the notch: the processor core by core, the memory and the swap, the network, and a row of
//! smaller figures (graphics, startup disk, heat, battery). One view draws all of it from a `SystemDisplay` that
//! the app builds once a second while the tab shows, so nothing here reads the system or knows a language.

use crate::{
    notch::Page,
    style::{self, Tone, Weight, white},
    widgets::{Fit, draw_page_note, draw_text, text_attributes},
};
use objc2::{DefinedClass, MainThreadMarker, MainThreadOnly, define_class, msg_send, rc::Retained};
use objc2_app_kit::{NSBezierPath, NSColor, NSStringDrawing, NSView};
use objc2_foundation::{NSCopying, NSPoint, NSRect, NSSize, NSString};
use std::{cell::RefCell, rc::Rc};

/// The size of the area the tab fills below the header.
pub const PAGE_SIZE: NSSize = NSSize::new(580.0, 236.0);
/// How many readings a sparkline holds: a minute at one reading a second.
pub const HISTORY: usize = 60;

const MARGIN: f64 = 18.0;
const GAP: f64 = 10.0;
const TOP_ROW: f64 = 124.0;
const LOW_ROW: f64 = 62.0;
const PAD: f64 = 12.0;
const RADIUS: f64 = 14.0;

/// The processor: a figure for the whole and a bar for each core.
#[derive(Debug, Clone)]
pub struct CpuDisplay {
    pub title: String,
    pub value: String,
    /// Each core's load, 0 to 1. On a chip with two kinds of core the first `slow` are the frugal ones.
    pub cores: Vec<f64>,
    pub slow: usize,
    pub slow_label: String,
    pub fast_label: String,
}

#[derive(Debug, Clone)]
pub struct MemoryDisplay {
    pub title: String,
    pub used: String,
    pub of: String,
    /// What apps hold, what is wired and what is compressed, as shares of all the memory.
    pub shares: [f64; 3],
    pub legend: [String; 3],
    pub pressure: String,
    pub pressure_tone: Tone,
    pub swap_title: String,
    pub swap: String,
}

#[derive(Debug, Clone)]
pub struct NetworkDisplay {
    pub title: String,
    pub down: String,
    pub up: String,
    /// Recent readings, 0 to 1, the newest last.
    pub down_history: Vec<f64>,
    pub up_history: Vec<f64>,
}

/// A small card: a name, a figure, a line under it, and how full a bar under that is.
#[derive(Debug, Clone)]
pub struct Stat {
    pub title: String,
    pub value: String,
    pub detail: String,
    pub fraction: Option<f64>,
    pub tone: Tone,
}

/// Words that stand in for the figures: the tab is off, or every card has been switched off.
#[derive(Debug, Clone)]
pub struct Note {
    pub title: String,
    pub hint: String,
}

/// What the tab shows. A card that is `None` is left out and the others share its room; with a note, only the
/// note shows.
#[derive(Debug, Clone, Default)]
pub struct SystemDisplay {
    pub cpu: Option<CpuDisplay>,
    pub memory: Option<MemoryDisplay>,
    pub network: Option<NetworkDisplay>,
    pub stats: Vec<Stat>,
    pub note: Option<Note>,
}

fn attrs(
    size: f64,
    weight: Weight,
    alpha: f64,
) -> Retained<objc2_foundation::NSMutableDictionary<NSString, objc2::runtime::AnyObject>> {
    text_attributes(&style::font(size, weight), &white(alpha), Fit::Tail, false)
}

fn digit_attrs(
    size: f64,
    weight: Weight,
    alpha: f64,
) -> Retained<objc2_foundation::NSMutableDictionary<NSString, objc2::runtime::AnyObject>> {
    text_attributes(
        &style::digits(size, weight),
        &white(alpha),
        Fit::Tail,
        false,
    )
}

/// Text with its top edge `line` points below `top`, in a view whose origin is at the bottom.
fn put(
    text: &str,
    x: f64,
    top: f64,
    height: f64,
    width: f64,
    attributes: &objc2_foundation::NSMutableDictionary<NSString, objc2::runtime::AnyObject>,
    view_height: f64,
) {
    draw_text(
        text,
        NSRect::new(
            NSPoint::new(x, view_height - top - height),
            NSSize::new(width, height),
        ),
        attributes,
    );
}

fn width_of(
    text: &str,
    attributes: &objc2_foundation::NSMutableDictionary<NSString, objc2::runtime::AnyObject>,
) -> f64 {
    // SAFETY: the attributes dictionary holds valid attribute objects.
    unsafe { NSString::from_str(text).sizeWithAttributes(Some(attributes)) }.width
}

fn tone_of_load(load: f64) -> Tone {
    if load >= 0.9 {
        Tone::Red
    } else if load >= 0.7 {
        Tone::Orange
    } else {
        Tone::Blue
    }
}

/// A horizontal bar: a recessed track and the share of it that is full.
fn bar(rect: NSRect, fraction: f64, color: &NSColor) {
    style::fill_capsule(rect, &white(0.10));
    let fraction = fraction.clamp(0.0, 1.0);
    if fraction > 0.0 {
        let full = (rect.size.width * fraction).max(rect.size.height);
        style::fill_capsule(
            NSRect::new(rect.origin, NSSize::new(full, rect.size.height)),
            color,
        );
    }
}

/// A line over the last readings with the area under it tinted. The newest reading is at the right edge.
fn sparkline(rect: NSRect, history: &[f64], tone: Tone, area: bool) {
    if history.len() < 2 {
        return;
    }
    let step = rect.size.width / (HISTORY - 1) as f64;
    let right = rect.origin.x + rect.size.width;
    let point = |i: usize, v: f64| {
        NSPoint::new(
            right - (history.len() - 1 - i) as f64 * step,
            rect.origin.y + v.clamp(0.0, 1.0) * rect.size.height,
        )
    };
    let line = NSBezierPath::bezierPath();
    line.moveToPoint(point(0, history[0]));
    for (i, v) in history.iter().enumerate().skip(1) {
        line.lineToPoint(point(i, *v));
    }
    if area {
        let shape = line.copy();
        shape.lineToPoint(NSPoint::new(right, rect.origin.y));
        shape.lineToPoint(NSPoint::new(point(0, 0.0).x, rect.origin.y));
        shape.closePath();
        tone.color().colorWithAlphaComponent(0.20).setFill();
        shape.fill();
    }
    tone.color().setStroke();
    line.setLineWidth(1.4);
    line.stroke();
}

fn title(text: &str, x: f64, top: f64, view_height: f64) {
    put(
        &text.to_uppercase(),
        x,
        top,
        12.0,
        150.0,
        &attrs(9.5, Weight::Semibold, style::TEXT_TERTIARY),
        view_height,
    );
}

fn draw_cpu(cpu: &CpuDisplay, card: NSRect, view_height: f64) {
    style::card(card, RADIUS);
    let (x, top) = (
        card.origin.x + PAD,
        view_height - card.origin.y - card.size.height,
    );
    title(&cpu.title, x, top + 10.0, view_height);
    put(
        &cpu.value,
        x,
        top + 22.0,
        34.0,
        card.size.width - 2.0 * PAD,
        &digit_attrs(28.0, Weight::Light, style::TEXT),
        view_height,
    );
    // The bars of the cores sit along the foot of the card, the frugal ones first, a wider gap between the kinds.
    let n = cpu.cores.len().max(1);
    let kinds_gap = if cpu.slow > 0 && cpu.slow < n {
        8.0
    } else {
        0.0
    };
    let space = card.size.width - 2.0 * PAD - kinds_gap;
    let bar_gap = 3.0;
    let bar_w = ((space - bar_gap * (n as f64 - 1.0)) / n as f64).clamp(2.0, 14.0);
    let used = bar_w * n as f64 + bar_gap * (n as f64 - 1.0) + kinds_gap;
    let height = 40.0;
    let base = card.origin.y + 24.0;
    let mut cursor = x + (card.size.width - 2.0 * PAD - used) / 2.0;
    for (i, load) in cpu.cores.iter().enumerate() {
        if kinds_gap > 0.0 && i == cpu.slow {
            cursor += kinds_gap;
        }
        let track = NSRect::new(NSPoint::new(cursor, base), NSSize::new(bar_w, height));
        style::fill_rounded(track, bar_w / 2.0, &white(0.09));
        let load = load.clamp(0.0, 1.0);
        let fill = (height * load).max(bar_w.min(4.0));
        let rect = NSRect::new(NSPoint::new(cursor, base), NSSize::new(bar_w, fill));
        style::fill_rounded(rect, bar_w / 2.0, &tone_of_load(load).color());
        cursor += bar_w + bar_gap;
    }
    if kinds_gap > 0.0 {
        let small = attrs(8.5, Weight::Medium, style::TEXT_TERTIARY);
        let slow_x = x + (card.size.width - 2.0 * PAD - used) / 2.0;
        let slow_w = bar_w * cpu.slow as f64 + bar_gap * (cpu.slow as f64 - 1.0);
        let fast_x = slow_x + slow_w + bar_gap + kinds_gap;
        let fast_w = used - slow_w - bar_gap - kinds_gap;
        let centred =
            |label: &str, left: f64, span: f64| left + (span - width_of(label, &small)) / 2.0;
        put(
            &cpu.slow_label,
            centred(&cpu.slow_label, slow_x, slow_w),
            view_height - card.origin.y - 20.0,
            11.0,
            60.0,
            &small,
            view_height,
        );
        put(
            &cpu.fast_label,
            centred(&cpu.fast_label, fast_x, fast_w),
            view_height - card.origin.y - 20.0,
            11.0,
            60.0,
            &small,
            view_height,
        );
    }
}

fn draw_memory(memory: &MemoryDisplay, card: NSRect, view_height: f64) {
    style::card(card, RADIUS);
    let (x, top) = (
        card.origin.x + PAD,
        view_height - card.origin.y - card.size.height,
    );
    let inner = card.size.width - 2.0 * PAD;
    title(&memory.title, x, top + 10.0, view_height);
    // The pressure, at the right of the title, with a dot in the colour of how hard it is.
    let pressure_attrs = attrs(9.5, Weight::Medium, style::TEXT_SECONDARY);
    let pressure_w = width_of(&memory.pressure, &pressure_attrs);
    let right = card.origin.x + card.size.width - PAD;
    put(
        &memory.pressure,
        right - pressure_w,
        top + 10.0,
        12.0,
        pressure_w + 4.0,
        &pressure_attrs,
        view_height,
    );
    style::fill_circle(
        NSRect::new(
            NSPoint::new(right - pressure_w - 11.0, view_height - top - 10.0 - 10.5),
            NSSize::new(6.0, 6.0),
        ),
        &memory.pressure_tone.color(),
    );
    let big = digit_attrs(28.0, Weight::Light, style::TEXT);
    put(&memory.used, x, top + 22.0, 34.0, inner, &big, view_height);
    let of_x = x + width_of(&memory.used, &big) + 5.0;
    put(
        &memory.of,
        of_x,
        top + 38.0,
        14.0,
        (inner - (of_x - x)).max(0.0),
        &attrs(11.0, Weight::Regular, style::TEXT_SECONDARY),
        view_height,
    );
    // One bar made of what apps hold, what is wired and what is compressed.
    let track = NSRect::new(
        NSPoint::new(x, view_height - top - 68.0 - 8.0),
        NSSize::new(inner, 8.0),
    );
    style::fill_capsule(track, &white(0.10));
    let colors = [Tone::Blue, Tone::Orange, Tone::Purple];
    let mut left = track.origin.x;
    let sum: f64 = memory.shares.iter().sum();
    let scale = if sum > 1.0 { 1.0 / sum } else { 1.0 };
    for (share, tone) in memory.shares.iter().zip(colors) {
        let span = (inner * share * scale).max(0.0);
        if span >= 1.0 {
            tone.color().setFill();
            NSBezierPath::bezierPathWithRect(NSRect::new(
                NSPoint::new(left, track.origin.y),
                NSSize::new(span, track.size.height),
            ))
            .fill();
            left += span;
        }
    }
    // Under the bar: what each part is, and the swap at the right.
    let small = attrs(10.0, Weight::Regular, style::TEXT_SECONDARY);
    for (i, (label, tone)) in memory.legend.iter().zip(colors).enumerate() {
        let row = top + 84.0 + i as f64 * 12.5;
        style::fill_circle(
            NSRect::new(
                NSPoint::new(x, view_height - row - 9.0),
                NSSize::new(5.0, 5.0),
            ),
            &tone.color(),
        );
        put(
            label,
            x + 10.0,
            row,
            12.0,
            inner * 0.66,
            &small,
            view_height,
        );
    }
    let swap_x = x + inner * 0.68;
    put(
        &memory.swap_title,
        swap_x,
        top + 84.0,
        12.0,
        inner * 0.32,
        &attrs(9.0, Weight::Medium, style::TEXT_TERTIARY),
        view_height,
    );
    put(
        &memory.swap,
        swap_x,
        top + 96.0,
        14.0,
        inner * 0.32,
        &digit_attrs(11.0, Weight::Medium, style::TEXT),
        view_height,
    );
}

fn draw_network(network: &NetworkDisplay, card: NSRect, view_height: f64) {
    style::card(card, RADIUS);
    let (x, top) = (
        card.origin.x + PAD,
        view_height - card.origin.y - card.size.height,
    );
    let inner = card.size.width - 2.0 * PAD;
    title(&network.title, x, top + 10.0, view_height);
    let value = digit_attrs(15.0, Weight::Medium, style::TEXT);
    for (i, (symbol, tone, text)) in [
        ("arrow.down", Tone::Blue, &network.down),
        ("arrow.up", Tone::Green, &network.up),
    ]
    .into_iter()
    .enumerate()
    {
        let row = top + 26.0 + i as f64 * 22.0;
        style::draw_symbol(
            symbol,
            NSPoint::new(x + 6.0, view_height - row - 10.0),
            11.0,
            &tone.color(),
        );
        put(text, x + 18.0, row, 20.0, inner - 18.0, &value, view_height);
    }
    let area = NSRect::new(
        NSPoint::new(x, card.origin.y + 12.0),
        NSSize::new(inner, 36.0),
    );
    style::fill_rounded(area, 6.0, &white(0.04));
    sparkline(area, &network.down_history, Tone::Blue, true);
    sparkline(area, &network.up_history, Tone::Green, false);
}

fn draw_stat(stat: &Stat, card: NSRect, view_height: f64) {
    style::card(card, RADIUS - 2.0);
    let (x, top) = (
        card.origin.x + PAD,
        view_height - card.origin.y - card.size.height,
    );
    let inner = card.size.width - 2.0 * PAD;
    title(&stat.title, x, top + 9.0, view_height);
    // A card without a bar says how it is going with a dot in front of its figure.
    let dot = if stat.fraction.is_none() { 14.0 } else { 0.0 };
    if dot > 0.0 {
        style::fill_circle(
            NSRect::new(
                NSPoint::new(x, view_height - top - 21.0 - 14.0),
                NSSize::new(8.0, 8.0),
            ),
            &stat.tone.color(),
        );
    }
    put(
        &stat.value,
        x + dot,
        top + 21.0,
        22.0,
        inner - dot,
        &digit_attrs(16.0, Weight::Medium, style::TEXT),
        view_height,
    );
    if let Some(fraction) = stat.fraction {
        bar(
            NSRect::new(
                NSPoint::new(x, card.origin.y + 18.0),
                NSSize::new(inner, 5.0),
            ),
            fraction,
            &stat.tone.color(),
        );
    }
    put(
        &stat.detail,
        x,
        top + 43.0,
        12.0,
        inner,
        &attrs(9.5, Weight::Regular, style::TEXT_SECONDARY),
        view_height,
    );
}

/// One of the big cards of the top row.
enum Big<'a> {
    Cpu(&'a CpuDisplay),
    Memory(&'a MemoryDisplay),
    Network(&'a NetworkDisplay),
}

fn draw(bounds: NSRect, display: &SystemDisplay) {
    let (w, h) = (bounds.size.width, bounds.size.height);
    if let Some(note) = &display.note {
        draw_page_note("cpu", &note.title, &note.hint, bounds.size);
        return;
    }
    let big: Vec<Big> = display
        .cpu
        .iter()
        .map(Big::Cpu)
        .chain(display.memory.iter().map(Big::Memory))
        .chain(display.network.iter().map(Big::Network))
        .collect();
    // The rows go down from the top, and a row that is switched off leaves no gap.
    let mut y = h - 6.0;
    if !big.is_empty() {
        let n = big.len() as f64;
        let card_w = (w - 2.0 * MARGIN - (n - 1.0) * GAP) / n;
        y -= TOP_ROW;
        for (i, card) in big.iter().enumerate() {
            let rect = NSRect::new(
                NSPoint::new(MARGIN + i as f64 * (card_w + GAP), y),
                NSSize::new(card_w, TOP_ROW),
            );
            match card {
                Big::Cpu(c) => draw_cpu(c, rect, h),
                Big::Memory(m) => draw_memory(m, rect, h),
                Big::Network(n) => draw_network(n, rect, h),
            }
        }
        y -= GAP;
    }
    if !display.stats.is_empty() {
        let count = display.stats.len() as f64;
        let stat_w = (w - 2.0 * MARGIN - (count - 1.0) * GAP) / count;
        y -= LOW_ROW;
        for (i, stat) in display.stats.iter().enumerate() {
            let card = NSRect::new(
                NSPoint::new(MARGIN + i as f64 * (stat_w + GAP), y),
                NSSize::new(stat_w, LOW_ROW),
            );
            draw_stat(stat, card, h);
        }
    }
}

pub struct CanvasIvars {
    display: RefCell<SystemDisplay>,
}

define_class!(
    // SAFETY: NSView allows subclassing; the ivars are only touched on the main thread and need no Drop.
    #[unsafe(super(NSView))]
    #[thread_kind = MainThreadOnly]
    #[name = "FigSystemCanvas"]
    #[ivars = CanvasIvars]
    pub struct SystemCanvas;

    impl SystemCanvas {
        #[unsafe(method(drawRect:))]
        fn draw_rect(&self, _dirty: NSRect) {
            draw(self.bounds(), &self.ivars().display.borrow());
        }
    }
);

impl SystemCanvas {
    fn new(mtm: MainThreadMarker, frame: NSRect) -> Retained<Self> {
        let this = mtm.alloc::<Self>().set_ivars(CanvasIvars {
            display: RefCell::default(),
        });
        // SAFETY: designated initializer with valid arguments on the main thread.
        unsafe { msg_send![super(this), initWithFrame: frame] }
    }
}

pub struct SystemView {
    canvas: Retained<SystemCanvas>,
}

impl SystemView {
    /// `frame` is `Notch::page_area(PAGE_SIZE.width, PAGE_SIZE.height)`.
    pub fn new(mtm: MainThreadMarker, frame: NSRect) -> Rc<Self> {
        Rc::new(Self {
            canvas: SystemCanvas::new(mtm, frame),
        })
    }

    /// Draws what the page is now to show.
    pub fn show(&self, display: SystemDisplay) {
        *self.canvas.ivars().display.borrow_mut() = display;
        self.canvas.setNeedsDisplay(true);
    }

    /// The notch tab for this view.
    pub fn page(
        self: &Rc<Self>,
        title: String,
        on_show: impl Fn() + 'static,
        on_hide: impl Fn() + 'static,
    ) -> Page {
        Page {
            id: "system",
            title,
            symbol: "cpu",
            root: Retained::into_super(Retained::clone(&self.canvas)),
            on_show: Box::new(on_show),
            on_hide: Box::new(on_hide),
            on_key: Box::new(|_| false),
            accepts_files: false,
            size: None,
        }
    }
}
