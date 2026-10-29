//! The Launcher's tiles: a grid of the apps, folders, files, web addresses and Shortcuts the person chose, with a
//! strip of its groups above it, drawn in a tab of the notch and in the dock at an edge of a screen. Everything on
//! it is plain data from the app, so nothing here reads a setting or launches anything.

use crate::{
    app::{self, Timer},
    button::{Look, TouchButton},
    notch::{Material, Page, glass_tint, vibrancy_of},
    panel::{FigPanel, Key},
    style::Tone,
    widgets::{ActionTarget, draw_page_note},
    workspace::{self, display_options},
};
use block2::RcBlock;
use objc2::{
    DefinedClass, MainThreadMarker, MainThreadOnly, define_class, msg_send,
    rc::Retained,
    runtime::{AnyClass, AnyObject, ProtocolObject},
    sel,
};
/// The event a tile's menu is popped up with, named in `LauncherCallbacks`.
pub use objc2_app_kit::NSEvent;
use objc2_app_kit::{
    NSApplicationDidChangeScreenParametersNotification, NSAutoresizingMaskOptions, NSColor,
    NSEventPhase, NSFloatingWindowLevel, NSGlassEffectView, NSMenu, NSMenuItem, NSScreen, NSView,
    NSVisualEffectBlendingMode, NSVisualEffectState, NSVisualEffectView,
    NSWindowCollectionBehavior,
};
use objc2_foundation::{
    NSNotification, NSNotificationCenter, NSObjectProtocol, NSPoint, NSRect, NSSize, NSString,
};
use std::{
    cell::{Cell, RefCell},
    path::PathBuf,
    ptr::NonNull,
    rc::{Rc, Weak},
};

/// The size of the area the tab fills below the header.
pub const PAGE_SIZE: NSSize = NSSize::new(580.0, 214.0);
/// How many tiles a row of the tab holds, and how many tiles a group holds at most.
pub const COLUMNS: usize = 6;
pub const MAX_TILES: usize = 12;

const TILE_WIDTH: f64 = 84.0;
const TILE_HEIGHT: f64 = 88.0;
const GAP: f64 = 6.0;
/// The height of the strip of groups, which with two rows of tiles fills the tab.
const STRIP: f64 = 28.0;
/// How far two fingers travel sideways before the next group comes.
const SWIPE: f64 = 30.0;

/// What a tile shows above its name.
#[derive(Debug, Clone)]
pub enum TileIcon {
    /// The icon the Finder shows for this path: an app, a folder, a file.
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
    /// The names in the strip above the grid; no strip unless there are two or more.
    pub groups: Vec<String>,
    /// The group that shows.
    pub group: usize,
}

/// A line of a tile's menu. An empty title is a separator; a line that is not `enabled` only says something.
#[derive(Debug, Clone)]
pub struct MenuEntry {
    pub title: String,
    pub icon: Option<PathBuf>,
    pub enabled: bool,
}

impl MenuEntry {
    pub fn new(title: &str, icon: Option<PathBuf>) -> Self {
        Self {
            title: title.to_owned(),
            icon,
            enabled: true,
        }
    }

    pub fn note(title: &str) -> Self {
        Self {
            enabled: false,
            ..Self::new(title, None)
        }
    }

    pub fn separator() -> Self {
        Self::note("")
    }
}

pub struct LauncherCallbacks {
    /// The number of the tile that was pressed.
    pub on_open: Box<dyn Fn(usize)>,
    /// A tile was clicked with the right button: its number, and the event to pop the menu up with.
    pub on_menu: Box<OnMenu>,
    /// The group to show: a chip of the strip was pressed, or two fingers swiped, or the wheel or an arrow key moved.
    pub on_group: Box<dyn Fn(usize)>,
}

pub type OnMenu = dyn Fn(usize, &NSEvent);
type Step = Rc<dyn Fn(i32) -> bool>;
type RightClick = Rc<dyn Fn(NSPoint, &NSEvent)>;

pub struct RootIvars {
    note: RefCell<Option<(String, String)>>,
    on_step: RefCell<Option<Step>>,
    on_right: RefCell<Option<RightClick>>,
    /// How far two fingers have gone sideways in this swipe, and whether it already moved a group.
    swipe: Cell<f64>,
    swiped: Cell<bool>,
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

        // A swipe of two fingers moves one group, however long it is; the glide after it is not another swipe. The
        // wheel of a mouse moves one group a click.
        #[unsafe(method(scrollWheel:))]
        fn scroll_wheel(&self, event: &NSEvent) {
            let ivars = self.ivars();
            if !event.momentumPhase().is_empty() {
                return;
            }
            let (dx, dy) = (event.scrollingDeltaX(), event.scrollingDeltaY());
            let step = if event.hasPreciseScrollingDeltas() {
                let phase = event.phase();
                if phase.contains(NSEventPhase::Began) {
                    ivars.swipe.set(0.0);
                    ivars.swiped.set(false);
                }
                let mut step = 0;
                if !ivars.swiped.get() && dx.abs() > dy.abs() {
                    let total = ivars.swipe.get() + dx;
                    ivars.swipe.set(total);
                    if total.abs() >= SWIPE {
                        ivars.swiped.set(true);
                        step = if total < 0.0 { 1 } else { -1 };
                    }
                }
                if phase.intersects(NSEventPhase::Ended | NSEventPhase::Cancelled) {
                    ivars.swipe.set(0.0);
                    ivars.swiped.set(false);
                }
                step
            } else {
                let delta = if dy.abs() >= dx.abs() { dy } else { dx };
                if delta < 0.0 { 1 } else if delta > 0.0 { -1 } else { 0 }
            };
            let handler = ivars.on_step.borrow().clone();
            if step != 0 && let Some(handler) = handler {
                handler(step);
            }
        }

        #[unsafe(method(rightMouseDown:))]
        fn right_mouse_down(&self, event: &NSEvent) {
            let point = self.convertPoint_fromView(event.locationInWindow(), None);
            let handler = self.ivars().on_right.borrow().clone();
            if let Some(handler) = handler {
                handler(point, event);
            }
        }
    }
);

impl LauncherRoot {
    fn new(mtm: MainThreadMarker, frame: NSRect) -> Retained<Self> {
        let this = mtm.alloc::<Self>().set_ivars(RootIvars {
            note: RefCell::new(None),
            on_step: RefCell::new(None),
            on_right: RefCell::new(None),
            swipe: Cell::new(0.0),
            swiped: Cell::new(false),
        });
        // SAFETY: designated initializer with valid arguments on the main thread.
        unsafe { msg_send![super(this), initWithFrame: frame] }
    }
}

pub struct LauncherView {
    root: Retained<LauncherRoot>,
    tiles: Rc<RefCell<Vec<Retained<TouchButton>>>>,
    chips: RefCell<Vec<Retained<TouchButton>>>,
    on_open: Rc<dyn Fn(usize)>,
    on_group: Rc<dyn Fn(usize)>,
    /// The group that shows and how many there are, as last shown, for the swipe and the arrow keys.
    shown: Rc<Cell<(usize, usize)>>,
    columns: Cell<usize>,
    mtm: MainThreadMarker,
}

impl LauncherView {
    /// `frame` is `Notch::page_area(PAGE_SIZE.width, PAGE_SIZE.height)` for the tab.
    pub fn new(mtm: MainThreadMarker, frame: NSRect, callbacks: LauncherCallbacks) -> Rc<Self> {
        let LauncherCallbacks {
            on_open,
            on_menu,
            on_group,
        } = callbacks;
        let this = Rc::new(Self {
            root: LauncherRoot::new(mtm, frame),
            tiles: Rc::default(),
            chips: RefCell::default(),
            on_open: Rc::from(on_open),
            on_group: Rc::from(on_group),
            shown: Rc::new(Cell::new((0, 0))),
            columns: Cell::new(COLUMNS),
            mtm,
        });
        *this.root.ivars().on_step.borrow_mut() = Some(this.stepper());
        let tiles = Rc::clone(&this.tiles);
        *this.root.ivars().on_right.borrow_mut() = Some(Rc::new(move |point, event| {
            let hit = tiles
                .borrow()
                .iter()
                .position(|tile| !tile.isHidden() && contains(tile.frame(), point));
            if let Some(index) = hit {
                on_menu(index, event);
            }
        }));
        this
    }

    /// Moves the group that shows by `step`; false when there is no group that way.
    fn stepper(&self) -> Step {
        let (shown, on_group) = (Rc::clone(&self.shown), Rc::clone(&self.on_group));
        Rc::new(move |step| {
            let (group, count) = shown.get();
            let next = group as i64 + i64::from(step);
            if count < 2 || next < 0 || next >= count as i64 {
                return false;
            }
            on_group(next as usize);
            true
        })
    }

    /// How many tiles a row holds; the tab has six, the dock as many as its edge gives room for.
    pub fn set_columns(&self, columns: usize) {
        self.columns.set(columns.max(1));
    }

    pub fn root(&self) -> &NSView {
        &self.root
    }

    /// Draws what is now to show.
    pub fn show(&self, display: &LauncherDisplay) {
        *self.root.ivars().note.borrow_mut() = display.note.clone();
        // The strip stays over a note too, so that an empty group can be left for another.
        let grouped = display.groups.len() > 1;
        self.shown.set((display.group, display.groups.len()));
        let shown: &[Tile] = if display.note.is_some() {
            &[]
        } else {
            &display.tiles[..display.tiles.len().min(MAX_TILES)]
        };
        let size = self.root.frame().size;
        self.show_strip(
            if grouped { &display.groups } else { &[] },
            display.group,
            size,
        );

        let mut pool = self.tiles.borrow_mut();
        while pool.len() < shown.len() {
            let index = pool.len();
            let tile = TouchButton::new(self.mtm, NSRect::ZERO, Look::Tile);
            let open = Rc::clone(&self.on_open);
            tile.set_on_click(move || open(index));
            self.root.addSubview(&tile);
            pool.push(tile);
        }
        let per_row = self.columns.get();
        let columns = shown.len().clamp(1, per_row);
        let rows = shown.len().div_ceil(per_row).max(1);
        let grid_width = columns as f64 * TILE_WIDTH + (columns as f64 - 1.0) * GAP;
        let grid_height = rows as f64 * TILE_HEIGHT + (rows as f64 - 1.0) * GAP;
        let area = size.height - if grouped { STRIP + GAP } else { 0.0 };
        let left = (size.width - grid_width) / 2.0;
        let top = (area - grid_height) / 2.0 + grid_height;
        for (i, button) in pool.iter().enumerate() {
            let Some(tile) = shown.get(i) else {
                button.setHidden(true);
                continue;
            };
            let (row, column) = (i / per_row, i % per_row);
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

    /// The chips of the groups along the top, centred; when their names do not fit, each gets an equal share.
    fn show_strip(&self, names: &[String], group: usize, size: NSSize) {
        let mut chips = self.chips.borrow_mut();
        while chips.len() < names.len() {
            let index = chips.len();
            let chip = TouchButton::new(self.mtm, NSRect::ZERO, Look::Chip);
            let pick = Rc::clone(&self.on_group);
            chip.set_on_click(move || pick(index));
            self.root.addSubview(&chip);
            chips.push(chip);
        }
        for (i, chip) in chips.iter().enumerate() {
            chip.setHidden(i >= names.len());
            if let Some(name) = names.get(i) {
                chip.set_title(name);
                chip.set_selected(i == group);
            }
        }
        let shown = &chips[..names.len()];
        let room = size.width - 2.0 * GAP - GAP * (shown.len() as f64 - 1.0).max(0.0);
        let mut widths: Vec<f64> = shown.iter().map(|c| c.fit_width()).collect();
        if widths.iter().sum::<f64>() > room {
            let share = room / shown.len().max(1) as f64;
            widths.iter_mut().for_each(|w| *w = w.min(share));
        }
        let total: f64 = widths.iter().sum::<f64>() + GAP * (shown.len() as f64 - 1.0).max(0.0);
        let mut left = (size.width - total) / 2.0;
        for (chip, width) in shown.iter().zip(widths) {
            chip.setFrame(NSRect::new(
                NSPoint::new(left, size.height - STRIP),
                NSSize::new(width, STRIP),
            ));
            left += width + GAP;
        }
    }

    /// Pops up a tile's menu where the right button was pressed. The chosen line runs on the next turn of the run
    /// loop, when the menu is certainly gone, so that what it opens never opens under the menu.
    pub fn pop_up(
        &self,
        entries: &[MenuEntry],
        event: &NSEvent,
        on_pick: impl Fn(usize) + 'static,
    ) {
        let mtm = self.mtm;
        let menu = NSMenu::new(mtm);
        menu.setAutoenablesItems(false);
        let on_pick: Rc<dyn Fn(usize)> = Rc::new(on_pick);
        // Kept until the menu is gone: a menu item does not retain its target.
        let mut targets = Vec::new();
        for (index, entry) in entries.iter().enumerate() {
            if entry.title.is_empty() {
                menu.addItem(&NSMenuItem::separatorItem(mtm));
                continue;
            }
            // SAFETY: `fire:` is implemented by ActionTarget, which outlives the menu (see above).
            let item = unsafe {
                NSMenuItem::initWithTitle_action_keyEquivalent(
                    NSMenuItem::alloc(mtm),
                    &NSString::from_str(&entry.title),
                    entry.enabled.then_some(sel!(fire:)),
                    &NSString::from_str(""),
                )
            };
            item.setEnabled(entry.enabled);
            if let Some(path) = &entry.icon {
                item.setImage(Some(&workspace::icon_of(path, 16.0)));
            }
            if entry.enabled {
                let pick = Rc::clone(&on_pick);
                let target = ActionTarget::new(mtm, move || {
                    let pick = Rc::clone(&pick);
                    app::after(0.0, move || pick(index));
                });
                let object: &AnyObject = &target;
                // SAFETY: the target outlives the menu (see above).
                unsafe { item.setTarget(Some(object)) };
                targets.push(target);
            }
            menu.addItem(&item);
        }
        NSMenu::popUpContextMenu_withEvent_forView(&menu, event, &self.root);
        drop(targets);
    }

    /// The notch tab for this view. The arrow keys move between the groups.
    pub fn page(
        self: &Rc<Self>,
        title: String,
        on_show: impl Fn() + 'static,
        on_hide: impl Fn() + 'static,
    ) -> Page {
        let step = self.stepper();
        Page {
            id: "launcher",
            title,
            symbol: "square.grid.2x2.fill",
            root: Retained::into_super(Retained::clone(&self.root)),
            on_show: Box::new(on_show),
            on_hide: Box::new(on_hide),
            on_key: Box::new(move |key| match key {
                Key::Left => step(-1),
                Key::Right => step(1),
                _ => false,
            }),
            accepts_files: false,
            size: None,
        }
    }
}

fn contains(frame: NSRect, point: NSPoint) -> bool {
    point.x >= frame.origin.x
        && point.x <= frame.origin.x + frame.size.width
        && point.y >= frame.origin.y
        && point.y <= frame.origin.y + frame.size.height
}

/// The edge of a screen the dock sits on. The top edge is the menu bar's and the notch's.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Edge {
    Left,
    Right,
    Bottom,
}

const DOCK_PAD: f64 = 10.0;
/// Between the dock and the edge of the part of the screen that windows use.
const DOCK_MARGIN: f64 = 6.0;
const DOCK_RADIUS: f64 = 22.0;
/// The size of the dock while it only says something: nothing is pinned in its group.
const NOTE_SIZE: NSSize = NSSize::new(280.0, 200.0);
/// How near the edge of the screen the pointer must come to bring a hidden dock out, how far beyond the dock it may
/// wander, and for how many looks, a tenth of a second apart, before the dock hides again.
const REACH: f64 = 2.0;
const SLACK: f64 = 30.0;
const AWAY_LOOKS: u32 = 6;

/// How many tiles a row of the dock holds: one or two columns on a side, so that six rows at most stand on it; along
/// the bottom, all of them in one row when the screen is wide enough.
pub fn dock_columns(edge: Edge, tiles: usize, width: f64) -> usize {
    let tiles = tiles.clamp(1, MAX_TILES);
    match edge {
        Edge::Left | Edge::Right => tiles.div_ceil(COLUMNS),
        Edge::Bottom => {
            let fits =
                ((width - 2.0 * (DOCK_PAD + DOCK_MARGIN) + GAP) / (TILE_WIDTH + GAP)).floor();
            if tiles as f64 <= fits {
                tiles
            } else {
                tiles.min(COLUMNS)
            }
        }
    }
}

/// The size of the dock for `tiles` in rows of `columns`, with the strip of groups when `strip`.
pub fn dock_size(tiles: usize, columns: usize, strip: bool) -> NSSize {
    let columns = columns.max(1);
    let rows = tiles.clamp(1, MAX_TILES).div_ceil(columns);
    let shown = tiles.clamp(1, columns);
    NSSize::new(
        shown as f64 * TILE_WIDTH + (shown as f64 - 1.0) * GAP + 2.0 * DOCK_PAD,
        rows as f64 * TILE_HEIGHT
            + (rows as f64 - 1.0) * GAP
            + 2.0 * DOCK_PAD
            + if strip { STRIP + GAP } else { 0.0 },
    )
}

/// The Launcher on Liquid Glass at an edge of a screen, above the windows and below the menus. It never takes the
/// keyboard from the app in front, and with `autohide` it waits off the screen until the pointer reaches its edge.
pub struct DockPanel {
    mtm: MainThreadMarker,
    panel: Retained<FigPanel>,
    view: Rc<LauncherView>,
    /// Whether the content sits on an opaque shape (Increase Contrast, Reduce Transparency), as last dressed.
    solid: Cell<Option<bool>>,
    edge: Cell<Edge>,
    /// The frame of the whole screen the dock is on, for the pointer to reach its edge.
    screen: Cell<NSRect>,
    away: Cell<u32>,
    watch: RefCell<Option<Timer>>,
    /// What was last shown and where, to put it back in place when a screen comes, goes or changes.
    last: RefCell<Option<(LauncherDisplay, Edge, String, bool)>>,
    screens: RefCell<Option<Retained<ProtocolObject<dyn NSObjectProtocol>>>>,
}

impl Drop for DockPanel {
    fn drop(&mut self) {
        if let Some(observer) = self.screens.borrow_mut().take() {
            // SAFETY: the observer came from addObserverForName on the default centre and is removed once.
            unsafe {
                NSNotificationCenter::defaultCenter()
                    .removeObserver(AsRef::<AnyObject>::as_ref(&*observer))
            };
        }
        self.panel.orderOut(None);
    }
}

impl DockPanel {
    pub fn new(mtm: MainThreadMarker, callbacks: LauncherCallbacks) -> Rc<Self> {
        let frame = NSRect::new(NSPoint::ZERO, NOTE_SIZE);
        let panel = FigPanel::new(mtm, frame);
        panel.setHasShadow(true);
        panel.setLevel(NSFloatingWindowLevel);
        // On every desktop, but not over an app that is full screen.
        panel.setCollectionBehavior(
            NSWindowCollectionBehavior::CanJoinAllSpaces
                | NSWindowCollectionBehavior::Stationary
                | NSWindowCollectionBehavior::IgnoresCycle,
        );
        let view = LauncherView::new(mtm, frame, callbacks);
        view.root.setAutoresizingMask(
            NSAutoresizingMaskOptions::ViewWidthSizable
                | NSAutoresizingMaskOptions::ViewHeightSizable,
        );
        let this = Rc::new(Self {
            mtm,
            panel,
            view,
            solid: Cell::new(None),
            edge: Cell::new(Edge::Left),
            screen: Cell::new(NSRect::ZERO),
            away: Cell::new(0),
            watch: RefCell::default(),
            last: RefCell::default(),
            screens: RefCell::default(),
        });
        // A screen that is unplugged and plugged back (a KVM switch, a lid closed) moves the dock back to its edge.
        let me = Rc::downgrade(&this);
        let block = RcBlock::new(move |_: NonNull<NSNotification>| {
            if let Some(dock) = me.upgrade() {
                let last = dock.last.borrow().clone();
                if let Some((display, edge, screen, autohide)) = last {
                    dock.show(&display, edge, &screen, autohide);
                }
            }
        });
        // SAFETY: the block is 'static and runs on the main thread, where the notification is posted.
        let observer = unsafe {
            NSNotificationCenter::defaultCenter().addObserverForName_object_queue_usingBlock(
                Some(NSApplicationDidChangeScreenParametersNotification),
                None,
                None,
                &block,
            )
        };
        *this.screens.borrow_mut() = Some(observer);
        this
    }

    pub fn view(&self) -> &Rc<LauncherView> {
        &self.view
    }

    /// Where the dock is, in the coordinates of the screens, and whether it shows now.
    pub fn frame(&self) -> NSRect {
        self.panel.frame()
    }

    pub fn is_visible(&self) -> bool {
        self.panel.isVisible()
    }

    /// Puts the glass behind the tiles. Under Increase Contrast or Reduce Transparency the desktop must not show
    /// through, so the dock is an opaque dark shape; before macOS 26 there is no Liquid Glass and a dark vibrancy
    /// material stands in.
    fn dress(&self) {
        let options = display_options();
        let solid = options.reduce_transparency || options.increase_contrast;
        if self.solid.replace(Some(solid)) == Some(solid) {
            return;
        }
        let size = NSRect::new(NSPoint::ZERO, self.panel.frame().size);
        let sizable = NSAutoresizingMaskOptions::ViewWidthSizable
            | NSAutoresizingMaskOptions::ViewHeightSizable;
        self.view.root.removeFromSuperview();
        let glass = glass_tint(Material::Glass)
            .filter(|_| !solid && AnyClass::get(c"NSGlassEffectView").is_some());
        if let Some((look, tint)) = glass {
            let glass = NSGlassEffectView::initWithFrame(self.mtm.alloc(), size);
            glass.setStyle(look);
            glass.setTintColor(Some(&tint));
            glass.setCornerRadius(DOCK_RADIUS);
            glass.setAutoresizingMask(sizable);
            self.view.root.setFrame(size);
            glass.setContentView(Some(&self.view.root));
            self.panel.setContentView(Some(&glass));
            return;
        }
        let base: Retained<NSView> = if solid {
            let plain = NSView::initWithFrame(self.mtm.alloc(), size);
            plain.setWantsLayer(true);
            if let Some(layer) = plain.layer() {
                let fill = NSColor::colorWithSRGBRed_green_blue_alpha(0.11, 0.11, 0.13, 1.0);
                // SAFETY: plain property setter on the view's backing layer.
                unsafe {
                    let _: () = msg_send![&*layer, setBackgroundColor: &*fill.CGColor()];
                }
            }
            plain
        } else {
            let effect = NSVisualEffectView::initWithFrame(self.mtm.alloc(), size);
            effect.setMaterial(vibrancy_of(Material::Glass));
            effect.setBlendingMode(NSVisualEffectBlendingMode::BehindWindow);
            effect.setState(NSVisualEffectState::Active);
            effect.setWantsLayer(true);
            Retained::into_super(effect)
        };
        if let Some(layer) = base.layer() {
            // SAFETY: plain property setters on the view's backing layer.
            unsafe {
                let _: () = msg_send![&*layer, setCornerRadius: DOCK_RADIUS];
                let _: () = msg_send![&*layer, setMasksToBounds: true];
            }
        }
        base.setAutoresizingMask(sizable);
        self.view.root.setFrame(size);
        base.addSubview(&self.view.root);
        self.panel.setContentView(Some(&base));
    }

    /// Shows `display` at `edge` of the screen called `screen` (the main screen when there is none of that name, or
    /// none is named). With `autohide` it shows only once the pointer reaches that edge.
    pub fn show(
        self: &Rc<Self>,
        display: &LauncherDisplay,
        edge: Edge,
        screen: &str,
        autohide: bool,
    ) {
        *self.last.borrow_mut() = Some((display.clone(), edge, screen.to_owned(), autohide));
        let screens = NSScreen::screens(self.mtm);
        let Some(target) = screens
            .iter()
            .find(|s| !screen.is_empty() && s.localizedName().to_string() == screen)
            .or_else(|| screens.firstObject())
        else {
            return;
        };
        let area = target.visibleFrame();
        self.screen.set(target.frame());
        self.edge.set(edge);
        let tiles = display.tiles.len().min(MAX_TILES);
        let strip = display.groups.len() > 1;
        let size = if display.note.is_some() {
            NOTE_SIZE
        } else {
            let columns = dock_columns(edge, tiles, area.size.width);
            self.view.set_columns(columns);
            dock_size(tiles, columns, strip)
        };
        let origin = match edge {
            Edge::Left => NSPoint::new(
                area.origin.x + DOCK_MARGIN,
                area.origin.y + (area.size.height - size.height) / 2.0,
            ),
            Edge::Right => NSPoint::new(
                area.origin.x + area.size.width - size.width - DOCK_MARGIN,
                area.origin.y + (area.size.height - size.height) / 2.0,
            ),
            Edge::Bottom => NSPoint::new(
                area.origin.x + (area.size.width - size.width) / 2.0,
                area.origin.y + DOCK_MARGIN,
            ),
        };
        self.panel.setFrame_display(NSRect::new(origin, size), true);
        self.dress();
        self.view.root.setFrame(NSRect::new(NSPoint::ZERO, size));
        self.view.show(display);
        if !autohide {
            self.watch.borrow_mut().take();
            self.panel.orderFrontRegardless();
            return;
        }
        if self.watch.borrow().is_none() {
            self.panel.orderOut(None);
            let me = Rc::downgrade(self);
            // ponytail: looks at the pointer ten times a second while the dock hides; a tracking area on a thin
            // window along the edge would cost nothing while the pointer is elsewhere, if this ever shows up.
            *self.watch.borrow_mut() = Some(app::every(0.1, move || {
                if let Some(dock) = Weak::upgrade(&me) {
                    dock.look();
                }
            }));
        }
    }

    /// One look at the pointer while the dock hides itself: out when the pointer is at the edge next to it, away
    /// again a moment after the pointer left it.
    fn look(&self) {
        let pointer = NSEvent::mouseLocation();
        let frame = self.panel.frame();
        if self.panel.isVisible() {
            let near = contains(
                NSRect::new(
                    NSPoint::new(frame.origin.x - SLACK, frame.origin.y - SLACK),
                    NSSize::new(
                        frame.size.width + 2.0 * SLACK,
                        frame.size.height + 2.0 * SLACK,
                    ),
                ),
                pointer,
            );
            let away = if near { 0 } else { self.away.get() + 1 };
            self.away.set(away);
            if away >= AWAY_LOOKS {
                self.panel.orderOut(None);
            }
            return;
        }
        if reaches(self.edge.get(), self.screen.get(), frame, pointer) {
            self.away.set(0);
            self.panel.orderFrontRegardless();
        }
    }

    /// Takes the dock off the screen and stops looking at the pointer.
    pub fn hide(&self) {
        self.last.borrow_mut().take();
        self.watch.borrow_mut().take();
        self.panel.orderOut(None);
    }
}

/// Whether the pointer is at `edge` of `screen`, beside where the dock stands.
fn reaches(edge: Edge, screen: NSRect, dock: NSRect, pointer: NSPoint) -> bool {
    let (left, bottom) = (screen.origin.x, screen.origin.y);
    let (right, top) = (left + screen.size.width, bottom + screen.size.height);
    let beside_y = pointer.y >= dock.origin.y && pointer.y <= dock.origin.y + dock.size.height;
    let beside_x = pointer.x >= dock.origin.x && pointer.x <= dock.origin.x + dock.size.width;
    let within_x = pointer.x >= left && pointer.x < right;
    let within_y = pointer.y >= bottom && pointer.y < top;
    match edge {
        Edge::Left => within_x && pointer.x <= left + REACH && beside_y,
        Edge::Right => within_x && pointer.x >= right - 1.0 - REACH && beside_y,
        Edge::Bottom => within_y && pointer.y <= bottom + REACH && beside_x,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_dock_on_a_side_stands_six_rows_at_most_and_along_the_bottom_one_row_when_it_fits() {
        assert_eq!(dock_columns(Edge::Left, 1, 1920.0), 1);
        assert_eq!(dock_columns(Edge::Right, 6, 1920.0), 1);
        assert_eq!(dock_columns(Edge::Left, 7, 1920.0), 2);
        assert_eq!(dock_columns(Edge::Left, 12, 1920.0), 2);
        assert_eq!(dock_columns(Edge::Bottom, 12, 1920.0), 12);
        assert_eq!(
            dock_columns(Edge::Bottom, 12, 800.0),
            6,
            "a narrow screen takes two rows"
        );
        assert_eq!(dock_columns(Edge::Bottom, 0, 1920.0), 1);
        let size = dock_size(7, 2, false);
        assert_eq!(size.width, 2.0 * TILE_WIDTH + GAP + 2.0 * DOCK_PAD);
        assert_eq!(size.height, 4.0 * TILE_HEIGHT + 3.0 * GAP + 2.0 * DOCK_PAD);
        assert_eq!(
            dock_size(3, 12, true).width,
            3.0 * TILE_WIDTH + 2.0 * GAP + 2.0 * DOCK_PAD
        );
        assert_eq!(
            dock_size(3, 12, true).height,
            TILE_HEIGHT + 2.0 * DOCK_PAD + STRIP + GAP
        );
    }

    #[test]
    fn the_pointer_brings_the_dock_out_only_at_its_edge_beside_it() {
        let screen = NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(1920.0, 1080.0));
        let dock = NSRect::new(NSPoint::new(6.0, 300.0), NSSize::new(104.0, 500.0));
        assert!(reaches(Edge::Left, screen, dock, NSPoint::new(0.0, 500.0)));
        assert!(
            !reaches(Edge::Left, screen, dock, NSPoint::new(40.0, 500.0)),
            "not at the edge"
        );
        assert!(
            !reaches(Edge::Left, screen, dock, NSPoint::new(0.0, 100.0)),
            "below the dock"
        );
        assert!(
            !reaches(Edge::Left, screen, dock, NSPoint::new(-1.0, 500.0)),
            "on the screen to the left"
        );
        let right = NSRect::new(NSPoint::new(1810.0, 300.0), NSSize::new(104.0, 500.0));
        assert!(reaches(
            Edge::Right,
            screen,
            right,
            NSPoint::new(1919.0, 500.0)
        ));
        let bottom = NSRect::new(NSPoint::new(400.0, 80.0), NSSize::new(1100.0, 108.0));
        assert!(reaches(
            Edge::Bottom,
            screen,
            bottom,
            NSPoint::new(900.0, 0.0)
        ));
        assert!(!reaches(
            Edge::Bottom,
            screen,
            bottom,
            NSPoint::new(100.0, 0.0)
        ));
    }
}
