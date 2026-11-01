//! What the Wi-Fi radio says of the connection it holds, from CoreWLAN: the strength of the signal, the noise, the
//! channel and the speed of the link. None of it needs a permission; the name of the network (SSID) does, since
//! macOS 14, and is not read.

use objc2::{
    msg_send,
    rc::Retained,
    runtime::{AnyClass, AnyObject},
};

#[link(name = "CoreWLAN", kind = "framework")]
unsafe extern "C" {}

#[derive(Debug, Clone, PartialEq)]
pub struct Radio {
    /// The interface name, `en0`.
    pub interface: String,
    /// Signal strength in dBm, such as -58.
    pub rssi: i32,
    /// Noise in dBm, such as -92.
    pub noise: i32,
    /// The channel number, 0 when there is none.
    pub channel: u32,
    /// `2.4`, `5` or `6` GHz; 0 when unknown.
    pub band_ghz: f64,
    /// 20, 40, 80, 160 or 320 MHz; 0 when unknown.
    pub width_mhz: u32,
    /// The speed the radio sends at, in megabits a second.
    pub rate_mbps: f64,
    /// `802.11ac` and the like; empty when unknown.
    pub standard: &'static str,
}

/// The radio of the first Wi-Fi interface, or `None` without one, with the radio off or not joined to a network.
pub fn read() -> Option<Radio> {
    let class = AnyClass::get(c"CWWiFiClient")?;
    // SAFETY: plain property reads on CoreWLAN's own objects, each with the selector's documented return type.
    unsafe {
        let client: Option<Retained<AnyObject>> = msg_send![class, sharedWiFiClient];
        let client = client?;
        let interface: Option<Retained<AnyObject>> = msg_send![&*client, interface];
        let interface = interface?;
        let on: bool = msg_send![&*interface, powerOn];
        let rssi: isize = msg_send![&*interface, rssiValue];
        // Joined to nothing: the radio answers 0 for the strength.
        if !on || rssi == 0 {
            return None;
        }
        let noise: isize = msg_send![&*interface, noiseMeasurement];
        let rate: f64 = msg_send![&*interface, transmitRate];
        let phy: isize = msg_send![&*interface, activePHYMode];
        let name: Option<Retained<AnyObject>> = msg_send![&*interface, interfaceName];
        let interface_name = name.map(|n| {
            let text: *const std::ffi::c_char = msg_send![&*n, UTF8String];
            std::ffi::CStr::from_ptr(text)
                .to_string_lossy()
                .into_owned()
        });
        let channel: Option<Retained<AnyObject>> = msg_send![&*interface, wlanChannel];
        let (number, band, width) = match channel {
            Some(c) => {
                let number: isize = msg_send![&*c, channelNumber];
                let band: isize = msg_send![&*c, channelBand];
                let width: isize = msg_send![&*c, channelWidth];
                (number, band, width)
            }
            None => (0, 0, 0),
        };
        Some(Radio {
            interface: interface_name.unwrap_or_else(|| "en0".to_owned()),
            rssi: rssi as i32,
            noise: noise as i32,
            channel: number.max(0) as u32,
            band_ghz: band_ghz(band),
            width_mhz: width_mhz(width),
            rate_mbps: rate,
            standard: standard(phy),
        })
    }
}

/// `CWChannelBand`: 1 is 2.4 GHz, 2 is 5 GHz, 3 is 6 GHz.
pub fn band_ghz(band: isize) -> f64 {
    match band {
        1 => 2.4,
        2 => 5.0,
        3 => 6.0,
        _ => 0.0,
    }
}

/// `CWChannelWidth`: 1 is 20 MHz, then 40, 80 and 160.
pub fn width_mhz(width: isize) -> u32 {
    match width {
        1 => 20,
        2 => 40,
        3 => 80,
        4 => 160,
        _ => 0,
    }
}

/// `CWPHYMode`.
pub fn standard(phy: isize) -> &'static str {
    match phy {
        1 => "802.11a",
        2 => "802.11b",
        3 => "802.11g",
        4 => "802.11n",
        5 => "802.11ac",
        6 => "802.11ax",
        7 => "802.11be",
        _ => "",
    }
}

/// How good a signal is for what it carries, from the signal-to-noise ratio in dB: `good` from 40, `fair` from 25,
/// else `poor`. A noise that is zero or above (not measured) is judged by the strength alone.
pub fn grade(rssi: i32, noise: i32) -> &'static str {
    let snr = if noise < 0 { rssi - noise } else { rssi + 95 };
    match snr {
        40.. => "good",
        25..=39 => "fair",
        _ => "poor",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_signal_is_graded_by_how_far_it_stands_above_the_noise() {
        assert_eq!(grade(-50, -92), "good", "42 dB");
        assert_eq!(grade(-55, -92), "fair", "37 dB");
        assert_eq!(grade(-80, -92), "poor", "12 dB");
        assert_eq!(
            grade(-60, 0),
            "fair",
            "no noise reading: the strength alone, as 35 dB"
        );
    }

    #[test]
    fn the_codes_of_corewlan_are_named() {
        assert_eq!(
            (band_ghz(1), band_ghz(2), band_ghz(3), band_ghz(9)),
            (2.4, 5.0, 6.0, 0.0)
        );
        assert_eq!(
            (width_mhz(1), width_mhz(3), width_mhz(4), width_mhz(0)),
            (20, 80, 160, 0)
        );
        assert_eq!(
            (standard(5), standard(6), standard(0)),
            ("802.11ac", "802.11ax", "")
        );
    }

    #[test]
    fn asking_never_fails_whether_or_not_there_is_a_radio() {
        let _ = read();
    }
}
