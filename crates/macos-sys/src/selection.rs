//! The text selected in the app in front: through Accessibility where the app offers it, and through a
//! copy that leaves the clipboard exactly as it was where it does not.

use crate::{ax::Element, pasteboard::Pasteboard, synth};
use std::{
    thread,
    time::{Duration, Instant},
};

fn selection_of(focused: Element) -> Option<String> {
    let text = focused.string("AXSelectedText")?;
    (!text.trim().is_empty()).then_some(text)
}

/// The selection of the app that has the keyboard focus. Native apps and Safari offer it; many browsers
/// and Electron apps do not, which is what `copy_selected_text` is for.
pub fn selected_text() -> Option<String> {
    selection_of(Element::system_wide()?.attribute("AXFocusedUIElement")?)
}

/// The same for one app, whether or not it is the one in front.
pub fn selected_text_of(pid: i32) -> Option<String> {
    selection_of(Element::application(pid)?.attribute("AXFocusedUIElement")?)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CopyError {
    /// The pasteboard holds something that cannot be put back in full, so it was left alone.
    NotRestorable,
    /// Nothing was copied: nothing is selected, or the app ignored the key.
    Nothing,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Copied {
    pub text: String,
    /// The change count of the pasteboard after the clipboard was put back, for the history watcher to skip.
    pub change_count: isize,
}

/// Presses Command+C in the app in front, reads what it copied and puts the previous clipboard back. It
/// blocks for at most `wait`, and needs Accessibility access to post the key. Keeps the clipboard intact: if
/// anything on it cannot be saved in full nothing is posted at all.
pub fn copy_selected_text(board: &Pasteboard, wait: Duration) -> Result<Copied, CopyError> {
    let saved = board.snapshot().ok_or(CopyError::NotRestorable)?;
    let before = board.change_count();
    synth::post_copy();
    let deadline = Instant::now() + wait;
    while board.change_count() == before {
        if Instant::now() >= deadline {
            return Err(CopyError::Nothing);
        }
        thread::sleep(Duration::from_millis(8));
    }
    // Some apps write in two steps; give the second one a moment.
    thread::sleep(Duration::from_millis(15));
    let text = board.string();
    let change_count = board.restore(&saved);
    match text.filter(|t| !t.trim().is_empty()) {
        Some(text) => Ok(Copied { text, change_count }),
        None => Err(CopyError::Nothing),
    }
}
