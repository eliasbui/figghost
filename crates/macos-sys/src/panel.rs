//! Borderless, transparent, non-activating panel shared by the notch and the cursor popup.

use objc2::{
    DefinedClass, MainThreadMarker, MainThreadOnly, define_class, msg_send, rc::Retained, sel,
};
use objc2_app_kit::{
    NSAppearance, NSAppearanceCustomization, NSAppearanceNameDarkAqua, NSBackingStoreType, NSColor,
    NSEvent, NSEventModifierFlags, NSEventType, NSMainMenuWindowLevel, NSPanel,
    NSWindowAnimationBehavior, NSWindowCollectionBehavior, NSWindowStyleMask,
};
use objc2_foundation::{NSObjectProtocol, NSRect};
use std::{cell::RefCell, rc::Rc};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Key {
    Up,
    Down,
    Left,
    Right,
    /// Command+1, Command+2, ...: switch to that page of the notch.
    Tab(u8),
    PageUp,
    PageDown,
    Enter,
    /// Option+Shift+Enter: paste the other way (with or without formatting).
    EnterAlt,
    Escape,
    Pin,
    Delete,
    /// Command plus a letter: pastes the pinned item that has that letter, if there is one.
    Shortcut(char),
}

/// The letter an ANSI virtual key code types.
pub fn letter_of_key_code(code: u16) -> Option<char> {
    Some(match code {
        0 => 'a',
        11 => 'b',
        8 => 'c',
        2 => 'd',
        14 => 'e',
        3 => 'f',
        5 => 'g',
        4 => 'h',
        34 => 'i',
        38 => 'j',
        40 => 'k',
        37 => 'l',
        46 => 'm',
        45 => 'n',
        31 => 'o',
        35 => 'p',
        12 => 'q',
        15 => 'r',
        1 => 's',
        17 => 't',
        32 => 'u',
        9 => 'v',
        13 => 'w',
        7 => 'x',
        16 => 'y',
        6 => 'z',
        _ => return None,
    })
}

fn decode(event: &NSEvent) -> Option<Key> {
    let flags = event.modifierFlags();
    let option = flags.contains(NSEventModifierFlags::Option);
    let command = flags.contains(NSEventModifierFlags::Command);
    let shift = flags.contains(NSEventModifierFlags::Shift);
    let control = flags.contains(NSEventModifierFlags::Control);
    // Arrow keys always carry the function and numeric-pad bits, so only real modifiers count.
    let plain = !flags.intersects(
        NSEventModifierFlags::Command
            | NSEventModifierFlags::Option
            | NSEventModifierFlags::Control
            | NSEventModifierFlags::Shift,
    );
    match event.keyCode() {
        126 => Some(Key::Up),
        125 => Some(Key::Down),
        123 if plain => Some(Key::Left),
        124 if plain => Some(Key::Right),
        18 if command => Some(Key::Tab(1)),
        19 if command => Some(Key::Tab(2)),
        20 if command => Some(Key::Tab(3)),
        21 if command => Some(Key::Tab(4)),
        116 => Some(Key::PageUp),
        121 => Some(Key::PageDown),
        36 | 76 if option && shift => Some(Key::EnterAlt),
        36 | 76 => Some(Key::Enter),
        53 => Some(Key::Escape),
        35 if option => Some(Key::Pin),
        51 | 117 if option || command => Some(Key::Delete),
        code if command && !option && !shift && !control => {
            letter_of_key_code(code).map(Key::Shortcut)
        }
        _ => None,
    }
}

type KeyHandler = Rc<dyn Fn(Key) -> bool>;

pub struct PanelIvars {
    on_key: RefCell<Option<KeyHandler>>,
}

define_class!(
    // SAFETY: NSPanel allows subclassing; the ivars are only touched on the main thread and need no Drop.
    #[unsafe(super(NSPanel))]
    #[thread_kind = MainThreadOnly]
    #[name = "FigPanel"]
    #[ivars = PanelIvars]
    pub struct FigPanel;

    impl FigPanel {
        // Borderless windows refuse key status by default; the search field needs it.
        #[unsafe(method(canBecomeKeyWindow))]
        fn can_become_key_window(&self) -> bool {
            true
        }

        #[unsafe(method(canBecomeMainWindow))]
        fn can_become_main_window(&self) -> bool {
            false
        }

        #[unsafe(method(sendEvent:))]
        fn send_event(&self, event: &NSEvent) {
            // Keys the list owns are taken before the text field sees them, except while an input
            // method is composing: Telex/VNI use Return and arrows to commit marked text.
            if event.r#type() == NSEventType::KeyDown && !self.composing() {
                if let Some(key) = decode(event) {
                    let handler = self.ivars().on_key.borrow().clone();
                    if handler.is_some_and(|h| h(key)) {
                        return;
                    }
                }
            }
            // SAFETY: forwards the same event to NSPanel's own implementation.
            let _: () = unsafe { msg_send![super(self), sendEvent: event] };
        }
    }
);

impl FigPanel {
    pub fn new(mtm: MainThreadMarker, frame: NSRect) -> Retained<Self> {
        // NonactivatingPanel must be in the init-time mask; setting it later never reaches the window server.
        let style = NSWindowStyleMask::Borderless | NSWindowStyleMask::NonactivatingPanel;
        // SAFETY: designated initializer with valid arguments on the main thread.
        let panel: Retained<Self> = unsafe {
            msg_send![
                super(mtm.alloc::<Self>().set_ivars(PanelIvars { on_key: RefCell::new(None) })),
                initWithContentRect: frame,
                styleMask: style,
                backing: NSBackingStoreType::Buffered,
                defer: false
            ]
        };
        // The Rust side owns the panel; AppKit must not release it on close.
        // SAFETY: false is the only value compatible with a window kept alive by `Retained`.
        unsafe { panel.setReleasedWhenClosed(false) };
        panel.setOpaque(false);
        panel.setBackgroundColor(Some(&NSColor::clearColor()));
        panel.setHasShadow(false);
        panel.setFloatingPanel(true);
        // NSPanel hides on deactivate by default, and an accessory app is never active.
        panel.setHidesOnDeactivate(false);
        panel.setBecomesKeyOnlyIfNeeded(true);
        panel.setLevel(NSMainMenuWindowLevel + 1);
        panel.setCollectionBehavior(
            NSWindowCollectionBehavior::CanJoinAllSpaces
                | NSWindowCollectionBehavior::Stationary
                | NSWindowCollectionBehavior::FullScreenAuxiliary
                | NSWindowCollectionBehavior::IgnoresCycle,
        );
        panel.setAnimationBehavior(NSWindowAnimationBehavior::None);
        // SAFETY: extern statics are process-lifetime constants.
        let dark = unsafe { NSAppearance::appearanceNamed(NSAppearanceNameDarkAqua) };
        panel.setAppearance(dark.as_deref());
        panel
    }

    /// The handler returns true when it consumed the key.
    pub fn set_key_handler(&self, handler: impl Fn(Key) -> bool + 'static) {
        *self.ivars().on_key.borrow_mut() = Some(Rc::new(handler));
    }

    fn composing(&self) -> bool {
        let Some(responder) = self.firstResponder() else {
            return false;
        };
        if !responder.respondsToSelector(sel!(hasMarkedText)) {
            return false;
        }
        // SAFETY: hasMarkedText is part of NSTextInputClient and takes no arguments.
        unsafe { msg_send![&*responder, hasMarkedText] }
    }
}
