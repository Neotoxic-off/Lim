//! The unit converter (`<Leader>xu`): `5 ft to cm`, `2 m` (every unit of its kind).

use super::calc::{calc, format_number};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Dim {
    Length,
    Area,
    Volume,
    Mass,
    Time,
    Speed,
    Temperature,
    Data,
    Energy,
    Power,
    Pressure,
    Angle,
}

impl Dim {
    fn name(self) -> &'static str {
        match self {
            Self::Length => "length",
            Self::Area => "area",
            Self::Volume => "volume",
            Self::Mass => "mass",
            Self::Time => "time",
            Self::Speed => "speed",
            Self::Temperature => "temperature",
            Self::Data => "data",
            Self::Energy => "energy",
            Self::Power => "power",
            Self::Pressure => "pressure",
            Self::Angle => "angle",
        }
    }
}

/// A unit: its names (the first is shown), what it measures, and `base = value * factor + offset` in the
/// dimension's base unit (m, m², L, kg, s, m/s, K, byte, J, W, Pa, rad).
struct Unit {
    names: &'static [&'static str],
    dim: Dim,
    factor: f64,
    offset: f64,
}

macro_rules! u {
    ($names:expr, $dim:expr, $factor:expr) => {
        Unit { names: $names, dim: $dim, factor: $factor, offset: 0.0 }
    };
}

const MILE: f64 = 1609.344;
const FOOT: f64 = 0.3048;
const INCH: f64 = 0.0254;
const POUND: f64 = 0.453_592_37;
const GALLON: f64 = 3.785_411_784;
const DAY: f64 = 86_400.0;

/// Names match exactly first (`Mb` megabit, `MB` megabyte), then ignoring case.
const UNITS: &[Unit] = &[
    u!(&["m", "meter", "meters", "metre", "metres", "mètre", "mètres"], Dim::Length, 1.0),
    u!(&["km", "kilometer", "kilometers", "kilometre", "kilomètre", "kilomètres"], Dim::Length, 1000.0),
    u!(&["cm", "centimeter", "centimeters", "centimètre", "centimètres"], Dim::Length, 0.01),
    u!(&["mm", "millimeter", "millimeters", "millimètre", "millimètres"], Dim::Length, 0.001),
    u!(&["µm", "um", "micron", "microns"], Dim::Length, 1e-6),
    u!(&["nm", "nanometer", "nanometers"], Dim::Length, 1e-9),
    u!(&["in", "inch", "inches", "pouce", "pouces", "\""], Dim::Length, INCH),
    u!(&["ft", "foot", "feet", "pied", "pieds", "'"], Dim::Length, FOOT),
    u!(&["yd", "yard", "yards"], Dim::Length, 0.9144),
    u!(&["mi", "mile", "miles"], Dim::Length, MILE),
    u!(&["nmi"], Dim::Length, 1852.0),
    u!(&["au"], Dim::Length, 149_597_870_700.0),
    u!(&["ly", "lightyear", "lightyears", "al"], Dim::Length, 9_460_730_472_580_800.0),
    u!(&["m2", "m²", "sqm"], Dim::Area, 1.0),
    u!(&["km2", "km²"], Dim::Area, 1e6),
    u!(&["cm2", "cm²"], Dim::Area, 1e-4),
    u!(&["mm2", "mm²"], Dim::Area, 1e-6),
    u!(&["ha", "hectare", "hectares"], Dim::Area, 1e4),
    u!(&["acre", "acres"], Dim::Area, 4_046.856_422_4),
    u!(&["ft2", "ft²", "sqft"], Dim::Area, FOOT * FOOT),
    u!(&["in2", "in²", "sqin"], Dim::Area, INCH * INCH),
    u!(&["mi2", "mi²", "sqmi"], Dim::Area, MILE * MILE),
    u!(&["l", "L", "liter", "liters", "litre", "litres"], Dim::Volume, 1.0),
    u!(&["ml", "mL", "milliliter", "millilitre"], Dim::Volume, 0.001),
    u!(&["cl", "cL", "centilitre"], Dim::Volume, 0.01),
    u!(&["dl", "dL", "decilitre"], Dim::Volume, 0.1),
    u!(&["m3", "m³"], Dim::Volume, 1000.0),
    u!(&["cm3", "cm³", "cc"], Dim::Volume, 0.001),
    u!(&["gal", "gallon", "gallons"], Dim::Volume, GALLON),
    u!(&["ukgal"], Dim::Volume, 4.546_09),
    u!(&["qt", "quart", "quarts"], Dim::Volume, GALLON / 4.0),
    u!(&["pt", "pint", "pints"], Dim::Volume, GALLON / 8.0),
    u!(&["cup", "cups", "tasse", "tasses"], Dim::Volume, GALLON / 16.0),
    u!(&["floz"], Dim::Volume, GALLON / 128.0),
    u!(&["tbsp", "tablespoon", "càs"], Dim::Volume, GALLON / 256.0),
    u!(&["tsp", "teaspoon", "càc"], Dim::Volume, GALLON / 768.0),
    u!(&["kg", "kilogram", "kilograms", "kilo", "kilos", "kilogramme", "kilogrammes"], Dim::Mass, 1.0),
    u!(&["g", "gram", "grams", "gramme", "grammes"], Dim::Mass, 0.001),
    u!(&["mg", "milligram", "milligrams", "milligramme"], Dim::Mass, 1e-6),
    u!(&["t", "tonne", "tonnes", "ton"], Dim::Mass, 1000.0),
    u!(&["lb", "lbs", "pound", "pounds", "livre", "livres"], Dim::Mass, POUND),
    u!(&["oz", "ounce", "ounces", "once", "onces"], Dim::Mass, POUND / 16.0),
    u!(&["st", "stone", "stones"], Dim::Mass, POUND * 14.0),
    u!(&["ct", "carat", "carats"], Dim::Mass, 0.0002),
    u!(&["s", "sec", "second", "seconds", "seconde", "secondes"], Dim::Time, 1.0),
    u!(&["ms", "millisecond", "milliseconds", "milliseconde"], Dim::Time, 0.001),
    u!(&["µs", "us", "microsecond", "microseconds"], Dim::Time, 1e-6),
    u!(&["ns", "nanosecond", "nanoseconds"], Dim::Time, 1e-9),
    u!(&["min", "minute", "minutes"], Dim::Time, 60.0),
    u!(&["h", "hr", "hour", "hours", "heure", "heures"], Dim::Time, 3600.0),
    u!(&["d", "day", "days", "jour", "jours"], Dim::Time, DAY),
    u!(&["wk", "week", "weeks", "semaine", "semaines"], Dim::Time, 7.0 * DAY),
    u!(&["mo", "month", "months", "mois"], Dim::Time, 30.436_875 * DAY),
    u!(&["yr", "year", "years", "an", "ans", "année", "années"], Dim::Time, 365.2425 * DAY),
    u!(&["m/s", "mps"], Dim::Speed, 1.0),
    u!(&["km/h", "kmh", "kph"], Dim::Speed, 1.0 / 3.6),
    u!(&["mph", "mi/h"], Dim::Speed, MILE / 3600.0),
    u!(&["kn", "knot", "knots", "noeud", "noeuds", "nœud", "nœuds"], Dim::Speed, 1852.0 / 3600.0),
    u!(&["ft/s", "fps"], Dim::Speed, FOOT),
    Unit { names: &["c", "°c", "celsius"], dim: Dim::Temperature, factor: 1.0, offset: 273.15 },
    Unit {
        names: &["f", "°f", "fahrenheit"],
        dim: Dim::Temperature,
        factor: 5.0 / 9.0,
        offset: 273.15 - 32.0 * 5.0 / 9.0,
    },
    u!(&["k", "kelvin"], Dim::Temperature, 1.0),
    u!(&["B", "byte", "bytes", "octet", "octets", "o"], Dim::Data, 1.0),
    u!(&["bit", "bits", "b"], Dim::Data, 0.125),
    u!(&["KB", "kB", "kilobyte", "Ko"], Dim::Data, 1e3),
    u!(&["MB", "megabyte", "Mo"], Dim::Data, 1e6),
    u!(&["GB", "gigabyte", "Go"], Dim::Data, 1e9),
    u!(&["TB", "terabyte", "To"], Dim::Data, 1e12),
    u!(&["PB", "petabyte"], Dim::Data, 1e15),
    u!(&["KiB", "kibibyte", "Kio"], Dim::Data, 1024.0),
    u!(&["MiB", "mebibyte", "Mio"], Dim::Data, 1_048_576.0),
    u!(&["GiB", "gibibyte", "Gio"], Dim::Data, 1_073_741_824.0),
    u!(&["TiB", "tebibyte", "Tio"], Dim::Data, 1_099_511_627_776.0),
    u!(&["Kb", "kbit", "kilobit"], Dim::Data, 125.0),
    u!(&["Mb", "mbit", "megabit"], Dim::Data, 125_000.0),
    u!(&["Gb", "gbit", "gigabit"], Dim::Data, 125_000_000.0),
    u!(&["J", "joule", "joules"], Dim::Energy, 1.0),
    u!(&["kJ", "kilojoule", "kilojoules"], Dim::Energy, 1000.0),
    u!(&["cal", "calorie", "calories"], Dim::Energy, 4.184),
    u!(&["kcal", "kilocalorie", "kilocalories", "Cal"], Dim::Energy, 4184.0),
    u!(&["Wh", "watt-hour"], Dim::Energy, 3600.0),
    u!(&["kWh", "kilowatt-hour"], Dim::Energy, 3.6e6),
    u!(&["eV", "electronvolt"], Dim::Energy, 1.602_176_634e-19),
    u!(&["BTU", "btu"], Dim::Energy, 1055.06),
    u!(&["W", "watt", "watts"], Dim::Power, 1.0),
    u!(&["kW", "kilowatt", "kilowatts"], Dim::Power, 1000.0),
    u!(&["MW", "megawatt", "megawatts"], Dim::Power, 1e6),
    u!(&["hp", "horsepower"], Dim::Power, 745.699_872),
    u!(&["ch", "cv", "cheval", "chevaux"], Dim::Power, 735.498_75),
    u!(&["Pa", "pascal", "pascals"], Dim::Pressure, 1.0),
    u!(&["hPa", "hectopascal"], Dim::Pressure, 100.0),
    u!(&["kPa", "kilopascal"], Dim::Pressure, 1000.0),
    u!(&["bar", "bars"], Dim::Pressure, 1e5),
    u!(&["mbar", "millibar"], Dim::Pressure, 100.0),
    u!(&["atm", "atmosphere", "atmosphère"], Dim::Pressure, 101_325.0),
    u!(&["psi"], Dim::Pressure, 6_894.757_293_168),
    u!(&["mmHg", "torr"], Dim::Pressure, 133.322_387_415),
    u!(&["rad", "radian", "radians"], Dim::Angle, 1.0),
    u!(&["deg", "°", "degree", "degrees", "degré", "degrés"], Dim::Angle, std::f64::consts::PI / 180.0),
    u!(&["grad", "gon"], Dim::Angle, std::f64::consts::PI / 200.0),
    u!(&["turn", "turns", "tour", "tours"], Dim::Angle, std::f64::consts::TAU),
];

fn unit(name: &str) -> Option<&'static Unit> {
    UNITS
        .iter()
        .find(|u| u.names.contains(&name))
        .or_else(|| UNITS.iter().find(|u| u.names.iter().any(|n| n.eq_ignore_ascii_case(name))))
}

/// Words that join the value and the target unit: `5 ft to cm`, `70 kg en lb`.
const JOINS: [&str; 7] = ["to", "in", "en", "as", "into", "->", "="];

/// `5 ft to cm`, `70kg lb`, `100 f in c`, `1.5 GiB MB`; without a target, the value in every unit of its kind.
pub fn convert(input: &str) -> Result<String, String> {
    let text: &str = input.trim();
    // The value is the leading arithmetic; the unit starts at the first other char (a letter, `°`, `µ`, `'`).
    let mut end: usize = 0;
    let chars: Vec<(usize, char)> = text.char_indices().collect();
    for (k, &(i, c)) in chars.iter().enumerate() {
        let exponent: bool = matches!(c, 'e' | 'E')
            && k > 0
            && chars[k - 1].1.is_ascii_digit()
            && chars.get(k + 1).is_some_and(|&(_, n)| n.is_ascii_digit() || n == '-' || n == '+');
        if !(c.is_ascii_digit() || "+-*/^.() ,_".contains(c) || exponent) {
            break;
        }
        end = i + c.len_utf8();
    }
    let (number, rest): (&str, &str) = text.split_at(end);
    let value: f64 = calc(&number.replace(',', "."), None).ok_or("start with a number: 5 ft to cm")?;
    // A joining word only between two units, so `in` stays inches in `1 in cm` and `1 in in cm`.
    let words: Vec<&str> = rest.split_whitespace().collect();
    let (from, to): (&str, Option<&str>) = match words[..] {
        [from] => (from, None),
        [from, to] => (from, Some(to)),
        [from, join, to] if JOINS.contains(&join.to_ascii_lowercase().as_str()) => (from, Some(to)),
        [] => return Err("which unit? 5 ft to cm".into()),
        _ => return Err("one unit to convert from, one to convert to: 5 ft to cm".into()),
    };
    let from_unit: &Unit = unit(from).ok_or_else(|| format!("unknown unit: {from}"))?;
    let base: f64 = value * from_unit.factor + from_unit.offset;
    let show = |u: &Unit| format!("{} {}", format_number((base - u.offset) / u.factor), u.names[0]);
    let shown: String = match to {
        Some(to) => {
            let to_unit: &Unit = unit(to).ok_or_else(|| format!("unknown unit: {to}"))?;
            if to_unit.dim != from_unit.dim {
                return Err(format!("{from} is a {}, {to} a {}", from_unit.dim.name(), to_unit.dim.name()));
            }
            show(to_unit)
        }
        None => UNITS
            .iter()
            .filter(|u| u.dim == from_unit.dim && !std::ptr::eq(*u, from_unit))
            .map(show)
            .collect::<Vec<String>>()
            .join(" · "),
    };
    Ok(shown)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn units() {
        assert_eq!(convert("5 ft to cm").unwrap(), "152.4 cm");
        assert_eq!(convert("70kg en lb").unwrap(), "154.3235835294 lb");
        assert_eq!(convert("100 f in c").unwrap(), "37.7777777778 c");
        assert_eq!(convert("-40 c f").unwrap(), "-40 f");
        assert_eq!(convert("1 GiB MB").unwrap(), "1073.741824 MB");
        assert_eq!(convert("8 Mb to MB").unwrap(), "1 MB");
        assert_eq!(convert("90 km/h mph").unwrap(), "55.9234073014 mph");
        assert_eq!(convert("1,5 h min").unwrap(), "90 min");
        assert!(convert("2 m").unwrap().contains("200 cm"), "every unit of the kind");
        assert_eq!(convert("1 m to kg").unwrap_err(), "m is a length, kg a mass");
        // Names are unique (case aside, which tells `Mb` from `MB`), so none hides another.
        let names: Vec<&str> = UNITS.iter().flat_map(|u| u.names.iter().copied()).collect();
        for (i, name) in names.iter().enumerate() {
            assert!(!names[i + 1..].contains(name), "{name} names two units");
        }
        // Every unit converts to the first of its kind, and names its kind when they differ.
        for unit in UNITS {
            let first: &Unit = UNITS.iter().find(|u| u.dim == unit.dim).unwrap();
            for name in unit.names {
                assert!(convert(&format!("2 {name} to {}", first.names[0])).is_ok(), "{name}");
            }
            let other: &Unit = UNITS.iter().find(|u| u.dim != unit.dim).unwrap();
            let message: String = convert(&format!("1 {} {}", unit.names[0], other.names[0])).unwrap_err();
            assert!(message.contains(unit.dim.name()), "{message}");
        }
        assert!(convert("5").is_err());
        assert!(convert("5 ft cm in").is_err());
        assert_eq!(convert("1 in cm").unwrap(), "2.54 cm", "in is inches");
        assert_eq!(convert("1 in in cm").unwrap(), "2.54 cm");
        assert_eq!(convert("1 ft to in").unwrap(), "12 in");
        assert!(convert("1 parsec cm").is_err());
        assert!(convert("ft to cm").is_err());
    }
}
