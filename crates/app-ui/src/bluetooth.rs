//! Bluetooth devices in the closed notch: when one connects or drops, the notch tells its name for a moment, with
//! its symbol breathing on connect and turning grey on disconnect.

use crate::i18n::{self, Language};
use macos_sys::{
    app,
    bluetooth::{self, Device, Kind},
    notch::{Activity, Detail, Ear, Notch},
    style::Tone,
};
use std::{
    cell::{Cell, RefCell},
    collections::HashMap,
    rc::{Rc, Weak},
};

const SLOT: &str = "bluetooth";
const POLL_SECONDS: f64 = 2.0;
const SHOW_SECONDS: f64 = 3.6;
/// Above the music, below a coding agent that waits for an answer.
const PRIORITY: u8 = 25;

pub struct Bluetooth {
    notch: Weak<Notch>,
    /// The connected devices by address; `None` until the first look, which only takes note.
    known: RefCell<Option<HashMap<String, Device>>>,
    serial: Cell<u64>,
    poll: RefCell<Option<app::Timer>>,
    me: Weak<Bluetooth>,
}

impl Bluetooth {
    pub fn attach(notch: &Rc<Notch>) -> Rc<Self> {
        let feature = Rc::new_cyclic(|me| Self {
            notch: Rc::downgrade(notch),
            known: RefCell::new(None),
            serial: Cell::new(0),
            poll: RefCell::new(None),
            me: me.clone(),
        });
        feature.look();
        let weak = Rc::downgrade(&feature);
        *feature.poll.borrow_mut() = Some(app::every(POLL_SECONDS, move || {
            if let Some(feature) = weak.upgrade() {
                feature.look();
            }
        }));
        feature
    }

    fn look(&self) {
        let now: HashMap<String, Device> = bluetooth::connected()
            .into_iter()
            .map(|d| (d.address.clone(), d))
            .collect();
        let Some(before) = self.known.replace(Some(now.clone())) else {
            return;
        };
        let joined = now.values().find(|d| !before.contains_key(&d.address));
        let left = before.values().find(|d| !now.contains_key(&d.address));
        // Both at once (a switch between two headsets): the one that arrived is the news.
        if let Some(device) = joined {
            self.show(device, true);
        } else if let Some(device) = left {
            self.show(device, false);
        }
    }

    fn show(&self, device: &Device, connected: bool) {
        let Some(notch) = self.notch.upgrade() else {
            return;
        };
        let language = i18n::current();
        let serial = self.serial.get() + 1;
        self.serial.set(serial);
        let (tone, state, mark) = if connected {
            (
                Tone::Green,
                language.bt_connected(),
                "checkmark.circle.fill",
            )
        } else {
            (Tone::Gray, language.bt_disconnected(), "xmark.circle.fill")
        };
        notch.set_activity(
            SLOT,
            Some(Activity {
                left: Ear::Symbol {
                    name: symbol(device.kind),
                    tone,
                    pulse: connected,
                },
                right: Ear::Symbol {
                    name: mark,
                    tone: if connected { Tone::Green } else { Tone::Red },
                    pulse: false,
                },
                detail: Some(Detail {
                    title: if device.name.is_empty() {
                        language.bt_unnamed().to_owned()
                    } else {
                        device.name.clone()
                    },
                    subtitle: state.to_owned(),
                }),
                page: None,
                priority: PRIORITY,
            }),
        );
        let weak = self.me.clone();
        app::after(SHOW_SECONDS, move || {
            if let Some(feature) = weak.upgrade()
                && feature.serial.get() == serial
                && let Some(notch) = feature.notch.upgrade()
            {
                notch.set_activity(SLOT, None);
            }
        });
    }
}

fn symbol(kind: Kind) -> &'static str {
    match kind {
        Kind::Headphones => "headphones",
        Kind::Speaker => "hifispeaker.fill",
        Kind::Keyboard => "keyboard",
        Kind::Mouse => "computermouse.fill",
        Kind::Other => "dot.radiowaves.left.and.right",
    }
}

impl Language {
    fn bt_connected(self) -> &'static str {
        match self {
            Language::English => "Connected",
            Language::Vietnamese => "Đã kết nối",
        }
    }

    fn bt_disconnected(self) -> &'static str {
        match self {
            Language::English => "Disconnected",
            Language::Vietnamese => "Đã ngắt kết nối",
        }
    }

    fn bt_unnamed(self) -> &'static str {
        match self {
            Language::English => "Bluetooth device",
            Language::Vietnamese => "Thiết bị Bluetooth",
        }
    }
}
