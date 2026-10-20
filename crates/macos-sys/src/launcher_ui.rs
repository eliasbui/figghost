//! The Launcher tab of the notch: a grid of tiles for the apps, folders and Shortcuts the person chose. Everything
//! on it is plain data from the app, so nothing here reads a setting or launches anything.

use crate::{
    button::{Look, TouchButton},
    notch::Page,
    style::Tone,
    widgets::draw_page_note,
    workspace,
};
use objc2::{DefinedClass, MainThreadMarker, MainThreadOnly, define_class, msg_send, rc::Retained};
use objc2_app_kit::NSView;
use objc2_foundation::{NSPoint, NSRect, NSSize};
use std::{cell::RefCell, path::PathBuf, rc::Rc};

/// The size of the area the tab fills below the header.
pub const PAGE_SIZE: NSSize = NSSize::new(580.0, 214.0);
/// How many tiles a row holds, and how many tiles the tab holds at most.
pub const COLUMNS: usize = 6;
pub const MAX_TILES: usize = 12;

const TILE_WIDTH: f64 = 84.0;
const TILE_HEIGHT: f64 = 88.0;
const GAP: f64 = 6.0;

/// What a tile shows above its name.
#[derive(Debug, Clone)]
pub enum TileIcon {
    /// The icon the Finder shows for this path: an app, a folder.
    File(PathBuf),
    /// A glyph, for what has no file to take an icon from.
    Symbol(&'static str, Tone),
}

#[derive(Debug, Clone)]
pub struct Tile {
    pub title: String,
    pub icon: TileIcon,
}

#[derive(Debug, Clone, Default)]
pub struct LauncherDisplay {
    pub tiles: Vec<Tile>,
    /// Words that replace the grid: the tab is off, or nothing is pinned.
    pub note: Option<(String, String)>,
}

pub struct RootIvars {
    note: RefCell<Option<(String, String)>>,
}

define_class!(
    // SAFETY: NSView allows subclassing; the ivars are only touched on the main thread and need no Drop.
    #[unsafe(super(NSView))]
    #[thread_kind = MainThreadOnly]
    #[name = "FigLauncherRoot"]
    #[ivars = RootIvars]
    pub struct LauncherRoot;

    impl LauncherRoot {
        #[unsafe(method(drawRect:))]
        fn draw_rect(&self, _dirty: NSRect) {
            if let Some((title, hint)) = self.ivars().note.borrow().as_ref() {
                draw_page_note("square.grid.2x2.fill", title, hint, self.bounds().size);
            }
        }
    }
);

impl LauncherRoot {
    fn new(mtm: MainThreadMarker, frame: NSRect) -> Retained<Self> {
        let this = mtm.alloc::<Self>().set_ivars(RootIvars {
            note: RefCell::new(None),
        });
        // SAFETY: designated initializer with valid arguments on the main thread.
        unsafe { msg_send![super(this), initWithFrame: frame] }
    }
}

pub struct LauncherView {
    root: Retained<LauncherRoot>,
    tiles: RefCell<Vec<Retained<TouchButton>>>,
    on_open: Rc<dyn Fn(usize)>,
    mtm: MainThreadMarker,
}

impl LauncherView {
    /// `frame` is `Notch::page_area(PAGE_SIZE.width, PAGE_SIZE.height)`; `on_open` is told the number of the tile
    /// that was pressed.
    pub fn new(
        mtm: MainThreadMarker,
        frame: NSRect,
        on_open: impl Fn(usize) + 'static,
    ) -> Rc<Self> {
        Rc::new(Self {
            root: LauncherRoot::new(mtm, frame),
            tiles: RefCell::default(),
            on_open: Rc::new(on_open),
            mtm,
        })
    }

    /// Draws what the tab is now to show.
    pub fn show(&self, display: &LauncherDisplay) {
        *self.root.ivars().note.borrow_mut() = display.note.clone();
        let shown: &[Tile] = if display.note.is_some() {
            &[]
        } else {
            &display.tiles[..display.tiles.len().min(MAX_TILES)]
        };
        let mut pool = self.tiles.borrow_mut();
        while pool.len() < shown.len() {
            let index = pool.len();
            let tile = TouchButton::new(self.mtm, NSRect::ZERO, Look::Tile);
            let open = Rc::clone(&self.on_open);
            tile.set_on_click(move || open(index));
            self.root.addSubview(&tile);
            pool.push(tile);
        }
        let size = self.root.frame().size;
        let columns = shown.len().clamp(1, COLUMNS);
        let rows = shown.len().div_ceil(COLUMNS).max(1);
        let grid_width = columns as f64 * TILE_WIDTH + (columns as f64 - 1.0) * GAP;
        let grid_height = rows as f64 * TILE_HEIGHT + (rows as f64 - 1.0) * GAP;
        let left = (size.width - grid_width) / 2.0;
        let top = (size.height - grid_height) / 2.0 + grid_height;
        for (i, button) in pool.iter().enumerate() {
            let Some(tile) = shown.get(i) else {
                button.setHidden(true);
                continue;
            };
            let (row, column) = (i / COLUMNS, i % COLUMNS);
            button.setFrame(NSRect::new(
                NSPoint::new(
                    left + column as f64 * (TILE_WIDTH + GAP),
                    top - (row as f64 + 1.0) * TILE_HEIGHT - row as f64 * GAP,
                ),
                NSSize::new(TILE_WIDTH, TILE_HEIGHT),
            ));
            button.setHidden(false);
            button.set_title(&tile.title);
            match &tile.icon {
                TileIcon::File(path) => {
                    button.set_symbol("");
                    button.set_image(Some(workspace::icon_of(path, 40.0)));
                }
                TileIcon::Symbol(name, tone) => {
                    button.set_image(None);
                    button.set_symbol(name);
                    button.set_tone(Some(*tone));
                }
            }
        }
        self.root.setNeedsDisplay(true);
    }

    /// The notch tab for this view.
    pub fn page(
        self: &Rc<Self>,
        title: String,
        on_show: impl Fn() + 'static,
        on_hide: impl Fn() + 'static,
    ) -> Page {
        Page {
            id: "launcher",
            title,
            symbol: "square.grid.2x2.fill",
            root: Retained::into_super(Retained::clone(&self.root)),
            on_show: Box::new(on_show),
            on_hide: Box::new(on_hide),
            on_key: Box::new(|_| false),
            accepts_files: false,
            size: None,
        }
    }
}
