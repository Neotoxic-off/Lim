//! Colors (`<Leader>xh`): a color as hex, `rgb()` and `hsl()`.

/// `#f80`, `#ff8800`, `ff8800`, `rgb(255, 136, 0)`, `255 136 0`, `hsl(32, 100%, 50%)`: the color in all three forms.
pub fn color(input: &str) -> Result<String, String> {
    let text: String = input.trim().to_ascii_lowercase();
    let numbers = |inner: &str| -> Option<Vec<f64>> {
        inner
            .split([',', ' ', '/'])
            .filter(|p| !p.is_empty())
            .map(|p| p.trim_end_matches(['%', '°']).parse().ok())
            .collect()
    };
    let rgb: [u8; 3] = if let Some(inner) = text.strip_prefix("hsl(").and_then(|t| t.strip_suffix(')')) {
        let [h, s, l]: [f64; 3] = numbers(inner).and_then(|n| n.try_into().ok()).ok_or("hsl(h, s%, l%)")?;
        hsl_to_rgb(h, s / 100.0, l / 100.0)
    } else if let Some(hex) =
        text.strip_prefix('#').or_else(|| text.chars().all(|c| c.is_ascii_hexdigit()).then_some(&text[..]))
    {
        let full: String = match hex.len() {
            3 => hex.chars().flat_map(|c| [c, c]).collect(),
            6 => hex.to_owned(),
            _ => return Err("#rgb or #rrggbb".into()),
        };
        let value: u32 = u32::from_str_radix(&full, 16).map_err(|_| "#rgb or #rrggbb")?;
        let [_, r, g, b]: [u8; 4] = value.to_be_bytes();
        [r, g, b]
    } else {
        let inner: &str = text.strip_prefix("rgb(").and_then(|t| t.strip_suffix(')')).unwrap_or(&text);
        let parts: Vec<f64> = numbers(inner).ok_or("rgb(r, g, b) with 0-255")?;
        let [r, g, b]: [f64; 3] = parts.try_into().map_err(|_| "rgb(r, g, b) with 0-255")?;
        if [r, g, b].iter().any(|c| !(0.0..=255.0).contains(c)) {
            return Err("rgb(r, g, b) with 0-255".into());
        }
        [channel(r / 255.0), channel(g / 255.0), channel(b / 255.0)]
    };
    let [r, g, b]: [u8; 3] = rgb;
    let (h, s, l): (f64, f64, f64) = rgb_to_hsl(rgb);
    Ok(format!("#{r:02x}{g:02x}{b:02x} · rgb({r}, {g}, {b}) · hsl({:.0}, {:.0}%, {:.0}%)", h, s * 100.0, l * 100.0))
}

/// A 0..=1 fraction as a 0..=255 channel.
#[expect(clippy::cast_possible_truncation, clippy::cast_sign_loss, reason = "clamped to 0..=255 first")]
fn channel(x: f64) -> u8 {
    (x.clamp(0.0, 1.0) * 255.0).round() as u8
}

fn hsl_to_rgb(h: f64, s: f64, l: f64) -> [u8; 3] {
    let (s, l): (f64, f64) = (s.clamp(0.0, 1.0), l.clamp(0.0, 1.0));
    let c: f64 = (1.0 - (2.0 * l - 1.0).abs()) * s;
    let h: f64 = h.rem_euclid(360.0) / 60.0;
    let x: f64 = c * (1.0 - (h % 2.0 - 1.0).abs());
    let (r, g, b): (f64, f64, f64) = match h {
        h if h < 1.0 => (c, x, 0.0),
        h if h < 2.0 => (x, c, 0.0),
        h if h < 3.0 => (0.0, c, x),
        h if h < 4.0 => (0.0, x, c),
        h if h < 5.0 => (x, 0.0, c),
        _ => (c, 0.0, x),
    };
    let m: f64 = l - c / 2.0;
    [channel(r + m), channel(g + m), channel(b + m)]
}

fn rgb_to_hsl([r, g, b]: [u8; 3]) -> (f64, f64, f64) {
    let (r, g, b): (f64, f64, f64) = (f64::from(r) / 255.0, f64::from(g) / 255.0, f64::from(b) / 255.0);
    let (max, min): (f64, f64) = (r.max(g).max(b), r.min(g).min(b));
    let l: f64 = f64::midpoint(max, min);
    let d: f64 = max - min;
    if d == 0.0 {
        return (0.0, 0.0, l);
    }
    let s: f64 = d / (1.0 - (2.0 * l - 1.0).abs());
    // `max` is one of the three channels exactly, so comparing floats for equality is right here.
    let h: f64 = if max.to_bits() == r.to_bits() {
        60.0 * ((g - b) / d).rem_euclid(6.0)
    } else if max.to_bits() == g.to_bits() {
        60.0 * ((b - r) / d + 2.0)
    } else {
        60.0 * ((r - g) / d + 4.0)
    };
    (h, s, l)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn colors() {
        assert_eq!(color("#f80").unwrap(), "#ff8800 · rgb(255, 136, 0) · hsl(32, 100%, 50%)");
        // Every sixth of the hue circle, and grey, both ways.
        for hue in [0, 90, 150, 210, 270, 330] {
            let shown: String = color(&format!("hsl({hue}, 100%, 50%)")).unwrap();
            assert!(shown.ends_with(&format!("hsl({hue}, 100%, 50%)")), "{shown}");
        }
        assert_eq!(color("#808080").unwrap(), "#808080 · rgb(128, 128, 128) · hsl(0, 0%, 50%)");
        assert!(color("hsl(1, 2)").is_err());
        assert!(color("1 2").is_err());
        assert_eq!(color("rgb(255, 136, 0)").unwrap(), color("ff8800").unwrap());
        assert_eq!(color("hsl(32, 100%, 50%)").unwrap().get(..7), Some("#ff8800"));
        assert!(color("#12").is_err());
        assert!(color("300 0 0").is_err());
    }
}
