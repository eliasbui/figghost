//! The folder picker of the settings.

use crate::MainThreadMarker;
use objc2_app_kit::{NSModalResponseOK, NSOpenPanel};
use objc2_foundation::{NSString, NSURL};
use std::path::{Path, PathBuf};

/// Lets the user choose a folder. Blocks until the dialog closes; `None` when cancelled.
pub fn choose(mtm: MainThreadMarker, message: &str, button: &str, start: &Path) -> Option<PathBuf> {
    let panel = NSOpenPanel::openPanel(mtm);
    panel.setCanChooseFiles(false);
    panel.setCanChooseDirectories(true);
    panel.setCanCreateDirectories(true);
    panel.setAllowsMultipleSelection(false);
    panel.setMessage(Some(&NSString::from_str(message)));
    panel.setPrompt(Some(&NSString::from_str(button)));
    // The folder may not exist yet (the default is made on first use): start in the nearest one that does.
    let start = start.ancestors().find(|p| p.is_dir());
    if let Some(start) = start {
        panel.setDirectoryURL(Some(&NSURL::fileURLWithPath(&NSString::from_str(
            &start.to_string_lossy(),
        ))));
    }
    if panel.runModal() != NSModalResponseOK {
        return None;
    }
    let url = panel.URLs().firstObject()?;
    url.path().map(|p| PathBuf::from(p.to_string()))
}
