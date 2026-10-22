#![cfg_attr(windows, windows_subsystem = "windows")]
#![forbid(unsafe_code)]
//! Figghost for Windows and Linux, a beta: an icon in the tray, the clipboard history with a shortcut, notes, the coding
//! agents and the MCP server, in one web page window. The rules are the shared `core-*` crates of the macOS app; the
//! notch, the scrolling, the snapping and the other system features of macOS are not here yet.

mod agents;
mod config;
mod watch;

use config::{Config, MASK_MINUTES, home_dir};
use core_agents::{Agent, HookEvent, server::Ticket, socket_path};
use core_clipboard::{Payload, Sensitivity, Store};
use core_notes::{Folder, Note};
use global_hotkey::{GlobalHotKeyEvent, GlobalHotKeyManager, HotKeyState, hotkey::HotKey};
use serde_json::{Value, json};
use std::{
    borrow::Cow,
    env,
    fs::{self, File},
    io,
    path::{Path, PathBuf},
    process::Command,
    str::FromStr,
    sync::Mutex,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};
use tao::{
    dpi::LogicalSize,
    event::{Event, StartCause, WindowEvent},
    event_loop::{ControlFlow, EventLoopBuilder, EventLoopProxy, EventLoopWindowTarget},
    window::{Window, WindowBuilder},
};
use tray_icon::{
    MouseButton, MouseButtonState, TrayIcon, TrayIconBuilder, TrayIconEvent,
    menu::{Menu, MenuEvent, MenuId, MenuItem, PredefinedMenuItem},
};
use watch::{SHOW_REQUEST, Watched};
use wry::{
    WebView, WebViewBuilder,
    http::{Request, Response, header::CONTENT_TYPE},
};

const ICON: &[u8] = include_bytes!("../assets/icon-64.png");
const WINDOW_ICON: &[u8] = include_bytes!("../assets/icon-256.png");
const INDEX_HTML: &str = include_str!("../web/index.html");
const APP_JS: &str = include_str!("../web/app.js");
const APP_CSS: &str = include_str!("../web/app.css");
/// Windows serves a custom protocol of the web view under http.
const PAGE_URL: &str = if cfg!(windows) {
    "http://figghost.localhost/"
} else {
    "figghost://localhost/"
};
/// While the window is open or an agent may ask something, the state is read again this often.
const BUSY_TICK: Duration = Duration::from_secs(2);
/// Otherwise only masked secrets expire, which can wait.
const IDLE_TICK: Duration = Duration::from_secs(60);
const SEARCH_LIMIT: usize = 200;

enum UserEvent {
    Ipc(String),
    Menu(MenuId),
    TrayClick,
    HotKey,
    Watched(Watched),
    Hook(HookEvent, Option<Ticket>),
}

fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_millis() as i64)
}

/// `en` or `vi`: the setting, or the language of the system.
fn language(config: &Config) -> &'static str {
    let tag = match config.language.as_str() {
        "auto" => sys_locale::get_locale().unwrap_or_default(),
        other => other.to_owned(),
    };
    if tag.to_lowercase().starts_with("vi") {
        "vi"
    } else {
        "en"
    }
}

fn main() {
    let home = home_dir();
    let args: Vec<String> = env::args().collect();
    // The coding agents start the app this way for each hook event and for the status line: no window.
    if let Some(at) = args.iter().position(|a| a == "--figghost-hook") {
        let agent = args.get(at + 1).and_then(|id| Agent::from_id(id));
        std::process::exit(agent.map_or(0, |agent| {
            core_agents::cli::run_hook(
                agent,
                io::stdin().lock(),
                &mut io::stdout().lock(),
                &socket_path(&home),
                now_ms(),
                |key| env::var(key).ok(),
            )
        }));
    }
    if args.iter().any(|a| a == "--figghost-statusline") {
        std::process::exit(core_agents::cli::run_statusline(
            io::stdin().lock(),
            &mut io::stdout().lock(),
            &home,
            now_ms(),
        ));
    }
    // An AI assistant starts the app this way to search the clipboard history over stdin and stdout.
    if args.iter().any(|a| a == "--mcp") {
        let dir = home.clone();
        std::process::exit(core_clipboard::mcp::run(&home, move || {
            Config::load(&dir).mcp.enabled
        }));
    }
    if let Err(e) = fs::create_dir_all(&home) {
        eprintln!("figghost: cannot make {}: {e}", home.display());
        std::process::exit(1);
    }
    // A second start asks the running app to show its window and leaves.
    let Some(_lock) = single_instance(&home) else {
        let _ = fs::write(home.join(SHOW_REQUEST), "");
        return;
    };
    let config = Config::load(&home);
    let mut store = match Store::open(&home) {
        Ok(store) => store,
        Err(e) => {
            eprintln!("figghost: cannot open the clipboard history: {e}");
            std::process::exit(1);
        }
    };
    let _ = store.set_limit(config.history_limit);

    let event_loop = EventLoopBuilder::<UserEvent>::with_user_event().build();
    let proxy = event_loop.create_proxy();
    let send = |proxy: &EventLoopProxy<UserEvent>| {
        let proxy = Mutex::new(proxy.clone());
        move |event: UserEvent| {
            if let Ok(proxy) = proxy.lock() {
                let _ = proxy.send_event(event);
            }
        }
    };
    {
        let send = send(&proxy);
        MenuEvent::set_event_handler(Some(move |e: MenuEvent| send(UserEvent::Menu(e.id))));
    }
    {
        let send = send(&proxy);
        TrayIconEvent::set_event_handler(Some(move |e: TrayIconEvent| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = e
            {
                send(UserEvent::TrayClick);
            }
        }));
    }
    {
        let send = send(&proxy);
        GlobalHotKeyEvent::set_event_handler(Some(move |e: GlobalHotKeyEvent| {
            if e.state() == HotKeyState::Pressed {
                send(UserEvent::HotKey);
            }
        }));
    }
    {
        let send = send(&proxy);
        watch::start(home.clone(), move |w| send(UserEvent::Watched(w)));
    }

    let show_now = !args.iter().any(|a| a == "--background");
    let mut app = App {
        agents: agents::Agents::new(&home),
        notes: Folder::new(home.join("notes")),
        board: arboard::Clipboard::new()
            .inspect_err(|e| eprintln!("clipboard: {e}"))
            .ok(),
        hotkeys: GlobalHotKeyManager::new()
            .inspect_err(|e| eprintln!("hotkey: {e}"))
            .ok(),
        hotkey: None,
        hotkey_error: None,
        autostart: autostart(),
        home,
        config,
        store,
        proxy,
        window: None,
        pending_page: None,
        tray: None,
        menu: Vec::new(),
        agents_error: None,
        last_agents: String::new(),
    };
    app.register_hotkey();
    app.apply_agents();

    let mut next_tick = Instant::now();
    event_loop.run(move |event, target, flow| {
        match event {
            Event::NewEvents(StartCause::Init) => {
                app.build_tray();
                if show_now {
                    app.show(target, "clipboard");
                }
            }
            Event::UserEvent(UserEvent::Menu(id)) => {
                if app.menu_quit(&id) {
                    *flow = ControlFlow::Exit;
                    return;
                }
                if let Some(page) = app.menu_page(&id) {
                    app.show(target, page);
                }
            }
            Event::UserEvent(event) => app.on(event, target),
            Event::WindowEvent {
                event: WindowEvent::CloseRequested,
                ..
            } => app.close_window(),
            _ => {}
        }
        if Instant::now() >= next_tick {
            app.tick();
            next_tick = Instant::now() + app.tick_every();
        }
        *flow = ControlFlow::WaitUntil(next_tick);
    });
}

/// Holds a lock on a file of the data directory for as long as the app runs.
fn single_instance(home: &Path) -> Option<File> {
    let file = File::options()
        .create(true)
        .truncate(false)
        .write(true)
        .open(home.join("figghost.lock"))
        .ok()?;
    file.try_lock().ok()?;
    Some(file)
}

fn autostart() -> Option<auto_launch::AutoLaunch> {
    let exe = env::current_exe().ok()?;
    auto_launch::AutoLaunchBuilder::new()
        .set_app_name("Figghost")
        .set_app_path(&exe.to_string_lossy())
        .set_args(&["--background"])
        .build()
        .inspect_err(|e| eprintln!("autostart: {e}"))
        .ok()
}

fn decode_png(bytes: &[u8]) -> Option<(Vec<u8>, u32, u32)> {
    let image = image::load_from_memory_with_format(bytes, image::ImageFormat::Png)
        .ok()?
        .to_rgba8();
    let (width, height) = image.dimensions();
    Some((image.into_raw(), width, height))
}

/// The page, its script and style, the icon, and the pictures of the history (by their hashed names only).
fn serve(images: &Path, request: &Request<Vec<u8>>) -> Response<Cow<'static, [u8]>> {
    let path = request.uri().path();
    let found: Option<(&str, Cow<'static, [u8]>)> = match path {
        "/" | "/index.html" => Some(("text/html; charset=utf-8", INDEX_HTML.as_bytes().into())),
        "/app.js" => Some(("text/javascript; charset=utf-8", APP_JS.as_bytes().into())),
        "/app.css" => Some(("text/css; charset=utf-8", APP_CSS.as_bytes().into())),
        "/icon.png" => Some(("image/png", WINDOW_ICON.into())),
        _ => path
            .strip_prefix("/image/")
            .filter(|name| is_image_name(name))
            .and_then(|name| fs::read(images.join(name)).ok())
            .map(|bytes| ("image/png", bytes.into())),
    };
    let (kind, body) = found.unwrap_or(("text/plain", b"not found".as_slice().into()));
    Response::builder()
        .status(if path == "/" || kind != "text/plain" { 200 } else { 404 })
        .header(CONTENT_TYPE, kind)
        .body(body)
        .unwrap_or_else(|_| Response::new(Cow::Borrowed(b"")))
}

/// The store names its pictures `<16 hex digits>.png`; nothing else is read from the disk.
fn is_image_name(name: &str) -> bool {
    name.strip_suffix(".png")
        .is_some_and(|stem| stem.len() == 16 && stem.bytes().all(|b| b.is_ascii_hexdigit()))
}

/// Opens a folder in the file manager.
fn reveal(path: &Path) {
    let program = if cfg!(windows) {
        "explorer"
    } else if cfg!(target_os = "macos") {
        "open"
    } else {
        "xdg-open"
    };
    if let Err(e) = Command::new(program).arg(path).spawn() {
        eprintln!("cannot open {}: {e}", path.display());
    }
}

struct App {
    home: PathBuf,
    config: Config,
    store: Store,
    notes: Folder,
    agents: agents::Agents,
    agents_error: Option<String>,
    board: Option<arboard::Clipboard>,
    hotkeys: Option<GlobalHotKeyManager>,
    hotkey: Option<HotKey>,
    hotkey_error: Option<String>,
    autostart: Option<auto_launch::AutoLaunch>,
    proxy: EventLoopProxy<UserEvent>,
    window: Option<(Window, WebView)>,
    /// A page asked for before the page's script said it was ready.
    pending_page: Option<String>,
    tray: Option<TrayIcon>,
    /// The tray menu entries: the page each opens (empty for Quit) and the item, kept to rename them.
    menu: Vec<(&'static str, MenuItem)>,
    /// The agents' state last sent to the page.
    last_agents: String,
}

impl App {
    fn label(&self, key: &str) -> &'static str {
        let vi = language(&self.config) == "vi";
        match (key, vi) {
            ("clipboard", false) => "Clipboard history",
            ("clipboard", true) => "Lịch sử clipboard",
            ("notes", false) => "Notes",
            ("notes", true) => "Ghi chú",
            ("agents", false) => "AI agents",
            ("agents", true) => "AI agents",
            ("settings", false) => "Settings",
            ("settings", true) => "Cài đặt",
            ("", false) => "Quit Figghost",
            ("", true) => "Thoát Figghost",
            _ => "",
        }
    }

    fn build_tray(&mut self) {
        let menu = Menu::new();
        for page in ["clipboard", "notes", "agents", "settings"] {
            let item = MenuItem::new(self.label(page), true, None);
            let _ = menu.append(&item);
            self.menu.push((page, item));
        }
        let _ = menu.append(&PredefinedMenuItem::separator());
        let quit = MenuItem::new(self.label(""), true, None);
        let _ = menu.append(&quit);
        self.menu.push(("", quit));
        let icon = decode_png(ICON)
            .and_then(|(rgba, w, h)| tray_icon::Icon::from_rgba(rgba, w, h).ok());
        let mut builder = TrayIconBuilder::new()
            .with_menu(Box::new(menu))
            .with_tooltip("Figghost (beta)")
            .with_menu_on_left_click(false);
        if let Some(icon) = icon {
            builder = builder.with_icon(icon);
        }
        match builder.build() {
            Ok(tray) => self.tray = Some(tray),
            Err(e) => eprintln!("tray: {e}"),
        }
    }

    fn relabel_tray(&self) {
        for (page, item) in &self.menu {
            item.set_text(self.label(page));
        }
    }

    fn menu_quit(&self, id: &MenuId) -> bool {
        self.menu.iter().any(|(page, item)| page.is_empty() && item.id() == id)
    }

    fn menu_page(&self, id: &MenuId) -> Option<&'static str> {
        self.menu
            .iter()
            .find(|(page, item)| !page.is_empty() && item.id() == id)
            .map(|(page, _)| *page)
    }

    fn register_hotkey(&mut self) {
        let Some(manager) = &self.hotkeys else {
            self.hotkey_error = Some("global shortcuts are not available here".into());
            return;
        };
        if let Some(old) = self.hotkey.take() {
            let _ = manager.unregister(old);
        }
        self.hotkey_error = None;
        if self.config.hotkey.trim().is_empty() {
            return;
        }
        match HotKey::from_str(&self.config.hotkey) {
            Ok(hotkey) => match manager.register(hotkey) {
                Ok(()) => self.hotkey = Some(hotkey),
                Err(e) => self.hotkey_error = Some(e.to_string()),
            },
            Err(e) => self.hotkey_error = Some(e.to_string()),
        }
    }

    fn apply_agents(&mut self) {
        let proxy = Mutex::new(self.proxy.clone());
        let applied = self.agents.set_enabled(
            self.config.agents.enabled,
            self.config.agents.approval_wait_seconds,
            move |event, ticket| {
                if let Ok(proxy) = proxy.lock() {
                    let _ = proxy.send_event(UserEvent::Hook(event, ticket));
                }
            },
        );
        self.agents_error = applied.err();
    }

    fn sensitivity(&self) -> Sensitivity {
        match self.config.secrets.as_str() {
            "keep" => Sensitivity::Keep,
            "skip" => Sensitivity::Skip,
            _ => Sensitivity::Mask {
                minutes: MASK_MINUTES,
            },
        }
    }

    fn tick_every(&self) -> Duration {
        if self.window.is_some() || self.config.agents.enabled {
            BUSY_TICK
        } else {
            IDLE_TICK
        }
    }

    fn tick(&mut self) {
        let now = now_ms();
        let _ = self.store.purge_expired(now);
        let waiting = self.agents.waiting();
        self.agents.sweep(now);
        if self.agents.waiting() != waiting {
            self.agents_changed();
        } else if self.window.is_some() {
            self.post_agents();
        }
    }

    /// Sends what the agents are doing, only when it changed, so that the page is not drawn again for nothing.
    fn post_agents(&mut self) {
        let agents = self.agents.state();
        let text = agents.to_string();
        if text != self.last_agents {
            self.last_agents = text;
            self.post(&json!({ "t": "agents", "agents": agents }));
        }
    }

    fn agents_changed(&mut self) {
        let waiting = self.agents.waiting();
        if let Some(tray) = &self.tray {
            let tip = match (waiting, language(&self.config)) {
                (0, _) => "Figghost (beta)".to_owned(),
                (n, "vi") => format!("Figghost: {n} câu hỏi đang chờ"),
                (n, _) => format!("Figghost: {n} waiting for you"),
            };
            let _ = tray.set_tooltip(Some(tip));
        }
        self.post_agents();
    }

    fn on(&mut self, event: UserEvent, target: &EventLoopWindowTarget<UserEvent>) {
        match event {
            UserEvent::Ipc(text) => self.handle(&text),
            UserEvent::TrayClick | UserEvent::Watched(Watched::ShowAsked) => {
                self.show(target, "clipboard");
            }
            UserEvent::HotKey => {
                let focused = self
                    .window
                    .as_ref()
                    .is_some_and(|(w, _)| w.is_visible() && w.is_focused());
                if focused {
                    self.close_window();
                } else {
                    self.show(target, "clipboard");
                    self.post(&json!({ "t": "focus_search" }));
                }
            }
            UserEvent::Watched(Watched::Copied(capture)) => {
                match self.store.record_with(capture, now_ms(), self.sensitivity()) {
                    Ok(Some(_)) => self.post(&json!({ "t": "changed" })),
                    Ok(None) => {}
                    Err(e) => eprintln!("clipboard: {e}"),
                }
            }
            UserEvent::Hook(event, ticket) => {
                if self.agents.receive(&event, ticket) || self.window.is_some() {
                    self.agents_changed();
                }
            }
            UserEvent::Menu(_) => {}
        }
    }

    fn show(&mut self, target: &EventLoopWindowTarget<UserEvent>, page: &str) {
        if self.window.is_none() {
            match self.open_window(target) {
                Ok(pair) => self.window = Some(pair),
                Err(e) => {
                    eprintln!("window: {e}");
                    return;
                }
            }
            self.pending_page = Some(page.to_owned());
            return;
        }
        if let Some((window, _)) = &self.window {
            window.set_visible(true);
            window.set_minimized(false);
            window.set_focus();
        }
        self.post(&json!({ "t": "go", "page": page }));
    }

    fn open_window(
        &self,
        target: &EventLoopWindowTarget<UserEvent>,
    ) -> Result<(Window, WebView), String> {
        let icon = decode_png(WINDOW_ICON)
            .and_then(|(rgba, w, h)| tao::window::Icon::from_rgba(rgba, w, h).ok());
        let window = WindowBuilder::new()
            .with_title("Figghost (beta)")
            .with_inner_size(LogicalSize::new(980.0, 660.0))
            .with_min_inner_size(LogicalSize::new(720.0, 480.0))
            .with_window_icon(icon)
            .build(target)
            .map_err(|e| e.to_string())?;
        let images = self.home.join("images");
        let proxy = self.proxy.clone();
        let builder = WebViewBuilder::new()
            .with_custom_protocol("figghost".into(), move |_, request| {
                serve(&images, &request)
            })
            .with_url(PAGE_URL)
            .with_ipc_handler(move |request: Request<String>| {
                let _ = proxy.send_event(UserEvent::Ipc(request.into_body()));
            })
            // The page never leaves itself.
            .with_navigation_handler(|url| url.starts_with(PAGE_URL))
            .with_devtools(cfg!(debug_assertions));
        #[cfg(not(target_os = "linux"))]
        let webview = builder.build(&window);
        #[cfg(target_os = "linux")]
        let webview = {
            use tao::platform::unix::WindowExtUnix;
            use wry::WebViewBuilderExtUnix;
            let vbox = window.default_vbox().ok_or("no GTK box in the window")?;
            builder.build_gtk(vbox)
        };
        let webview = webview.map_err(|e| e.to_string())?;
        Ok((window, webview))
    }

    /// The window goes away entirely, so that the web view costs nothing while the app sits in the tray.
    fn close_window(&mut self) {
        self.window = None;
    }

    fn post(&self, message: &Value) {
        if let Some((_, webview)) = &self.window {
            let _ = webview.evaluate_script(&format!("window.fig && fig.receive({message})"));
        }
    }

    fn state(&self) -> Value {
        let exe = env::current_exe()
            .map(|p| p.to_string_lossy().into_owned())
            .unwrap_or_default();
        let os = if cfg!(windows) {
            "windows"
        } else if cfg!(target_os = "macos") {
            "macos"
        } else {
            "linux"
        };
        json!({
            "t": "state",
            "env": {
                "os": os,
                "version": env!("CARGO_PKG_VERSION"),
                "lang": language(&self.config),
                "wayland": env::var_os("WAYLAND_DISPLAY").is_some(),
                "hotkey_error": self.hotkey_error,
                "autostart": self.autostart.as_ref().and_then(|a| a.is_enabled().ok()),
                "exe": exe,
                "data_dir": self.home.to_string_lossy(),
                "count": self.store.count().unwrap_or(0),
                "agents_error": self.agents_error,
            },
            "config": self.config,
            "agents": self.agents.state(),
        })
    }

    fn items(&self, query: &str) -> Value {
        let items = self.store.search(query, SEARCH_LIMIT).unwrap_or_default();
        Value::Array(
            items
                .iter()
                .map(|i| {
                    json!({
                        "id": i.id,
                        "kind": i.kind.as_str(),
                        "preview": i.preview,
                        "image": i.image,
                        "width": i.width,
                        "height": i.height,
                        "used": i.used_at,
                        "pinned": i.pinned,
                        "masked": i.masked,
                    })
                })
                .collect(),
        )
    }

    fn notes_state(&self) -> Value {
        Value::Array(
            self.notes
                .load()
                .iter()
                .map(|n| json!({ "id": n.id, "text": n.text, "updated": n.updated }))
                .collect(),
        )
    }

    /// Puts a history item back on the clipboard. The watcher then sees it and moves it to the top.
    fn copy(&mut self, id: i64) -> Result<(), String> {
        let board = self.board.as_mut().ok_or("no clipboard")?;
        let payload = self.store.payload(id).map_err(|e| e.to_string())?;
        match payload.ok_or("the item is gone")? {
            Payload::Text(text) => board.set_text(text),
            Payload::Files(paths) => board.set().file_list(&paths),
            Payload::Image { png, .. } => {
                let (rgba, width, height) = decode_png(&png).ok_or("the picture cannot be read")?;
                board.set_image(arboard::ImageData {
                    width: width as usize,
                    height: height as usize,
                    bytes: rgba.into(),
                })
            }
        }
        .map_err(|e| e.to_string())?;
        let _ = self.store.touch(id, now_ms());
        Ok(())
    }

    fn set(&mut self, key: &str, value: &Value) -> Result<(), String> {
        self.config.set(key, value)?;
        self.config.save(&self.home).map_err(|e| e.to_string())?;
        match key {
            "language" => self.relabel_tray(),
            "hotkey" => self.register_hotkey(),
            "history_limit" => {
                let _ = self.store.set_limit(self.config.history_limit);
            }
            "agents.enabled" | "agents.approval_wait_seconds" => self.apply_agents(),
            _ => {}
        }
        Ok(())
    }

    fn handle(&mut self, text: &str) {
        let Ok(message) = serde_json::from_str::<Value>(text) else {
            return;
        };
        let id = message["id"].as_i64().unwrap_or(-1);
        let done: Result<(), String> = match message["t"].as_str().unwrap_or("") {
            "ready" => {
                self.last_agents.clear();
                self.post(&self.state());
                if let Some(page) = self.pending_page.take() {
                    self.post(&json!({ "t": "go", "page": page }));
                }
                Ok(())
            }
            "search" => {
                let query = message["q"].as_str().unwrap_or("");
                self.post(&json!({ "t": "items", "q": query, "items": self.items(query) }));
                Ok(())
            }
            "copy" => self.copy(id).map(|()| self.close_window()),
            "pin" => self.store.toggle_pinned(id).map_err(|e| e.to_string()),
            "delete" => self.store.delete(id).map_err(|e| e.to_string()),
            "clear" => self.store.clear_unpinned().map_err(|e| e.to_string()),
            "set" => {
                let key = message["key"].as_str().unwrap_or("");
                let set = self.set(key, &message["value"]);
                self.post(&self.state());
                set
            }
            "autostart" => match &self.autostart {
                Some(auto) => {
                    let on = message["value"].as_bool().unwrap_or(false);
                    let changed = if on { auto.enable() } else { auto.disable() };
                    self.post(&self.state());
                    changed.map_err(|e| e.to_string())
                }
                None => Err("starting at login is not available here".into()),
            },
            "open_data" => {
                reveal(&self.home);
                Ok(())
            }
            "notes" => {
                self.post(&json!({ "t": "notes", "notes": self.notes_state() }));
                Ok(())
            }
            "note_save" => {
                let note = Note {
                    id: message["id"].as_u64().unwrap_or(0),
                    text: message["text"].as_str().unwrap_or("").to_owned(),
                    updated: now_ms(),
                };
                if note.id == 0 {
                    Err("a note needs an id".into())
                } else {
                    self.notes.save(&note).map_err(|e| e.to_string())
                }
            }
            "note_delete" => {
                let id = message["id"].as_u64().unwrap_or(0);
                let removed = self.notes.remove(id, |path| {
                    trash::delete(path).map_err(io::Error::other)
                });
                self.post(&json!({ "t": "notes", "notes": self.notes_state() }));
                removed.map_err(|e| e.to_string())
            }
            "agent_install" | "agent_remove" => {
                let agent = message["agent"].as_str().and_then(Agent::from_id);
                let exe = env::current_exe().map(|p| p.to_string_lossy().into_owned());
                let done = match (agent, exe) {
                    (Some(agent), Ok(exe)) if message["t"] == "agent_install" => {
                        self.agents.install(agent, &exe)
                    }
                    (Some(agent), Ok(_)) => self.agents.remove(agent),
                    _ => Err("unknown agent".into()),
                };
                self.post_agents();
                done
            }
            "answer" => {
                let allow = message["allow"].as_bool().unwrap_or(false);
                self.agents.answer(id.max(0) as u64, allow);
                self.agents_changed();
                Ok(())
            }
            other => Err(format!("unknown message {other}")),
        };
        if let Err(error) = done {
            eprintln!("figghost: {error}");
            self.post(&json!({ "t": "error", "message": error }));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_the_store_s_own_picture_names_are_served() {
        assert!(is_image_name("00ff00ff00ff00ff.png"));
        assert!(!is_image_name("../config.json"));
        assert!(!is_image_name("..%2f..%2fsecret.png"));
        assert!(!is_image_name("00ff00ff00ff00f.png"));
        let images = env::temp_dir();
        let request = Request::builder()
            .uri("figghost://localhost/image/../config.json")
            .body(Vec::new())
            .unwrap();
        assert_eq!(serve(&images, &request).status(), 404);
        let page = Request::builder()
            .uri("figghost://localhost/")
            .body(Vec::new())
            .unwrap();
        assert_eq!(serve(&images, &page).status(), 200);
    }
}
