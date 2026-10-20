//! The battery of this Mac, read from the power sources of IOKit. A Mac without a battery has none.

use core_foundation::{
    array::{CFArray, CFArrayRef},
    base::{CFType, CFTypeRef, TCFType},
    boolean::CFBoolean,
    dictionary::{CFDictionary, CFDictionaryRef},
    number::CFNumber,
    string::CFString,
};

#[link(name = "IOKit", kind = "framework")]
unsafe extern "C" {
    fn IOPSCopyPowerSourcesInfo() -> CFTypeRef;
    fn IOPSCopyPowerSourcesList(blob: CFTypeRef) -> CFArrayRef;
    fn IOPSGetPowerSourceDescription(blob: CFTypeRef, source: CFTypeRef) -> CFDictionaryRef;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Battery {
    /// 0 to 100.
    pub percent: u8,
    /// Taking charge right now.
    pub charging: bool,
    /// The power adapter is plugged in, whether or not the battery takes charge from it.
    pub on_ac: bool,
    /// The system says the battery is full.
    pub charged: bool,
}

type Description = CFDictionary<CFString, CFType>;

fn value(description: &Description, key: &'static str) -> Option<CFType> {
    description
        .find(CFString::from_static_string(key))
        .map(|item| item.clone())
}

fn text(description: &Description, key: &'static str) -> Option<String> {
    value(description, key)?
        .downcast::<CFString>()
        .map(|s| s.to_string())
}

fn number(description: &Description, key: &'static str) -> Option<i64> {
    value(description, key)?.downcast::<CFNumber>()?.to_i64()
}

fn flag(description: &Description, key: &'static str) -> Option<bool> {
    value(description, key)?
        .downcast::<CFBoolean>()
        .map(bool::from)
}

/// From the description of one power source; `None` for a source that is not the internal battery.
fn read(description: &Description) -> Option<Battery> {
    if text(description, "Type").as_deref() != Some("InternalBattery") {
        return None;
    }
    let current = number(description, "Current Capacity")?;
    let max = number(description, "Max Capacity")
        .filter(|m| *m > 0)
        .unwrap_or(100);
    let percent = (current.saturating_mul(100) + max / 2) / max;
    Some(Battery {
        percent: u8::try_from(percent.clamp(0, 100)).unwrap_or(100),
        charging: flag(description, "Is Charging").unwrap_or(false),
        on_ac: text(description, "Power Source State").as_deref() == Some("AC Power"),
        charged: flag(description, "Is Charged").unwrap_or(false),
    })
}

/// The internal battery, or `None` on a Mac that has none. Cheap enough to ask every few seconds.
pub fn battery() -> Option<Battery> {
    // SAFETY: each call follows the Create or Get rule of IOPowerSources.h: the blob and the list are owned
    // and released by their wrappers, and a description is only borrowed from the blob while it is alive.
    unsafe {
        let blob = IOPSCopyPowerSourcesInfo();
        if blob.is_null() {
            return None;
        }
        let blob = CFType::wrap_under_create_rule(blob);
        let list = IOPSCopyPowerSourcesList(blob.as_CFTypeRef());
        if list.is_null() {
            return None;
        }
        let list: CFArray<CFType> = CFArray::wrap_under_create_rule(list);
        for source in list.iter() {
            let description =
                IOPSGetPowerSourceDescription(blob.as_CFTypeRef(), source.as_CFTypeRef());
            if description.is_null() {
                continue;
            }
            let description: Description = CFDictionary::wrap_under_get_rule(description);
            if let Some(battery) = read(&description) {
                return Some(battery);
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A description as IOKit gives it, with the keys it uses.
    fn description(
        kind: &str,
        current: i32,
        max: i32,
        ac: bool,
        charging: bool,
        charged: bool,
    ) -> Description {
        let flag = |on: bool| {
            if on {
                CFBoolean::true_value()
            } else {
                CFBoolean::false_value()
            }
            .as_CFType()
        };
        let key = |name: &'static str| CFString::from_static_string(name);
        CFDictionary::from_CFType_pairs(&[
            (key("Type"), CFString::new(kind).as_CFType()),
            (key("Current Capacity"), CFNumber::from(current).as_CFType()),
            (key("Max Capacity"), CFNumber::from(max).as_CFType()),
            (
                key("Power Source State"),
                CFString::new(if ac { "AC Power" } else { "Battery Power" }).as_CFType(),
            ),
            (key("Is Charging"), flag(charging)),
            (key("Is Charged"), flag(charged)),
        ])
    }

    #[test]
    fn a_battery_on_the_adapter_is_read_as_it_is_described() {
        let battery = read(&description("InternalBattery", 87, 100, true, true, false)).unwrap();
        assert_eq!(
            battery,
            Battery {
                percent: 87,
                charging: true,
                on_ac: true,
                charged: false
            }
        );
    }

    #[test]
    fn a_battery_in_use_and_a_full_one_are_told_apart() {
        let in_use = read(&description(
            "InternalBattery",
            20,
            100,
            false,
            false,
            false,
        ))
        .unwrap();
        assert!(!in_use.on_ac && !in_use.charging && in_use.percent == 20);
        let full = read(&description("InternalBattery", 100, 100, true, false, true)).unwrap();
        assert!(full.on_ac && full.charged && full.percent == 100);
    }

    #[test]
    fn a_capacity_in_units_other_than_percent_is_turned_into_a_percentage() {
        let battery = read(&description(
            "InternalBattery",
            4000,
            8000,
            false,
            false,
            false,
        ))
        .unwrap();
        assert_eq!(battery.percent, 50);
        let nothing_max =
            read(&description("InternalBattery", 40, 0, false, false, false)).unwrap();
        assert_eq!(nothing_max.percent, 40, "a missing maximum counts as 100");
    }

    #[test]
    fn a_power_source_that_is_not_the_battery_is_left_out() {
        assert!(read(&description("UPS", 90, 100, true, false, false)).is_none());
    }

    #[test]
    fn asking_never_fails_whether_or_not_there_is_a_battery() {
        if let Some(battery) = battery() {
            assert!(battery.percent <= 100);
        }
    }
}
