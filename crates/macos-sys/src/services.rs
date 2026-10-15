//! The item in the Services menu, which Finder also shows when files are right clicked, that puts files on
//! the shelf. It is declared under NSServices in the app's Info.plist (scripts/bundle.sh); this answers it.

use crate::pasteboard::file_paths;
use objc2::{
    DefinedClass, MainThreadMarker, MainThreadOnly, define_class, msg_send, rc::Retained,
    runtime::AnyObject,
};
use objc2_app_kit::{NSApplication, NSPasteboard, NSUpdateDynamicServices};
use objc2_foundation::{NSObject, NSObjectProtocol, NSString};

pub struct ProviderIvars {
    on_files: Box<dyn Fn(Vec<String>)>,
}

define_class!(
    // SAFETY: NSObject allows subclassing; the ivars are only touched on the main thread and need no Drop.
    #[unsafe(super(NSObject))]
    #[thread_kind = MainThreadOnly]
    #[name = "FigServices"]
    #[ivars = ProviderIvars]
    struct Provider;

    impl Provider {
        /// `NSMessage` "addToShelf" of the Info.plist entry.
        #[unsafe(method(addToShelf:userData:error:))]
        fn add_to_shelf(
            &self,
            pasteboard: &NSPasteboard,
            _user_data: Option<&NSString>,
            _error: *mut *mut AnyObject,
        ) {
            let paths = file_paths(pasteboard);
            if !paths.is_empty() {
                (self.ivars().on_files)(paths);
            }
        }
    }

    unsafe impl NSObjectProtocol for Provider {}
);

/// Answers the Services menu while it is alive.
pub struct Services(#[allow(dead_code)] Retained<Provider>);

/// Hands the files of "Add to Figghost Shelf" to `on_files`, on the main thread.
pub fn provide(mtm: MainThreadMarker, on_files: impl Fn(Vec<String>) + 'static) -> Services {
    let ivars = ProviderIvars {
        on_files: Box::new(on_files),
    };
    // SAFETY: plain NSObject initializer on the main thread.
    let provider: Retained<Provider> =
        unsafe { msg_send![super(mtm.alloc::<Provider>().set_ivars(ivars)), init] };
    let app = NSApplication::sharedApplication(mtm);
    // SAFETY: the provider answers the message the Info.plist names and lives as long as `Services`.
    unsafe { app.setServicesProvider(Some(&provider)) };
    // A freshly installed app is in the menu without logging out and in again.
    NSUpdateDynamicServices();
    Services(provider)
}
