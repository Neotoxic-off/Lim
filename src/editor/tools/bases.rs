//! Number bases (`<Leader>xb`): a number in decimal, hex, octal and binary, and as a character.

use std::fmt::Write;

use super::calc::calc;

/// `255`, `0xff`, `0b1010`, `0o17`, `-42` or any whole calculation: the number in decimal, hex, octal and binary,
/// and as a character when it is one.
pub fn bases(input: &str, ans: Option<f64>) -> Result<String, String> {
    let text: String = input.trim().replace('_', "");
    let (negative, digits): (bool, &str) = text.strip_prefix('-').map_or((false, text.as_str()), |d| (true, d));
    let lower: String = digits.to_ascii_lowercase();
    let parsed: Option<u128> = match lower.get(..2) {
        Some("0x") => u128::from_str_radix(&lower[2..], 16).ok(),
        Some("0b") => u128::from_str_radix(&lower[2..], 2).ok(),
        Some("0o") => u128::from_str_radix(&lower[2..], 8).ok(),
        _ => lower.parse().ok(),
    };
    let (negative, magnitude): (bool, u128) = if let Some(n) = parsed {
        (negative, n)
    } else {
        let value: f64 = calc(input, ans).filter(|v| v.fract() == 0.0 && v.abs() < 1e30).ok_or("not a whole number")?;
        let magnitude: u128 = format!("{:.0}", value.abs()).parse().map_err(|_| "not a whole number")?;
        (value < 0.0, magnitude)
    };
    let sign: &str = if negative && magnitude != 0 { "-" } else { "" };
    let binary: String = format!("{magnitude:b}");
    // Binary digits in groups of four, from the right.
    let grouped: String = binary.chars().enumerate().fold(String::new(), |mut out, (i, c)| {
        if i > 0 && (binary.len() - i).is_multiple_of(4) {
            out.push('_');
        }
        out.push(c);
        out
    });
    let mut shown: String =
        format!("{sign}{magnitude} · {sign}0x{magnitude:x} · {sign}0o{magnitude:o} · {sign}0b{grouped}");
    if !negative && let Some(c) = u32::try_from(magnitude).ok().and_then(char::from_u32).filter(|c| !c.is_control()) {
        let _: std::fmt::Result = write!(shown, " · '{c}' U+{magnitude:04X}");
    }
    Ok(shown)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shows_every_base() {
        assert_eq!(bases("255", None).unwrap(), "255 · 0xff · 0o377 · 0b1111_1111 · 'ÿ' U+00FF");
        assert_eq!(bases("0x41", None).unwrap(), "65 · 0x41 · 0o101 · 0b100_0001 · 'A' U+0041");
        assert_eq!(bases("-0b101", None).unwrap(), "-5 · -0x5 · -0o5 · -0b101");
        assert_eq!(bases("2^40", None).unwrap().split(" · ").nth(1), Some("0x10000000000"));
        assert!(bases("1.5", None).is_err());
    }
}
