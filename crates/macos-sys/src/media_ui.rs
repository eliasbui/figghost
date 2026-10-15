//! The media page: a cover card for the player, the current track, and previous / play-pause / next.

use crate::{
    app,
    button::{Look, TouchButton},
    labels,
    notch::Page,
    now_playing::{Player, Track},
    panel::Key,
    style::{self, Tone, Weight, black, white},
    widgets::PageRoot,
};
use objc2::{
    AnyThread, DefinedClass, MainThreadMarker, MainThreadOnly, define_class, msg_send, rc::Retained,
};
use objc2_app_kit::{
    NSAppearance, NSAppearanceCustomization, NSAppearanceNameDarkAqua, NSAutoresizingMaskOptions,
    NSBezierPath, NSBitmapImageFileType, NSBorderType, NSBox, NSBoxType, NSColor, NSGradient,
    NSLineBreakMode, NSScrollView, NSScrollerStyle, NSTextAlignment, NSTextField, NSTextView,
    NSView,
};
use objc2_foundation::{NSDictionary, NSObjectProtocol, NSPoint, NSRect, NSSize, NSString};
use std::{
    cell::{Cell, RefCell},
    collections::hash_map::DefaultHasher,
    hash::{Hash, Hasher},
    rc::Rc,
    time::Instant,
};

const MARGIN: f64 = 24.0;
const ART: f64 = 116.0;
const SIDE_BUTTON: f64 = 48.0;
const MAIN_BUTTON: f64 = 66.0;
const BUTTON_GAP: f64 = 30.0;
/// With lyrics the cover shrinks to the corner and the controls drop to the bottom edge to make room.
const ART_SMALL: f64 = 72.0;
const CONTROLS_Y_WITH_LYRICS: f64 = 58.0;
const LYRICS_BOTTOM: f64 = 100.0;
const LYRICS_TOP_GAP: f64 = 108.0;
const LINE_HEIGHT: f64 = 20.0;
const CURRENT_HEIGHT: f64 = 56.0;
const LINE_GAP: f64 = 14.0;
/// How far the lines travel when the sung line changes, and for how long.
const SLIDE: f64 = 22.0;
const SLIDE_SECONDS: f64 = 0.32;

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
}

pub struct ArtIvars {
    tone: Cell<Tone>,
    glyph: RefCell<String>,
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
        });
        // SAFETY: designated initializer with valid arguments on the main thread.
        unsafe { msg_send![super(this), initWithFrame: frame] }
    }

    fn set(&self, tone: Tone, glyph: &str) {
        self.ivars().tone.set(tone);
        glyph.clone_into(&mut self.ivars().glyph.borrow_mut());
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
    shown: RefCell<Shown>,
    /// A fingerprint of what the lyrics area shows, so that being told the same thing again does no work.
    drawn: Cell<Option<u64>>,
    /// The timer of the movement between two lines; empty when nothing moves, so an idle page runs no timer.
    slide: Rc<RefCell<Option<app::Timer>>>,
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
        let root = PageRoot::new(mtm, frame, &texts.media_hint);
        let (width, top) = (frame.size.width, frame.size.height);
        let callbacks = Rc::new(callbacks);

        let art = ArtCard::new(
            mtm,
            NSRect::new(
                NSPoint::new((width - ART) / 2.0, top - 22.0 - ART),
                NSSize::new(ART, ART),
            ),
        );
        root.addSubview(&art);

        let row = |y: f64, height: f64| {
            NSRect::new(
                NSPoint::new(MARGIN, y),
                NSSize::new(width - 2.0 * MARGIN, height),
            )
        };
        let title = label(
            mtm,
            19.0,
            Weight::Semibold,
            style::TEXT,
            row(top - 176.0, 26.0),
        );
        let artist = label(
            mtm,
            14.0,
            Weight::Regular,
            style::TEXT_SECONDARY,
            row(top - 200.0, 20.0),
        );
        let status = label(
            mtm,
            11.0,
            Weight::Medium,
            style::TEXT_TERTIARY,
            row(top - 220.0, 16.0),
        );
        root.addSubview(&title);
        root.addSubview(&artist);
        root.addSubview(&status);

        let middle = NSPoint::new(width / 2.0, top - 292.0);
        let side = MAIN_BUTTON / 2.0 + BUTTON_GAP + SIDE_BUTTON / 2.0;
        let previous = round(
            mtm,
            NSPoint::new(middle.x - side, middle.y),
            SIDE_BUTTON,
            false,
            "backward.fill",
            &texts.media_previous,
        );
        let play = round(
            mtm,
            middle,
            MAIN_BUTTON,
            true,
            "play.fill",
            &texts.media_play_pause,
        );
        let next = round(
            mtm,
            NSPoint::new(middle.x + side, middle.y),
            SIDE_BUTTON,
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
            shown: RefCell::default(),
            drawn: Cell::new(None),
            slide: Rc::default(),
        });
        this.show(None);
        this
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

    /// Moves the track and the controls between the roomy layout and the one that leaves space for lyrics.
    fn set_compact(&self, compact: bool) {
        if self.compact.replace(compact) == compact {
            return;
        }
        let (width, top) = {
            let size = self.root.frame().size;
            (size.width, size.height)
        };
        let rect =
            |x: f64, y: f64, w: f64, h: f64| NSRect::new(NSPoint::new(x, y), NSSize::new(w, h));
        if compact {
            let text_x = MARGIN + ART_SMALL + 14.0;
            let text_w = width - text_x - MARGIN;
            self.art
                .setFrame(rect(MARGIN, top - 20.0 - ART_SMALL, ART_SMALL, ART_SMALL));
            self.title.setFrame(rect(text_x, top - 48.0, text_w, 26.0));
            self.artist.setFrame(rect(text_x, top - 68.0, text_w, 20.0));
            self.status.setFrame(rect(text_x, top - 84.0, text_w, 16.0));
        } else {
            let row = |y: f64, h: f64| rect(MARGIN, y, width - 2.0 * MARGIN, h);
            self.art
                .setFrame(rect((width - ART) / 2.0, top - 22.0 - ART, ART, ART));
            self.title.setFrame(row(top - 176.0, 26.0));
            self.artist.setFrame(row(top - 200.0, 20.0));
            self.status.setFrame(row(top - 220.0, 16.0));
        }
        let alignment = if compact {
            NSTextAlignment::Left
        } else {
            NSTextAlignment::Center
        };
        for field in [&self.title, &self.artist, &self.status] {
            field.setAlignment(alignment);
        }
        let middle = if compact {
            CONTROLS_Y_WITH_LYRICS
        } else {
            top - 292.0
        };
        let side = MAIN_BUTTON / 2.0 + BUTTON_GAP + SIDE_BUTTON / 2.0;
        for (button, x, size) in [
            (&self.previous, width / 2.0 - side, SIDE_BUTTON),
            (&self.play, width / 2.0, MAIN_BUTTON),
            (&self.next, width / 2.0 + side, SIDE_BUTTON),
        ] {
            button.setFrame(rect(x - size / 2.0, middle - size / 2.0, size, size));
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

    /// Shows the track, or the idle message when nothing is playing.
    pub fn show(&self, track: Option<&Track>) {
        let texts = labels::get();
        let (title, artist, status, tone, symbol) = match track {
            Some(t) => (
                t.title.clone(),
                t.artist.clone(),
                format!(
                    "{}  -  {}",
                    t.player.name().to_uppercase(),
                    if t.playing {
                        texts.media_playing.to_uppercase()
                    } else {
                        texts.media_paused.to_uppercase()
                    }
                ),
                match t.player {
                    Player::Music => Tone::Red,
                    Player::Spotify => Tone::Green,
                },
                if t.playing { "pause.fill" } else { "play.fill" },
            ),
            None => (
                texts.media_idle.clone(),
                String::new(),
                String::new(),
                Tone::Gray,
                "play.fill",
            ),
        };
        self.art.set(tone, "music.note");
        self.title.setStringValue(&NSString::from_str(&title));
        self.artist.setStringValue(&NSString::from_str(&artist));
        self.status.setStringValue(&NSString::from_str(&status));
        self.play.set_symbol(symbol);
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
            title: labels::get().media_tab.clone(),
            symbol: "music.note",
            root: Retained::into_super(Retained::clone(&self.root)),
            on_show: Box::new(on_show),
            on_hide: Box::new(on_hide),
            on_key: Box::new(move |key| keys.handle_key(key)),
            accepts_files: false,
        }
    }
}
