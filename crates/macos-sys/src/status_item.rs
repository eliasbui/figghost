//! Menu bar status item. Titles, check marks and visibility are recomputed every time the menu opens,
//! so entries can reflect the frontmost app, permissions and login-item state without any bookkeeping.

use objc2::{
    DefinedClass, MainThreadMarker, MainThreadOnly, define_class, msg_send, rc::Retained,
    runtime::ProtocolObject, sel,
};
use objc2_app_kit::{
    NSControlStateValueOff, NSControlStateValueOn, NSMenu, NSMenuDelegate, NSMenuItem, NSStatusBar,
    NSStatusItem, NSVariableStatusItemLength,
};
use objc2_foundation::{NSObject, NSObjectProtocol, NSString};
use std::rc::Rc;

type Predicate = Box<dyn Fn() -> bool>;

pub struct Item {
    title: Box<dyn Fn() -> String>,
    checked: Option<Predicate>,
    visible: Option<Predicate>,
    action: Box<dyn Fn()>,
}

impl Item {
    pub fn new(title: &str, action: impl Fn() + 'static) -> Self {
        let title = title.to_owned();
        Self {
            title: Box::new(move || title.clone()),
            checked: None,
            visible: None,
            action: Box::new(action),
        }
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

/// Keeps the status item, its menu and the action targets alive.
pub struct StatusMenu {
    _item: Retained<NSStatusItem>,
    _menu: Retained<NSMenu>,
    _targets: Vec<Retained<MenuTarget>>,
    _delegate: Retained<MenuDelegate>,
}

impl StatusMenu {
    pub fn new(mtm: MainThreadMarker, title: &str, entries: Vec<Entry>) -> Self {
        let item = NSStatusBar::systemStatusBar().statusItemWithLength(NSVariableStatusItemLength);
        if let Some(button) = item.button(mtm) {
            button.setTitle(&NSString::from_str(title));
        }
        let menu = NSMenu::new(mtm);
        // The menu must not auto-enable items: our targets are not in the responder chain.
        menu.setAutoenablesItems(false);

        let mut targets = Vec::new();
        let mut pairs: Vec<(Retained<NSMenuItem>, Rc<Item>)> = Vec::new();
        for entry in entries {
            let spec = match entry {
                Entry::Separator => {
                    menu.addItem(&NSMenuItem::separatorItem(mtm));
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
            menu.addItem(&menu_item);
            targets.push(target);
            pairs.push((menu_item, spec));
        }

        let refresh = move || {
            for (menu_item, spec) in &pairs {
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
        };
        refresh();
        let delegate = MenuDelegate::new(mtm, refresh);
        menu.setDelegate(Some(ProtocolObject::from_ref(&*delegate)));
        item.setMenu(Some(&menu));
        Self {
            _item: item,
            _menu: menu,
            _targets: targets,
            _delegate: delegate,
        }
    }
}
