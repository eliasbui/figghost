//! The media page: the current track and previous / play-pause / next buttons.

use crate::{
    labels,
    notch::Page,
    now_playing::Track,
    panel::Key,
    widgets::{ActionTarget, PageRoot},
};
use objc2::{MainThreadMarker, rc::Retained, runtime::AnyObject, sel};
use objc2_app_kit::{
    NSAutoresizingMaskOptions, NSButton, NSColor, NSFont, NSLineBreakMode, NSTextAlignment,
    NSTextField,
};
use objc2_foundation::{NSPoint, NSRect, NSSize, NSString};
use std::rc::Rc;

const BUTTON_WIDTH: f64 = 120.0;
const BUTTON_HEIGHT: f64 = 30.0;
const BUTTON_GAP: f64 = 14.0;
const MARGIN: f64 = 24.0;

type ButtonAction = fn(&MediaCallbacks);

/// What the buttons do; the app decides how to reach the player.
pub struct MediaCallbacks {
    pub on_previous: Box<dyn Fn()>,
    pub on_play_pause: Box<dyn Fn()>,
    pub on_next: Box<dyn Fn()>,
}

pub struct MediaView {
    pub root: Retained<PageRoot>,
    title: Retained<NSTextField>,
    artist: Retained<NSTextField>,
    status: Retained<NSTextField>,
    callbacks: Rc<MediaCallbacks>,
    _targets: Vec<Retained<ActionTarget>>,
}

fn label(mtm: MainThreadMarker, size: f64, white: f64, frame: NSRect) -> Retained<NSTextField> {
    let label = NSTextField::labelWithString(&NSString::from_str(""), mtm);
    label.setFont(Some(&NSFont::systemFontOfSize(size)));
    label.setTextColor(Some(&NSColor::colorWithWhite_alpha(1.0, white)));
    label.setAlignment(NSTextAlignment::Center);
    label.setLineBreakMode(NSLineBreakMode::ByTruncatingTail);
    label.setFrame(frame);
    label.setAutoresizingMask(NSAutoresizingMaskOptions::ViewWidthSizable);
    label
}

impl MediaView {
    pub fn new(mtm: MainThreadMarker, frame: NSRect, callbacks: MediaCallbacks) -> Rc<Self> {
        let texts = labels::get();
        let root = PageRoot::new(mtm, frame, &texts.media_hint);
        let width = frame.size.width - 2.0 * MARGIN;
        let middle = frame.size.height * 0.56;
        let row = |offset: f64, height: f64| {
            NSRect::new(
                NSPoint::new(MARGIN, middle + offset),
                NSSize::new(width, height),
            )
        };
        let title = label(mtm, 19.0, 0.95, row(40.0, 26.0));
        let artist = label(mtm, 14.0, 0.65, row(14.0, 20.0));
        let status = label(mtm, 11.0, 0.4, row(-8.0, 16.0));
        root.addSubview(&title);
        root.addSubview(&artist);
        root.addSubview(&status);

        let callbacks = Rc::new(callbacks);
        let mut targets = Vec::new();
        let total = 3.0 * BUTTON_WIDTH + 2.0 * BUTTON_GAP;
        let left = (frame.size.width - total) / 2.0;
        let buttons: [(&str, ButtonAction); 3] = [
            (&texts.media_previous, |c| (c.on_previous)()),
            (&texts.media_play_pause, |c| (c.on_play_pause)()),
            (&texts.media_next, |c| (c.on_next)()),
        ];
        for (i, (text, act)) in buttons.into_iter().enumerate() {
            let shared = Rc::clone(&callbacks);
            let target = ActionTarget::new(mtm, move || act(&shared));
            let target_object: &AnyObject = &target;
            // SAFETY: `fire:` takes the sender as its only argument; the target is kept alive by the view.
            let button = unsafe {
                NSButton::buttonWithTitle_target_action(
                    &NSString::from_str(text),
                    Some(target_object),
                    Some(sel!(fire:)),
                    mtm,
                )
            };
            button.setFrame(NSRect::new(
                NSPoint::new(left + i as f64 * (BUTTON_WIDTH + BUTTON_GAP), middle - 70.0),
                NSSize::new(BUTTON_WIDTH, BUTTON_HEIGHT),
            ));
            root.addSubview(&button);
            targets.push(target);
        }

        let this = Rc::new(Self {
            root,
            title,
            artist,
            status,
            callbacks,
            _targets: targets,
        });
        this.show(None);
        this
    }

    /// Shows the track, or the idle message when nothing is playing.
    pub fn show(&self, track: Option<&Track>) {
        let texts = labels::get();
        let (title, artist, status) = match track {
            Some(t) => (
                t.title.clone(),
                t.artist.clone(),
                format!(
                    "{} - {}",
                    t.player.name(),
                    if t.playing {
                        &texts.media_playing
                    } else {
                        &texts.media_paused
                    }
                ),
            ),
            None => (texts.media_idle.clone(), String::new(), String::new()),
        };
        self.title.setStringValue(&NSString::from_str(&title));
        self.artist.setStringValue(&NSString::from_str(&artist));
        self.status.setStringValue(&NSString::from_str(&status));
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
            root: Retained::into_super(Retained::clone(&self.root)),
            on_show: Box::new(on_show),
            on_hide: Box::new(on_hide),
            on_key: Box::new(move |key| keys.handle_key(key)),
            accepts_files: false,
        }
    }
}
