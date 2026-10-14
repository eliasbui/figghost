//! A WKWebView hosted in a glass window, with a JSON message bridge in both directions.
//!
//! The page can only reach the app through `window.fig.send(object)`, and the app only reaches the page
//! through `window.__fig.recv(object)`. Nothing is loaded from disk or the network: the page is a string,
//! navigation away from it is cancelled, and links to the web open in the default browser.

use block2::{DynBlock, RcBlock};
use objc2::{
    DefinedClass, MainThreadMarker, MainThreadOnly, define_class, msg_send,
    rc::{Retained, Weak},
    runtime::{AnyClass, ProtocolObject},
};
use objc2_app_kit::{
    NSAppearance, NSAppearanceCustomization, NSAppearanceNameAqua, NSAppearanceNameDarkAqua,
    NSApplication, NSAutoresizingMaskOptions, NSBackingStoreType, NSBitmapImageFileType,
    NSBitmapImageRep, NSColor, NSGlassEffectView, NSGlassEffectViewStyle, NSImage, NSView,
    NSVisualEffectBlendingMode, NSVisualEffectMaterial, NSVisualEffectState, NSVisualEffectView,
    NSWindow, NSWindowDelegate, NSWindowStyleMask, NSWindowTitleVisibility, NSWorkspace,
};
use objc2_foundation::{
    NSDictionary, NSError, NSNotification, NSNumber, NSObject, NSObjectProtocol, NSPoint, NSRect,
    NSSize, NSString, NSURL,
};
use objc2_web_kit::{
    WKNavigationAction, WKNavigationActionPolicy, WKNavigationDelegate, WKNavigationType,
    WKScriptMessage, WKScriptMessageHandler, WKSnapshotConfiguration, WKUserContentController,
    WKUserScript, WKUserScriptInjectionTime, WKWebView, WKWebViewConfiguration, WKWebsiteDataStore,
};
use std::{cell::RefCell, ptr::NonNull, rc::Rc};

/// The page's own address. A real origin lets WebKit report script errors in full (an opaque one masks
/// them as "Script error."), and the reserved `.invalid` domain can never resolve, so nothing can leave.
const PAGE_HOST: &str = "figghost.invalid";

fn page_url() -> Option<Retained<NSURL>> {
    NSURL::URLWithString(&NSString::from_str(&format!("https://{PAGE_HOST}/")))
}

/// Defines `window.fig.send` for the page and reports script errors, before any page script runs.
const BOOTSTRAP: &str = r#"
window.fig = {
  send: function (message) {
    window.webkit.messageHandlers.fig.postMessage(JSON.stringify(message));
  },
  drag: function () { window.webkit.messageHandlers.drag.postMessage(0); },
  zoom: function () { window.webkit.messageHandlers.zoom.postMessage(0); }
};
window.addEventListener('error', function (e) {
  window.fig.send({ t: 'js_error', message: String(e.message), line: e.lineno || 0 });
});
window.addEventListener('unhandledrejection', function (e) {
  window.fig.send({ t: 'js_error', message: 'unhandled rejection: ' + String(e.reason), line: 0 });
});
"#;

type Callback = Box<dyn Fn(String)>;

pub struct HandlerIvars {
    on_message: Callback,
}

define_class!(
    // SAFETY: NSObject allows subclassing; the ivars are only touched on the main thread and need no Drop.
    #[unsafe(super(NSObject))]
    #[thread_kind = MainThreadOnly]
    #[name = "FigScriptHandler"]
    #[ivars = HandlerIvars]
    struct ScriptHandler;

    unsafe impl NSObjectProtocol for ScriptHandler {}

    unsafe impl WKScriptMessageHandler for ScriptHandler {
        #[unsafe(method(userContentController:didReceiveScriptMessage:))]
        fn did_receive(&self, _controller: &WKUserContentController, message: &WKScriptMessage) {
            // SAFETY: plain property read on the message delivered to this callback.
            let body = unsafe { message.body() };
            let text = body
                .downcast_ref::<NSString>()
                .map(|s| s.to_string())
                .unwrap_or_default();
            (self.ivars().on_message)(text);
        }
    }
);

impl ScriptHandler {
    fn new(mtm: MainThreadMarker, on_message: impl Fn(String) + 'static) -> Retained<Self> {
        let this = mtm.alloc::<Self>().set_ivars(HandlerIvars {
            on_message: Box::new(on_message),
        });
        // SAFETY: NSObject's designated initializer.
        unsafe { msg_send![super(this), init] }
    }
}

pub struct NavIvars {
    html: String,
}

define_class!(
    // SAFETY: NSObject allows subclassing; the ivars are only touched on the main thread and need no Drop.
    #[unsafe(super(NSObject))]
    #[thread_kind = MainThreadOnly]
    #[name = "FigNavDelegate"]
    #[ivars = NavIvars]
    struct NavDelegate;

    unsafe impl NSObjectProtocol for NavDelegate {}

    unsafe impl WKNavigationDelegate for NavDelegate {
        #[unsafe(method(webView:decidePolicyForNavigationAction:decisionHandler:))]
        fn decide_policy(
            &self,
            _web: &WKWebView,
            action: &WKNavigationAction,
            handler: &DynBlock<dyn Fn(WKNavigationActionPolicy)>,
        ) {
            // SAFETY: plain property reads on the action delivered to this callback.
            let url = unsafe { action.request().URL() };
            let scheme = url.as_ref().and_then(|u| u.scheme()).map(|s| s.to_string());
            let own_page = url
                .as_ref()
                .and_then(|u| u.host())
                .is_some_and(|h| h.to_string() == PAGE_HOST);
            // SAFETY: plain property read on the action delivered to this callback.
            let programmatic = unsafe { action.navigationType() } == WKNavigationType::Other;
            match scheme.as_deref() {
                // Loading the page from its string (or reloading it) is the only navigation allowed.
                _ if own_page && programmatic => handler.call((WKNavigationActionPolicy::Allow,)),
                None | Some("about") => handler.call((WKNavigationActionPolicy::Allow,)),
                Some("http" | "https" | "mailto") => {
                    if let Some(url) = &url {
                        NSWorkspace::sharedWorkspace().openURL(url);
                    }
                    handler.call((WKNavigationActionPolicy::Cancel,));
                }
                Some(_) => handler.call((WKNavigationActionPolicy::Cancel,)),
            }
        }

        // The web content runs in its own process, which the system may end under memory pressure.
        #[unsafe(method(webViewWebContentProcessDidTerminate:))]
        fn content_process_terminated(&self, web: &WKWebView) {
            // SAFETY: loading a string with the page's own base URL.
            unsafe {
                web.loadHTMLString_baseURL(
                    &NSString::from_str(&self.ivars().html),
                    page_url().as_deref(),
                )
            };
        }
    }
);

impl NavDelegate {
    fn new(mtm: MainThreadMarker, html: &str) -> Retained<Self> {
        let this = mtm.alloc::<Self>().set_ivars(NavIvars {
            html: html.to_owned(),
        });
        // SAFETY: NSObject's designated initializer.
        unsafe { msg_send![super(this), init] }
    }
}

/// The web view with its bridge. It can live inside a window of its own ([`WebWindow`]) or any view.
pub struct WebHost {
    pub view: Retained<WKWebView>,
    handlers: Vec<Retained<ScriptHandler>>,
    nav: Retained<NavDelegate>,
}

impl WebHost {
    /// `on_message` receives the JSON text the page sends with `window.fig.send`. `on_drag` and `on_zoom`
    /// are for the page's title area: `fig.drag()` on mouse down and `fig.zoom()` on double click.
    pub fn new(
        mtm: MainThreadMarker,
        frame: NSRect,
        html: &str,
        on_message: impl Fn(String) + 'static,
        on_drag: impl Fn(String) + 'static,
        on_zoom: impl Fn(String) + 'static,
    ) -> Self {
        // SAFETY: plain configuration calls with valid arguments on the main thread.
        let (view, handlers, nav) = unsafe {
            let config = WKWebViewConfiguration::new(mtm);
            // Nothing the page does is written to disk.
            config.setWebsiteDataStore(&WKWebsiteDataStore::nonPersistentDataStore(mtm));
            config
                .preferences()
                .setJavaScriptCanOpenWindowsAutomatically(false);
            let controller = config.userContentController();
            let bootstrap = WKUserScript::initWithSource_injectionTime_forMainFrameOnly(
                mtm.alloc(),
                &NSString::from_str(BOOTSTRAP),
                WKUserScriptInjectionTime::AtDocumentStart,
                true,
            );
            controller.addUserScript(&bootstrap);
            let handlers = vec![
                (ScriptHandler::new(mtm, on_message), "fig"),
                (ScriptHandler::new(mtm, on_drag), "drag"),
                (ScriptHandler::new(mtm, on_zoom), "zoom"),
            ];
            for (handler, name) in &handlers {
                controller.addScriptMessageHandler_name(
                    ProtocolObject::from_ref(&**handler),
                    &NSString::from_str(name),
                );
            }
            let view = WKWebView::initWithFrame_configuration(mtm.alloc(), frame, &config);
            let nav = NavDelegate::new(mtm, html);
            view.setNavigationDelegate(Some(ProtocolObject::from_ref(&*nav)));
            // The page draws its own background; the window's glass shows through where it is clear.
            let _: () = msg_send![&*view, setValue: &*NSNumber::numberWithBool(false), forKey: &*NSString::from_str("drawsBackground")];
            view.setAutoresizingMask(
                NSAutoresizingMaskOptions::ViewWidthSizable
                    | NSAutoresizingMaskOptions::ViewHeightSizable,
            );
            view.loadHTMLString_baseURL(&NSString::from_str(html), page_url().as_deref());
            (view, handlers.into_iter().map(|(h, _)| h).collect(), nav)
        };
        Self {
            view,
            handlers,
            nav,
        }
    }

    /// Runs script in the page and ignores the result.
    pub fn eval(&self, js: &str) {
        // SAFETY: the script is a plain string; no completion handler is needed.
        unsafe {
            self.view
                .evaluateJavaScript_completionHandler(&NSString::from_str(js), None);
        }
    }

    /// Runs script and hands its string result (or `None` on an error or a non-string) to `done`.
    pub fn eval_string(&self, js: &str, done: impl FnOnce(Option<String>) + 'static) {
        let done = RefCell::new(Some(done));
        let handler = RcBlock::new(
            move |value: *mut objc2::runtime::AnyObject, _error: *mut NSError| {
                let text = NonNull::new(value).and_then(|v| {
                    // SAFETY: WebKit passes a live object that stays valid for this callback.
                    let object = unsafe { v.as_ref() };
                    object.downcast_ref::<NSString>().map(|s| s.to_string())
                });
                if let Some(done) = done.take() {
                    done(text);
                }
            },
        );
        // SAFETY: the block is 'static and runs on the main thread.
        unsafe {
            self.view
                .evaluateJavaScript_completionHandler(&NSString::from_str(js), Some(&handler));
        }
    }

    /// Delivers `json` (a JSON document) to the page's `window.__fig.recv`.
    pub fn post(&self, json: &str) {
        self.eval(&format!("window.__fig && window.__fig.recv({json})"));
    }

    /// Renders the page to PNG bytes at `width` pixels wide, for tests and design checks.
    pub fn snapshot_png(
        &self,
        mtm: MainThreadMarker,
        width: f64,
        done: impl FnOnce(Option<Vec<u8>>) + 'static,
    ) {
        let done = RefCell::new(Some(done));
        let handler = RcBlock::new(move |image: *mut NSImage, _error: *mut NSError| {
            let png = NonNull::new(image).and_then(|i| {
                // SAFETY: WebKit passes a live image that stays valid for this callback.
                png_of(unsafe { i.as_ref() })
            });
            if let Some(done) = done.take() {
                done(png);
            }
        });
        // SAFETY: plain configuration calls; the block is 'static and runs on the main thread.
        unsafe {
            let config = WKSnapshotConfiguration::new(mtm);
            config.setSnapshotWidth(Some(&NSNumber::numberWithDouble(width)));
            self.view
                .takeSnapshotWithConfiguration_completionHandler(Some(&config), &handler);
        }
    }

    /// Breaks the retain cycles the web view's controller would otherwise keep.
    fn detach(&self) {
        // SAFETY: plain calls on objects this host created.
        unsafe {
            self.view
                .configuration()
                .userContentController()
                .removeAllScriptMessageHandlers();
            self.view.setNavigationDelegate(None);
        }
        let _ = (&self.handlers, &self.nav);
    }
}

pub(crate) fn png_of(image: &NSImage) -> Option<Vec<u8>> {
    let tiff = image.TIFFRepresentation()?;
    let rep = NSBitmapImageRep::imageRepWithData(&tiff)?;
    // SAFETY: an empty properties dictionary is valid for PNG output.
    let png = unsafe {
        rep.representationUsingType_properties(NSBitmapImageFileType::PNG, &NSDictionary::new())
    }?;
    Some(png.to_vec())
}

/// The window's background: real Liquid Glass where the system has it, a vibrancy material elsewhere.
/// Returns the view that becomes the window's content view and the one the web view goes into.
fn glass_background(
    mtm: MainThreadMarker,
    frame: NSRect,
) -> (Retained<NSView>, Option<Retained<NSGlassEffectView>>) {
    if AnyClass::get(c"NSGlassEffectView").is_some() {
        let glass = NSGlassEffectView::initWithFrame(mtm.alloc(), frame);
        glass.setStyle(NSGlassEffectViewStyle::Regular);
        glass.setCornerRadius(0.0);
        let view: Retained<NSView> = Retained::into_super(Retained::clone(&glass));
        return (view, Some(glass));
    }
    let effect = NSVisualEffectView::initWithFrame(mtm.alloc(), frame);
    effect.setMaterial(NSVisualEffectMaterial::UnderWindowBackground);
    effect.setBlendingMode(NSVisualEffectBlendingMode::BehindWindow);
    effect.setState(NSVisualEffectState::Active);
    (Retained::into_super(effect), None)
}

/// A web view in a window nobody can see (fully transparent, click-through), for checks and snapshots.
pub struct HiddenStage {
    pub host: WebHost,
    window: Retained<NSWindow>,
}

impl HiddenStage {
    /// `dark` picks the dark appearance, which the page sees as `prefers-color-scheme: dark`.
    pub fn new(
        mtm: MainThreadMarker,
        size: (f64, f64),
        dark: bool,
        html: &str,
        on_message: impl Fn(String) + 'static,
    ) -> Self {
        let frame = NSRect::new(NSPoint::ZERO, NSSize::new(size.0, size.1));
        // SAFETY: designated initializer with valid arguments on the main thread.
        let window = unsafe {
            NSWindow::initWithContentRect_styleMask_backing_defer(
                mtm.alloc(),
                frame,
                NSWindowStyleMask::Borderless,
                NSBackingStoreType::Buffered,
                false,
            )
        };
        // SAFETY: false is the only value compatible with a window kept alive by `Retained`.
        unsafe { window.setReleasedWhenClosed(false) };
        window.setAlphaValue(0.0);
        window.setIgnoresMouseEvents(true);
        // Either way is forced, so a check does not depend on the appearance the machine happens to use.
        // SAFETY: extern statics are process-lifetime constants.
        let appearance = unsafe {
            NSAppearance::appearanceNamed(if dark {
                NSAppearanceNameDarkAqua
            } else {
                NSAppearanceNameAqua
            })
        };
        window.setAppearance(appearance.as_deref());
        let host = WebHost::new(mtm, frame, html, on_message, |_| {}, |_| {});
        window.setContentView(Some(&host.view));
        window.orderFrontRegardless();
        Self { host, window }
    }

    pub fn window_number(&self) -> isize {
        self.window.windowNumber()
    }
}

pub struct DelegateIvars {
    on_close: Box<dyn Fn()>,
}

define_class!(
    // SAFETY: NSObject allows subclassing; the ivars are only touched on the main thread and need no Drop.
    #[unsafe(super(NSObject))]
    #[thread_kind = MainThreadOnly]
    #[name = "FigWindowDelegate"]
    #[ivars = DelegateIvars]
    struct WindowDelegate;

    unsafe impl NSObjectProtocol for WindowDelegate {}

    unsafe impl NSWindowDelegate for WindowDelegate {
        #[unsafe(method(windowWillClose:))]
        fn window_will_close(&self, _notification: &NSNotification) {
            (self.ivars().on_close)();
        }
    }
);

impl WindowDelegate {
    fn new(mtm: MainThreadMarker, on_close: impl Fn() + 'static) -> Retained<Self> {
        let this = mtm.alloc::<Self>().set_ivars(DelegateIvars {
            on_close: Box::new(on_close),
        });
        // SAFETY: NSObject's designated initializer.
        unsafe { msg_send![super(this), init] }
    }
}

/// Sizes are in points.
pub struct WindowSpec<'a> {
    pub title: &'a str,
    pub width: f64,
    pub height: f64,
    pub min_width: f64,
    pub min_height: f64,
}

/// A titled window with a glass background and a web view filling it, title bar included.
pub struct WebWindow {
    pub host: WebHost,
    window: Retained<NSWindow>,
    delegate: Retained<WindowDelegate>,
}

impl WebWindow {
    /// `on_close` runs when the user closes the window; the owner should drop the `WebWindow` afterwards
    /// (not from inside the callback).
    pub fn new(
        mtm: MainThreadMarker,
        spec: &WindowSpec<'_>,
        html: &str,
        on_message: impl Fn(String) + 'static,
        on_close: impl Fn() + 'static,
    ) -> Rc<Self> {
        let frame = NSRect::new(NSPoint::ZERO, NSSize::new(spec.width, spec.height));
        let style = NSWindowStyleMask::Titled
            | NSWindowStyleMask::Closable
            | NSWindowStyleMask::Miniaturizable
            | NSWindowStyleMask::Resizable
            | NSWindowStyleMask::FullSizeContentView;
        // SAFETY: designated initializer with valid arguments on the main thread.
        let window = unsafe {
            NSWindow::initWithContentRect_styleMask_backing_defer(
                mtm.alloc(),
                frame,
                style,
                NSBackingStoreType::Buffered,
                false,
            )
        };
        // The Rust side owns the window; AppKit must not release it on close.
        // SAFETY: false is the only value compatible with a window kept alive by `Retained`.
        unsafe { window.setReleasedWhenClosed(false) };
        window.setTitle(&NSString::from_str(spec.title));
        window.setTitleVisibility(NSWindowTitleVisibility::Hidden);
        window.setTitlebarAppearsTransparent(true);
        window.setOpaque(false);
        window.setBackgroundColor(Some(&NSColor::clearColor()));
        window.setMinSize(NSSize::new(spec.min_width, spec.min_height));

        // Weak, because the web view's controller keeps these callbacks alive and the window owns the web view.
        let (drag_target, zoom_target) =
            (Weak::from_retained(&window), Weak::from_retained(&window));
        let host = WebHost::new(
            mtm,
            frame,
            html,
            on_message,
            move |_| {
                if let (Some(window), Some(event)) = (
                    drag_target.load(),
                    NSApplication::sharedApplication(mtm).currentEvent(),
                ) {
                    window.performWindowDragWithEvent(&event);
                }
            },
            move |_| {
                if let Some(window) = zoom_target.load() {
                    window.zoom(None);
                }
            },
        );

        let (background, glass) = glass_background(mtm, frame);
        window.setContentView(Some(&background));
        match glass {
            Some(glass) => glass.setContentView(Some(&host.view)),
            None => background.addSubview(&host.view),
        }
        host.view.setFrame(background.bounds());

        let delegate = WindowDelegate::new(mtm, on_close);
        window.setDelegate(Some(ProtocolObject::from_ref(&*delegate)));
        window.center();
        Rc::new(Self {
            host,
            window,
            delegate,
        })
    }

    /// Brings the window to the front and makes this app active so it takes keyboard focus.
    pub fn show(&self, mtm: MainThreadMarker) {
        self.window.makeKeyAndOrderFront(None);
        #[allow(deprecated)]
        NSApplication::sharedApplication(mtm).activateIgnoringOtherApps(true);
    }

    pub fn is_visible(&self) -> bool {
        self.window.isVisible()
    }

    pub fn is_key(&self) -> bool {
        self.window.isKeyWindow()
    }

    pub fn window_number(&self) -> isize {
        self.window.windowNumber()
    }

    pub fn close(&self) {
        self.window.close();
    }
}

impl Drop for WebWindow {
    fn drop(&mut self) {
        self.host.detach();
        self.window.setDelegate(None);
        self.window.close();
        self.window.setContentView(None);
        let _ = &self.delegate;
    }
}
