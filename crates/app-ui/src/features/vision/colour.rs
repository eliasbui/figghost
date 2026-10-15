//! Colour values and their text forms: HEX, RGB, HSL and HSB. Pure arithmetic, so the rounding rules are
//! pinned down by plain unit tests.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rgb {
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Format {
    Hex,
    Rgb,
    Hsl,
    Hsb,
}

impl Format {
    pub const ALL: [Format; 4] = [Format::Hex, Format::Rgb, Format::Hsl, Format::Hsb];

    /// The name in the config file and on the page.
    pub const fn id(self) -> &'static str {
        match self {
            Format::Hex => "hex",
            Format::Rgb => "rgb",
            Format::Hsl => "hsl",
            Format::Hsb => "hsb",
        }
    }

    pub fn from_id(id: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|f| f.id() == id)
    }

    /// The place of the format in `ALL`, which is the order the picker cycles through.
    pub fn index(self) -> usize {
        Self::ALL.iter().position(|f| *f == self).unwrap_or(0)
    }
}

/// Hue in degrees (0 to 359), saturation and the third component (lightness or brightness) in percent.
type Triple = (u16, u8, u8);

fn percent(x: f64) -> u8 {
    (x * 100.0).round().clamp(0.0, 100.0) as u8
}

fn hue_of(max: f64, min: f64, r: f64, g: f64, b: f64) -> f64 {
    let d = max - min;
    if d == 0.0 {
        return 0.0;
    }
    let sector = if max == r {
        ((g - b) / d).rem_euclid(6.0)
    } else if max == g {
        (b - r) / d + 2.0
    } else {
        (r - g) / d + 4.0
    };
    sector * 60.0
}

fn channels(rgb: Rgb) -> (f64, f64, f64) {
    (
        f64::from(rgb.r) / 255.0,
        f64::from(rgb.g) / 255.0,
        f64::from(rgb.b) / 255.0,
    )
}

/// A hue that rounds up to 360 is red again.
fn degrees(hue: f64) -> u16 {
    (hue.round() as u16) % 360
}

pub fn to_hsl(rgb: Rgb) -> Triple {
    let (r, g, b) = channels(rgb);
    let (max, min) = (r.max(g).max(b), r.min(g).min(b));
    let l = (max + min) / 2.0;
    let d = max - min;
    let s = if d == 0.0 {
        0.0
    } else {
        d / (1.0 - (2.0 * l - 1.0).abs())
    };
    (degrees(hue_of(max, min, r, g, b)), percent(s), percent(l))
}

pub fn to_hsb(rgb: Rgb) -> Triple {
    let (r, g, b) = channels(rgb);
    let (max, min) = (r.max(g).max(b), r.min(g).min(b));
    let s = if max == 0.0 { 0.0 } else { (max - min) / max };
    (degrees(hue_of(max, min, r, g, b)), percent(s), percent(max))
}

fn from_unit(r: f64, g: f64, b: f64) -> Rgb {
    let byte = |x: f64| (x * 255.0).round().clamp(0.0, 255.0) as u8;
    Rgb {
        r: byte(r),
        g: byte(g),
        b: byte(b),
    }
}

/// The RGB of a hue with the given chroma and lightness offset, the common tail of both conversions.
fn from_chroma(hue: u16, chroma: f64, offset: f64) -> Rgb {
    let h = f64::from(hue % 360) / 60.0;
    let x = chroma * (1.0 - (h.rem_euclid(2.0) - 1.0).abs());
    let (r, g, b) = match h as u8 {
        0 => (chroma, x, 0.0),
        1 => (x, chroma, 0.0),
        2 => (0.0, chroma, x),
        3 => (0.0, x, chroma),
        4 => (x, 0.0, chroma),
        _ => (chroma, 0.0, x),
    };
    from_unit(r + offset, g + offset, b + offset)
}

pub fn from_hsl(hue: u16, saturation: u8, lightness: u8) -> Rgb {
    let (s, l) = (
        f64::from(saturation.min(100)) / 100.0,
        f64::from(lightness.min(100)) / 100.0,
    );
    let chroma = (1.0 - (2.0 * l - 1.0).abs()) * s;
    from_chroma(hue, chroma, l - chroma / 2.0)
}

pub fn from_hsb(hue: u16, saturation: u8, brightness: u8) -> Rgb {
    let (s, v) = (
        f64::from(saturation.min(100)) / 100.0,
        f64::from(brightness.min(100)) / 100.0,
    );
    let chroma = v * s;
    from_chroma(hue, chroma, v - chroma)
}

/// The text a click copies: `#1A2B3C`, `rgb(26, 43, 60)`, `hsl(210, 40%, 17%)` or `hsb(210, 57%, 24%)`.
pub fn format(rgb: Rgb, format: Format) -> String {
    match format {
        Format::Hex => format!("#{:02X}{:02X}{:02X}", rgb.r, rgb.g, rgb.b),
        Format::Rgb => format!("rgb({}, {}, {})", rgb.r, rgb.g, rgb.b),
        Format::Hsl => {
            let (h, s, l) = to_hsl(rgb);
            format!("hsl({h}, {s}%, {l}%)")
        }
        Format::Hsb => {
            let (h, s, b) = to_hsb(rgb);
            format!("hsb({h}, {s}%, {b}%)")
        }
    }
}

/// A colour read back from text, with its alpha (255 unless an 8 digit hex said otherwise).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Parsed {
    pub rgb: Rgb,
    pub alpha: u8,
}

/// The inverse of `format`: HEX with 3, 6 or 8 digits (with or without the hash), and the function forms
/// `rgb(...)`, `hsl(...)`, `hsb(...)` or `hsv(...)`. The inputs `format` writes must come back as the same
/// colour; the lossy ones (HSL and HSB keep whole degrees and percents) to within the rounding.
// The picker only writes colours; reading them back is what its tests rely on to prove the output format.
#[allow(dead_code)]
pub fn parse(text: &str) -> Option<Parsed> {
    let text = text.trim();
    if let Some(open) = text.find('(') {
        let body = text[open + 1..].strip_suffix(')')?;
        let name = text[..open].trim().to_ascii_lowercase();
        let numbers = body
            .split(',')
            .map(|part| part.trim().trim_end_matches('%').parse::<u16>().ok())
            .collect::<Option<Vec<u16>>>()?;
        let [a, b, c] = numbers[..] else { return None };
        let rgb = match name.as_str() {
            "rgb" if a <= 255 && b <= 255 && c <= 255 => Rgb {
                r: a as u8,
                g: b as u8,
                b: c as u8,
            },
            "hsl" if a <= 360 && b <= 100 && c <= 100 => from_hsl(a, b as u8, c as u8),
            "hsb" | "hsv" if a <= 360 && b <= 100 && c <= 100 => from_hsb(a, b as u8, c as u8),
            _ => return None,
        };
        return Some(Parsed { rgb, alpha: 255 });
    }
    let digits = text.strip_prefix('#').unwrap_or(text);
    if !digits.chars().all(|c| c.is_ascii_hexdigit()) {
        return None;
    }
    let pair = |i: usize| u8::from_str_radix(&digits[i..i + 2], 16).ok();
    let (r, g, b, alpha) = match digits.len() {
        // A digit stands for the pair of itself: #abc is #aabbcc.
        3 => {
            let d = |i: usize| u8::from_str_radix(&digits[i..=i], 16).ok().map(|v| v * 17);
            (d(0)?, d(1)?, d(2)?, 255)
        }
        6 => (pair(0)?, pair(2)?, pair(4)?, 255),
        8 => (pair(0)?, pair(2)?, pair(4)?, pair(6)?),
        _ => return None,
    };
    Some(Parsed {
        rgb: Rgb { r, g, b },
        alpha,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const fn rgb(r: u8, g: u8, b: u8) -> Rgb {
        Rgb { r, g, b }
    }

    fn parsed(text: &str) -> Rgb {
        parse(text).unwrap().rgb
    }

    #[test]
    fn the_four_forms_of_the_same_colour() {
        let c = rgb(26, 43, 60);
        assert_eq!(format(c, Format::Hex), "#1A2B3C");
        assert_eq!(format(c, Format::Rgb), "rgb(26, 43, 60)");
        assert_eq!(format(c, Format::Hsl), "hsl(210, 40%, 17%)");
        assert_eq!(format(c, Format::Hsb), "hsb(210, 57%, 24%)");
    }

    #[test]
    fn black_white_and_greys_have_no_hue_or_saturation() {
        for (c, hsl, hsb) in [
            (rgb(0, 0, 0), (0, 0, 0), (0, 0, 0)),
            (rgb(255, 255, 255), (0, 0, 100), (0, 0, 100)),
            (rgb(128, 128, 128), (0, 0, 50), (0, 0, 50)),
        ] {
            assert_eq!(to_hsl(c), hsl);
            assert_eq!(to_hsb(c), hsb);
        }
        assert_eq!(format(rgb(0, 0, 0), Format::Hex), "#000000");
        assert_eq!(format(rgb(255, 255, 255), Format::Hex), "#FFFFFF");
    }

    #[test]
    fn the_primaries_and_secondaries_sit_on_the_hue_wheel() {
        for (c, hue) in [
            (rgb(255, 0, 0), 0),
            (rgb(255, 255, 0), 60),
            (rgb(0, 255, 0), 120),
            (rgb(0, 255, 255), 180),
            (rgb(0, 0, 255), 240),
            (rgb(255, 0, 255), 300),
        ] {
            assert_eq!(to_hsl(c), (hue, 100, 50), "{c:?}");
            assert_eq!(to_hsb(c), (hue, 100, 100), "{c:?}");
            assert_eq!(from_hsl(hue, 100, 50), c);
            assert_eq!(from_hsb(hue, 100, 100), c);
        }
    }

    #[test]
    fn a_hue_that_rounds_to_a_full_turn_is_red() {
        // 359.6 degrees: rgb(255, 0, 1).
        assert_eq!(to_hsb(rgb(255, 0, 1)).0, 0);
        assert_eq!(to_hsb(rgb(255, 0, 3)).0, 359);
    }

    #[test]
    fn percents_round_to_the_nearest_whole_number() {
        // 127/255 is 49.8 percent, 128/255 is 50.2.
        assert_eq!(to_hsb(rgb(127, 127, 127)).2, 50);
        assert_eq!(to_hsb(rgb(128, 128, 128)).2, 50);
        assert_eq!(to_hsb(rgb(3, 3, 3)).2, 1);
        assert_eq!(to_hsb(rgb(1, 1, 1)).2, 0);
    }

    #[test]
    fn hex_reads_three_six_and_eight_digits() {
        assert_eq!(parsed("#fff"), rgb(255, 255, 255));
        assert_eq!(parsed("1a2"), rgb(0x11, 0xaa, 0x22));
        assert_eq!(parsed("#1A2B3C"), rgb(26, 43, 60));
        assert_eq!(parsed("  #1a2b3c "), rgb(26, 43, 60));
        let with_alpha = parse("#1A2B3C80").unwrap();
        assert_eq!((with_alpha.rgb, with_alpha.alpha), (rgb(26, 43, 60), 0x80));
        assert_eq!(parse("#abc").unwrap().alpha, 255);
        for bad in [
            "",
            "#",
            "#12",
            "#1234",
            "#12345",
            "#1234567",
            "#ggg",
            "#12345678 9",
            "zzzzzz",
        ] {
            assert_eq!(parse(bad), None, "{bad:?}");
        }
    }

    #[test]
    fn the_function_forms_are_read_and_checked() {
        assert_eq!(parsed("rgb(26, 43, 60)"), rgb(26, 43, 60));
        assert_eq!(parsed("RGB(0,0,0)"), rgb(0, 0, 0));
        assert_eq!(parsed("hsl(0, 100%, 50%)"), rgb(255, 0, 0));
        assert_eq!(parsed("hsb(240, 100%, 100%)"), rgb(0, 0, 255));
        assert_eq!(parsed("hsv(120, 100, 100)"), rgb(0, 255, 0));
        for bad in [
            "rgb(256, 0, 0)",
            "rgb(1, 2)",
            "rgb(1, 2, 3, 4)",
            "rgb(-1, 0, 0)",
            "hsl(361, 0%, 0%)",
            "hsl(0, 101%, 0%)",
            "cmyk(1, 2, 3)",
            "rgb(1, 2, 3",
        ] {
            assert_eq!(parse(bad), None, "{bad:?}");
        }
    }

    #[test]
    fn hex_and_rgb_round_trip_for_every_value_of_a_channel() {
        for v in 0..=255u8 {
            for c in [rgb(v, 0, 0), rgb(0, v, 0), rgb(0, 0, v), rgb(v, v, 255 - v)] {
                assert_eq!(parsed(&format(c, Format::Hex)), c);
                assert_eq!(parsed(&format(c, Format::Rgb)), c);
            }
        }
    }

    #[test]
    fn hsl_and_hsb_come_back_to_within_their_own_rounding() {
        // Whole degrees and percents cannot carry every colour, but the error stays small.
        let mut worst = 0u8;
        for r in (0..=255u8).step_by(17) {
            for g in (0..=255u8).step_by(17) {
                for b in (0..=255u8).step_by(17) {
                    let c = rgb(r, g, b);
                    for f in [Format::Hsl, Format::Hsb] {
                        let back = parsed(&format(c, f));
                        let d =
                            c.r.abs_diff(back.r)
                                .max(c.g.abs_diff(back.g))
                                .max(c.b.abs_diff(back.b));
                        worst = worst.max(d);
                        assert!(d <= 5, "{c:?} {f:?} -> {back:?}");
                    }
                }
            }
        }
        assert!(worst > 0, "the check is not vacuous");
    }

    #[test]
    fn format_ids_round_trip_and_cycle_in_order() {
        for f in Format::ALL {
            assert_eq!(Format::from_id(f.id()), Some(f));
            assert_eq!(Format::ALL[f.index()], f);
        }
        assert_eq!(Format::from_id("cmyk"), None);
        assert_eq!(Format::from_id("HEX"), None);
    }
}
