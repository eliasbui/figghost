//! The media page: the artwork (or a card in the app's colour), the current track, where it is in the track,
//! and previous / play-pause / next.

use crate::{
    app,
    button::{Look, TouchButton},
    labels,
    notch::Page,
    now_playing::{Artwork, Sample},
    panel::Key,
    style::{self, Tone, Weight, black, white},
    widgets::{PageRoot, ProgressBar},
};
use objc2::{
    AnyThread, DefinedClass, MainThreadMarker, MainThreadOnly, define_class, msg_send, rc::Retained,
};
use objc2_app_kit::{
    NSAppearance, NSAppearanceCustomization, NSAppearanceNameDarkAqua, NSAutoresizingMaskOptions,
    NSBezierPath, NSBitmapImageFileType, NSBorderType, NSBox, NSBoxType, NSColor, NSGradient,
    NSGraphicsContext, NSImage, NSImageInterpolation, NSLineBreakMode, NSScrollView,
    NSScrollerStyle, NSTextAlignment, NSTextField, NSTextView, NSView,
};
use objc2_foundation::{NSData, NSDictionary, NSObjectProtocol, NSPoint, NSRect, NSSize, NSString};
use std::{
    cell::{Cell, RefCell},
    collections::hash_map::DefaultHasher,
    hash::{Hash, Hasher},
    rc::Rc,
    time::{Duration, Instant},
};

const MARGIN: f64 = 24.0;
const ART: f64 = 116.0;
/// The height of the page without lyrics: the cover at the left, and the track, its bar and the buttons in a column
/// beside it. With lyrics the page takes the full height of the notch again.
const ROOMY_HEIGHT: f64 = 176.0;
const ROOMY_GAP: f64 = 22.0;
const ROOMY_SIDE: f64 = 40.0;
const ROOMY_MAIN: f64 = 52.0;
const ROOMY_BUTTON_GAP: f64 = 20.0;
const SIDE_BUTTON: f64 = 48.0;
const MAIN_BUTTON: f64 = 66.0;
const BUTTON_GAP: f64 = 30.0;
/// With lyrics the cover shrinks to the corner and the controls drop to the bottom edge to make room.
const ART_SMALL: f64 = 72.0;
const CONTROLS_Y_WITH_LYRICS: f64 = 58.0;
const LYRICS_BOTTOM: f64 = 100.0;
/// Leaves room for the progress row under the track's text.
const LYRICS_TOP_GAP: f64 = 116.0;
const LINE_HEIGHT: f64 = 20.0;
const CURRENT_HEIGHT: f64 = 56.0;
const LINE_GAP: f64 = 14.0;
/// How far the lines travel when the sung line changes, and for how long.
const SLIDE: f64 = 22.0;
const SLIDE_SECONDS: f64 = 0.32;
/// The row with the time played, the bar and the time left.
const TIME_WIDTH: f64 = 54.0;
const TIME_HEIGHT: f64 = 14.0;
/// The bar's view, which can be dragged: taller than the thin track it draws, so it is easy to hit.
const BAR_HEIGHT: f64 = 20.0;
const BAR_GAP: f64 = 8.0;
/// After a seek the bar stays where it was sent until the player reports a position this close to it, or for at
/// most this long.
const SEEK_CAUGHT_UP_MS: u64 = 1_500;
const SEEK_PATIENCE: Duration = Duration::from_secs(3);

/// What the lyrics area of the page shows.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Lyrics<'a> {
    /// The page looks as it did before lyrics existed.
    Hidden,
    /// An honest empty state: a short title and an explanation.
    Message { title: &'a str, detail: &'a str },
    /// The sung line large, with the one before and the one after dim.
    Synced {
        previous: &'a str,
        current: &'a str,
        next: &'a str,
    },
    /// Lyrics without timing, as text that scrolls.
    Plain(&'a str),
}

/// What the buttons do; the app decides how to reach the player.
pub struct MediaCallbacks {
    pub on_previous: Box<dyn Fn()>,
    pub on_play_pause: Box<dyn Fn()>,
    pub on_next: Box<dyn Fn()>,
    /// The bar was dragged or clicked: move to this position, in milliseconds from the start.
    pub on_seek: Box<dyn Fn(u64)>,
}

pub struct ArtIvars {
    tone: Cell<Tone>,
    glyph: RefCell<String>,
    /// The id of the artwork on the card and its decoded picture, `None` when it could not be decoded.
    picture: RefCell<Option<(u64, Option<Retained<NSImage>>)>>,
}

define_class!(
    // SAFETY: NSView allows subclassing; the ivars are only touched on the main thread and need no Drop.
    #[unsafe(super(NSView))]
    #[thread_kind = MainThreadOnly]
    #[name = "FigArtCard"]
    #[ivars = ArtIvars]
    struct ArtCard;

    impl ArtCard {
        #[unsafe(method(drawRect:))]
        fn draw_rect(&self, _dirty: NSRect) {
            let bounds = self.bounds();
            let path = NSBezierPath::bezierPathWithRoundedRect_xRadius_yRadius(
                bounds,
                style::CARD_RADIUS + 6.0,
                style::CARD_RADIUS + 6.0,
            );
            if let Some((_, Some(image))) = &*self.ivars().picture.borrow() {
                let size = image.size();
                if size.width > 0.0 && size.height > 0.0 {
                    // Aspect fill: the picture covers the card, and the rounded corners clip what overflows.
                    let scale = (bounds.size.width / size.width).max(bounds.size.height / size.height);
                    let drawn = NSSize::new(size.width * scale, size.height * scale);
                    let rect = NSRect::new(
                        NSPoint::new(
                            bounds.origin.x + (bounds.size.width - drawn.width) / 2.0,
                            bounds.origin.y + (bounds.size.height - drawn.height) / 2.0,
                        ),
                        drawn,
                    );
                    NSGraphicsContext::saveGraphicsState_class();
                    if let Some(context) = NSGraphicsContext::currentContext() {
                        context.setImageInterpolation(NSImageInterpolation::High);
                    }
                    path.addClip();
                    image.drawInRect(rect);
                    NSGraphicsContext::restoreGraphicsState_class();
                    return;
                }
            }
            let base = self.ivars().tone.get().color();
            let deep = base
                .blendedColorWithFraction_ofColor(0.55, &black(1.0))
                .unwrap_or_else(|| black(1.0));
            if let Some(gradient) =
                NSGradient::initWithStartingColor_endingColor(NSGradient::alloc(), &base, &deep)
            {
                gradient.drawInBezierPath_angle(&path, -90.0);
            }
            style::draw_symbol(
                &self.ivars().glyph.borrow(),
                NSPoint::new(bounds.size.width / 2.0, bounds.size.height / 2.0),
                bounds.size.width * 0.38,
                &white(0.88),
            );
        }
    }

    unsafe impl NSObjectProtocol for ArtCard {}
);

impl ArtCard {
    fn new(mtm: MainThreadMarker, frame: NSRect) -> Retained<Self> {
        let this = mtm.alloc::<Self>().set_ivars(ArtIvars {
            tone: Cell::new(Tone::Gray),
            glyph: RefCell::new("music.note".to_owned()),
            picture: RefCell::new(None),
        });
        // SAFETY: designated initializer with valid arguments on the main thread.
        unsafe { msg_send![super(this), initWithFrame: frame] }
    }

    /// Shows the artwork when there is one, otherwise a gradient of `tone` with `glyph`. A picture is decoded once
    /// per artwork id, and the card is only redrawn when something changed.
    fn set(&self, tone: Tone, glyph: &str, artwork: Option<&Artwork>) {
        let ivars = self.ivars();
        let shown = ivars.picture.borrow().as_ref().map(|(id, _)| *id);
        let new_picture = shown != artwork.map(|a| a.id);
        if new_picture {
            *ivars.picture.borrow_mut() = artwork.map(|a| {
                let data = NSData::with_bytes(&a.bytes);
                (a.id, NSImage::initWithData(NSImage::alloc(), &data))
            });
        }
        if !new_picture && ivars.tone.get() == tone && ivars.glyph.borrow().as_str() == glyph {
            return;
        }
        ivars.tone.set(tone);
        glyph.clone_into(&mut ivars.glyph.borrow_mut());
        self.setNeedsDisplay(true);
    }
}

/// What is on screen in the lyrics area, to know when a change deserves a movement.
#[derive(Debug, Default, PartialEq, Eq)]
struct Shown {
    synced: bool,
    current: String,
}

pub struct MediaView {
    pub root: Retained<PageRoot>,
    art: Retained<ArtCard>,
    title: Retained<NSTextField>,
    artist: Retained<NSTextField>,
    status: Retained<NSTextField>,
    elapsed: Retained<NSTextField>,
    bar: Retained<ProgressBar>,
    remaining: Retained<NSTextField>,
    previous: Retained<TouchButton>,
    play: Retained<TouchButton>,
    next: Retained<TouchButton>,
    callbacks: Rc<MediaCallbacks>,
    line_before: Retained<NSTextField>,
    line_now: Retained<NSTextField>,
    line_after: Retained<NSTextField>,
    plain_scroll: Retained<NSScrollView>,
    plain_text: Retained<NSTextView>,
    compact: Cell<bool>,
    /// The height of the page with lyrics: what the notch gives a page of the default size.
    full_height: f64,
    shown: RefCell<Shown>,
    /// A fingerprint of what the lyrics area shows, so that being told the same thing again does no work.
    drawn: Cell<Option<u64>>,
    /// The timer of the movement between two lines; empty when nothing moves, so an idle page runs no timer.
    slide: Rc<RefCell<Option<app::Timer>>>,
    /// The length of what plays, in milliseconds; 0 when unknown, and then the bar is hidden.
    length: Cell<u64>,
    /// The position a seek asked for, and when, until the player reports it.
    sought: Cell<Option<(u64, Instant)>>,
}

/// Where the bar stands just after a seek: where the seek sent it, moving on with the music, until the player
/// reports a position close to that or it has been too long. `None` once the seek is behind.
fn after_seek(reported: u64, sought: u64, since: Duration, playing: bool) -> Option<u64> {
    let expected = sought + if playing { since.as_millis() as u64 } else { 0 };
    (since < SEEK_PATIENCE && reported.abs_diff(expected) > SEEK_CAUGHT_UP_MS).then_some(expected)
}

/// A colour for the app's card when it gives no artwork, the same from one run to the next. Music and Spotify
/// keep the colours they had before any other app was shown.
fn app_tone(bundle: &str) -> Tone {
    const TONES: [Tone; 5] = [
        Tone::Blue,
        Tone::Orange,
        Tone::Green,
        Tone::Red,
        Tone::Purple,
    ];
    match bundle {
        "com.apple.Music" => Tone::Red,
        "com.spotify.client" => Tone::Green,
        _ => {
            let hash = bundle
                .bytes()
                .fold(0u32, |h, b| h.wrapping_mul(31).wrapping_add(u32::from(b)));
            TONES[hash as usize % TONES.len()]
        }
    }
}

/// 4:05, or 1:02:05 from an hour on.
fn clock(seconds: u64) -> String {
    let (hours, minutes, seconds) = (seconds / 3600, seconds / 60 % 60, seconds % 60);
    if hours > 0 {
        format!("{hours}:{minutes:02}:{seconds:02}")
    } else {
        format!("{minutes}:{seconds:02}")
    }
}

/// The frames of the time played, the bar and the time left, between `left` and `right`, centred on `middle`.
fn progress_frames(left: f64, right: f64, middle: f64) -> [NSRect; 3] {
    let rect =
        |x: f64, w: f64, h: f64| NSRect::new(NSPoint::new(x, middle - h / 2.0), NSSize::new(w, h));
    [
        rect(left, TIME_WIDTH, TIME_HEIGHT),
        rect(
            left + TIME_WIDTH + BAR_GAP,
            right - left - 2.0 * (TIME_WIDTH + BAR_GAP),
            BAR_HEIGHT,
        ),
        rect(right - TIME_WIDTH, TIME_WIDTH, TIME_HEIGHT),
    ]
}

/// Where everything on the page goes.
struct Frames {
    art: NSRect,
    title: NSRect,
    artist: NSRect,
    status: NSRect,
    times: [NSRect; 3],
    previous: NSRect,
    play: NSRect,
    next: NSRect,
}

/// The frames for a page of this size: the cover beside a column of text, bar and buttons, or, when lyrics take the
/// middle of the page, the small cover with its text at the top and the buttons at the bottom edge.
fn frames(width: f64, height: f64, compact: bool) -> Frames {
    let rect = |x: f64, y: f64, w: f64, h: f64| NSRect::new(NSPoint::new(x, y), NSSize::new(w, h));
    let round = |x: f64, y: f64, side: f64| rect(x - side / 2.0, y - side / 2.0, side, side);
    if compact {
        let text_x = MARGIN + ART_SMALL + 14.0;
        let text_w = width - text_x - MARGIN;
        let side = MAIN_BUTTON / 2.0 + BUTTON_GAP + SIDE_BUTTON / 2.0;
        let middle = CONTROLS_Y_WITH_LYRICS;
        Frames {
            art: rect(MARGIN, height - 20.0 - ART_SMALL, ART_SMALL, ART_SMALL),
            title: rect(text_x, height - 48.0, text_w, 26.0),
            artist: rect(text_x, height - 68.0, text_w, 20.0),
            status: rect(text_x, height - 84.0, text_w, 16.0),
            times: progress_frames(text_x, width - MARGIN, height - 100.0),
            previous: round(width / 2.0 - side, middle, SIDE_BUTTON),
            play: round(width / 2.0, middle, MAIN_BUTTON),
            next: round(width / 2.0 + side, middle, SIDE_BUTTON),
        }
    } else {
        let x = MARGIN + ART + ROOMY_GAP;
        let w = width - x - MARGIN;
        let side = ROOMY_MAIN / 2.0 + ROOMY_BUTTON_GAP + ROOMY_SIDE / 2.0;
        let (centre, buttons) = (x + w / 2.0, 50.0);
        Frames {
            art: rect(MARGIN, 40.0, ART, ART),
            title: rect(x, height - 36.0, w, 26.0),
            artist: rect(x, height - 56.0, w, 20.0),
            status: rect(x, height - 72.0, w, 16.0),
            times: progress_frames(x, width - MARGIN, height - 84.0),
            previous: round(centre - side, buttons, ROOMY_SIDE),
            play: round(centre, buttons, ROOMY_MAIN),
            next: round(centre + side, buttons, ROOMY_SIDE),
        }
    }
}

/// A label that may wrap onto a second line instead of being cut.
fn wrapped(mtm: MainThreadMarker) -> Retained<NSTextField> {
    let label = NSTextField::wrappingLabelWithString(&NSString::from_str(""), mtm);
    label.setSelectable(false);
    label.setAlignment(NSTextAlignment::Center);
    label.setMaximumNumberOfLines(2);
    label.setLineBreakMode(NSLineBreakMode::ByWordWrapping);
    if let Some(cell) = label.cell() {
        cell.setWraps(true);
        cell.setUsesSingleLineMode(false);
        cell.setTruncatesLastVisibleLine(true);
    }
    label.setAutoresizingMask(NSAutoresizingMaskOptions::ViewWidthSizable);
    label
}

fn label(
    mtm: MainThreadMarker,
    size: f64,
    weight: Weight,
    alpha: f64,
    frame: NSRect,
) -> Retained<NSTextField> {
    let label = NSTextField::labelWithString(&NSString::from_str(""), mtm);
    label.setFont(Some(&style::font(size, weight)));
    label.setTextColor(Some(&white(alpha)));
    label.setAlignment(NSTextAlignment::Center);
    label.setLineBreakMode(NSLineBreakMode::ByTruncatingTail);
    label.setFrame(frame);
    label.setAutoresizingMask(NSAutoresizingMaskOptions::ViewWidthSizable);
    label
}

fn round(
    mtm: MainThreadMarker,
    centre: NSPoint,
    side: f64,
    filled: bool,
    symbol: &str,
    title: &str,
) -> Retained<TouchButton> {
    let button = TouchButton::new(
        mtm,
        NSRect::new(
            NSPoint::new(centre.x - side / 2.0, centre.y - side / 2.0),
            NSSize::new(side, side),
        ),
        Look::Round { filled },
    );
    button.set_symbol(symbol);
    button.set_title(title);
    button
}

impl MediaView {
    pub fn new(mtm: MainThreadMarker, frame: NSRect, callbacks: MediaCallbacks) -> Rc<Self> {
        let texts = labels::get();
        // The page is as tall as the notch lets it be only while it shows lyrics; without them it is the short one.
        let full_height = frame.size.height;
        let width = frame.size.width;
        let short = NSRect::new(frame.origin, NSSize::new(width, ROOMY_HEIGHT));
        let root = PageRoot::new(mtm, short, &texts.media_hint);
        let callbacks = Rc::new(callbacks);
        let f = frames(width, ROOMY_HEIGHT, false);

        let art = ArtCard::new(mtm, f.art);
        root.addSubview(&art);

        let title = label(mtm, 19.0, Weight::Semibold, style::TEXT, f.title);
        let artist = label(mtm, 14.0, Weight::Regular, style::TEXT_SECONDARY, f.artist);
        let status = label(mtm, 11.0, Weight::Medium, style::TEXT_TERTIARY, f.status);
        for field in [&title, &artist, &status] {
            field.setAlignment(NSTextAlignment::Left);
            root.addSubview(field);
        }

        let [elapsed_frame, bar_frame, remaining_frame] = f.times;
        let time = |frame: NSRect, alignment: NSTextAlignment| {
            let field = label(mtm, 10.5, Weight::Medium, style::TEXT_TERTIARY, frame);
            field.setFont(Some(&style::digits(10.5, Weight::Medium)));
            field.setAlignment(alignment);
            field.setAutoresizingMask(NSAutoresizingMaskOptions::empty());
            field.setHidden(true);
            field
        };
        let elapsed = time(elapsed_frame, NSTextAlignment::Left);
        let remaining = time(remaining_frame, NSTextAlignment::Right);
        let bar = ProgressBar::new(mtm, bar_frame);
        bar.setHidden(true);
        root.addSubview(&elapsed);
        root.addSubview(&bar);
        root.addSubview(&remaining);

        let centre = |r: NSRect| {
            NSPoint::new(
                r.origin.x + r.size.width / 2.0,
                r.origin.y + r.size.height / 2.0,
            )
        };
        let previous = round(
            mtm,
            centre(f.previous),
            f.previous.size.width,
            false,
            "backward.fill",
            &texts.media_previous,
        );
        let play = round(
            mtm,
            centre(f.play),
            f.play.size.width,
            true,
            "play.fill",
            &texts.media_play_pause,
        );
        let next = round(
            mtm,
            centre(f.next),
            f.next.size.width,
            false,
            "forward.fill",
            &texts.media_next,
        );
        let shared = Rc::clone(&callbacks);
        previous.set_on_click(move || (shared.on_previous)());
        let shared = Rc::clone(&callbacks);
        play.set_on_click(move || (shared.on_play_pause)());
        let shared = Rc::clone(&callbacks);
        next.set_on_click(move || (shared.on_next)());
        for button in [&previous, &play, &next] {
            root.addSubview(button);
        }

        let line_before = label(
            mtm,
            14.0,
            Weight::Regular,
            style::TEXT_SECONDARY,
            NSRect::ZERO,
        );
        let line_after = label(
            mtm,
            14.0,
            Weight::Regular,
            style::TEXT_SECONDARY,
            NSRect::ZERO,
        );
        let line_now = wrapped(mtm);
        let pane = Self::pane(frame);
        let plain_text =
            NSTextView::initWithFrame(mtm.alloc(), NSRect::new(NSPoint::ZERO, pane.size));
        plain_text.setEditable(false);
        plain_text.setSelectable(false);
        plain_text.setDrawsBackground(false);
        plain_text.setAlignment(NSTextAlignment::Center);
        plain_text.setFont(Some(&style::font(15.0, Weight::Regular)));
        plain_text.setTextColor(Some(&white(0.86)));
        plain_text.setHorizontallyResizable(false);
        plain_text.setVerticallyResizable(true);
        plain_text.setAutoresizingMask(NSAutoresizingMaskOptions::ViewWidthSizable);
        plain_text.setMinSize(NSSize::new(0.0, pane.size.height));
        plain_text.setMaxSize(NSSize::new(pane.size.width, 1.0e7));
        let plain_scroll = NSScrollView::initWithFrame(mtm.alloc(), pane);
        plain_scroll.setHasVerticalScroller(true);
        plain_scroll.setAutohidesScrollers(true);
        plain_scroll.setScrollerStyle(NSScrollerStyle::Overlay);
        plain_scroll.setDrawsBackground(false);
        plain_scroll.setBorderType(NSBorderType::NoBorder);
        plain_scroll.setDocumentView(Some(&plain_text));
        plain_scroll.setHidden(true);
        for view in [&line_before, &line_now, &line_after] {
            view.setHidden(true);
            root.addSubview(view);
        }
        root.addSubview(&plain_scroll);

        let this = Rc::new(Self {
            root,
            art,
            title,
            artist,
            status,
            elapsed,
            bar,
            remaining,
            previous,
            play,
            next,
            callbacks,
            line_before,
            line_now,
            line_after,
            plain_scroll,
            plain_text,
            compact: Cell::new(false),
            full_height,
            shown: RefCell::default(),
            drawn: Cell::new(None),
            slide: Rc::default(),
            length: Cell::new(0),
            sought: Cell::new(None),
        });
        let weak = Rc::downgrade(&this);
        this.bar
            .set_on_seek(&texts.media_position, move |fraction, done| {
                if let Some(view) = weak.upgrade() {
                    view.scrubbed(fraction, done);
                }
            });
        this.show(None);
        this
    }

    /// The bar is being dragged: the times follow the pointer, and letting go moves what plays there.
    fn scrubbed(&self, fraction: f64, done: bool) {
        let length = self.length.get();
        if length == 0 {
            return;
        }
        let at = (fraction * length as f64).round() as u64;
        self.show_times(at, length);
        if done {
            self.sought.set(Some((at, Instant::now())));
            (self.callbacks.on_seek)(at);
        }
    }

    fn show_times(&self, at: u64, length: u64) {
        self.elapsed
            .setStringValue(&NSString::from_str(&clock(at / 1000)));
        let left = length.saturating_sub(at).div_ceil(1000);
        self.remaining
            .setStringValue(&NSString::from_str(&format!("-{}", clock(left))));
    }

    /// The area the lyrics use, between the track and the controls.
    fn pane(page: NSRect) -> NSRect {
        NSRect::new(
            NSPoint::new(MARGIN, LYRICS_BOTTOM),
            NSSize::new(
                page.size.width - 2.0 * MARGIN,
                page.size.height - LYRICS_TOP_GAP - LYRICS_BOTTOM,
            ),
        )
    }

    /// Moves the track and the controls between the short layout and the one that leaves space for lyrics. The page
    /// changes height with it, and the notch follows.
    fn set_compact(&self, compact: bool) {
        if self.compact.replace(compact) == compact {
            return;
        }
        let width = self.root.frame().size.width;
        let height = if compact {
            self.full_height
        } else {
            ROOMY_HEIGHT
        };
        self.root.setFrameSize(NSSize::new(width, height));
        let f = frames(width, height, compact);
        self.art.setFrame(f.art);
        self.title.setFrame(f.title);
        self.artist.setFrame(f.artist);
        self.status.setFrame(f.status);
        let [elapsed, bar, remaining] = f.times;
        self.elapsed.setFrame(elapsed);
        self.bar.setFrame(bar);
        self.remaining.setFrame(remaining);
        for (button, frame) in [
            (&self.previous, f.previous),
            (&self.play, f.play),
            (&self.next, f.next),
        ] {
            button.setFrame(frame);
        }
    }

    /// Frames of the previous, current and next line, with `shift` added to their height. The current line is as
    /// tall as its text, so the dim lines stay the same distance from it whether it takes one line or two.
    fn line_frames(&self, shift: f64) -> [NSRect; 3] {
        let pane = Self::pane(self.root.frame());
        let fit = self.line_now.cell().map_or(0.0, |cell| {
            cell.cellSizeForBounds(NSRect::new(
                NSPoint::ZERO,
                NSSize::new(pane.size.width, 1000.0),
            ))
            .height
        });
        let now_h = fit.clamp(LINE_HEIGHT, CURRENT_HEIGHT);
        let middle = pane.origin.y + pane.size.height / 2.0 + shift;
        let rect = |y: f64, h: f64| {
            NSRect::new(
                NSPoint::new(pane.origin.x, y),
                NSSize::new(pane.size.width, h),
            )
        };
        let now_y = middle - now_h / 2.0;
        [
            rect(now_y + now_h + LINE_GAP, LINE_HEIGHT),
            rect(now_y, now_h),
            rect(now_y - LINE_GAP - LINE_HEIGHT, LINE_HEIGHT),
        ]
    }

    fn set_lines(&self, shift: f64) {
        let frames = self.line_frames(shift);
        for (view, frame) in [&self.line_before, &self.line_now, &self.line_after]
            .into_iter()
            .zip(frames)
        {
            view.setFrame(frame);
        }
    }

    /// Shows the lyrics area, or takes it away. Cheap to call again with the same thing.
    pub fn show_lyrics(&self, lyrics: &Lyrics) {
        let mut hasher = DefaultHasher::new();
        lyrics.hash(&mut hasher);
        if self.drawn.replace(Some(hasher.finish())) == Some(hasher.finish()) {
            return;
        }
        self.set_compact(!matches!(lyrics, Lyrics::Hidden));
        let synced = matches!(lyrics, Lyrics::Synced { .. });
        let [before, now, after] = [&self.line_before, &self.line_now, &self.line_after];
        let lines_visible = matches!(lyrics, Lyrics::Synced { .. } | Lyrics::Message { .. });
        for line in [before, now, after] {
            line.setHidden(!lines_visible);
        }
        self.plain_scroll
            .setHidden(!matches!(lyrics, Lyrics::Plain(_)));
        let mut shown = self.shown.borrow_mut();
        let current = match lyrics {
            Lyrics::Synced { current, .. } => (*current).to_owned(),
            _ => String::new(),
        };
        let moved = synced && shown.synced && shown.current != current;
        *shown = Shown { synced, current };
        drop(shown);
        if !synced {
            // A movement that is still running must not carry on over other content.
            self.slide.borrow_mut().take();
        }
        match *lyrics {
            Lyrics::Hidden => {}
            Lyrics::Message { title, detail } => {
                let pane = self.line_frames(0.0);
                now.setFrame(NSRect::new(
                    NSPoint::new(pane[1].origin.x, pane[1].origin.y + 4.0),
                    NSSize::new(pane[1].size.width, 26.0),
                ));
                now.setFont(Some(&style::font(16.0, Weight::Semibold)));
                now.setTextColor(Some(&white(style::TEXT)));
                after.setFrame(NSRect::new(
                    NSPoint::new(pane[1].origin.x, pane[1].origin.y - 40.0),
                    NSSize::new(pane[1].size.width, 40.0),
                ));
                after.setMaximumNumberOfLines(2);
                after.setLineBreakMode(NSLineBreakMode::ByWordWrapping);
                after.setFont(Some(&style::font(13.0, Weight::Regular)));
                before.setStringValue(&NSString::from_str(""));
                now.setStringValue(&NSString::from_str(title));
                after.setStringValue(&NSString::from_str(detail));
                now.setAlphaValue(1.0);
            }
            Lyrics::Synced {
                previous,
                current,
                next,
            } => {
                now.setFont(Some(&style::font(21.0, Weight::Semibold)));
                now.setTextColor(Some(&white(style::TEXT)));
                after.setMaximumNumberOfLines(1);
                after.setLineBreakMode(NSLineBreakMode::ByTruncatingTail);
                after.setFont(Some(&style::font(14.0, Weight::Regular)));
                before.setStringValue(&NSString::from_str(previous));
                now.setStringValue(&NSString::from_str(current));
                after.setStringValue(&NSString::from_str(next));
                if moved {
                    self.start_slide();
                } else if self.slide.borrow().is_none() {
                    self.set_lines(0.0);
                }
            }
            Lyrics::Plain(text) => {
                if self.plain_text.string().to_string() != text {
                    self.plain_text.setString(&NSString::from_str(text));
                    self.plain_text.sizeToFit();
                    self.plain_text
                        .scrollRangeToVisible(objc2_foundation::NSRange::new(0, 0));
                }
            }
        }
    }

    /// The lines rise into place from below, fast at first and slowing down; with Reduce Motion they just change.
    fn start_slide(&self) {
        self.slide.borrow_mut().take();
        if crate::workspace::display_options().reduce_motion {
            self.set_lines(0.0);
            return;
        }
        let views = [
            Retained::clone(&self.line_before),
            Retained::clone(&self.line_now),
            Retained::clone(&self.line_after),
        ];
        let (rest, low) = (self.line_frames(0.0), self.line_frames(-SLIDE));
        // The new text takes its size at once; only its position and brightness move.
        self.set_lines(-SLIDE);
        self.line_now.setAlphaValue(0.35);
        let started = Instant::now();
        let slot = Rc::clone(&self.slide);
        let timer = app::every(1.0 / 60.0, move || {
            let t = (started.elapsed().as_secs_f64() / SLIDE_SECONDS).min(1.0);
            let eased = style::ease_out(t);
            for ((view, from), to) in views.iter().zip(low).zip(rest) {
                let y = from.origin.y + (to.origin.y - from.origin.y) * eased;
                view.setFrameOrigin(NSPoint::new(to.origin.x, y));
            }
            views[1].setAlphaValue(0.35 + 0.65 * eased);
            if t >= 1.0 {
                // Dropping the timer stops it and breaks the cycle between the timer and this closure.
                slot.borrow_mut().take();
            }
        });
        *self.slide.borrow_mut() = Some(timer);
    }

    /// Shows what plays, with its position carried forward to now, or the idle message when nothing does.
    pub fn show(&self, sample: Option<&Sample>) {
        let texts = labels::get();
        let track = sample.map(|s| &s.track);
        let (title, artist, status, tone, symbol) = match track {
            Some(t) => {
                let state = if t.playing {
                    &texts.media_playing
                } else {
                    &texts.media_paused
                }
                .to_uppercase();
                (
                    t.title.clone(),
                    t.artist.clone(),
                    if t.app_name.is_empty() {
                        state
                    } else {
                        format!("{}  -  {state}", t.app_name.to_uppercase())
                    },
                    app_tone(&t.app),
                    if t.playing { "pause.fill" } else { "play.fill" },
                )
            }
            None => (
                texts.media_idle.clone(),
                String::new(),
                String::new(),
                Tone::Gray,
                "play.fill",
            ),
        };
        self.art
            .set(tone, "music.note", track.and_then(|t| t.artwork.as_ref()));
        self.title.setStringValue(&NSString::from_str(&title));
        self.artist.setStringValue(&NSString::from_str(&artist));
        self.status.setStringValue(&NSString::from_str(&status));
        self.play.set_symbol(symbol);
        self.show_progress(sample);
    }

    /// The bar with the time played and the time left, while the length of the track is known.
    fn show_progress(&self, sample: Option<&Sample>) {
        let length = sample.map_or(0, |s| s.track.duration_ms);
        let views: [&NSView; 3] = [&self.elapsed, &self.bar, &self.remaining];
        for view in views {
            view.setHidden(length == 0);
        }
        self.length.set(length);
        let Some(sample) = sample.filter(|_| length > 0) else {
            return;
        };
        // While the bar is held it follows the pointer, and so do the times.
        if self.bar.held() {
            return;
        }
        let now = Instant::now();
        let mut at = sample.position_ms(now).min(length);
        if let Some((sought, when)) = self.sought.get() {
            match after_seek(at, sought, now - when, sample.track.playing) {
                Some(expected) => at = expected.min(length),
                None => self.sought.set(None),
            }
        }
        self.bar.set(at as f64 / length as f64, Tone::Gray);
        self.show_times(at, length);
    }

    /// A page at the size of the notch's pages whose buttons do nothing, for drawing it in a preview.
    pub fn preview(mtm: MainThreadMarker) -> Rc<Self> {
        Self::new(
            mtm,
            NSRect::new(NSPoint::ZERO, NSSize::new(500.0, 396.0)),
            MediaCallbacks {
                on_previous: Box::new(|| {}),
                on_play_pause: Box::new(|| {}),
                on_next: Box::new(|| {}),
                on_seek: Box::new(|_| {}),
            },
        )
    }

    /// The page drawn on the notch's black into PNG bytes, without showing anything on screen. For checking the design.
    pub fn snapshot_png(&self, mtm: MainThreadMarker) -> Option<Vec<u8>> {
        let backdrop = NSBox::new(mtm);
        backdrop.setBoxType(NSBoxType::Custom);
        backdrop.setBorderWidth(0.0);
        backdrop.setFillColor(&NSColor::blackColor());
        backdrop.setContentViewMargins(NSSize::ZERO);
        backdrop.setFrame(self.root.frame());
        backdrop.setContentView(Some(&self.root));
        // SAFETY: extern statics are process-lifetime constants.
        let dark = unsafe { NSAppearance::appearanceNamed(NSAppearanceNameDarkAqua) };
        backdrop.setAppearance(dark.as_deref());
        let bounds = backdrop.bounds();
        let rep = backdrop.bitmapImageRepForCachingDisplayInRect(bounds)?;
        backdrop.cacheDisplayInRect_toBitmapImageRep(bounds, &rep);
        // SAFETY: an empty properties dictionary is valid for PNG output.
        let png = unsafe {
            rep.representationUsingType_properties(NSBitmapImageFileType::PNG, &NSDictionary::new())
        };
        // The backdrop took the page as its content; give it back so the page can be shown again.
        self.root.removeFromSuperview();
        png.map(|data| data.to_vec())
    }

    /// Returns true when the key was consumed.
    pub fn handle_key(&self, key: Key) -> bool {
        match key {
            Key::Left => (self.callbacks.on_previous)(),
            Key::Right => (self.callbacks.on_next)(),
            Key::Enter => (self.callbacks.on_play_pause)(),
            _ => return false,
        }
        true
    }

    /// The notch tab for this view.
    pub fn page(
        self: &Rc<Self>,
        on_show: impl Fn() + 'static,
        on_hide: impl Fn() + 'static,
    ) -> Page {
        let keys = Rc::clone(self);
        Page {
            id: "media",
            title: labels::get().media_tab.clone(),
            symbol: "music.note",
            root: Retained::into_super(Retained::clone(&self.root)),
            on_show: Box::new(on_show),
            on_hide: Box::new(on_hide),
            on_key: Box::new(move |key| keys.handle_key(key)),
            accepts_files: false,
            size: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clocks_show_minutes_or_hours() {
        assert_eq!(clock(0), "0:00");
        assert_eq!(clock(65), "1:05");
        assert_eq!(clock(2304), "38:24");
        assert_eq!(clock(3600), "1:00:00");
        assert_eq!(clock(3725), "1:02:05");
    }

    #[test]
    fn an_app_keeps_its_colour() {
        assert_eq!(app_tone("com.apple.Music"), Tone::Red);
        assert_eq!(app_tone("com.spotify.client"), Tone::Green);
        assert_eq!(
            app_tone("com.microsoft.edgemac"),
            app_tone("com.microsoft.edgemac")
        );
        assert_ne!(app_tone(""), Tone::Gray, "gray is for nothing playing");
    }

    #[test]
    fn after_a_seek_the_bar_waits_for_the_player_then_follows_it() {
        let second = Duration::from_secs(1);
        // The player still reports the old place: the bar shows the new one, moving on while it plays.
        assert_eq!(after_seek(10_000, 90_000, second, true), Some(91_000));
        assert_eq!(after_seek(10_000, 90_000, second, false), Some(90_000));
        // It caught up, or it never will: the player is believed again.
        assert_eq!(after_seek(90_800, 90_000, second, true), None);
        assert_eq!(after_seek(10_000, 90_000, SEEK_PATIENCE, true), None);
    }

    #[test]
    fn everything_on_the_page_stays_inside_it_in_both_layouts() {
        // The short page, and the full one that lyrics need.
        for (compact, height) in [(false, ROOMY_HEIGHT), (true, 398.0)] {
            let f = frames(500.0, height, compact);
            let [elapsed, bar, remaining] = f.times;
            assert!(elapsed.origin.x >= MARGIN);
            assert!(remaining.origin.x + remaining.size.width <= 500.0 - MARGIN);
            assert!(elapsed.origin.x + elapsed.size.width < bar.origin.x);
            assert!(bar.origin.x + bar.size.width < remaining.origin.x);
            assert!(bar.size.width > 100.0);
            for (name, r) in [
                ("art", f.art),
                ("title", f.title),
                ("artist", f.artist),
                ("status", f.status),
                ("elapsed", elapsed),
                ("bar", bar),
                ("remaining", remaining),
                ("previous", f.previous),
                ("play", f.play),
                ("next", f.next),
            ] {
                assert!(
                    r.origin.x >= 0.0
                        && r.origin.y >= 0.0
                        && r.origin.x + r.size.width <= 500.0
                        && r.origin.y + r.size.height <= height,
                    "{name} leaves the page (compact {compact}): {r:?}"
                );
            }
        }
    }

    #[test]
    fn the_short_page_puts_the_text_beside_the_cover_and_the_buttons_under_the_bar() {
        let f = frames(500.0, ROOMY_HEIGHT, false);
        assert!(f.title.origin.x >= f.art.origin.x + f.art.size.width);
        let [_, bar, _] = f.times;
        assert!(f.play.origin.y + f.play.size.height < bar.origin.y);
        assert!(bar.origin.y + bar.size.height < f.status.origin.y);
    }
}
