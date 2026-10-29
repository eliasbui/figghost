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
    NSBitmapImageRep, NSColor, NSEvent, NSEventMask, NSEventType, NSGlassEffectView,
    NSGlassEffectViewStyle, NSImage, NSPanel, NSScreen, NSStatusWindowLevel, NSView,
    NSVisualEffectBlendingMode, NSVisualEffectMaterial, NSVisualEffectState, NSVisualEffectView,
    NSWindow, NSWindowDelegate, NSWindowOrderingMode, NSWindowStyleMask, NSWindowTitleVisibility,
    NSWorkspace,
};
use objc2_core_graphics::{CGColor, CGMutablePath};
use objc2_foundation::{
    NSDictionary, NSError, NSNotification, NSNumber, NSObject, NSObjectProtocol, NSPoint, NSRect,
    NSSize, NSString, NSURL,
};
use objc2_quartz_core::{CAShapeLayer, kCAFillRuleEvenOdd};
use objc2_web_kit::{
    WKNavigationAction, WKNavigationActionPolicy, WKNavigationDelegate, WKNavigationType,
    WKScriptMessage, WKScriptMessageHandler, WKSnapshotConfiguration, WKUserContentController,
    WKUserScript, WKUserScriptInjectionTime, WKWebView, WKWebViewConfiguration, WKWebsiteDataStore,
};
use std::{
    cell::{Cell, RefCell},
    ptr::NonNull,
    rc::Rc,
};

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
    radius: f64,
) -> (Retained<NSView>, Option<Retained<NSGlassEffectView>>) {
    if AnyClass::get(c"NSGlassEffectView").is_some() {
        let glass = NSGlassEffectView::initWithFrame(mtm.alloc(), frame);
        glass.setStyle(NSGlassEffectViewStyle::Regular);
        glass.setCornerRadius(radius);
        let view: Retained<NSView> = Retained::into_super(Retained::clone(&glass));
        return (view, Some(glass));
    }
    let effect = NSVisualEffectView::initWithFrame(mtm.alloc(), frame);
    effect.setMaterial(NSVisualEffectMaterial::UnderWindowBackground);
    effect.setBlendingMode(NSVisualEffectBlendingMode::BehindWindow);
    effect.setState(NSVisualEffectState::Active);
    if radius > 0.0 {
        effect.setWantsLayer(true);
        if let Some(layer) = effect.layer() {
            layer.setCornerRadius(radius);
            layer.setMasksToBounds(true);
        }
    }
    (Retained::into_super(effect), None)
}

/// The window's own background and the panes of glass on it, once the page has asked for them.
#[derive(Default)]
struct GlassBed {
    canvas: RefCell<Option<Retained<NSView>>>,
    panes: RefCell<Vec<Retained<NSGlassEffectView>>>,
}

/// Lays the system's Liquid Glass under the page where the page asks for it with
/// `webkit.messageHandlers.glass.postMessage('{"canvas":[r,g,b],"panes":[[x,y,width,height,radius,see],...]}')`, in
/// points from the top left, channels 0 to 255. From the first message on the window paints the page's background
/// itself, so the glass sits between it and the page, which leaves those places clear. A pane with `see` set cuts
/// through the background and shows what is behind the window, as a macOS 26 sidebar does; the others sit on it.
fn lay_glass(
    mtm: MainThreadMarker,
    window: &NSWindow,
    web: &WKWebView,
    bed: &GlassBed,
    text: &str,
) {
    let Ok(message) = serde_json::from_str::<serde_json::Value>(text) else {
        return;
    };
    let num = |v: &serde_json::Value| v.as_f64().unwrap_or(0.0);
    let panes: Vec<[f64; 6]> = message["panes"]
        .as_array()
        .map(|list| {
            list.iter()
                .map(|p| std::array::from_fn(|i| num(&p[i])))
                .collect()
        })
        .unwrap_or_default();
    let mut slot = bed.canvas.borrow_mut();
    let canvas = slot.get_or_insert_with(|| {
        // The web view leaves the full-window glass for a plain root: background, panes, then the page. The glass
        // held it with constraints, so it goes back to its autoresizing mask, or Auto Layout shrinks it to nothing.
        let old = window.contentView();
        let bounds = old.as_ref().map_or(web.frame(), |v| v.bounds());
        if let Some(glass) = old
            .as_ref()
            .and_then(|v| v.downcast_ref::<NSGlassEffectView>())
        {
            glass.setContentView(None);
        }
        let root = NSView::initWithFrame(mtm.alloc(), bounds);
        let canvas = NSView::initWithFrame(mtm.alloc(), bounds);
        for view in [&root, &canvas] {
            view.setAutoresizingMask(
                NSAutoresizingMaskOptions::ViewWidthSizable
                    | NSAutoresizingMaskOptions::ViewHeightSizable,
            );
        }
        canvas.setWantsLayer(true);
        root.addSubview(&canvas);
        root.addSubview(web);
        window.setContentView(Some(&root));
        web.setTranslatesAutoresizingMaskIntoConstraints(true);
        web.setFrame(root.bounds());
        window.makeFirstResponder(Some(web));
        canvas
    });
    // SAFETY: a plain read on the main thread of a view this window keeps in its hierarchy.
    let Some(root) = (unsafe { canvas.superview() }) else {
        return;
    };
    let size = root.bounds().size;
    let rect = |p: &[f64; 6]| {
        NSRect::new(
            NSPoint::new(p[0], size.height - p[1] - p[3]),
            NSSize::new(p[2], p[3]),
        )
    };
    // CGPathAddRoundedRect refuses a corner wider than half a side.
    let radius = |p: &[f64; 6]| p[4].min(p[2] / 2.0).min(p[3] / 2.0).max(0.0);
    if let Some(layer) = canvas.layer() {
        let c = &message["canvas"];
        layer.setBackgroundColor(Some(&CGColor::new_srgb(
            num(&c[0]) / 255.0,
            num(&c[1]) / 255.0,
            num(&c[2]) / 255.0,
            1.0,
        )));
        // The outer edge runs far past the window, so a window grown before the page answers is not cut off.
        let path = CGMutablePath::new();
        let outer = NSRect::new(NSPoint::ZERO, NSSize::new(20_000.0, 20_000.0));
        // SAFETY: a fresh path and plain rectangles with corners that fit; a null transform is allowed.
        unsafe {
            CGMutablePath::add_rect(Some(&path), std::ptr::null(), outer);
            for p in panes.iter().filter(|p| p[5] != 0.0) {
                let r = radius(p);
                CGMutablePath::add_rounded_rect(Some(&path), std::ptr::null(), rect(p), r, r);
            }
        }
        let mask = CAShapeLayer::layer();
        mask.setFrame(outer);
        mask.setPath(Some(&path));
        // SAFETY: an extern static of Core Animation, a process-lifetime constant; the mask is a fresh layer.
        unsafe {
            mask.setFillRule(kCAFillRuleEvenOdd);
            layer.setMask(Some(&mask));
        }
    }
    let mut pool = bed.panes.borrow_mut();
    while pool.len() > panes.len() {
        if let Some(pane) = pool.pop() {
            pane.removeFromSuperview();
        }
    }
    while pool.len() < panes.len() {
        let pane = NSGlassEffectView::initWithFrame(mtm.alloc(), NSRect::ZERO);
        pane.setStyle(NSGlassEffectViewStyle::Regular);
        root.addSubview_positioned_relativeTo(&pane, NSWindowOrderingMode::Below, Some(&**web));
        pool.push(pane);
    }
    for (pane, p) in pool.iter().zip(&panes) {
        pane.setFrame(rect(p));
        pane.setCornerRadius(radius(p));
        // Until the page answers a resize: a tall pane stretches, the others keep to the top, and to the right
        // edge when they are on its half.
        let mut mask = if p[3] > size.height / 2.0 {
            NSAutoresizingMaskOptions::ViewHeightSizable
        } else {
            NSAutoresizingMaskOptions::ViewMinYMargin
        };
        if p[0] + p[2] / 2.0 > size.width / 2.0 {
            mask |= NSAutoresizingMaskOptions::ViewMinXMargin;
        }
        pane.setAutoresizingMask(mask);
    }
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
        // The content area keeps a usable size when the user resizes the window.
        window.setContentMinSize(NSSize::new(spec.min_width, spec.min_height));

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
                ) && event.r#type() == NSEventType::LeftMouseDown
                {
                    // The message arrives a moment after the click; only the click itself can start a drag.
                    window.performWindowDragWithEvent(&event);
                }
            },
            move |_| {
                if let Some(window) = zoom_target.load() {
                    window.zoom(None);
                }
            },
        );

        let (background, glass) = glass_background(mtm, frame, 0.0);
        window.setContentView(Some(&background));
        match glass {
            Some(glass) => glass.setContentView(Some(&host.view)),
            None => background.addSubview(&host.view),
        }
        host.view.setFrame(background.bounds());

        // macOS 26: the page may ask for the system's glass under the places it leaves clear (`lay_glass`).
        if AnyClass::get(c"NSGlassEffectView").is_some() {
            let (window_ref, web_ref) = (
                Weak::from_retained(&window),
                Weak::from_retained(&host.view),
            );
            let bed = GlassBed::default();
            let handler = ScriptHandler::new(mtm, move |text| {
                if let (Some(window), Some(web)) = (window_ref.load(), web_ref.load()) {
                    lay_glass(mtm, &window, &web, &bed, &text);
                }
            });
            // SAFETY: plain calls on the controller of the web view this window created; `detach` removes it.
            unsafe {
                host.view
                    .configuration()
                    .userContentController()
                    .addScriptMessageHandler_name(
                        ProtocolObject::from_ref(&*handler),
                        &NSString::from_str("glass"),
                    );
            }
        }

        let delegate = WindowDelegate::new(mtm, on_close);
        window.setDelegate(Some(ProtocolObject::from_ref(&*delegate)));
        // The page takes the keyboard from the start, so Tab and shortcuts work without a click first.
        window.makeFirstResponder(Some(&host.view));
        window.center();
        Rc::new(Self {
            host,
            window,
            delegate,
        })
    }

    /// Brings the window to the front and makes this app active so it takes keyboard focus.
    pub fn show(&self, mtm: MainThreadMarker) {
        if self.window.isMiniaturized() {
            self.window.deminiaturize(None);
        }
        self.window.makeKeyAndOrderFront(None);
        #[allow(deprecated)]
        NSApplication::sharedApplication(mtm).activateIgnoringOtherApps(true);
    }

    pub fn is_visible(&self) -> bool {
        self.window.isVisible()
    }

    /// Dark (`Some(true)`) or light (`Some(false)`) whatever the system uses, or the system's own (`None`). The
    /// page follows, since WebKit reports the window's appearance to `prefers-color-scheme`.
    pub fn set_dark(&self, dark: Option<bool>) {
        // SAFETY: extern statics are process-lifetime constants.
        let appearance = dark.and_then(|dark| unsafe {
            NSAppearance::appearanceNamed(if dark {
                NSAppearanceNameDarkAqua
            } else {
                NSAppearanceNameAqua
            })
        });
        self.window.setAppearance(appearance.as_deref());
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

/// The width of the panel under the menu bar item; its height follows the page.
const TRAY_WIDTH: f64 = 276.0;
const TRAY_RADIUS: f64 = 18.0;
/// The gap between the menu bar and the panel, and between the panel and the edge of the screen.
const TRAY_GAP: f64 = 6.0;
const TRAY_MARGIN: f64 = 8.0;
const KEY_ESCAPE: u16 = 53;

/// A borderless glass panel with a web view, shown under a menu bar item. It never takes the focus from the
/// app in front, and goes away on a click anywhere else or on Escape.
pub struct TrayPanel {
    pub host: WebHost,
    panel: Retained<NSPanel>,
    monitors: RefCell<Vec<Retained<objc2::runtime::AnyObject>>>,
    shown: Cell<bool>,
}

impl TrayPanel {
    pub fn new(
        mtm: MainThreadMarker,
        html: &str,
        on_message: impl Fn(String) + 'static,
    ) -> Rc<Self> {
        let frame = NSRect::new(NSPoint::ZERO, NSSize::new(TRAY_WIDTH, 400.0));
        let panel = crate::capture_ui::panel(mtm, frame, NSStatusWindowLevel);
        let host = WebHost::new(mtm, frame, html, on_message, |_| {}, |_| {});
        let (background, glass) = glass_background(mtm, frame, TRAY_RADIUS);
        panel.setContentView(Some(&background));
        match glass {
            Some(glass) => glass.setContentView(Some(&host.view)),
            None => background.addSubview(&host.view),
        }
        host.view.setFrame(background.bounds());
        Rc::new(Self {
            host,
            panel,
            monitors: RefCell::new(Vec::new()),
            shown: Cell::new(false),
        })
    }

    pub fn is_visible(&self) -> bool {
        self.shown.get() && self.panel.isVisible()
    }

    /// Resizes to `height` points, keeping the top edge where it is.
    pub fn set_height(&self, height: f64) {
        let f = self.panel.frame();
        let top = f.origin.y + f.size.height;
        let frame = NSRect::new(
            NSPoint::new(f.origin.x, top - height),
            NSSize::new(TRAY_WIDTH, height),
        );
        self.panel.setFrame_display(frame, true);
    }

    /// Shows the panel under `anchor` (the menu bar item's window, in screen coordinates). A press on `anchor`
    /// does not close it: that is the item itself, which closes the panel by its own click.
    pub fn show_under(&self, mtm: MainThreadMarker, anchor: NSRect) {
        let mid = NSPoint::new(anchor.origin.x + anchor.size.width / 2.0, anchor.origin.y);
        let screen = NSScreen::screens(mtm)
            .iter()
            .find(|s| {
                let f = s.frame();
                mid.x >= f.origin.x
                    && mid.x <= f.origin.x + f.size.width
                    && mid.y >= f.origin.y
                    && mid.y <= f.origin.y + f.size.height + 1.0
            })
            .or_else(|| NSScreen::mainScreen(mtm));
        let height = self.panel.frame().size.height;
        let mut x = mid.x - TRAY_WIDTH / 2.0;
        if let Some(screen) = screen {
            let v = screen.visibleFrame();
            x = x.clamp(
                v.origin.x + TRAY_MARGIN,
                (v.origin.x + v.size.width - TRAY_WIDTH - TRAY_MARGIN).max(v.origin.x),
            );
        }
        let frame = NSRect::new(
            NSPoint::new(x, anchor.origin.y - TRAY_GAP - height),
            NSSize::new(TRAY_WIDTH, height),
        );
        self.panel.setFrame_display(frame, true);
        self.panel.orderFrontRegardless();
        self.shown.set(true);
        self.watch(mtm, anchor);
    }

    pub fn hide(&self) {
        self.unwatch();
        self.shown.set(false);
        self.panel.orderOut(None);
    }

    /// Closes the panel on a press outside it (in any app) or on Escape.
    fn watch(&self, mtm: MainThreadMarker, anchor: NSRect) {
        self.unwatch();
        // Where the pointer is, not which window the press is for: the system gives the menu bar item another
        // window number to the monitors of other apps than to this one, and a press that closed the panel
        // before the item's click came would only have it opened again by that click.
        let on_item = move || {
            let at = NSEvent::mouseLocation();
            at.x >= anchor.origin.x
                && at.x < anchor.origin.x + anchor.size.width
                && at.y >= anchor.origin.y
                && at.y < anchor.origin.y + anchor.size.height
        };
        *self.monitors.borrow_mut() = close_outside(&self.panel, on_item);
        let _ = mtm;
    }

    fn unwatch(&self) {
        for token in self.monitors.borrow_mut().drain(..) {
            // SAFETY: each token came from an add...Monitor call and is removed once.
            unsafe { NSEvent::removeMonitor(&token) };
        }
    }
}

impl Drop for TrayPanel {
    fn drop(&mut self) {
        self.unwatch();
        self.host.detach();
        self.panel.setContentView(None);
        self.panel.close();
    }
}

/// Orders `panel` out on a press outside it (in any app) or on Escape, except for a press `exempt` says to leave
/// alone. Returns the monitors, which the caller removes.
fn close_outside(
    panel: &Retained<NSPanel>,
    exempt: impl Fn() -> bool + Clone + 'static,
) -> Vec<Retained<objc2::runtime::AnyObject>> {
    let own = panel.windowNumber();
    let panel = Weak::from_retained(panel);
    let close = {
        let panel = panel.clone();
        move || {
            if let Some(panel) = panel.load() {
                panel.orderOut(None);
            }
        }
    };
    let mask = NSEventMask::LeftMouseDown
        | NSEventMask::RightMouseDown
        | NSEventMask::OtherMouseDown
        | NSEventMask::KeyDown;
    let outside = {
        let close = close.clone();
        let exempt = exempt.clone();
        RcBlock::new(move |event: NonNull<NSEvent>| {
            // SAFETY: AppKit passes a live event for the duration of the call.
            let event = unsafe { event.as_ref() };
            if event.r#type() == NSEventType::KeyDown {
                if event.keyCode() == KEY_ESCAPE {
                    close();
                }
            } else if !exempt() {
                close();
            }
        })
    };
    let inside = RcBlock::new(move |event: NonNull<NSEvent>| -> *mut NSEvent {
        // SAFETY: AppKit passes a live event for the duration of the call.
        let e = unsafe { event.as_ref() };
        let number = e.windowNumber();
        let key = e.r#type() == NSEventType::KeyDown;
        if (!key && number != own && !exempt()) || (key && e.keyCode() == KEY_ESCAPE) {
            close();
        }
        event.as_ptr()
    });
    let mut tokens = Vec::new();
    if let Some(token) = NSEvent::addGlobalMonitorForEventsMatchingMask_handler(mask, &outside) {
        tokens.push(token);
    }
    // SAFETY: the block returns the event it was given, which AppKit then delivers as usual.
    if let Some(token) =
        unsafe { NSEvent::addLocalMonitorForEventsMatchingMask_handler(mask, &inside) }
    {
        tokens.push(token);
    }
    tokens
}

/// A clear panel with a web view for a card in the middle of a screen, which may first fall from the notch as a drop.
/// It never takes the focus from the app in front, and goes away on a press anywhere else or on Escape.
pub struct DropPanel {
    pub host: WebHost,
    panel: Retained<NSPanel>,
    monitors: RefCell<Vec<Retained<objc2::runtime::AnyObject>>>,
    /// Where the card is, in screen coordinates.
    card: Cell<NSRect>,
}

impl DropPanel {
    pub fn new(
        mtm: MainThreadMarker,
        html: &str,
        on_message: impl Fn(String) + 'static,
    ) -> Rc<Self> {
        let frame = NSRect::new(NSPoint::ZERO, NSSize::new(400.0, 300.0));
        let panel = crate::capture_ui::panel(mtm, frame, NSStatusWindowLevel);
        // The page draws the card's shadow; the window's own would outline the clear column the drop falls in.
        panel.setHasShadow(false);
        let host = WebHost::new(mtm, frame, html, on_message, |_| {}, |_| {});
        panel.setContentView(Some(&host.view));
        Rc::new(Self {
            host,
            panel,
            monitors: RefCell::new(Vec::new()),
            card: Cell::new(frame),
        })
    }

    /// Shows a card of `size` in the middle of `screen`. With `fall` the panel first reaches from the card up to the
    /// top of the screen, where the notch is, and lets presses through while the page lets the drop fall; the page
    /// says when it has landed, and from then the panel is only the card. The page keeps the card at its bottom, so
    /// it does not move when the panel shrinks.
    pub fn show(&self, screen: NSRect, size: NSSize, fall: bool) {
        self.unwatch();
        let card = NSRect::new(
            NSPoint::new(
                screen.origin.x + (screen.size.width - size.width) / 2.0,
                screen.origin.y + (screen.size.height - size.height) / 2.0,
            ),
            size,
        );
        self.card.set(card);
        let frame = if fall {
            let top = screen.origin.y + screen.size.height;
            NSRect::new(card.origin, NSSize::new(size.width, top - card.origin.y))
        } else {
            card
        };
        self.panel.setIgnoresMouseEvents(fall);
        self.panel.setFrame_display(frame, true);
        self.panel.orderFrontRegardless();
        if !fall {
            self.landed();
        }
    }

    /// The drop has landed: the panel becomes the card, takes presses, and closes on one elsewhere.
    pub fn landed(&self) {
        if !self.panel.isVisible() {
            return;
        }
        self.panel.setFrame_display(self.card.get(), true);
        self.panel.setIgnoresMouseEvents(false);
        self.unwatch();
        *self.monitors.borrow_mut() = close_outside(&self.panel, || false);
    }

    pub fn is_visible(&self) -> bool {
        self.panel.isVisible()
    }

    pub fn hide(&self) {
        self.unwatch();
        self.panel.orderOut(None);
    }

    fn unwatch(&self) {
        for token in self.monitors.borrow_mut().drain(..) {
            // SAFETY: each token came from an add...Monitor call and is removed once.
            unsafe { NSEvent::removeMonitor(&token) };
        }
    }
}

impl Drop for DropPanel {
    fn drop(&mut self) {
        self.unwatch();
        self.host.detach();
        self.panel.setContentView(None);
        self.panel.close();
    }
}
