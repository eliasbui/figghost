//! Thin FFI layer over AppKit, CoreGraphics and ApplicationServices.
//! Every `unsafe` block in the workspace lives here; the other crates forbid it.

pub mod agents_ui;
pub mod annotate;
pub mod app;
pub mod ax;
pub mod button;
pub mod calendar_ui;
pub mod capture;
pub mod event_tap;
pub mod eventkit;
pub mod focus_ui;
pub mod hotkey;
pub mod hud;
pub mod labels;
pub mod locale;
pub mod login_item;
pub mod media_ui;
pub mod menubar;
pub mod notch;
pub mod now_playing;
pub mod panel;
pub mod pasteboard;
pub mod permissions;
pub mod picker;
pub mod qos;
pub mod selection;
pub mod shelf_ui;
pub mod smart_ui;
pub mod snap_ui;
pub mod stack;
pub mod status_item;
pub mod style;
pub mod switcher;
pub mod synth;
pub mod system_keys;
pub mod timezone;
pub mod ui;
pub mod vision;
pub mod volume;
pub mod webview;
pub mod widgets;
pub mod window;
pub mod workspace;

pub use objc2::MainThreadMarker;
