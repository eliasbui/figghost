//! Borderless, transparent, non-activating panel shared by the notch and the cursor popup.

use crate::{
    notch::{Material, glass_tint, vibrancy_of},
    workspace::display_options,
};
use objc2::{
    DefinedClass, MainThreadMarker, MainThreadOnly, define_class, msg_send, rc::Retained,
    runtime::AnyClass, sel,
};
use objc2_app_kit::{
    NSAppearance, NSAppearanceCustomization, NSAppearanceNameDarkAqua, NSAutoresizingMaskOptions,
    NSBackingStoreType, NSColor, NSEvent, NSEventModifierFlags, NSEventType, NSGlassEffectView,
    NSMainMenuWindowLevel, NSPanel, NSView, NSVisualEffectBlendingMode, NSVisualEffectState,
    NSVisualEffectView, NSWindowAnimationBehavior, NSWindowCollectionBehavior, NSWindowStyleMask,
};
use objc2_foundation::{NSObjectProtocol, NSPoint, NSRect};
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};

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
    /// Paste the other way (with or without formatting); Option+Shift+Return unless the user chose another key.
    EnterAlt,
    Escape,
    Pin,
    /// File the selected item in the next board; Option+B unless the user chose another key.
    Board,
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

/// The page a digit key selects with Command held: the number row, which is not in order on a keyboard.
fn tab_of_key_code(code: u16) -> Option<u8> {
    Some(match code {
        18 => 1,
        19 => 2,
        20 => 3,
        21 => 4,
        23 => 5,
        22 => 6,
        26 => 7,
        28 => 8,
        25 => 9,
        _ => return None,
    })
}

/// A key and the modifier keys held with it, as an action of a page is bound to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Combo {
    pub key_code: u16,
    pub command: bool,
    pub option: bool,
    pub shift: bool,
    pub control: bool,
}

impl Combo {
    const fn option(key_code: u16) -> Self {
        Self {
            key_code,
            command: false,
            option: true,
            shift: false,
            control: false,
        }
    }
}

/// The keys of the actions that are the user's to choose. `None` leaves an action without a key. The rest
/// (arrows, Return, Escape, Page Up and Down, Command with a number or a letter, Command+Delete) keep the meaning
/// they have everywhere on the Mac.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ActionKeys {
    pub pin: Option<Combo>,
    pub board: Option<Combo>,
    /// Pastes the other way, with or without formatting.
    pub paste_other: Option<Combo>,
    /// Deletes the selected item; refuses an agent's request; dismisses an alert.
    pub delete: Option<Combo>,
}

impl Default for ActionKeys {
    /// Option+P, Option+B, Option+Shift+Return and Option+Delete.
    fn default() -> Self {
        Self {
            pin: Some(Combo::option(35)),
            board: Some(Combo::option(11)),
            paste_other: Some(Combo {
                shift: true,
                ..Combo::option(36)
            }),
            delete: Some(Combo::option(51)),
        }
    }
}

thread_local! {
    static ACTION_KEYS: Cell<ActionKeys> = Cell::new(ActionKeys::default());
}

/// Binds the actions of the pages to these keys from the next key press on.
pub fn set_action_keys(keys: ActionKeys) {
    ACTION_KEYS.with(|k| k.set(keys));
}

/// The modifier keys held during a key press.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
struct Held {
    command: bool,
    option: bool,
    shift: bool,
    control: bool,
}

fn decode(event: &NSEvent) -> Option<Key> {
    let flags = event.modifierFlags();
    // Arrow keys always carry the function and numeric-pad bits, so only real modifiers count.
    let held = Held {
        command: flags.contains(NSEventModifierFlags::Command),
        option: flags.contains(NSEventModifierFlags::Option),
        shift: flags.contains(NSEventModifierFlags::Shift),
        control: flags.contains(NSEventModifierFlags::Control),
    };
    decode_key(event.keyCode(), held, ACTION_KEYS.with(Cell::get))
}

/// What a key press means, with the actions bound to `keys`.
fn decode_key(code: u16, held: Held, keys: ActionKeys) -> Option<Key> {
    let plain = held == Held::default();
    let command_only = held
        == Held {
            command: true,
            ..Held::default()
        };
    // Return and Enter are one key to an action, and so are Delete and Forward Delete.
    let same_key = |a: u16, b: u16| a == b || ([a, b] == [36, 76]) || ([a, b] == [51, 117]);
    let pressed = |combo: Combo| {
        same_key(combo.key_code, code)
            && held
                == Held {
                    command: combo.command,
                    option: combo.option,
                    shift: combo.shift,
                    control: combo.control,
                }
    };
    let actions = [
        (keys.paste_other, Key::EnterAlt),
        (keys.pin, Key::Pin),
        (keys.board, Key::Board),
        (keys.delete, Key::Delete),
    ];
    if let Some((_, key)) = actions
        .into_iter()
        .find(|(combo, _)| combo.is_some_and(pressed))
    {
        return Some(key);
    }
    match code {
        126 => Some(Key::Up),
        125 => Some(Key::Down),
        123 if plain => Some(Key::Left),
        124 if plain => Some(Key::Right),
        code if command_only && tab_of_key_code(code).is_some() => {
            tab_of_key_code(code).map(Key::Tab)
        }
        116 => Some(Key::PageUp),
        121 => Some(Key::PageDown),
        36 | 76 => Some(Key::Enter),
        53 => Some(Key::Escape),
        // Command+Delete deletes, as it does in Finder, whatever key the action has.
        51 | 117 if command_only => Some(Key::Delete),
        code if command_only => letter_of_key_code(code).map(Key::Shortcut),
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

    /// Puts `content` over the whole panel on Liquid Glass with the notch's tint and corners of `radius`, as every
    /// panel over other apps is. Under Increase Contrast or Reduce Transparency the desktop must not show through, so
    /// it sits on an opaque dark shape; before macOS 26 there is no Liquid Glass and a dark vibrancy material stands
    /// in. Returns whether it is the opaque shape, which is what changes when those settings do.
    pub fn set_glass_content(&self, mtm: MainThreadMarker, content: &NSView, radius: f64) -> bool {
        let options = display_options();
        let solid = options.reduce_transparency || options.increase_contrast;
        let size = NSRect::new(NSPoint::ZERO, self.frame().size);
        let sizable = NSAutoresizingMaskOptions::ViewWidthSizable
            | NSAutoresizingMaskOptions::ViewHeightSizable;
        content.removeFromSuperview();
        content.setFrame(size);
        content.setAutoresizingMask(sizable);
        let glass = glass_tint(Material::Glass)
            .filter(|_| !solid && AnyClass::get(c"NSGlassEffectView").is_some());
        if let Some((look, tint)) = glass {
            let glass = NSGlassEffectView::initWithFrame(mtm.alloc(), size);
            glass.setStyle(look);
            glass.setTintColor(Some(&tint));
            glass.setCornerRadius(radius);
            glass.setAutoresizingMask(sizable);
            glass.setContentView(Some(content));
            self.setContentView(Some(&glass));
            return false;
        }
        let base: Retained<NSView> = if solid {
            let plain = NSView::initWithFrame(mtm.alloc(), size);
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
            let effect = NSVisualEffectView::initWithFrame(mtm.alloc(), size);
            effect.setMaterial(vibrancy_of(Material::Glass));
            effect.setBlendingMode(NSVisualEffectBlendingMode::BehindWindow);
            effect.setState(NSVisualEffectState::Active);
            effect.setWantsLayer(true);
            Retained::into_super(effect)
        };
        if let Some(layer) = base.layer() {
            // SAFETY: plain property setters on the view's backing layer.
            unsafe {
                let _: () = msg_send![&*layer, setCornerRadius: radius];
                let _: () = msg_send![&*layer, setMasksToBounds: true];
            }
        }
        base.setAutoresizingMask(sizable);
        base.addSubview(content);
        self.setContentView(Some(&base));
        solid
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_number_row_selects_pages_one_to_nine() {
        // Virtual key codes of 1 to 9 on an ANSI keyboard, which are not in order.
        let codes = [18, 19, 20, 21, 23, 22, 26, 28, 25];
        for (i, code) in codes.into_iter().enumerate() {
            assert_eq!(tab_of_key_code(code), Some(i as u8 + 1));
        }
        assert_eq!(tab_of_key_code(29), None, "zero selects nothing");
        assert_eq!(tab_of_key_code(0), None, "letters are not pages");
    }

    #[test]
    fn the_actions_follow_the_keys_the_user_chose() {
        let option = Held {
            option: true,
            ..Held::default()
        };
        let command = Held {
            command: true,
            ..Held::default()
        };
        let control = Held {
            control: true,
            ..Held::default()
        };
        let defaults = ActionKeys::default();
        // As before: Option+P pins, Option+B files, Option+Shift+Return pastes the other way, Option+Delete deletes.
        assert_eq!(decode_key(35, option, defaults), Some(Key::Pin));
        assert_eq!(decode_key(11, option, defaults), Some(Key::Board));
        let option_shift = Held {
            shift: true,
            ..option
        };
        assert_eq!(decode_key(76, option_shift, defaults), Some(Key::EnterAlt));
        assert_eq!(decode_key(117, option, defaults), Some(Key::Delete));
        // Pin moved to Control+P: Option+P is free, and Control+P pins.
        let moved = ActionKeys {
            pin: Some(Combo {
                option: false,
                control: true,
                ..Combo::option(35)
            }),
            delete: None,
            ..defaults
        };
        assert_eq!(decode_key(35, option, moved), None);
        assert_eq!(decode_key(35, control, moved), Some(Key::Pin));
        // Without a key of its own, Delete still deletes with Command, as in Finder.
        assert_eq!(decode_key(51, option, moved), None);
        assert_eq!(decode_key(51, command, moved), Some(Key::Delete));
        // The keys every page shares are not the user's to move.
        assert_eq!(decode_key(36, Held::default(), moved), Some(Key::Enter));
        assert_eq!(decode_key(53, Held::default(), moved), Some(Key::Escape));
        assert_eq!(decode_key(18, command, moved), Some(Key::Tab(1)));
        assert_eq!(decode_key(0, command, moved), Some(Key::Shortcut('a')));
        assert_eq!(decode_key(123, option, moved), None);
    }
}
