//! The services the menu and the settings window act on.

use crate::{
    agents::AgentsFeature, clipboard::Clipboard, config::Config, features::Features,
    shelf::ShelfFeature, tiling::Tiling, volume_hud::VolumeHud,
};
use core_mouse::Control;
use macos_sys::MainThreadMarker;
use std::{cell::RefCell, path::PathBuf, rc::Rc, sync::Arc};

pub struct AppContext {
    pub mtm: MainThreadMarker,
    pub home: PathBuf,
    pub config: Rc<RefCell<Config>>,
    pub control: Arc<Control>,
    pub clipboard: Option<Rc<Clipboard>>,
    pub shelf: Option<Rc<ShelfFeature>>,
    pub agents: Option<Rc<AgentsFeature>>,
    pub live_media: Option<Rc<crate::live_media::LiveMedia>>,
    pub features: Features,
    pub volume_hud: Rc<VolumeHud>,
    pub tiling: Rc<Tiling>,
}

impl AppContext {
    /// Writes the config file, reporting a failure on stderr: a settings change should still take
    /// effect for this run even when the disk is full or read-only.
    pub fn save_config(&self) {
        if let Err(e) = self.config.borrow().save(&self.home) {
            eprintln!("config: cannot save: {e}");
        }
    }
}
