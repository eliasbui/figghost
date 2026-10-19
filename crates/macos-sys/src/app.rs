use crate::{labels, widgets::ActionTarget};
use block2::RcBlock;
use objc2::{
    DefinedClass, MainThreadMarker, MainThreadOnly, define_class, msg_send,
    rc::Retained,
    runtime::{ProtocolObject, Sel},
    sel,
};
use objc2_app_kit::{
    NSApplication, NSApplicationActivationPolicy, NSApplicationDelegate,
    NSApplicationDidFinishLaunchingNotification, NSEventModifierFlags, NSMenu, NSMenuItem,
};
use objc2_foundation::{
    NSAppleEventManager, NSNotification, NSNotificationCenter, NSObject, NSObjectProtocol,
    NSRunLoop, NSRunLoopCommonModes, NSString, NSTimer,
};
use std::{cell::RefCell, collections::HashMap, ptr::NonNull, rc::Rc, time::Instant};

const APP_NAME: &str = "Figghost";

thread_local! {
    /// What the items of the application menu do, by name ("settings", "updates", "whats_new", "config"); set by
    /// the app once what they open exists.
    static MENU_HANDLERS: RefCell<HashMap<&'static str, Rc<dyn Fn()>>> = RefCell::new(HashMap::new());
    /// The targets of the menu items; AppKit holds targets weakly.
    static MENU_TARGETS: RefCell<Vec<Retained<ActionTarget>>> = const { RefCell::new(Vec::new()) };
}

pub fn set_menu_handler(name: &'static str, handler: impl Fn() + 'static) {
    MENU_HANDLERS.with(|slots| slots.borrow_mut().insert(name, Rc::new(handler)));
}

pub fn set_settings_handler(handler: impl Fn() + 'static) {
    set_menu_handler("settings", handler);
}

/// Runs AppKit as an accessory app: no Dock icon, and it never becomes the active app on its own,
/// so the user's current window keeps keyboard focus.
///
/// `setup` runs from applicationDidFinishLaunching and its result stays alive until the loop ends.
/// It must not run earlier: AppKit's launch sequence asks the window server to bring the app's front
/// window forward, and that request takes effect only on windows already visible at that moment.
pub fn run_accessory<T: 'static>(mtm: MainThreadMarker, setup: impl FnOnce() -> T + 'static) {
    let app = NSApplication::sharedApplication(mtm);
    app.setActivationPolicy(NSApplicationActivationPolicy::Accessory);
    install_main_menu(mtm, &app);

    let keep = Rc::new(RefCell::new(None));
    let setup = RefCell::new(Some(setup));
    let slot = Rc::clone(&keep);
    let on_launched = RcBlock::new(move |_: NonNull<NSNotification>| {
        if let Some(setup) = setup.borrow_mut().take() {
            *slot.borrow_mut() = Some(setup());
        }
    });
    // SAFETY: the block is 'static and runs on the main thread, where the notification is posted.
    let _observer = unsafe {
        NSNotificationCenter::defaultCenter().addObserverForName_object_queue_usingBlock(
            Some(NSApplicationDidFinishLaunchingNotification),
            None,
            None,
            &on_launched,
        )
    };
    app.run();
}

pub fn quit(mtm: MainThreadMarker) {
    NSApplication::sharedApplication(mtm).terminate(None);
}

/// The application, Edit and Window menus. Text fields only honour Cmd+C/V/X/A/Z when the main menu carries
/// the matching items, and a window needs Cmd+W and Cmd+M; an accessory app never shows the menu bar, but
/// the same menu serves while the settings window makes the app a regular one.
fn install_main_menu(mtm: MainThreadMarker, app: &NSApplication) {
    let text = labels::get();
    let main = NSMenu::new(mtm);

    let plain = |title: &str, key: &str, action: Sel| {
        // SAFETY: the actions are standard responder-chain selectors; a nil target resolves them at dispatch.
        unsafe {
            NSMenuItem::initWithTitle_action_keyEquivalent(
                NSMenuItem::alloc(mtm),
                &NSString::from_str(title),
                Some(action),
                &NSString::from_str(key),
            )
        }
    };
    let submenu = |title: &str, items: Vec<Retained<NSMenuItem>>| {
        let root = NSMenuItem::new(mtm);
        let menu = NSMenu::initWithTitle(NSMenu::alloc(mtm), &NSString::from_str(title));
        for item in items {
            menu.addItem(&item);
        }
        root.setSubmenu(Some(&menu));
        main.addItem(&root);
        menu
    };

    // An item that runs the handler of `name`, looked up at the click, so the app may set it after the menu.
    let custom = |title: &str, key: &str, name: &'static str| {
        let item = plain(title, key, sel!(fire:));
        let target = ActionTarget::new(mtm, move || {
            let handler = MENU_HANDLERS.with(|slots| slots.borrow().get(name).cloned());
            if let Some(handler) = handler {
                handler();
            }
        });
        // SAFETY: `fire:` is implemented by ActionTarget; the target is kept alive in MENU_TARGETS.
        unsafe { item.setTarget(Some(&target)) };
        MENU_TARGETS.with(|targets| targets.borrow_mut().push(target));
        item
    };
    let settings = custom(&text.menu_settings, ",", "settings");
    let hide_others = plain(&text.menu_hide_others, "h", sel!(hideOtherApplications:));
    hide_others
        .setKeyEquivalentModifierMask(NSEventModifierFlags::Command | NSEventModifierFlags::Option);
    submenu(
        APP_NAME,
        vec![
            plain(
                &format!("{} {APP_NAME}", text.menu_about),
                "",
                sel!(orderFrontStandardAboutPanel:),
            ),
            custom(&text.menu_updates, "", "updates"),
            custom(&text.menu_whats_new, "", "whats_new"),
            NSMenuItem::separatorItem(mtm),
            settings,
            custom(&text.menu_config, "", "config"),
            NSMenuItem::separatorItem(mtm),
            plain(&format!("{} {APP_NAME}", text.menu_hide), "h", sel!(hide:)),
            hide_others,
            plain(&text.menu_show_all, "", sel!(unhideAllApplications:)),
            NSMenuItem::separatorItem(mtm),
            plain(
                &format!("{} {APP_NAME}", text.menu_quit),
                "q",
                sel!(terminate:),
            ),
        ],
    );
    submenu(
        &text.menu_edit,
        vec![
            plain(&text.menu_undo, "z", sel!(undo:)),
            plain(&text.menu_cut, "x", sel!(cut:)),
            plain(&text.menu_copy, "c", sel!(copy:)),
            plain(&text.menu_paste, "v", sel!(paste:)),
            plain(&text.menu_select_all, "a", sel!(selectAll:)),
        ],
    );
    let window_menu = submenu(
        &text.menu_window,
        vec![
            plain(&text.menu_minimize, "m", sel!(performMiniaturize:)),
            plain(&text.menu_zoom, "", sel!(performZoom:)),
            NSMenuItem::separatorItem(mtm),
            plain(&text.menu_close, "w", sel!(performClose:)),
        ],
    );
    app.setMainMenu(Some(&main));
    app.setWindowsMenu(Some(&window_menu));
}

/// A regular app has a Dock icon and a menu bar; an accessory app has neither. The settings window turns
/// the app regular while it is open.
pub fn set_regular(mtm: MainThreadMarker, regular: bool) {
    let policy = if regular {
        NSApplicationActivationPolicy::Regular
    } else {
        NSApplicationActivationPolicy::Accessory
    };
    NSApplication::sharedApplication(mtm).setActivationPolicy(policy);
}

/// Makes this app the active one so its window takes keyboard focus.
pub fn activate(mtm: MainThreadMarker) {
    #[allow(deprecated)]
    NSApplication::sharedApplication(mtm).activateIgnoringOtherApps(true);
}

/// True when the system started the app as a login item. Only meaningful while the launch event is
/// being handled, so call it first thing from the setup closure.
pub fn launched_at_login() -> bool {
    const OPEN_APPLICATION: u32 = u32::from_be_bytes(*b"oapp");
    const PROPERTY_DATA: u32 = u32::from_be_bytes(*b"prdt");
    const LAUNCHED_AS_LOGIN_ITEM: u32 = u32::from_be_bytes(*b"lgit");
    let Some(event) = NSAppleEventManager::sharedAppleEventManager().currentAppleEvent() else {
        return false;
    };
    // SAFETY: plain property reads on a live descriptor; the selectors take the argument types given.
    unsafe {
        let id: u32 = msg_send![&*event, eventID];
        let data: Option<Retained<objc2_foundation::NSAppleEventDescriptor>> =
            msg_send![&*event, paramDescriptorForKeyword: PROPERTY_DATA];
        id == OPEN_APPLICATION
            && data.is_some_and(|d| {
                let code: u32 = msg_send![&*d, enumCodeValue];
                code == LAUNCHED_AS_LOGIN_ITEM
            })
    }
}

pub struct DelegateIvars {
    on_reopen: Box<dyn Fn()>,
}

define_class!(
    // SAFETY: NSObject allows subclassing; the ivars are only touched on the main thread and need no Drop.
    #[unsafe(super(NSObject))]
    #[thread_kind = MainThreadOnly]
    #[name = "FigAppDelegate"]
    #[ivars = DelegateIvars]
    struct AppDelegate;

    unsafe impl NSObjectProtocol for AppDelegate {}

    unsafe impl NSApplicationDelegate for AppDelegate {
        // The user opened the app again (Finder, Spotlight, the Dock icon) while it was already running.
        #[unsafe(method(applicationShouldHandleReopen:hasVisibleWindows:))]
        fn should_handle_reopen(&self, _app: &NSApplication, _has_visible_windows: bool) -> bool {
            (self.ivars().on_reopen)();
            false
        }
    }
);

/// Keeps the application delegate alive (AppKit holds it weakly).
pub struct ReopenHandler {
    _delegate: Retained<AppDelegate>,
}

/// Calls `on_reopen` when the user opens the already-running app again.
pub fn on_reopen(mtm: MainThreadMarker, on_reopen: impl Fn() + 'static) -> ReopenHandler {
    let this = mtm.alloc::<AppDelegate>().set_ivars(DelegateIvars {
        on_reopen: Box::new(on_reopen),
    });
    // SAFETY: NSObject's designated initializer.
    let delegate: Retained<AppDelegate> = unsafe { msg_send![super(this), init] };
    NSApplication::sharedApplication(mtm).setDelegate(Some(ProtocolObject::from_ref(&*delegate)));
    ReopenHandler {
        _delegate: delegate,
    }
}

fn schedule(seconds: f64, repeats: bool, f: impl Fn() + 'static) -> Retained<NSTimer> {
    let block = RcBlock::new(move |_: NonNull<NSTimer>| f());
    // SAFETY: the block is 'static; the timer is added to the main run loop and fires on the main thread.
    unsafe {
        let timer = NSTimer::timerWithTimeInterval_repeats_block(seconds, repeats, &block);
        NSRunLoop::mainRunLoop().addTimer_forMode(&timer, NSRunLoopCommonModes);
        timer
    }
}

/// A repeating main-thread timer; stops when dropped.
pub struct Timer(Retained<NSTimer>);

impl Drop for Timer {
    fn drop(&mut self) {
        self.0.invalidate();
    }
}

/// Calls `f` on the main thread every `seconds`; also fires while menus or drags are tracking.
pub fn every(seconds: f64, f: impl Fn() + 'static) -> Timer {
    Timer(schedule(seconds, true, f))
}

/// Polls `ready` every `interval` seconds on the main thread and calls `done(true)` as soon as it holds,
/// or `done(false)` once `timeout` seconds have passed.
pub fn wait_until(
    interval: f64,
    timeout: f64,
    ready: impl Fn() -> bool + 'static,
    done: impl FnOnce(bool) + 'static,
) {
    let started = Instant::now();
    let done = RefCell::new(Some(done));
    let slot: Rc<RefCell<Option<Timer>>> = Rc::default();
    let timer = {
        let slot = Rc::clone(&slot);
        every(interval, move || {
            let ok = ready();
            if ok || started.elapsed().as_secs_f64() >= timeout {
                if let Some(done) = done.take() {
                    done(ok);
                }
                // Dropping the timer invalidates it and breaks the timer-closure-timer cycle.
                slot.borrow_mut().take();
            }
        })
    };
    *slot.borrow_mut() = Some(timer);
}

/// Runs `f` on the main thread soon, from any thread.
pub fn on_main(f: impl FnOnce() + Send + 'static) {
    dispatch2::DispatchQueue::main().exec_async(f);
}

pub fn after(seconds: f64, f: impl FnOnce() + 'static) {
    let f = RefCell::new(Some(f));
    schedule(seconds, false, move || {
        if let Some(f) = f.take() {
            f();
        }
    });
}
