//! The Bluetooth devices that are connected to this Mac right now, read from IOBluetooth.

use objc2::{
    msg_send,
    rc::Retained,
    runtime::{AnyClass, AnyObject},
};
use objc2_foundation::{NSArray, NSString};

#[link(name = "IOBluetooth", kind = "framework")]
unsafe extern "C" {}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Headphones,
    Speaker,
    Keyboard,
    Mouse,
    Other,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Device {
    /// Tells one device from another across polls; a name is not unique.
    pub address: String,
    pub name: String,
    pub kind: Kind,
}

const MAJOR_AUDIO: u32 = 4;
const MAJOR_PERIPHERAL: u32 = 5;

/// From the Bluetooth class of device: the major class, then the minor one (a peripheral keeps keyboard and
/// pointing device in bits 4 and 5 of it).
fn kind_of(major: u32, minor: u32, name: &str) -> Kind {
    match major {
        MAJOR_AUDIO if matches!(minor, 5 | 7 | 8) => Kind::Speaker,
        MAJOR_AUDIO => Kind::Headphones,
        MAJOR_PERIPHERAL if minor & 0x10 != 0 => Kind::Keyboard,
        MAJOR_PERIPHERAL if minor & 0x20 != 0 => Kind::Mouse,
        // Low-energy devices (most mice and keyboards) have no class of device, only a name.
        _ => {
            let name = name.to_lowercase();
            if name.contains("mouse") || name.contains("trackpad") {
                Kind::Mouse
            } else if name.contains("keyboard") {
                Kind::Keyboard
            } else {
                Kind::Other
            }
        }
    }
}

/// Every paired device that is connected now. Cheap enough to poll every couple of seconds.
pub fn connected() -> Vec<Device> {
    let Some(class) = AnyClass::get(c"IOBluetoothDevice") else {
        return Vec::new();
    };
    // SAFETY: `pairedDevices` is a class method returning an NSArray of IOBluetoothDevice or nil.
    let paired: Option<Retained<NSArray<AnyObject>>> = unsafe { msg_send![class, pairedDevices] };
    let Some(paired) = paired else {
        return Vec::new();
    };
    paired
        .iter()
        .filter_map(|device| {
            // SAFETY: every element is an IOBluetoothDevice, which answers all of these selectors.
            unsafe {
                let on: bool = msg_send![&*device, isConnected];
                if !on {
                    return None;
                }
                let address: Option<Retained<NSString>> = msg_send![&*device, addressString];
                let name: Option<Retained<NSString>> = msg_send![&*device, name];
                let major: u32 = msg_send![&*device, deviceClassMajor];
                let minor: u32 = msg_send![&*device, deviceClassMinor];
                let address = address?.to_string();
                let name = name.map(|n| n.to_string()).unwrap_or_default();
                let kind = kind_of(major, minor, &name);
                Some(Device {
                    address,
                    name,
                    kind,
                })
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn class_of_device_maps_to_a_kind() {
        assert_eq!(kind_of(4, 6, ""), Kind::Headphones);
        assert_eq!(kind_of(4, 1, ""), Kind::Headphones);
        assert_eq!(kind_of(4, 5, ""), Kind::Speaker);
        assert_eq!(kind_of(5, 0x10, ""), Kind::Keyboard);
        assert_eq!(kind_of(5, 0x20, ""), Kind::Mouse);
        assert_eq!(kind_of(0, 0, "BT5.0 Mouse"), Kind::Mouse);
        assert_eq!(kind_of(0, 0, "Thing"), Kind::Other);
    }
}
