//! Thin FFI layer over AppKit, CoreGraphics and ApplicationServices.
//! Every `unsafe` block in the workspace lives here; the other crates forbid it.

pub mod app;
pub mod ax;
pub mod event_tap;
pub mod hotkey;
pub mod hud;
pub mod labels;
pub mod locale;
pub mod login_item;
pub mod media_ui;
pub mod notch;
pub mod now_playing;
pub mod panel;
pub mod pasteboard;
pub mod permissions;
pub mod qos;
pub mod shelf_ui;
pub mod status_item;
pub mod synth;
pub mod system_keys;
pub mod ui;
pub mod volume;
pub mod webview;
pub mod widgets;
pub mod window;
pub mod workspace;

pub use objc2::MainThreadMarker;
