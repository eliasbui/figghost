//! Lengths, weights, volumes, areas, speeds, temperatures and sizes of data, converted between the two
//! systems people mix: metric and imperial, decimal and binary, Celsius and Fahrenheit.

use crate::number::{self, NumberStyle};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Dim {
    Length,
    Mass,
    Volume,
    Area,
    Speed,
    Temperature,
    Data,
}

/// The two sides of a dimension: metric or imperial; decimal or binary for data.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Side {
    A,
    B,
}

struct Unit {
    dim: Dim,
    symbol: &'static str,
    /// Lowercase spellings, which is also how the text is compared.
    names: &'static [&'static str],
    per_base: f64,
    side: Side,
    /// Offered as an answer. Hidden units are only understood as input (tablespoons, yards).
    show: bool,
}

const fn unit(
    dim: Dim,
    symbol: &'static str,
    names: &'static [&'static str],
    per_base: f64,
    side: Side,
    show: bool,
) -> Unit {
    Unit {
        dim,
        symbol,
        names,
        per_base,
        side,
        show,
    }
}

use Dim::{Area, Data, Length, Mass, Speed, Volume};
use Side::{A, B};

// Bases: metre, gram, litre, square metre, metre per second, byte. Speeds are not offered by size: the
// order they are listed in is the order of preference.
#[rustfmt::skip]
const UNITS: &[Unit] = &[
    unit(Length, "mm", &["mm", "millimeter", "millimeters", "millimetre", "millimetres"], 0.001, A, true),
    unit(Length, "cm", &["cm", "centimeter", "centimeters", "centimetre", "centimetres"], 0.01, A, true),
    unit(Length, "m", &["m", "meter", "meters", "metre", "metres"], 1.0, A, true),
    unit(Length, "km", &["km", "kilometer", "kilometers", "kilometre", "kilometres"], 1000.0, A, true),
    unit(Length, "in", &["in", "inch", "inches", "\""], 0.0254, B, true),
    unit(Length, "ft", &["ft", "foot", "feet", "'"], 0.3048, B, true),
    unit(Length, "yd", &["yd", "yard", "yards"], 0.9144, B, false),
    unit(Length, "mi", &["mi", "mile", "miles"], 1609.344, B, true),
    unit(Mass, "mg", &["mg", "milligram", "milligrams"], 0.001, A, true),
    unit(Mass, "g", &["g", "gram", "grams", "gramme", "grammes"], 1.0, A, true),
    unit(Mass, "kg", &["kg", "kilogram", "kilograms", "kilo", "kilos"], 1000.0, A, true),
    unit(Mass, "oz", &["oz", "ounce", "ounces"], 28.349_523_125, B, true),
    unit(Mass, "lb", &["lb", "lbs", "pound", "pounds"], 453.592_37, B, true),
    unit(Volume, "ml", &["ml", "milliliter", "milliliters", "millilitre", "millilitres"], 0.001, A, true),
    unit(Volume, "l", &["l", "liter", "liters", "litre", "litres"], 1.0, A, true),
    unit(Volume, "tsp", &["tsp", "teaspoon", "teaspoons"], 0.004_928_921_593_75, B, false),
    unit(Volume, "tbsp", &["tbsp", "tablespoon", "tablespoons"], 0.014_786_764_781_25, B, false),
    unit(Volume, "fl oz", &["fl oz", "floz", "fl. oz", "fluid ounce", "fluid ounces"], 0.029_573_529_562_5, B, true),
    unit(Volume, "cup", &["cup", "cups"], 0.236_588_236_5, B, true),
    unit(Volume, "pt", &["pt", "pint", "pints"], 0.473_176_473, B, false),
    unit(Volume, "qt", &["qt", "quart", "quarts"], 0.946_352_946, B, false),
    unit(Volume, "gal", &["gal", "gallon", "gallons"], 3.785_411_784, B, true),
    unit(Area, "cm\u{b2}", &["cm2", "cm\u{b2}", "sq cm"], 0.0001, A, false),
    unit(Area, "m\u{b2}", &["m2", "m\u{b2}", "sqm", "sq m", "square meter", "square meters", "square metre", "square metres"], 1.0, A, true),
    unit(Area, "ha", &["ha", "hectare", "hectares"], 10_000.0, A, true),
    unit(Area, "km\u{b2}", &["km2", "km\u{b2}", "sq km", "square kilometer", "square kilometers"], 1_000_000.0, A, true),
    unit(Area, "in\u{b2}", &["in2", "in\u{b2}", "sq in"], 0.000_645_16, B, false),
    unit(Area, "ft\u{b2}", &["ft2", "ft\u{b2}", "sqft", "sq ft", "square foot", "square feet"], 0.092_903_04, B, true),
    unit(Area, "yd\u{b2}", &["yd2", "yd\u{b2}", "sq yd"], 0.836_127_36, B, false),
    unit(Area, "acre", &["acre", "acres"], 4_046.856_422_4, B, true),
    unit(Area, "mi\u{b2}", &["mi2", "mi\u{b2}", "sq mi", "square mile", "square miles"], 2_589_988.110_336, B, true),
    unit(Speed, "km/h", &["km/h", "kph", "kmh", "km/hr", "kmph"], 1.0 / 3.6, A, true),
    unit(Speed, "m/s", &["m/s", "mps"], 1.0, A, true),
    unit(Speed, "mph", &["mph", "mi/h", "miles per hour"], 0.447_04, B, true),
    unit(Speed, "ft/s", &["ft/s", "fps"], 0.3048, B, true),
    unit(Speed, "kn", &["kn", "kt", "kts", "knot", "knots"], 1852.0 / 3600.0, B, false),
    unit(Data, "B", &["b", "byte", "bytes"], 1.0, A, true),
    unit(Data, "KB", &["kb", "kilobyte", "kilobytes"], 1e3, A, true),
    unit(Data, "MB", &["mb", "megabyte", "megabytes"], 1e6, A, true),
    unit(Data, "GB", &["gb", "gigabyte", "gigabytes"], 1e9, A, true),
    unit(Data, "TB", &["tb", "terabyte", "terabytes"], 1e12, A, true),
    unit(Data, "KiB", &["kib", "kibibyte", "kibibytes"], 1024.0, B, true),
    unit(Data, "MiB", &["mib", "mebibyte", "mebibytes"], 1_048_576.0, B, true),
    unit(Data, "GiB", &["gib", "gibibyte", "gibibytes"], 1_073_741_824.0, B, true),
    unit(Data, "TiB", &["tib", "tebibyte", "tebibytes"], 1_099_511_627_776.0, B, true),
];

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Quantity {
    pub value: f64,
    pub symbol: &'static str,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Converted {
    pub dim: Dim,
    pub from: Quantity,
    pub to: Vec<Quantity>,
}

/// Lowercase, one space between words, no full stop at the end: how a unit is looked up.
fn normalise(text: &str) -> String {
    let words: Vec<&str> = text.split_whitespace().collect();
    words
        .join(" ")
        .to_lowercase()
        .trim_end_matches(['.', ','])
        .to_owned()
}

#[derive(Clone, Copy)]
enum Temp {
    C,
    F,
    K,
}

fn temperature(name: &str) -> Option<Temp> {
    Some(match name {
        "\u{b0}c" | "c" | "celsius" | "centigrade" | "deg c" | "degrees celsius" => Temp::C,
        "\u{b0}f" | "f" | "fahrenheit" | "deg f" | "degrees fahrenheit" => Temp::F,
        "\u{b0}k" | "kelvin" | "kelvins" => Temp::K,
        _ => return None,
    })
}

fn temperature_answers(value: f64, from: Temp) -> Converted {
    let celsius = match from {
        Temp::C => value,
        Temp::F => (value - 32.0) * 5.0 / 9.0,
        Temp::K => value - 273.15,
    };
    let in_scale = |t: Temp| match t {
        Temp::C => Quantity {
            value: celsius,
            symbol: "\u{b0}C",
        },
        Temp::F => Quantity {
            value: celsius * 9.0 / 5.0 + 32.0,
            symbol: "\u{b0}F",
        },
        Temp::K => Quantity {
            value: celsius + 273.15,
            symbol: "K",
        },
    };
    let others = match from {
        Temp::C => [Temp::F, Temp::K],
        Temp::F => [Temp::C, Temp::K],
        Temp::K => [Temp::C, Temp::F],
    };
    Converted {
        dim: Dim::Temperature,
        from: Quantity {
            value,
            symbol: in_scale(from).symbol,
        },
        to: others.into_iter().map(in_scale).collect(),
    }
}

fn find(name: &str) -> Option<&'static Unit> {
    UNITS.iter().find(|u| u.names.contains(&name))
}

fn shown(dim: Dim, side: Side) -> Vec<&'static Unit> {
    UNITS
        .iter()
        .filter(|u| u.dim == dim && u.side == side && u.show)
        .collect()
}

/// The largest unit of `side` in which `base` is at least one, or the smallest one when it is below all.
/// Zero has no size, so it takes the unit closest in scale to `like`. Speeds take the first one listed.
fn natural(dim: Dim, side: Side, base: f64, like: f64) -> Option<&'static Unit> {
    let mut candidates = shown(dim, side);
    if dim == Speed {
        return candidates.first().copied();
    }
    candidates.sort_by(|a, b| a.per_base.total_cmp(&b.per_base));
    if base == 0.0 {
        return candidates.into_iter().min_by(|a, b| {
            let distance = |u: &Unit| (u.per_base / like).ln().abs();
            distance(a).total_cmp(&distance(b))
        });
    }
    let smallest = candidates.first().copied();
    candidates
        .into_iter()
        .rfind(|u| base.abs() / u.per_base >= 1.0)
        .or(smallest)
}

/// The units next to `first` that are worth showing as well: the next larger one when the amount is a good
/// fraction of it (1.5 km is 0.93 mi), and the next smaller one when the amount is only a few of `first`
/// (3.1 mi is 16,404 ft). For speeds, the next one listed.
fn neighbours(first: &Unit, base: f64) -> Vec<&'static Unit> {
    let mut list = shown(first.dim, first.side);
    if first.dim != Speed {
        list.sort_by(|a, b| a.per_base.total_cmp(&b.per_base));
    }
    let Some(at) = list.iter().position(|u| u.symbol == first.symbol) else {
        return Vec::new();
    };
    if first.dim == Speed {
        return list.get(at + 1).copied().into_iter().collect();
    }
    let mut out = Vec::new();
    if let Some(bigger) = list.get(at + 1)
        && (0.1..1.0).contains(&(base.abs() / bigger.per_base))
    {
        out.push(*bigger);
    }
    if at > 0 && base.abs() / first.per_base < 10.0 {
        out.push(list[at - 1]);
    }
    out
}

fn quantity(unit: &Unit, base: f64) -> Quantity {
    Quantity {
        value: base / unit.per_base,
        symbol: unit.symbol,
    }
}

/// What a text like "5 km" or "72 F" is in the other units, or `None` when it is not a quantity.
pub fn convert(text: &str, style: NumberStyle) -> Option<Converted> {
    let text = text.trim();
    let (value, used) = number::parse_prefix(text, style)?;
    let name = normalise(&text[used..]);
    if name.is_empty() {
        return None;
    }
    if let Some(from) = temperature(&name) {
        return Some(temperature_answers(value, from));
    }
    let unit = find(&name)?;
    let base = value * unit.per_base;
    let other = if unit.side == Side::A {
        Side::B
    } else {
        Side::A
    };
    let mut to = Vec::new();
    let first = natural(unit.dim, other, base, unit.per_base);
    if let Some(first) = first {
        to.push(quantity(first, base));
    }
    // The same side in a size that reads better: 5000 m is 5 km.
    if let Some(own) = natural(unit.dim, unit.side, base, unit.per_base)
        && own.symbol != unit.symbol
    {
        to.push(quantity(own, base));
    }
    if let Some(first) = first.filter(|_| base != 0.0) {
        to.extend(
            neighbours(first, base)
                .into_iter()
                .map(|n| quantity(n, base)),
        );
    }
    to.truncate(3);
    if to.is_empty() {
        return None;
    }
    Some(Converted {
        dim: unit.dim,
        from: quantity(unit, base),
        to,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const EN: NumberStyle = NumberStyle::ENGLISH;

    fn answers(text: &str) -> Vec<(String, &'static str)> {
        convert(text, EN)
            .map(|c| {
                c.to.iter()
                    .map(|q| (number::format(q.value, EN), q.symbol))
                    .collect()
            })
            .unwrap_or_default()
    }

    fn one(text: &str) -> (String, &'static str) {
        answers(text).into_iter().next().unwrap()
    }

    #[test]
    fn kilometres_become_miles_and_feet() {
        assert_eq!(
            answers("5 km"),
            vec![("3.107".into(), "mi"), ("16,404".into(), "ft")]
        );
    }

    #[test]
    fn small_lengths_become_inches() {
        assert_eq!(
            answers("30 cm"),
            vec![("11.81".into(), "in"), ("0.9843".into(), "ft")]
        );
        assert_eq!(
            answers("1.8 m"),
            vec![("5.906".into(), "ft"), ("70.87".into(), "in")]
        );
        assert_eq!(
            answers("1.5 km"),
            vec![("4,921".into(), "ft"), ("0.9321".into(), "mi")]
        );
    }

    #[test]
    fn imperial_becomes_metric_and_a_better_imperial_size() {
        assert_eq!(
            answers("5000 ft"),
            vec![("1.524".into(), "km"), ("1,524".into(), "m")],
            "below a mile, feet are still the natural size"
        );
        assert_eq!(
            answers("10000 ft"),
            vec![
                ("3.048".into(), "km"),
                ("1.894".into(), "mi"),
                ("3,048".into(), "m")
            ]
        );
        assert_eq!(one("26.2 miles"), ("42.16".into(), "km"));
    }

    #[test]
    fn the_unit_of_the_other_side_is_chosen_by_size() {
        assert_eq!(
            answers("500 g"),
            vec![("1.102".into(), "lb"), ("17.64".into(), "oz")]
        );
        assert_eq!(one("2 kg"), ("4.409".into(), "lb"));
        assert_eq!(one("3 lb"), ("1.361".into(), "kg"));
        assert_eq!(
            answers("100 ml"),
            vec![("3.381".into(), "fl oz"), ("0.4227".into(), "cup")]
        );
        assert_eq!(one("1 gallon"), ("3.785".into(), "l"));
        assert_eq!(
            answers("2 cups"),
            vec![("473.2".into(), "ml"), ("0.4732".into(), "l")]
        );
    }

    #[test]
    fn hidden_units_are_understood_but_not_offered() {
        assert_eq!(one("2 tbsp"), ("29.57".into(), "ml"));
        assert_eq!(one("10 yd"), ("9.144".into(), "m"));
        assert!(
            answers("5 km")
                .iter()
                .all(|(_, symbol)| !["yd", "tsp"].contains(symbol))
        );
    }

    #[test]
    fn temperatures_convert_both_ways() {
        assert_eq!(
            answers("72 F"),
            vec![("22.22".into(), "\u{b0}C"), ("295.4".into(), "K")]
        );
        let boiling = convert("100\u{b0}C", EN).unwrap();
        assert_eq!(number::format(boiling.to[0].value, EN), "212");
        assert!((boiling.to[1].value - 373.15).abs() < 1e-9);
        assert_eq!(one("0 celsius"), ("32".into(), "\u{b0}F"));
        assert_eq!(one("-40 f"), ("-40".into(), "\u{b0}C"));
        assert_eq!(one("0 kelvin"), ("-273.2".into(), "\u{b0}C"));
    }

    #[test]
    fn speeds_and_areas() {
        assert_eq!(
            answers("100 km/h"),
            vec![("62.14".into(), "mph"), ("91.13".into(), "ft/s")]
        );
        assert_eq!(
            answers("60 mph"),
            vec![("96.56".into(), "km/h"), ("26.82".into(), "m/s")]
        );
        assert_eq!(one("10 knots"), ("18.52".into(), "km/h"));
        assert_eq!(answers("100 m2"), vec![("1,076".into(), "ft\u{b2}")]);
        assert_eq!(
            answers("2 acres"),
            vec![("8,094".into(), "m\u{b2}"), ("0.8094".into(), "ha")]
        );
        assert_eq!(
            answers("5 acres"),
            vec![("2.023".into(), "ha"), ("20,234".into(), "m\u{b2}")]
        );
    }

    #[test]
    fn data_sizes_go_between_decimal_and_binary() {
        assert_eq!(
            answers("1.5 GB"),
            vec![("1.397".into(), "GiB"), ("1,431".into(), "MiB")]
        );
        assert_eq!(one("1 GiB"), ("1.074".into(), "GB"));
    }

    #[test]
    fn spellings_numbers_and_spacing_are_forgiving() {
        assert_eq!(one("5km"), one("5 KM"));
        assert_eq!(one("5 Kilometers"), one("5 km"));
        assert_eq!(one("  5   km.  "), one("5 km"));
        assert_eq!(one("6'"), ("1.829".into(), "m"));
        assert_eq!(one("70\""), ("1.778".into(), "m"));
        assert_eq!(
            convert("1,5 km", NumberStyle::VIETNAMESE)
                .unwrap()
                .from
                .value,
            1.5
        );
        assert_eq!(
            convert("1.500 m", NumberStyle::VIETNAMESE)
                .unwrap()
                .from
                .value,
            1500.0
        );
    }

    #[test]
    fn zero_and_negative_values_work() {
        assert_eq!(answers("0 km"), vec![("0".into(), "mi")]);
        assert_eq!(answers("0 cm"), vec![("0".into(), "in")]);
        assert_eq!(one("-5 km"), ("-3.107".into(), "mi"));
    }

    #[test]
    fn what_is_not_a_quantity_is_left_alone() {
        for text in [
            "hello",
            "5",
            "km",
            "5 bananas",
            "5 km and 3 m",
            "",
            "1.2.3 km",
        ] {
            assert!(convert(text, EN).is_none(), "{text}");
        }
    }

    #[test]
    fn every_name_belongs_to_one_unit() {
        let mut seen = std::collections::HashSet::new();
        for unit in UNITS {
            for name in unit.names {
                assert_eq!(*name, name.to_lowercase(), "names are lowercase");
                assert!(seen.insert(*name), "{name} is spelled twice");
                assert!(temperature(name).is_none(), "{name} is a temperature too");
            }
        }
    }
}
