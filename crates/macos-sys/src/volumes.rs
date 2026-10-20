//! The drives that can be taken out of this Mac: which are plugged in, when one arrives or leaves, and
//! ejecting one.

use block2::RcBlock;
use objc2::{
    msg_send,
    rc::Retained,
    runtime::{AnyObject, ProtocolObject},
};
use objc2_app_kit::{
    NSWorkspace, NSWorkspaceDidMountNotification, NSWorkspaceDidUnmountNotification,
    NSWorkspaceVolumeLocalizedNameKey, NSWorkspaceVolumeURLKey,
};
use objc2_foundation::{
    NSArray, NSFileManager, NSNotification, NSNotificationCenter, NSNotificationName,
    NSObjectProtocol, NSOperationQueue, NSString, NSURL, NSURLVolumeIsEjectableKey,
    NSURLVolumeIsInternalKey, NSURLVolumeLocalizedNameKey, NSVolumeEnumerationOptions,
};
use std::{path::Path, ptr::NonNull};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Volume {
    /// The name Finder shows.
    pub name: String,
    pub path: String,
}

/// What happened to a drive.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Event {
    Mounted(Volume),
    /// By the name and the place it had, since it is gone.
    Unmounted(Volume),
}

fn file_name(path: &str) -> String {
    Path::new(path)
        .file_name()
        .map_or_else(|| path.to_owned(), |n| n.to_string_lossy().into_owned())
}

/// The drive at `url` when it can be ejected and is not inside this Mac; `None` for the startup disk and the
/// volumes that belong to it.
fn describe(url: &NSURL) -> Option<Volume> {
    // SAFETY: the keys are process-lifetime constants.
    let keys = unsafe {
        NSArray::from_slice(&[
            NSURLVolumeIsEjectableKey,
            NSURLVolumeIsInternalKey,
            NSURLVolumeLocalizedNameKey,
        ])
    };
    let values = url.resourceValuesForKeys_error(&keys).ok()?;
    let flag = |key: &NSString| -> bool {
        values.objectForKey(key).is_some_and(|v| {
            // SAFETY: the volume flags are NSNumber booleans.
            unsafe { msg_send![&*v, boolValue] }
        })
    };
    // SAFETY: the keys are process-lifetime constants.
    let (ejectable, internal) = unsafe {
        (
            flag(NSURLVolumeIsEjectableKey),
            flag(NSURLVolumeIsInternalKey),
        )
    };
    if !ejectable || internal {
        return None;
    }
    let path = url.path()?.to_string();
    // SAFETY: the key is a process-lifetime constant.
    let name = values
        .objectForKey(unsafe { NSURLVolumeLocalizedNameKey })
        .and_then(|v| {
            // SAFETY: the name is an NSString.
            let s: Option<Retained<NSString>> = unsafe { msg_send![&*v, description] };
            s.map(|s| s.to_string())
        })
        .filter(|n| !n.is_empty())
        .unwrap_or_else(|| file_name(&path));
    Some(Volume { name, path })
}

/// Every drive that is plugged in and can be ejected.
pub fn external() -> Vec<Volume> {
    let Some(urls) = NSFileManager::defaultManager()
        .mountedVolumeURLsIncludingResourceValuesForKeys_options(
            None,
            NSVolumeEnumerationOptions::SkipHiddenVolumes,
        )
    else {
        return Vec::new();
    };
    urls.iter().filter_map(|url| describe(&url)).collect()
}

/// Unmounts the drive and lets it be unplugged. The error is the system's own sentence.
pub fn eject(path: &str) -> Result<(), String> {
    let url = NSURL::fileURLWithPath(&NSString::from_str(path));
    NSWorkspace::sharedWorkspace()
        .unmountAndEjectDeviceAtURL_error(&url)
        .map_err(|e| e.localizedDescription().to_string())
}

/// Stops reporting when dropped.
pub struct Observer {
    center: Retained<NSNotificationCenter>,
    tokens: Vec<Retained<ProtocolObject<dyn NSObjectProtocol>>>,
}

impl Drop for Observer {
    fn drop(&mut self) {
        for token in &self.tokens {
            // SAFETY: each token was returned by this center's addObserverForName.
            unsafe {
                self.center
                    .removeObserver(AsRef::<AnyObject>::as_ref(&**token))
            };
        }
    }
}

fn event_of(note: &NSNotification, mounted: bool) -> Option<Event> {
    let info = note.userInfo()?;
    // SAFETY: the keys are process-lifetime constants.
    let (url, name) = unsafe {
        (
            info.objectForKey(NSWorkspaceVolumeURLKey),
            info.objectForKey(NSWorkspaceVolumeLocalizedNameKey),
        )
    };
    let url = url?;
    // SAFETY: the value under the URL key is an NSURL.
    let path: Option<Retained<NSString>> = unsafe { msg_send![&*url, path] };
    let path = path?.to_string();
    // SAFETY: the value under the name key is an NSString.
    let name: Option<String> = name.and_then(|n| {
        let s: Option<Retained<NSString>> = unsafe { msg_send![&*n, description] };
        s.map(|s| s.to_string())
    });
    let name = name
        .filter(|n| !n.is_empty())
        .unwrap_or_else(|| file_name(&path));
    if mounted {
        let url = NSURL::fileURLWithPath(&NSString::from_str(&path));
        describe(&url).map(Event::Mounted)
    } else {
        Some(Event::Unmounted(Volume { name, path }))
    }
}

/// Calls `changed` on the main thread whenever a drive is mounted or unmounted. A drive that cannot be
/// ejected (the startup disk's own volumes) is not reported when it arrives.
pub fn observe(changed: impl Fn(Event) + 'static) -> Observer {
    let center = NSWorkspace::sharedWorkspace().notificationCenter();
    let changed = std::rc::Rc::new(changed);
    // SAFETY: the names are process-lifetime constants.
    let names: [(&NSNotificationName, bool); 2] = unsafe {
        [
            (NSWorkspaceDidMountNotification, true),
            (NSWorkspaceDidUnmountNotification, false),
        ]
    };
    let tokens = names
        .into_iter()
        .map(|(name, mounted)| {
            let changed = std::rc::Rc::clone(&changed);
            let block = RcBlock::new(move |note: NonNull<NSNotification>| {
                // SAFETY: the notification is alive for the length of the call.
                if let Some(event) = event_of(unsafe { note.as_ref() }, mounted) {
                    changed(event);
                }
            });
            // SAFETY: the block is 'static and runs on the main queue.
            unsafe {
                center.addObserverForName_object_queue_usingBlock(
                    Some(name),
                    None,
                    Some(&NSOperationQueue::mainQueue()),
                    &block,
                )
            }
        })
        .collect();
    Observer { center, tokens }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_list_can_be_read() {
        // Whatever is plugged in, none of it is the startup disk.
        for volume in external() {
            assert!(!volume.name.is_empty() && volume.path.starts_with('/'));
        }
    }

    #[test]
    fn a_name_falls_back_to_the_last_part_of_the_path() {
        assert_eq!(file_name("/Volumes/Backup"), "Backup");
        assert_eq!(file_name("/"), "/");
    }
}
