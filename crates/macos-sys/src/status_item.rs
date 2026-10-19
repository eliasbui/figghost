//! Menu bar status item: the ghost of the logo with a menu. Titles, check marks and visibility are recomputed
//! every time the menu opens, so entries can reflect the frontmost app, permissions and login-item state
//! without any bookkeeping.

use block2::RcBlock;
use objc2::{
    DefinedClass, MainThreadMarker, MainThreadOnly, define_class, msg_send,
    rc::Retained,
    runtime::{AnyObject, Bool, ProtocolObject},
    sel,
};
use objc2_app_kit::{
    NSAccessibility, NSApplication, NSBezierPath, NSColor, NSEventMask, NSEventType, NSControlStateValueOff, NSControlStateValueOn, NSImage,
    NSMenu, NSMenuDelegate, NSMenuItem, NSStatusBar, NSStatusItem, NSVariableStatusItemLength,
    NSWindingRule,
};
use objc2_foundation::{
    NSAffineTransform, NSObject, NSObjectProtocol, NSPoint, NSRect, NSSize, NSString,
};
use crate::webview::TrayPanel;
use serde_json::{Value, json};
use std::{cell::RefCell, rc::Rc};

type Predicate = Box<dyn Fn() -> bool>;

pub struct Item {
    title: Box<dyn Fn() -> String>,
    symbol: Option<&'static str>,
    checked: Option<Predicate>,
    visible: Option<Predicate>,
    action: Box<dyn Fn()>,
}

impl Item {
    pub fn new(title: &str, action: impl Fn() + 'static) -> Self {
        let title = title.to_owned();
        Self {
            title: Box::new(move || title.clone()),
            symbol: None,
            checked: None,
            visible: None,
            action: Box::new(action),
        }
    }

    /// Gives the item the SF Symbol `name` as its icon, drawn before the title.
    pub fn symbol(mut self, name: &'static str) -> Self {
        self.symbol = Some(name);
        self
    }

    /// Replaces the fixed title with one computed when the menu opens.
    pub fn titled(mut self, title: impl Fn() -> String + 'static) -> Self {
        self.title = Box::new(title);
        self
    }

    /// Gives the item a check mark that follows `checked` each time the menu opens.
    pub fn checked(mut self, checked: impl Fn() -> bool + 'static) -> Self {
        self.checked = Some(Box::new(checked));
        self
    }

    pub fn visible_when(mut self, visible: impl Fn() -> bool + 'static) -> Self {
        self.visible = Some(Box::new(visible));
        self
    }
}

pub enum Entry {
    Item(Item),
    Separator,
    /// A titled submenu with an optional SF Symbol; its entries are refreshed with the rest when the menu
    /// opens, and it is hidden while all of them are.
    Submenu {
        title: String,
        symbol: Option<&'static str>,
        entries: Vec<Entry>,
    },
}

pub struct TargetIvars {
    action: Rc<Item>,
}

define_class!(
    // SAFETY: NSObject allows subclassing; the ivars are only touched on the main thread and need no Drop.
    #[unsafe(super(NSObject))]
    #[thread_kind = MainThreadOnly]
    #[name = "FigMenuTarget"]
    #[ivars = TargetIvars]
    struct MenuTarget;

    impl MenuTarget {
        #[unsafe(method(fire:))]
        fn fire(&self, _sender: &NSMenuItem) {
            (self.ivars().action.action)();
        }
    }
);

impl MenuTarget {
    fn new(mtm: MainThreadMarker, item: Rc<Item>) -> Retained<Self> {
        let this = mtm.alloc::<Self>().set_ivars(TargetIvars { action: item });
        // SAFETY: NSObject's designated initializer.
        unsafe { msg_send![super(this), init] }
    }
}

pub struct DelegateIvars {
    on_open: Box<dyn Fn()>,
}

define_class!(
    // SAFETY: NSObject allows subclassing; the ivars are only touched on the main thread and need no Drop.
    #[unsafe(super(NSObject))]
    #[thread_kind = MainThreadOnly]
    #[name = "FigMenuDelegate"]
    #[ivars = DelegateIvars]
    struct MenuDelegate;

    unsafe impl NSObjectProtocol for MenuDelegate {}

    unsafe impl NSMenuDelegate for MenuDelegate {
        #[unsafe(method(menuNeedsUpdate:))]
        fn menu_needs_update(&self, _menu: &NSMenu) {
            (self.ivars().on_open)();
        }
    }
);

impl MenuDelegate {
    fn new(mtm: MainThreadMarker, on_open: impl Fn() + 'static) -> Retained<Self> {
        let this = mtm.alloc::<Self>().set_ivars(DelegateIvars {
            on_open: Box::new(on_open),
        });
        // SAFETY: NSObject's designated initializer.
        unsafe { msg_send![super(this), init] }
    }
}

pub struct ClickIvars {
    on_click: Box<dyn Fn()>,
}

define_class!(
    // SAFETY: NSObject allows subclassing; the ivars are only touched on the main thread and need no Drop.
    #[unsafe(super(NSObject))]
    #[thread_kind = MainThreadOnly]
    #[name = "FigStatusClick"]
    #[ivars = ClickIvars]
    struct ClickTarget;

    impl ClickTarget {
        #[unsafe(method(clicked:))]
        fn clicked(&self, _sender: &AnyObject) {
            (self.ivars().on_click)();
        }
    }
);

impl ClickTarget {
    fn new(mtm: MainThreadMarker, on_click: impl Fn() + 'static) -> Retained<Self> {
        let this = mtm.alloc::<Self>().set_ivars(ClickIvars {
            on_click: Box::new(on_click),
        });
        // SAFETY: NSObject's designated initializer.
        unsafe { msg_send![super(this), init] }
    }
}

type Submenus = Vec<(Retained<NSMenuItem>, Retained<NSMenu>)>;

/// A menu with `entries`; the targets, the items to refresh and the submenus are collected for the status
/// menu. A submenu is collected after the submenus inside it.
fn fill(
    mtm: MainThreadMarker,
    entries: Vec<Entry>,
    targets: &mut Vec<Retained<MenuTarget>>,
    pairs: &mut Vec<(Retained<NSMenuItem>, Rc<Item>)>,
    submenus: &mut Submenus,
) -> Retained<NSMenu> {
    let menu = NSMenu::new(mtm);
    // The menu must not auto-enable items: our targets are not in the responder chain.
    menu.setAutoenablesItems(false);
    for entry in entries {
        let spec = match entry {
            Entry::Separator => {
                menu.addItem(&NSMenuItem::separatorItem(mtm));
                continue;
            }
            Entry::Submenu {
                title,
                symbol,
                entries,
            } => {
                let item = NSMenuItem::new(mtm);
                item.setTitle(&NSString::from_str(&title));
                set_symbol(&item, symbol);
                let submenu = fill(mtm, entries, targets, pairs, submenus);
                item.setSubmenu(Some(&submenu));
                menu.addItem(&item);
                submenus.push((item, submenu));
                continue;
            }
            Entry::Item(spec) => Rc::new(spec),
        };
        let target = MenuTarget::new(mtm, Rc::clone(&spec));
        // SAFETY: `fire:` is implemented by MenuTarget with the (id) signature AppKit expects.
        let menu_item = unsafe {
            NSMenuItem::initWithTitle_action_keyEquivalent(
                NSMenuItem::alloc(mtm),
                &NSString::from_str(""),
                Some(sel!(fire:)),
                &NSString::from_str(""),
            )
        };
        // SAFETY: the target is kept alive in `targets` for as long as the menu.
        unsafe { menu_item.setTarget(Some(&target)) };
        set_symbol(&menu_item, spec.symbol);
        menu.addItem(&menu_item);
        targets.push(target);
        pairs.push((menu_item, spec));
    }
    menu
}

/// The size of the menu bar icon in points: 18 is the height the menu bar gives a template image, and the
/// width keeps the shape of the ghost.
const ICON_WIDTH: f64 = 13.0;
const ICON_HEIGHT: f64 = 18.0;
/// Points per unit of the logo's 24 by 24 box: the ghost, 15 units wide, comes to the 13 points of the icon
/// and 16 in height, so its sides and its top fall on whole pixels at 1x and 2x.
const ICON_SCALE: f64 = 13.0 / 15.0;
/// `NSMenuItemImageVisibilityVisible`.
const IMAGE_VISIBLE: isize = 1;

/// The ghost of the logo as the settings page draws it, in the units of its 24 by 24 box with y growing
/// downward. The two eyes are part of the path: the even-odd rule leaves them out of the fill.
fn ghost_path() -> Retained<NSBezierPath> {
    let at = NSPoint::new;
    let path = NSBezierPath::bezierPath();
    path.moveToPoint(at(12.0, 2.5));
    path.relativeCurveToPoint_controlPoint1_controlPoint2(
        at(-7.5, 7.3),
        at(-4.2, 0.0),
        at(-7.5, 3.2),
    );
    path.lineToPoint(at(4.5, 20.0));
    path.relativeCurveToPoint_controlPoint1_controlPoint2(at(1.5, 0.7), at(0.0, 0.8), at(0.9, 1.2));
    // The wavy hem: four rounded feet with a notch between each two.
    path.relativeLineToPoint(at(1.6, -1.4));
    path.relativeLineToPoint(at(1.6, 1.4));
    path.relativeCurveToPoint_controlPoint1_controlPoint2(at(1.7, 0.0), at(0.5, 0.4), at(1.2, 0.4));
    path.relativeLineToPoint(at(1.1, -1.0));
    path.relativeLineToPoint(at(1.1, 1.0));
    path.relativeCurveToPoint_controlPoint1_controlPoint2(at(1.7, 0.0), at(0.5, 0.4), at(1.2, 0.4));
    path.relativeLineToPoint(at(1.6, -1.4));
    path.relativeLineToPoint(at(1.6, 1.4));
    path.relativeCurveToPoint_controlPoint1_controlPoint2(
        at(1.5, -0.7),
        at(0.6, 0.5),
        at(1.5, 0.1),
    );
    path.lineToPoint(at(19.5, 9.8));
    path.relativeCurveToPoint_controlPoint1_controlPoint2(
        at(-7.5, -7.3),
        at(0.0, -4.1),
        at(-3.3, -7.3),
    );
    path.closePath();
    for x in [9.2, 14.8] {
        path.appendBezierPathWithOvalInRect(NSRect::new(
            at(x - 1.5, 10.2 - 1.5),
            NSSize::new(3.0, 3.0),
        ));
    }
    path.setWindingRule(NSWindingRule::EvenOdd);
    path
}

/// The ghost as the icon of the menu bar item. A template image: the menu bar reads only its shape and paints
/// it for a light or a dark bar and for the item being highlighted. Drawn by code, it is sharp at any scale.
pub fn ghost_icon() -> Retained<NSImage> {
    let draw = RcBlock::new(|_: NSRect| {
        let path = ghost_path();
        let place = NSAffineTransform::transform();
        // The middle of the ghost goes to the middle of the icon and its top to a point below the top edge.
        place.translateXBy_yBy(ICON_WIDTH / 2.0 - 12.0 * ICON_SCALE, 1.0 - 2.5 * ICON_SCALE);
        place.scaleBy(ICON_SCALE);
        path.transformUsingAffineTransform(&place);
        NSColor::blackColor().setFill();
        path.fill();
        Bool::YES
    });
    // Flipped, so that the path keeps the downward y of the SVG it comes from.
    let icon = NSImage::imageWithSize_flipped_drawingHandler(
        NSSize::new(ICON_WIDTH, ICON_HEIGHT),
        true,
        &draw,
    );
    icon.setTemplate(true);
    icon
}

/// The SF Symbol `name` as the icon of a menu entry, or `None` for a name the system does not know. The menu
/// draws it at its own size, in the colour of the text, also while the entry is highlighted, which is what a
/// template image is for; a symbol already is one, and this keeps it so.
pub fn menu_symbol(name: &str) -> Option<Retained<NSImage>> {
    let image = NSImage::imageWithSystemSymbolName_accessibilityDescription(
        &NSString::from_str(name),
        None,
    )?;
    image.setTemplate(true);
    Some(image)
}

/// Gives a menu entry the icon `symbol` names, if it names one. From macOS 27 the menu hides the images of its
/// items unless each item asks for them to be visible; the bindings predate that call, so it is sent only to a
/// system that knows it.
fn set_symbol(item: &NSMenuItem, symbol: Option<&str>) {
    let Some(image) = symbol.and_then(menu_symbol) else {
        return;
    };
    item.setImage(Some(&image));
    if item.respondsToSelector(sel!(setPreferredImageVisibility:)) {
        // SAFETY: the system has the selector, which takes an NSMenuItemImageVisibility, an NSInteger.
        unsafe { msg_send![item, setPreferredImageVisibility: IMAGE_VISIBLE] }
    }
}

/// Keeps the status item, its menu and the action targets alive.
pub struct StatusMenu {
    item: Retained<NSStatusItem>,
    _menu: Retained<NSMenu>,
    _targets: Vec<Retained<MenuTarget>>,
    _delegate: Retained<MenuDelegate>,
    _click: Retained<ClickTarget>,
    _tray: Rc<TrayPanel>,
}

impl StatusMenu {
    /// The item shows the ghost; `label` is what it is called for assistive technology and in its tooltip. A
    /// click opens the panel that `tray_html` draws from the entries; a right click opens the plain menu.
    pub fn new(mtm: MainThreadMarker, label: &str, entries: Vec<Entry>, tray_html: &str) -> Self {
        let item = NSStatusBar::systemStatusBar().statusItemWithLength(NSVariableStatusItemLength);
        let button = item.button(mtm);
        if let Some(button) = &button {
            button.setImage(Some(&ghost_icon()));
            button.setToolTip(Some(&NSString::from_str(label)));
            button.setAccessibilityLabel(Some(&NSString::from_str(label)));
        }
        let mut targets = Vec::new();
        let mut pairs: Vec<(Retained<NSMenuItem>, Rc<Item>)> = Vec::new();
        let mut submenus = Submenus::new();
        let menu = fill(mtm, entries, &mut targets, &mut pairs, &mut submenus);
        let pairs = Rc::new(pairs);

        let refresh = {
            let pairs = Rc::clone(&pairs);
            move || {
                for (menu_item, spec) in pairs.iter() {
                    menu_item.setTitle(&NSString::from_str(&(spec.title)()));
                    if let Some(checked) = &spec.checked {
                        menu_item.setState(if checked() {
                            NSControlStateValueOn
                        } else {
                            NSControlStateValueOff
                        });
                    }
                    if let Some(visible) = &spec.visible {
                        menu_item.setHidden(!visible());
                    }
                }
                // Inner submenus come first, so an outer one sees them already hidden or shown.
                for (item, submenu) in &submenus {
                    let empty = submenu
                        .itemArray()
                        .iter()
                        .all(|i| i.isHidden() || i.isSeparatorItem());
                    item.setHidden(empty);
                }
            }
        };
        refresh();
        let delegate = MenuDelegate::new(mtm, refresh);
        menu.setDelegate(Some(ProtocolObject::from_ref(&*delegate)));

        // The panel is told every entry as it is now, each time it opens and after a switch is flipped.
        let tray_cell: Rc<RefCell<Option<Rc<TrayPanel>>>> = Rc::default();
        let post_state = {
            let (pairs, cell) = (Rc::clone(&pairs), Rc::clone(&tray_cell));
            Rc::new(move || {
                let Some(tray) = cell.borrow().clone() else {
                    return;
                };
                let items: Vec<Value> = pairs
                    .iter()
                    .enumerate()
                    .map(|(id, (_, spec))| {
                        json!({
                            "id": id,
                            "title": (spec.title)(),
                            "symbol": spec.symbol,
                            "checked": spec.checked.as_ref().map(|c| c()),
                            "visible": spec.visible.as_ref().is_none_or(|v| v()),
                        })
                    })
                    .collect();
                tray.host.post(&json!({ "t": "state", "items": items }).to_string());
            })
        };
        let on_message = {
            let (pairs, cell, post_state) =
                (Rc::clone(&pairs), Rc::clone(&tray_cell), Rc::clone(&post_state));
            move |text: String| {
                let Ok(message) = serde_json::from_str::<Value>(&text) else {
                    return;
                };
                let Some(tray) = cell.borrow().clone() else {
                    return;
                };
                match message["t"].as_str() {
                    Some("ready") => post_state(),
                    Some("size") => {
                        if let Some(height) = message["h"].as_f64() {
                            tray.set_height(height);
                        }
                    }
                    Some("run") => {
                        let Some((_, spec)) = message["id"]
                            .as_u64()
                            .and_then(|id| pairs.get(id as usize))
                        else {
                            return;
                        };
                        if message["keep"].as_bool() == Some(true) {
                            (spec.action)();
                            post_state();
                        } else {
                            // The action runs after the panel is gone, so a capture never shows it.
                            tray.hide();
                            (spec.action)();
                        }
                    }
                    _ => {}
                }
            }
        };
        let tray = TrayPanel::new(mtm, tray_html, on_message);
        *tray_cell.borrow_mut() = Some(Rc::clone(&tray));

        let click = {
            let (item, menu, tray, button) = (
                item.clone(),
                menu.clone(),
                Rc::clone(&tray),
                button.clone(),
            );
            ClickTarget::new(mtm, move || {
                let Some(button) = &button else {
                    return;
                };
                let right = NSApplication::sharedApplication(mtm)
                    .currentEvent()
                    .is_some_and(|e| e.r#type() == NSEventType::RightMouseUp);
                if right {
                    tray.hide();
                    // A menu set only for this click, so that the left click is free for the panel.
                    item.setMenu(Some(&menu));
                    // SAFETY: a plain click on a button of ours; with the menu set, it only opens the menu.
                    unsafe { button.performClick(None) };
                    item.setMenu(None);
                } else if tray.is_visible() {
                    tray.hide();
                } else if let Some(window) = button.window() {
                    post_state();
                    tray.show_under(mtm, window.frame(), window.windowNumber());
                }
            })
        };
        if let Some(button) = &button {
            // SAFETY: `clicked:` is implemented by ClickTarget with the (id) signature AppKit expects, and the
            // target is kept alive in the returned value.
            unsafe {
                button.setTarget(Some(&click));
                button.setAction(Some(sel!(clicked:)));
            }
            button.sendActionOn(NSEventMask::LeftMouseUp | NSEventMask::RightMouseUp);
        }
        Self {
            item,
            _menu: menu,
            _targets: targets,
            _delegate: delegate,
            _click: click,
            _tray: tray,
        }
    }

    /// The menu itself, for another place to show: the three dots of the notch.
    pub fn menu(&self) -> Retained<NSMenu> {
        self._menu.clone()
    }

    /// Shows the icon in the menu bar or takes it out; the menu and the item's place are kept. The system
    /// remembers the choice for the next launch, so the app sets it at every start.
    pub fn set_visible(&self, visible: bool) {
        self.item.setVisible(visible);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_ghost_is_the_logo_of_the_settings_page() {
        // 15 units wide from x 4.5, and from y 2.5 down to the feet, which reach about 21.
        let b = ghost_path().bounds();
        assert!(
            (b.origin.x - 4.5).abs() < 0.01 && (b.size.width - 15.0).abs() < 0.01,
            "{b:?}"
        );
        assert!((b.origin.y - 2.5).abs() < 0.01, "{b:?}");
        assert!((b.origin.y + b.size.height - 21.0).abs() < 0.1, "{b:?}");
    }

    #[test]
    fn the_icon_is_a_template_of_the_menu_bar_size() {
        let icon = ghost_icon();
        assert!(icon.isTemplate());
        assert_eq!(icon.size(), NSSize::new(ICON_WIDTH, ICON_HEIGHT));
    }

    #[test]
    fn a_symbol_is_a_template_and_an_unknown_name_is_none() {
        assert!(menu_symbol("gearshape").is_some_and(|image| image.isTemplate()));
        assert!(menu_symbol("figghost.no.such.symbol").is_none());
    }
}
