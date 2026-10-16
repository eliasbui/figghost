//! The clipboard mascot (off by default): a small pixel character that comes out of the notch to fetch what
//! was copied, brings files that Finder put on the shelf ("Add to Figghost Shelf" in the Services menu), and
//! sees files off when they are dragged out of the shelf, guarding them or missing them.
//!
//! It only shows a stand-in for what was copied (a text chip, a picture, a file icon), never the content.
//! Nothing runs while it is off but a look at a flag on each copy, and nothing at all with Reduce motion on.
//! The pictures are `assets/mascots/*.png`, cut by `docs/mascots/build.py` from generated sprite sheets.

use super::AttachEnv;
use crate::{history::Copied, settings::model::Fault, trace};
use macos_sys::{
    MainThreadMarker, NSPoint, NSRect, NSSize,
    mascot::{self, Chip, Sheet, Stage},
    notch::{Notch, NotchGeometry},
    permissions, selection,
    shelf_ui::Drag,
    status_item::Entry,
    workspace,
};
use scene::{Farewell, Follow, Place, Style};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{
    cell::RefCell,
    collections::HashMap,
    rc::{Rc, Weak},
};

pub mod cast;
pub mod scene;
pub mod settings;

pub use cast::Character;

/// The longest a drag is followed; the end of the drag normally comes long before.
const FOLLOW_SECONDS: f64 = 30.0;

impl Farewell {
    pub fn as_str(self) -> &'static str {
        match self {
            Farewell::Guard => "guard",
            Farewell::Regret => "regret",
        }
    }

    pub fn parse(text: &str) -> Option<Self> {
        [Farewell::Guard, Farewell::Regret]
            .into_iter()
            .find(|f| f.as_str() == text)
    }
}

impl Serialize for Farewell {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(self.as_str())
    }
}

impl<'de> Deserialize<'de> for Farewell {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let text = String::deserialize(d)?;
        Farewell::parse(&text).ok_or_else(|| serde::de::Error::custom("expected guard or regret"))
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    pub enabled: bool,
    pub character: Character,
    /// How a file dragged out of the shelf is seen off.
    pub farewell: Farewell,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            enabled: false,
            character: Character::Spider,
            farewell: Farewell::Guard,
        }
    }
}

/// The running feature. Created by `attach`, kept alive by the app context.
pub struct Feature {
    me: Weak<Feature>,
    mtm: MainThreadMarker,
    config: RefCell<Config>,
    notch: Option<Rc<Notch>>,
    /// Made the first time something plays.
    stage: RefCell<Option<Rc<Stage>>>,
    sheets: RefCell<HashMap<Character, Rc<Sheet>>>,
    /// The drag out of the shelf under way, while the mascot follows it.
    follow: Rc<RefCell<Option<Follow>>>,
}

/// Starts the feature. It listens to copies and to the shelf whether or not it is on; while it is off, each
/// of those only looks at the switch.
pub fn attach(env: &AttachEnv) -> Option<Rc<Feature>> {
    let feature = Rc::new_cyclic(|me| Feature {
        me: me.clone(),
        mtm: env.mtm,
        config: RefCell::new(env.config.mascot.clone()),
        notch: env.notch.cloned(),
        stage: RefCell::new(None),
        sheets: RefCell::new(HashMap::new()),
        follow: Rc::default(),
    });
    if let Some(clipboard) = env.clipboard {
        let me = feature.me.clone();
        clipboard
            .history
            .set_on_copied(Some(Box::new(move |copied| {
                if let Some(f) = me.upgrade() {
                    f.copied(copied);
                }
            })));
    }
    if let Some(shelf) = env.shelf {
        let me = feature.me.clone();
        shelf.on_added(move |paths| {
            if let (Some(f), Some(path)) = (me.upgrade(), paths.first()) {
                f.fetch(mascot::pointer(), Chip::File(path.clone()));
            }
        });
        let me = feature.me.clone();
        shelf.on_drag(move |drag| {
            if let Some(f) = me.upgrade() {
                f.dragged(drag);
            }
        });
    }
    Some(feature)
}

/// The notch-sized hole at the top of `screen`, centred: where the character comes out of and goes back to.
fn place_on(screen: NSRect, size: NSSize) -> Place {
    let hole = NSRect::new(
        NSPoint::new(
            screen.origin.x + (screen.size.width - size.width) / 2.0,
            screen.origin.y + screen.size.height - size.height,
        ),
        size,
    );
    Place { hole, screen }
}

fn centre(r: NSRect) -> NSPoint {
    NSPoint::new(
        r.origin.x + r.size.width / 2.0,
        r.origin.y + r.size.height / 2.0,
    )
}

impl Feature {
    fn active(&self) -> bool {
        self.config.borrow().enabled && !workspace::display_options().reduce_motion
    }

    fn style(&self) -> Style {
        self.config.borrow().character.style()
    }

    fn sheet(&self) -> Option<Rc<Sheet>> {
        let character = self.config.borrow().character;
        let mut sheets = self.sheets.borrow_mut();
        if let Some(sheet) = sheets.get(&character) {
            return Some(Rc::clone(sheet));
        }
        let sheet = Rc::new(Sheet::from_png(character.png())?);
        debug_assert_eq!(
            sheet.cell(),
            scene::CELL,
            "a sheet cut by docs/mascots/build.py"
        );
        sheets.insert(character, Rc::clone(&sheet));
        Some(sheet)
    }

    fn stage(&self) -> Rc<Stage> {
        Rc::clone(
            self.stage
                .borrow_mut()
                .get_or_insert_with(|| Stage::new(self.mtm)),
        )
    }

    /// The notch as it is drawn now, open or closed. Without a notch panel, a notch-sized gap at the top of
    /// the screen that would have one, which the character comes down from all the same.
    fn place(&self) -> Option<Place> {
        let (screen, size) = match &self.notch {
            Some(notch) => {
                let geometry = notch.geometry();
                let shown = notch.visible_size();
                let size = if shown.height >= 4.0 {
                    shown
                } else {
                    geometry.notch
                };
                (geometry.screen, size)
            }
            None => {
                let geometry = NotchGeometry::detect(self.mtm)?;
                (geometry.screen, geometry.notch)
            }
        };
        Some(place_on(screen, size))
    }

    /// Where a chip taken from `target` plays: the notch's own place while the target is on the notch's screen,
    /// and the top of the target's screen otherwise, so a copy made on another display is fetched there instead
    /// of being dragged over to the notch's screen.
    fn place_for(&self, target: NSPoint) -> Option<Place> {
        let place = self.place();
        if let Some(notch) = &self.notch
            && mascot::screen_at(self.mtm, target) == Some(notch.geometry().screen)
        {
            return place;
        }
        // A copy made on another display is fetched there; with no screen for that point (the checks run with
        // the notch off every screen) the notch's own place is all there is.
        NotchGeometry::detect_at(self.mtm, target)
            .map(|geometry| place_on(geometry.screen, geometry.notch))
            .or(place)
    }

    fn copied(&self, copied: Copied) {
        if !self.active() {
            return;
        }
        // Selected text is where it can be found; everything else is wherever the pointer is.
        let target = match &copied {
            Copied::Text => {
                selection::selection_frame(self.mtm).map_or_else(mascot::pointer, centre)
            }
            Copied::Image | Copied::Files(_) => mascot::pointer(),
        };
        let chip = match copied {
            Copied::Text => Chip::Text,
            Copied::Image => Chip::Picture,
            Copied::Files(files) => files.into_iter().next().map_or(Chip::Picture, Chip::File),
        };
        self.fetch(target, chip);
    }

    /// Goes out to `target` and brings `chip` back into the notch.
    fn fetch(&self, target: NSPoint, chip: Chip) {
        // A drag out of the shelf has the stage until it ends.
        if !self.active() || self.follow.borrow().is_some() {
            return;
        }
        let (Some(place), Some(sheet)) = (self.place_for(target), self.sheet()) else {
            return;
        };
        trace::log(&format!(
            "mascot: {} fetches {chip:?} from {:.0},{:.0}",
            self.config.borrow().character.as_str(),
            target.x,
            target.y
        ));
        let script = scene::fetch(self.style(), place, target, chip);
        let seconds = script.seconds;
        self.stage()
            .play(place.screen, &sheet, seconds, move |t| script.at(t));
    }

    fn dragged(&self, drag: Drag) {
        match drag {
            Drag::Began { at, .. } => {
                if !self.active() {
                    return;
                }
                let (Some(place), Some(sheet)) = (self.place(), self.sheet()) else {
                    return;
                };
                let farewell = self.config.borrow().farewell;
                trace::log(&format!(
                    "mascot: follows a file out of the shelf ({})",
                    farewell.as_str()
                ));
                *self.follow.borrow_mut() = Some(Follow::new(self.style(), place, farewell, at));
                let follow = Rc::clone(&self.follow);
                self.stage()
                    .play(place.screen, &sheet, FOLLOW_SECONDS, move |t| {
                        follow
                            .borrow_mut()
                            .as_mut()
                            .map(|f| f.frame(t))
                            .unwrap_or_default()
                    });
            }
            Drag::Moved(at) => {
                let place = self.place();
                if let Some(follow) = self.follow.borrow_mut().as_mut() {
                    follow.moved(at);
                    if let Some(place) = place {
                        follow.set_place(place);
                    }
                }
            }
            Drag::Ended { at, dropped } => {
                let Some(mut follow) = self.follow.borrow_mut().take() else {
                    return;
                };
                follow.moved(at);
                if let Some(place) = self.place() {
                    follow.set_place(place);
                }
                let (Some(place), Some(sheet)) = (self.place(), self.sheet()) else {
                    self.stage().hide();
                    return;
                };
                trace::log(&format!(
                    "mascot: the file was {}",
                    if dropped { "dropped" } else { "kept" }
                ));
                let script = follow.finale(dropped);
                let seconds = script.seconds;
                self.stage()
                    .play(place.screen, &sheet, seconds, move |t| script.at(t));
            }
        }
    }

    /// Applies the settings after a change. An error is shown on the settings page and the change is undone.
    pub fn configure(&self, config: &Config) -> Result<(), Fault> {
        *self.config.borrow_mut() = config.clone();
        if !config.enabled {
            self.follow.borrow_mut().take();
            if let Some(stage) = &*self.stage.borrow() {
                stage.hide();
            }
        }
        Ok(())
    }

    /// There are no global shortcuts to release.
    pub fn suspend_hotkeys(&self) {}

    pub fn resume_hotkeys(&self, _config: &Config) {}

    /// Facts only the running app knows: whether the selected text can be found (Accessibility).
    pub fn runtime(&self) -> Value {
        json!({ "accessibility": permissions::accessibility_trusted(false) })
    }

    /// "mascot_try" plays a fetch toward the pointer, wherever the settings window is, even with the
    /// feature off; that is how a character is chosen.
    pub fn action(&self, name: &str, _config: &mut Config) -> Option<Result<(), Fault>> {
        if name != "mascot_try" {
            return None;
        }
        if workspace::display_options().reduce_motion || self.follow.borrow().is_some() {
            return Some(Ok(()));
        }
        let (Some(place), Some(sheet)) = (self.place(), self.sheet()) else {
            return Some(Ok(()));
        };
        let script = scene::fetch(self.style(), place, mascot::pointer(), Chip::Text);
        let seconds = script.seconds;
        self.stage()
            .play(place.screen, &sheet, seconds, move |t| script.at(t));
        Some(Ok(()))
    }

    /// Nothing in the menu bar item.
    pub fn menu_entries(_this: &Rc<Self>) -> Vec<Entry> {
        Vec::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_character_has_a_sheet_of_twelve_frames() {
        for c in Character::ALL {
            let png = c.png();
            assert_eq!(&png[..8], b"\x89PNG\r\n\x1a\n", "{c:?}");
            // IHDR width and height, big endian, after the signature and the chunk header.
            let width = u32::from_be_bytes(png[16..20].try_into().unwrap());
            let height = u32::from_be_bytes(png[20..24].try_into().unwrap());
            assert_eq!(f64::from(height), scene::CELL, "{c:?}");
            assert_eq!(width, height * 12, "{c:?}");
            assert_eq!(Character::parse(c.as_str()), Some(c));
        }
        assert_eq!(Character::parse("dragon"), None);
    }

    #[test]
    fn only_the_spider_hangs_and_the_wizard_strikes_first() {
        for c in Character::ALL {
            let style = c.style();
            assert_eq!(style.hangs, c == Character::Spider);
            assert_eq!(style.strike != style.ray, c == Character::Wizard);
        }
    }

    #[test]
    fn the_config_starts_off_and_keeps_defaults_for_what_is_missing() {
        let config = Config::default();
        assert!(!config.enabled);
        let partial: Config =
            serde_json::from_str(r#"{"enabled": true, "farewell": "regret"}"#).unwrap();
        assert!(
            partial.enabled
                && partial.character == Character::Spider
                && partial.farewell == Farewell::Regret
        );
        assert!(serde_json::from_str::<Config>(r#"{"character": "dragon"}"#).is_err());
        assert!(serde_json::from_str::<Config>(r#"{"farewell": "shrug"}"#).is_err());
        let text = serde_json::to_string(&Config::default()).unwrap();
        assert_eq!(
            serde_json::from_str::<Config>(&text).unwrap(),
            Config::default()
        );
    }
}
