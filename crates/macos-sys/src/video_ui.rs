//! The Video tab of the notch: one stream or file played by AVPlayer in a layer of the page. The player lives only
//! while the tab shows, so nothing is fetched or decoded while the notch is closed. Everything the page says is given
//! by the app, so nothing here knows a language.

use crate::{notch::Page, widgets::draw_page_note};
use objc2::{DefinedClass, MainThreadMarker, MainThreadOnly, define_class, msg_send, rc::Retained};
use objc2_app_kit::NSView;
use objc2_av_foundation::{
    AVLayerVideoGravityResizeAspect, AVPlayer, AVPlayerItemStatus, AVPlayerLayer,
};
use objc2_foundation::{NSPoint, NSRect, NSSize, NSString, NSURL};
use std::{cell::RefCell, rc::Rc};

/// The size of the area the tab fills below the header: room for a 16:9 picture.
pub const PAGE_SIZE: NSSize = NSSize::new(580.0, 300.0);
const MARGIN: f64 = 12.0;
const RADIUS: f64 = 12.0;

pub struct CanvasIvars {
    /// Title and hint shown while there is no picture.
    note: RefCell<Option<(String, String)>>,
}

define_class!(
    // SAFETY: NSView allows subclassing; the ivars are only touched on the main thread and need no Drop.
    #[unsafe(super(NSView))]
    #[thread_kind = MainThreadOnly]
    #[name = "FigVideoCanvas"]
    #[ivars = CanvasIvars]
    pub struct VideoCanvas;

    impl VideoCanvas {
        #[unsafe(method(drawRect:))]
        fn draw_rect(&self, _dirty: NSRect) {
            if let Some((title, hint)) = self.ivars().note.borrow().as_ref() {
                draw_page_note("play.rectangle.fill", title, hint, self.bounds().size);
            }
        }
    }
);

impl VideoCanvas {
    fn new(mtm: MainThreadMarker, frame: NSRect) -> Retained<Self> {
        let this = mtm.alloc::<Self>().set_ivars(CanvasIvars {
            note: RefCell::default(),
        });
        // SAFETY: designated initializer with valid arguments on the main thread.
        unsafe { msg_send![super(this), initWithFrame: frame] }
    }
}

pub struct VideoView {
    canvas: Retained<VideoCanvas>,
    layer: Retained<AVPlayerLayer>,
    player: RefCell<Option<Retained<AVPlayer>>>,
}

impl VideoView {
    /// `frame` is `Notch::page_area(PAGE_SIZE.width, PAGE_SIZE.height)`.
    pub fn new(mtm: MainThreadMarker, frame: NSRect) -> Rc<Self> {
        let canvas = VideoCanvas::new(mtm, frame);
        canvas.setWantsLayer(true);
        // SAFETY: a constant of AVFoundation, read on the main thread.
        let layer = unsafe { AVPlayerLayer::new() };
        if let Some(gravity) = unsafe { AVLayerVideoGravityResizeAspect } {
            unsafe { layer.setVideoGravity(gravity) };
        }
        layer.setFrame(NSRect::new(
            NSPoint::new(MARGIN, MARGIN),
            NSSize::new(
                frame.size.width - 2.0 * MARGIN,
                frame.size.height - 2.0 * MARGIN,
            ),
        ));
        layer.setCornerRadius(RADIUS);
        layer.setMasksToBounds(true);
        layer.setHidden(true);
        if let Some(root) = canvas.layer() {
            root.addSublayer(&layer);
        }
        Rc::new(Self {
            canvas,
            layer,
            player: RefCell::default(),
        })
    }

    /// Shows words in place of the picture, or the picture again for `None`.
    pub fn note(&self, note: Option<(String, String)>) {
        self.layer.setHidden(note.is_some());
        *self.canvas.ivars().note.borrow_mut() = note;
        self.canvas.setNeedsDisplay(true);
    }

    /// Starts `url`, muted. False when it is not an address AVPlayer can be given.
    pub fn play(&self, url: &str) -> bool {
        let Some(url) = NSURL::URLWithString(&NSString::from_str(url)) else {
            return false;
        };
        // SAFETY: plain AVFoundation calls on the main thread with valid objects.
        unsafe {
            let player = AVPlayer::playerWithURL(&url, MainThreadMarker::from(&*self.canvas));
            player.setMuted(true);
            self.layer.setPlayer(Some(&player));
            player.play();
            *self.player.borrow_mut() = Some(player);
        }
        true
    }

    /// Stops the player and drops its connection.
    pub fn stop(&self) {
        if let Some(player) = self.player.borrow_mut().take() {
            // SAFETY: plain AVFoundation calls on the main thread.
            unsafe {
                player.pause();
                self.layer.setPlayer(None);
            }
        }
    }

    /// Whether the item the player was given could not be opened or broke off.
    pub fn failed(&self) -> bool {
        self.player.borrow().as_ref().is_some_and(|player| {
            // SAFETY: reads of an item the player owns, on the main thread.
            unsafe { player.currentItem() }
                .is_some_and(|item| unsafe { item.status() } == AVPlayerItemStatus::Failed)
        })
    }

    /// The first picture is on screen.
    pub fn ready(&self) -> bool {
        // SAFETY: a read of the layer on the main thread.
        unsafe { self.layer.isReadyForDisplay() }
    }

    pub fn page(
        self: &Rc<Self>,
        title: String,
        on_show: impl Fn() + 'static,
        on_hide: impl Fn() + 'static,
    ) -> Page {
        Page {
            id: "video",
            title,
            symbol: "play.rectangle.fill",
            root: Retained::into_super(Retained::clone(&self.canvas)),
            on_show: Box::new(on_show),
            on_hide: Box::new(on_hide),
            on_key: Box::new(|_| false),
            accepts_files: false,
            size: None,
        }
    }
}
