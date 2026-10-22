use block2::RcBlock;
use objc2::{
    MainThreadMarker,
    rc::Retained,
    runtime::{AnyObject, Bool, ProtocolObject},
};
use objc2_app_kit::{
    NSApplicationActivationPolicy, NSImage, NSModalResponseOK, NSOpenPanel, NSScreen, NSWorkspace,
    NSWorkspaceActiveSpaceDidChangeNotification, NSWorkspaceDidActivateApplicationNotification,
};
use objc2_foundation::{
    NSArray, NSBundle, NSFileManager, NSNotification, NSNotificationCenter, NSObjectProtocol,
    NSRect, NSSize, NSString, NSURL,
};
use std::{
    path::{Path, PathBuf},
    ptr::NonNull,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct App {
    pub bundle_id: String,
    pub name: String,
    pub pid: i32,
}

pub fn frontmost_app() -> Option<App> {
    let app = NSWorkspace::sharedWorkspace().frontmostApplication()?;
    Some(App {
        bundle_id: app.bundleIdentifier()?.to_string(),
        name: app
            .localizedName()
            .map(|n| n.to_string())
            .unwrap_or_default(),
        pid: app.processIdentifier(),
    })
}

/// Opens a file or folder with its default app (Finder for folders).
pub fn open_path(path: &std::path::Path) {
    let url = NSURL::fileURLWithPath(&NSString::from_str(&path.to_string_lossy()));
    NSWorkspace::sharedWorkspace().openURL(&url);
}

/// Keeps a workspace observer registered; unregisters on drop.
pub struct Observer {
    center: Retained<NSNotificationCenter>,
    token: Retained<ProtocolObject<dyn NSObjectProtocol>>,
}

impl Drop for Observer {
    fn drop(&mut self) {
        // SAFETY: `token` was returned by this center's addObserverForName.
        unsafe {
            self.center
                .removeObserver(AsRef::<AnyObject>::as_ref(&*self.token))
        };
    }
}

/// Calls `on_change` on the main thread whenever another app becomes frontmost.
pub fn observe_frontmost(on_change: impl Fn(App) + 'static) -> Observer {
    let block = RcBlock::new(move |_: NonNull<NSNotification>| {
        if let Some(app) = frontmost_app() {
            on_change(app);
        }
    });
    let center = NSWorkspace::sharedWorkspace().notificationCenter();
    // SAFETY: the block is 'static and is invoked on the main thread, where the notification is posted.
    let token = unsafe {
        center.addObserverForName_object_queue_usingBlock(
            Some(NSWorkspaceDidActivateApplicationNotification),
            None,
            None,
            &block,
        )
    };
    Observer { center, token }
}

/// Calls `on_change` on the main thread whenever another space comes to the front, such as when an app goes full
/// screen or leaves it.
pub fn observe_space(on_change: impl Fn() + 'static) -> Observer {
    let block = RcBlock::new(move |_: NonNull<NSNotification>| on_change());
    let center = NSWorkspace::sharedWorkspace().notificationCenter();
    // SAFETY: the block is 'static and is invoked on the main thread, where the notification is posted.
    let token = unsafe {
        center.addObserverForName_object_queue_usingBlock(
            Some(NSWorkspaceActiveSpaceDidChangeNotification),
            None,
            None,
            &block,
        )
    };
    Observer { center, token }
}

/// Shows the file in Finder, selected.
pub fn reveal(path: &Path) {
    let url = NSURL::fileURLWithPath(&NSString::from_str(&path.to_string_lossy()));
    NSWorkspace::sharedWorkspace()
        .activateFileViewerSelectingURLs(&NSArray::from_retained_slice(&[url]));
}

/// The system alert sound, for an action that did not work.
/// Moves a file to the Trash, where the person can still get it back.
pub fn trash(path: &Path) -> std::io::Result<()> {
    let url = NSURL::fileURLWithPath(&NSString::from_str(&path.to_string_lossy()));
    NSFileManager::defaultManager()
        .trashItemAtURL_resultingItemURL_error(&url, None)
        .map_err(|e| std::io::Error::other(e.localizedDescription().to_string()))
}

pub fn beep() {
    objc2_app_kit::NSBeep();
}

/// Plays one of the system sounds by name, such as "Glass" or "Ping".
pub fn play_sound(name: &str) {
    if let Some(sound) = objc2_app_kit::NSSound::soundNamed(&NSString::from_str(name)) {
        sound.play();
    }
}

/// Opens a URL with its default handler, such as a System Settings pane.
pub fn open_url(url: &str) {
    if let Some(url) = NSURL::URLWithString(&NSString::from_str(url)) {
        NSWorkspace::sharedWorkspace().openURL(&url);
    }
}

/// Where the app with this bundle identifier is installed.
pub fn app_path(bundle_id: &str) -> Option<PathBuf> {
    let url = NSWorkspace::sharedWorkspace()
        .URLForApplicationWithBundleIdentifier(&NSString::from_str(bundle_id))?;
    url.path().map(|p| PathBuf::from(p.to_string()))
}

/// The name Finder shows for the app, or `None` when it is not installed.
pub fn app_name(bundle_id: &str) -> Option<String> {
    let path = app_path(bundle_id)?;
    let name = NSFileManager::defaultManager()
        .displayNameAtPath(&NSString::from_str(&path.to_string_lossy()))
        .to_string();
    Some(
        name.strip_suffix(".app")
            .map_or(name.clone(), str::to_owned),
    )
}

/// The bundle identifier and the name Finder shows of the app bundle at this path.
pub fn bundle_info(path: &Path) -> Option<(String, String)> {
    let text = NSString::from_str(&path.to_string_lossy());
    let id = NSBundle::bundleWithPath(&text)?
        .bundleIdentifier()?
        .to_string();
    let name = NSFileManager::defaultManager()
        .displayNameAtPath(&text)
        .to_string();
    let name = name
        .strip_suffix(".app")
        .map_or(name.clone(), str::to_owned);
    Some((id, name))
}

/// Apps with a Dock icon that are running now, without this one.
pub fn running_apps() -> Vec<App> {
    let ours = std::process::id() as i32;
    NSWorkspace::sharedWorkspace()
        .runningApplications()
        .iter()
        .filter(|app| {
            app.activationPolicy() == NSApplicationActivationPolicy::Regular
                && app.processIdentifier() != ours
        })
        .filter_map(|app| {
            Some(App {
                bundle_id: app.bundleIdentifier()?.to_string(),
                name: app
                    .localizedName()
                    .map(|n| n.to_string())
                    .unwrap_or_default(),
                pid: app.processIdentifier(),
            })
        })
        .collect()
}

/// Lets the user choose an app from disk. Blocks until the dialog closes; `None` if cancelled.
pub fn pick_app(mtm: MainThreadMarker, prompt: &str) -> Option<App> {
    let panel = NSOpenPanel::openPanel(mtm);
    panel.setCanChooseFiles(true);
    panel.setCanChooseDirectories(false);
    panel.setAllowsMultipleSelection(false);
    panel.setMessage(Some(&NSString::from_str(prompt)));
    #[allow(deprecated)]
    panel.setAllowedFileTypes(Some(&NSArray::from_slice(&[&*NSString::from_str("app")])));
    panel.setDirectoryURL(Some(&NSURL::fileURLWithPath(&NSString::from_str(
        "/Applications",
    ))));
    if panel.runModal() != NSModalResponseOK {
        return None;
    }
    let url = panel.URLs().firstObject()?;
    let path = url.path()?.to_string();
    let bundle = NSBundle::bundleWithPath(&NSString::from_str(&path))?;
    let bundle_id = bundle.bundleIdentifier()?.to_string();
    let name = app_name(&bundle_id)
        .or_else(|| {
            Path::new(&path)
                .file_stem()
                .map(|s| s.to_string_lossy().into_owned())
        })
        .unwrap_or_default();
    Some(App {
        bundle_id,
        name,
        pid: 0,
    })
}

/// The bundle identifier of this app, when it runs from a bundle.
pub fn own_bundle_id() -> Option<String> {
    NSBundle::mainBundle()
        .bundleIdentifier()
        .map(|id| id.to_string())
}

/// Lets the user choose a folder. Blocks until the dialog closes; `None` if cancelled.
pub fn pick_folder(mtm: MainThreadMarker, prompt: &str) -> Option<PathBuf> {
    let panel = NSOpenPanel::openPanel(mtm);
    panel.setCanChooseFiles(false);
    panel.setCanChooseDirectories(true);
    panel.setCanCreateDirectories(false);
    panel.setAllowsMultipleSelection(false);
    panel.setMessage(Some(&NSString::from_str(prompt)));
    if panel.runModal() != NSModalResponseOK {
        return None;
    }
    let url = panel.URLs().firstObject()?;
    Some(PathBuf::from(url.path()?.to_string()))
}

/// The icon the Finder shows for a file or folder, `points` square.
pub fn icon_of(path: &Path, points: f64) -> Retained<NSImage> {
    let icon =
        NSWorkspace::sharedWorkspace().iconForFile(&NSString::from_str(&path.to_string_lossy()));
    icon.setSize(NSSize::new(points, points));
    icon
}

/// The app's icon as PNG bytes, `size` pixels square.
pub fn app_icon_png(bundle_id: &str, size: f64) -> Option<Vec<u8>> {
    let path = app_path(bundle_id)?;
    let icon =
        NSWorkspace::sharedWorkspace().iconForFile(&NSString::from_str(&path.to_string_lossy()));
    let drawn = NSImage::imageWithSize_flipped_drawingHandler(
        NSSize::new(size, size),
        false,
        &RcBlock::new(move |rect: NSRect| {
            icon.drawInRect(rect);
            Bool::YES
        }),
    );
    crate::webview::png_of(&drawn)
}

/// The accessibility display preferences the web interface has to honour itself.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct DisplayOptions {
    pub reduce_transparency: bool,
    pub reduce_motion: bool,
    pub increase_contrast: bool,
}

pub fn display_options() -> DisplayOptions {
    let workspace = NSWorkspace::sharedWorkspace();
    DisplayOptions {
        reduce_transparency: workspace.accessibilityDisplayShouldReduceTransparency(),
        reduce_motion: workspace.accessibilityDisplayShouldReduceMotion(),
        increase_contrast: workspace.accessibilityDisplayShouldIncreaseContrast(),
    }
}

/// The names of the screens, main first, as the notch's display setting offers them.
pub fn screen_names(mtm: MainThreadMarker) -> Vec<String> {
    NSScreen::screens(mtm)
        .iter()
        .map(|s| s.localizedName().to_string())
        .collect()
}

/// The app running as `pid`, when it is one the user can see: it has a Dock icon or a menu bar item.
pub fn running_app(pid: i32) -> Option<App> {
    let app = objc2_app_kit::NSRunningApplication::runningApplicationWithProcessIdentifier(pid)?;
    if app.activationPolicy() == NSApplicationActivationPolicy::Prohibited {
        return None;
    }
    Some(App {
        bundle_id: app.bundleIdentifier()?.to_string(),
        name: app
            .localizedName()
            .map(|n| n.to_string())
            .unwrap_or_default(),
        pid,
    })
}
