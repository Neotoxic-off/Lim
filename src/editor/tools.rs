//! `<Leader>x` tools: a calculator, a unit converter, number bases, timestamps, colors, word case, JSON layout and
//! generated text (dates, UUIDs, passwords, lorem ipsum). Each tool is a pure function of the typed text; the editor
//! shows the answer, yanks it into `"` and keeps the prompt open for the next question.

use std::collections::hash_map::RandomState;
use std::fmt::Write;
use std::hash::BuildHasher;
use std::time::{SystemTime, UNIX_EPOCH};

use super::Editor;
use super::prompt::PromptAction;
use super::registers::Register;
use crate::buffer::{Buffer, Pos};
use crate::config::json;
use crate::term;

/// The tools that answer a line of input, each in its own window (a notebook buffer).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tool {
    Calc,
    Convert,
    Base,
    Time,
    Color,
}

/// Text the `<Leader>x` keys insert at the cursor.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Generated {
    Date,
    DateTime,
    Uuid,
    Password,
    Lorem,
}

/// Identifier and phrase cases for `<Leader>xk` and `:case`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WordCase {
    Snake,
    Camel,
    Pascal,
    Kebab,
    Constant,
    Title,
    Lower,
    Upper,
}

impl WordCase {
    pub const NAMES: [(&'static str, Self); 8] = [
        ("snake", Self::Snake),
        ("camel", Self::Camel),
        ("pascal", Self::Pascal),
        ("kebab", Self::Kebab),
        ("constant", Self::Constant),
        ("title", Self::Title),
        ("lower", Self::Lower),
        ("upper", Self::Upper),
    ];

    /// The case named `name`, or by a prefix of its name (`sn`, `k`).
    pub fn parse(name: &str) -> Option<Self> {
        let name: String = name.trim().to_ascii_lowercase();
        Self::NAMES.iter().find(|(n, _)| !name.is_empty() && n.starts_with(name.as_str())).map(|&(_, case)| case)
    }

    /// `text` split into words (at blanks, punctuation and camelCase humps), joined in this case.
    pub fn apply(self, text: &str) -> String {
        let words: Vec<String> = words(text);
        let capital = |w: &String| -> String {
            let mut chars = w.chars();
            chars
                .next()
                .map(|c| c.to_uppercase().chain(chars.flat_map(char::to_lowercase)).collect())
                .unwrap_or_default()
        };
        let lower: Vec<String> = words.iter().map(|w| w.to_lowercase()).collect();
        match self {
            Self::Snake => lower.join("_"),
            Self::Kebab => lower.join("-"),
            Self::Constant => words.iter().map(|w| w.to_uppercase()).collect::<Vec<String>>().join("_"),
            Self::Lower => lower.join(" "),
            Self::Upper => words.iter().map(|w| w.to_uppercase()).collect::<Vec<String>>().join(" "),
            Self::Title => words.iter().map(capital).collect::<Vec<String>>().join(" "),
            Self::Pascal => words.iter().map(capital).collect(),
            Self::Camel => {
                let mut out: String = lower.first().cloned().unwrap_or_default();
                out.extend(words.iter().skip(1).map(capital));
                out
            }
        }
    }
}

/// The words of `text`: runs of letters and digits, also split where a lowercase letter meets an uppercase one
/// (`fooBar`) and before the last capital of an acronym (`HTTPServer`).
fn words(text: &str) -> Vec<String> {
    let chars: Vec<char> = text.chars().collect();
    let mut words: Vec<String> = Vec::new();
    let mut word: String = String::new();
    for (i, &c) in chars.iter().enumerate() {
        if !c.is_alphanumeric() {
            if !word.is_empty() {
                words.push(std::mem::take(&mut word));
            }
            continue;
        }
        if !word.is_empty() && c.is_uppercase() {
            let prev: char = chars[i - 1];
            let hump: bool = prev.is_lowercase() || prev.is_numeric();
            let acronym_end: bool = prev.is_uppercase() && chars.get(i + 1).is_some_and(|n| n.is_lowercase());
            if hump || acronym_end {
                words.push(std::mem::take(&mut word));
            }
        }
        word.push(c);
    }
    if !word.is_empty() {
        words.push(word);
    }
    words
}

impl Tool {
    pub const ALL: [Self; 5] = [Self::Calc, Self::Convert, Self::Base, Self::Time, Self::Color];

    /// The window's buffer name.
    pub fn title(self) -> &'static str {
        match self {
            Self::Calc => "[Calculator]",
            Self::Convert => "[Convert]",
            Self::Base => "[Bases]",
            Self::Time => "[Time]",
            Self::Color => "[Color]",
        }
    }

    /// What to type, shown when the window opens.
    fn hint(self) -> &'static str {
        match self {
            Self::Calc => "2+3*4, sqrt(2), 2pi, ans*2",
            Self::Convert => "5 ft to cm, 70 kg en lb, 100 f c, 2 m",
            Self::Base => "255, 0xff, 0b1010, 2^40",
            Self::Time => "now (or an empty line), 1700000000, 2026-10-18 14:30, two dates",
            Self::Color => "#f80, rgb(255, 136, 0), hsl(32, 100%, 50%)",
        }
    }

    /// The answer to `input`; `ans` is the calculator's previous result.
    pub fn run(self, input: &str, ans: Option<f64>) -> Result<String, String> {
        match self {
            Self::Calc => evaluate(input, ans).map(format_number),
            Self::Convert => convert(input),
            Self::Base => bases(input, ans),
            Self::Time => time(input, now()),
            Self::Color => color(input),
        }
    }
}

// --- Calculator ---------------------------------------------------------------------------------------------

/// `expr` evaluated: `+ - * / % ^ !`, `x%` (a percentage), `√`, `²`, `³`, parentheses (also `[ ]` and `{ }`; the
/// ones left open at the end are closed), `0x`/`0b`/`0o` literals, `1e3`, `1,5` (a decimal comma outside function
/// arguments), functions (`sqrt(2)`, `log(8, 2)`, `max(1, 2, 3)`), constants (`pi`, `e`, `tau`, `phi`) and `ans`.
/// `×` `·` `÷` `−` and `**` are read as `*` `*` `/` `-` `^`. A value next to a name or a parenthesis multiplies
/// (`2pi`, `3(4+1)`, `(1+1)(2)`). The error says what is wrong and where.
pub fn evaluate(expr: &str, ans: Option<f64>) -> Result<f64, String> {
    let mut tokens: Vec<(char, usize)> = Vec::new();
    for (col, c) in expr.chars().enumerate() {
        let c: char = match c {
            '[' | '{' => '(',
            ']' | '}' => ')',
            '×' | '·' | '∙' => '*',
            '÷' | '∶' => '/',
            '−' | '–' => '-',
            // `**` is a power: the second `*` turns the pair into `^`.
            '*' if tokens.last().is_some_and(|&(prev, _)| prev == '*') => {
                tokens.pop();
                '^'
            }
            c if c.is_whitespace() || c == '_' => continue,
            c => c,
        };
        tokens.push((c, col + 1));
    }
    if tokens.is_empty() {
        return Err("type an expression, e.g. 2+3*4".into());
    }
    let mut parser: Calc = Calc { tokens, pos: 0, depth: 0, args: 0, ans };
    let value: f64 = parser.sum()?;
    if let Some(&(c, col)) = parser.tokens.get(parser.pos) {
        return Err(if c == ')' { format!("unmatched ) at column {col}") } else { parser.unexpected() });
    }
    if value.is_nan() {
        Err("undefined result (like sqrt(-1), log(0) or 0/0)".into())
    } else if value.is_infinite() {
        Err("infinite result (a division by zero or an overflow)".into())
    } else {
        Ok(value)
    }
}

/// `evaluate` for the callers that only need the value.
pub fn calc(expr: &str, ans: Option<f64>) -> Option<f64> {
    evaluate(expr, ans).ok()
}

/// How deep parentheses, signs and powers may nest: `((((…` or `----…` must not overflow the stack.
const MAX_DEPTH: usize = 64;

struct Calc {
    /// The chars without blanks, each with its column in the input (1-based), for error messages.
    tokens: Vec<(char, usize)>,
    pos: usize,
    depth: usize,
    /// Function calls open around the current token: inside one, `,` separates arguments.
    args: usize,
    ans: Option<f64>,
}

type Value = Result<f64, String>;

impl Calc {
    fn peek(&self) -> Option<char> {
        self.tokens.get(self.pos).map(|&(c, _)| c)
    }

    fn peek_at(&self, ahead: usize) -> Option<char> {
        self.tokens.get(self.pos + ahead).map(|&(c, _)| c)
    }

    fn eat(&mut self, ops: &str) -> Option<char> {
        let c: char = self.peek().filter(|c| ops.contains(*c))?;
        self.pos += 1;
        Some(c)
    }

    /// The error for the token at `pos`: an unexpected char, or the input ending too soon.
    fn unexpected(&self) -> String {
        match self.tokens.get(self.pos) {
            Some(&(c, col)) => format!("unexpected {c} at column {col}"),
            None => format!("incomplete: something is missing after {}", self.tokens.last().map_or(' ', |t| t.0)),
        }
    }

    /// Whether an operand starts here: a number, a name, a parenthesis, `π` or `√`.
    fn at_operand(&self) -> bool {
        self.peek().is_some_and(|c| c.is_alphanumeric() || matches!(c, '.' | '(' | 'π' | '√'))
    }

    /// Enters one nesting level, failing past `MAX_DEPTH`.
    fn deeper(&mut self) -> Result<(), String> {
        self.depth += 1;
        if self.depth > MAX_DEPTH { Err("too deeply nested".into()) } else { Ok(()) }
    }

    fn sum(&mut self) -> Value {
        let mut value: f64 = self.product()?;
        while let Some(op) = self.eat("+-") {
            let rhs: f64 = self.product()?;
            value = if op == '+' { value + rhs } else { value - rhs };
        }
        Ok(value)
    }

    fn product(&mut self) -> Value {
        let mut value: f64 = self.unary()?;
        loop {
            // `%` before an operand is a remainder; `postfix` already read any other `%` as a percentage.
            let op: char = match self.eat("*/%") {
                Some(op) => op,
                None if self.at_operand() => '*',
                None => return Ok(value),
            };
            let rhs: f64 = self.unary()?;
            value = match op {
                '*' => value * rhs,
                '/' => value / rhs,
                _ => value % rhs,
            };
        }
    }

    /// Signs bind looser than `^`: `-2^2` is -4.
    fn unary(&mut self) -> Value {
        self.deeper()?;
        let value: Value = match self.eat("+-") {
            Some('-') => self.unary().map(|v| -v),
            Some(_) => self.unary(),
            None => self.power(),
        };
        self.depth -= 1;
        value
    }

    /// `^` binds right: `2^3^2` is `2^9`.
    fn power(&mut self) -> Value {
        let base: f64 = self.postfix()?;
        if self.eat("^").is_some() {
            return Ok(base.powf(self.unary()?));
        }
        Ok(base)
    }

    fn postfix(&mut self) -> Value {
        let mut value: f64 = self.primary()?;
        loop {
            match self.peek() {
                Some('!') => value = factorial(value)?,
                Some('²') => value = value.powi(2),
                Some('³') => value = value.powi(3),
                Some('%') if !self.peek_at(1).is_some_and(|c| c.is_alphanumeric() || ".(π√+-".contains(c)) => {
                    value /= 100.0;
                }
                _ => return Ok(value),
            }
            self.pos += 1;
        }
    }

    /// `)` or the end of the input, which closes what is still open.
    fn close(&mut self) -> Result<(), String> {
        if self.eat(")").is_some() || self.peek().is_none() { Ok(()) } else { Err(self.unexpected()) }
    }

    fn primary(&mut self) -> Value {
        let Some(c): Option<char> = self.peek() else { return Err(self.unexpected()) };
        match c {
            '(' => {
                self.pos += 1;
                if self.peek() == Some(')') {
                    return Err(format!("empty parentheses at column {}", self.tokens[self.pos - 1].1));
                }
                self.deeper()?;
                let saved: usize = std::mem::take(&mut self.args);
                let value: Value = self.sum();
                self.args = saved;
                self.depth -= 1;
                let value: f64 = value?;
                self.close()?;
                Ok(value)
            }
            'π' => {
                self.pos += 1;
                Ok(std::f64::consts::PI)
            }
            '√' => {
                self.pos += 1;
                self.deeper()?;
                let value: Value = self.postfix().map(f64::sqrt);
                self.depth -= 1;
                value
            }
            c if c.is_ascii_digit() || c == '.' => self.number(),
            c if c.is_alphabetic() => self.name(),
            _ => Err(self.unexpected()),
        }
    }

    /// A constant, `ans`, or a function call.
    fn name(&mut self) -> Value {
        let start: usize = self.pos;
        while self.peek().is_some_and(char::is_alphanumeric) {
            self.pos += 1;
        }
        let name: String = self.tokens[start..self.pos].iter().map(|&(c, _)| c).collect::<String>().to_lowercase();
        // A constant before `(` multiplies it (`2pi(1+1)`); any other name before `(` is a function call.
        match name.as_str() {
            "pi" => return Ok(std::f64::consts::PI),
            "e" => return Ok(std::f64::consts::E),
            "tau" => return Ok(std::f64::consts::TAU),
            "phi" => return Ok(1.618_033_988_749_895),
            "ans" => return self.ans.ok_or_else(|| "no previous answer for ans yet".into()),
            _ => {}
        }
        if self.eat("(").is_none() {
            return Err(format!("unknown name {name} at column {}", self.tokens[start].1));
        }
        self.deeper()?;
        self.args += 1;
        let mut args: Vec<f64> = Vec::new();
        let parsed: Result<(), String> = loop {
            match self.sum() {
                Ok(value) => args.push(value),
                Err(e) => break Err(e),
            }
            if self.eat(",").is_none() {
                break Ok(());
            }
        };
        self.args -= 1;
        self.depth -= 1;
        parsed?;
        self.close()?;
        function(&name, &args)
    }

    /// `12`, `1.5`, `.5`, `1,5`, `1e-3`, `0xff`, `0b101`, `0o17`.
    fn number(&mut self) -> Value {
        let start: usize = self.pos;
        let radix: Option<u32> = match (self.peek(), self.peek_at(1).map(|c| c.to_ascii_lowercase())) {
            (Some('0'), Some('x')) => Some(16),
            (Some('0'), Some('b')) => Some(2),
            (Some('0'), Some('o')) => Some(8),
            _ => None,
        };
        if let Some(radix) = radix {
            self.pos += 2;
            let first: usize = self.pos;
            while self.peek().is_some_and(|c| c.is_digit(radix)) {
                self.pos += 1;
            }
            let digits: String = self.tokens[first..self.pos].iter().map(|&(c, _)| c).collect();
            return u64::from_str_radix(&digits, radix)
                .map(u64_to_f64)
                .map_err(|_| format!("bad number at column {}", self.tokens[start].1));
        }
        let mut text: String = String::new();
        loop {
            match self.peek() {
                Some(c) if c.is_ascii_digit() || c == '.' => text.push(c),
                // A decimal comma, unless it separates function arguments.
                Some(',') if self.args == 0 && self.peek_at(1).is_some_and(|c| c.is_ascii_digit()) => text.push('.'),
                _ => break,
            }
            self.pos += 1;
        }
        // An exponent only when digits follow, so `2e` stays 2 times e.
        let sign: usize = usize::from(matches!(self.peek_at(1), Some('+' | '-')));
        if matches!(self.peek(), Some('e' | 'E')) && self.peek_at(1 + sign).is_some_and(|c| c.is_ascii_digit()) {
            text.push('e');
            self.pos += 1;
            while self.peek().is_some_and(|c| c.is_ascii_digit() || c == '+' || c == '-') {
                let c: char = self.peek().unwrap_or('0');
                // Only the sign right after `e` belongs to the number.
                if !c.is_ascii_digit() && !text.ends_with('e') {
                    break;
                }
                text.push(c);
                self.pos += 1;
            }
        }
        text.parse().map_err(|_| format!("bad number {text} at column {}", self.tokens[start].1))
    }
}

#[expect(clippy::cast_precision_loss, reason = "a calculator works in f64; huge literals round like any float")]
fn u64_to_f64(n: u64) -> f64 {
    n as f64
}

fn factorial(n: f64) -> Value {
    if n.fract() != 0.0 || !(0.0..=170.0).contains(&n) {
        return Err("! needs a whole number from 0 to 170".into());
    }
    let mut product: f64 = 1.0;
    let mut k: f64 = 2.0;
    while k <= n {
        product *= k;
        k += 1.0;
    }
    Ok(product)
}

fn function(name: &str, args: &[f64]) -> Value {
    let wrong = || format!("wrong number of arguments for {name}");
    let one = |f: fn(f64) -> f64| match args {
        [x] => Ok(f(*x)),
        _ => Err(wrong()),
    };
    let two = |f: fn(f64, f64) -> f64| match args {
        [x, y] => Ok(f(*x, *y)),
        _ => Err(wrong()),
    };
    match name {
        "sqrt" => one(f64::sqrt),
        "cbrt" => one(f64::cbrt),
        "abs" => one(f64::abs),
        "floor" => one(f64::floor),
        "ceil" => one(f64::ceil),
        "round" => one(f64::round),
        "trunc" => one(f64::trunc),
        "sign" => one(f64::signum),
        "exp" => one(f64::exp),
        "ln" => one(f64::ln),
        "log2" => one(f64::log2),
        "log" if args.len() == 2 => two(f64::log),
        "log" | "log10" => one(f64::log10),
        "sin" => one(f64::sin),
        "cos" => one(f64::cos),
        "tan" => one(f64::tan),
        "asin" => one(f64::asin),
        "acos" => one(f64::acos),
        "atan" => one(f64::atan),
        "sinh" => one(f64::sinh),
        "cosh" => one(f64::cosh),
        "tanh" => one(f64::tanh),
        "rad" => one(f64::to_radians),
        "deg" => one(f64::to_degrees),
        "hypot" => two(f64::hypot),
        "pow" => two(f64::powf),
        "min" => args.iter().copied().reduce(f64::min).ok_or_else(wrong),
        "max" => args.iter().copied().reduce(f64::max).ok_or_else(wrong),
        _ => Err(format!("unknown function {name}")),
    }
}

/// A result as people write it: whole numbers without `.0`, at most 10 decimals, very large or small ones in
/// scientific notation.
pub fn format_number(v: f64) -> String {
    let abs: f64 = v.abs();
    if abs != 0.0 && !(1e-4..1e15).contains(&abs) {
        let text: String = format!("{v:.11e}");
        let (mantissa, exponent): (&str, &str) = text.split_once('e').unwrap_or((&text, "0"));
        return format!("{}e{exponent}", trim_decimals(mantissa));
    }
    let text: String = trim_decimals(&format!("{v:.10}")).to_owned();
    if text == "-0" { "0".into() } else { text }
}

fn trim_decimals(text: &str) -> &str {
    if text.contains('.') { text.trim_end_matches('0').trim_end_matches('.') } else { text }
}

// --- Units --------------------------------------------------------------------------------------------------

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

// --- Number bases -------------------------------------------------------------------------------------------

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

// --- Time ---------------------------------------------------------------------------------------------------

fn now() -> i64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| i64::try_from(d.as_secs()).unwrap_or(0))
}

/// Days since 1970-01-01 of a civil date (proleptic Gregorian), after Howard Hinnant's `days_from_civil`.
fn days_from_civil(year: i64, month: i64, day: i64) -> i64 {
    let y: i64 = if month <= 2 { year - 1 } else { year };
    let era: i64 = y.div_euclid(400);
    let yoe: i64 = y - era * 400;
    let doy: i64 = (153 * (if month > 2 { month - 3 } else { month + 9 }) + 2) / 5 + day - 1;
    let doe: i64 = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

/// The civil date of a day count since 1970-01-01: `(year, month, day)`.
fn civil_from_days(days: i64) -> (i64, i64, i64) {
    let z: i64 = days + 719_468;
    let era: i64 = z.div_euclid(146_097);
    let doe: i64 = z - era * 146_097;
    let yoe: i64 = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy: i64 = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp: i64 = (5 * doy + 2) / 153;
    let day: i64 = doy - (153 * mp + 2) / 5 + 1;
    let month: i64 = if mp < 10 { mp + 3 } else { mp - 9 };
    (yoe + era * 400 + i64::from(month <= 2), month, day)
}

const WEEKDAYS: [&str; 7] = ["Thursday", "Friday", "Saturday", "Sunday", "Monday", "Tuesday", "Wednesday"];

/// `YYYY-MM-DD HH:MM:SS` of unix time `ts` shifted by `offset` seconds.
fn datetime(ts: i64, offset: i64) -> String {
    let local: i64 = ts + offset;
    let (days, secs): (i64, i64) = (local.div_euclid(86_400), local.rem_euclid(86_400));
    let (y, m, d): (i64, i64, i64) = civil_from_days(days);
    format!("{y:04}-{m:02}-{d:02} {:02}:{:02}:{:02}", secs / 3600, secs / 60 % 60, secs % 60)
}

fn zone(offset: i64) -> String {
    let sign: char = if offset < 0 { '-' } else { '+' };
    format!("UTC{sign}{:02}:{:02}", offset.abs() / 3600, offset.abs() / 60 % 60)
}

/// A unix time described: local and UTC dates, weekday.
fn describe(ts: i64) -> String {
    let offset: i64 = term::utc_offset(ts);
    let weekday: &str = WEEKDAYS[usize::try_from(ts.div_euclid(86_400).rem_euclid(7)).unwrap_or(0)];
    format!("{ts} · {} {} · {} UTC · {weekday}", datetime(ts, offset), zone(offset), datetime(ts, 0))
}

/// `2026-10-18`, `2026-10-18 14:30`, `2026-10-18T14:30:05Z` (Z: UTC, else local time) as a unix time; the number
/// of words it used.
fn parse_date(words: &[&str]) -> Option<(i64, usize)> {
    let first: &str = words.first()?;
    let (date, mut clock): (&str, Option<&str>) = match first.split_once('T') {
        Some((date, clock)) => (date, Some(clock)),
        None => (first, None),
    };
    let mut used: usize = 1;
    if clock.is_none()
        && let Some(next) = words.get(1).copied().filter(|w| w.contains(':') && !w.contains('-'))
    {
        (clock, used) = (Some(next), 2);
    }
    let parts: Vec<i64> =
        date.trim_end_matches('Z').split('-').map(str::parse).collect::<Result<Vec<i64>, _>>().ok()?;
    let [y, m, d]: [i64; 3] = parts.try_into().ok()?;
    if !(1..=12).contains(&m) || !(1..=31).contains(&d) {
        return None;
    }
    let utc: bool = clock.is_some_and(|c| c.ends_with('Z')) || date.ends_with('Z');
    let clock: Vec<i64> = match clock.map(|c| c.trim_end_matches('Z')) {
        Some(c) => c.split(':').map(str::parse).collect::<Result<Vec<i64>, _>>().ok()?,
        None => Vec::new(),
    };
    if clock.len() > 3 || clock.iter().any(|n| !(0..60).contains(n)) || clock.first().is_some_and(|h| *h > 23) {
        return None;
    }
    let secs: i64 = clock.iter().chain([&0, &0, &0]).take(3).fold(0, |acc, n| acc * 60 + n);
    let local: i64 = days_from_civil(y, m, d) * 86_400 + secs;
    // The offset in effect at that local time: taken at the UTC guess, which is right outside the hour of a switch.
    Some((if utc { local } else { local - term::utc_offset(local) }, used))
}

/// `now`, a unix time (seconds, or milliseconds when it is that large), a date, or two dates (their difference).
pub fn time(input: &str, now: i64) -> Result<String, String> {
    let text: &str = input.trim();
    if text.is_empty() || text.eq_ignore_ascii_case("now") {
        return Ok(describe(now));
    }
    if let Ok(n) = text.parse::<i64>() {
        return Ok(if n.abs() > 100_000_000_000 {
            format!("{} ms · {}", n, describe(n.div_euclid(1000)))
        } else {
            describe(n)
        });
    }
    let words: Vec<&str> = text
        .split_whitespace()
        .filter(|w| !matches!(w.to_ascii_lowercase().as_str(), "to" | "-" | "à" | "->"))
        .collect();
    let (start, used): (i64, usize) = parse_date(&words).ok_or("a date is YYYY-MM-DD [HH:MM[:SS]]")?;
    if used == words.len() {
        return Ok(describe(start));
    }
    let (end, more): (i64, usize) = parse_date(&words[used..]).ok_or("a date is YYYY-MM-DD [HH:MM[:SS]]")?;
    if used + more != words.len() {
        return Err("at most two dates".into());
    }
    let secs: i64 = (end - start).abs();
    let sign: &str = if end < start { "-" } else { "" };
    Ok(format!("{sign}{} days {} h {} min · {sign}{secs} s", secs / 86_400, secs / 3600 % 24, secs / 60 % 60))
}

// --- Colors -------------------------------------------------------------------------------------------------

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

// --- JSON ---------------------------------------------------------------------------------------------------

/// `text` re-laid out (two-space indent, or `minify`d), token by token, so numbers and strings stay exactly as
/// written. Invalid JSON (and comments, which JSON has not) is refused.
pub fn json(text: &str, minify: bool) -> Result<String, String> {
    json::parse(text).map_err(|e| format!("invalid JSON: {e}"))?;
    let chars: Vec<char> = text.chars().collect();
    let mut out: String = String::with_capacity(text.len());
    let (mut depth, mut in_string, mut escaped, mut empty): (usize, bool, bool, bool) = (0, false, false, false);
    let newline = |out: &mut String, depth: usize| {
        if !minify {
            out.push('\n');
            out.push_str(&"  ".repeat(depth));
        }
    };
    for (i, &c) in chars.iter().enumerate() {
        if in_string {
            out.push(c);
            if escaped {
                escaped = false;
            } else if c == '\\' {
                escaped = true;
            } else if c == '"' {
                in_string = false;
            }
            continue;
        }
        match c {
            '"' => {
                in_string = true;
                out.push(c);
            }
            '/' => return Err("comments are not JSON".into()),
            '{' | '[' => {
                out.push(c);
                depth += 1;
                empty = chars[i + 1..].iter().find(|c| !c.is_whitespace()).is_some_and(|n| matches!(n, '}' | ']'));
                if !empty {
                    newline(&mut out, depth);
                }
            }
            '}' | ']' => {
                depth = depth.saturating_sub(1);
                if !std::mem::take(&mut empty) {
                    newline(&mut out, depth);
                }
                out.push(c);
            }
            ',' => {
                out.push(c);
                newline(&mut out, depth);
            }
            ':' => out.push_str(if minify { ":" } else { ": " }),
            c if c.is_whitespace() => {}
            c => out.push(c),
        }
    }
    Ok(out)
}

// --- Generated text -----------------------------------------------------------------------------------------

/// Random bytes from the system's source (`/dev/urandom`, `RtlGenRandom`); if that fails, `SipHash` over a counter
/// keyed by std's per-process random keys.
fn random_bytes(n: usize) -> Vec<u8> {
    random_bytes_from(n, term::random)
}

/// `random_bytes` with the system source given, so the fallback can be tested.
fn random_bytes_from(n: usize, system: fn(&mut [u8]) -> bool) -> Vec<u8> {
    let mut bytes: Vec<u8> = vec![0; n];
    if !system(&mut bytes) {
        let keys: RandomState = RandomState::new();
        bytes = (0..).map(|i: u64| keys.hash_one(i)).flat_map(u64::to_le_bytes).take(n).collect();
    }
    bytes
}

fn uuid() -> String {
    let mut b: Vec<u8> = random_bytes(16);
    b[6] = (b[6] & 0x0f) | 0x40;
    b[8] = (b[8] & 0x3f) | 0x80;
    let hex: String = b.iter().fold(String::new(), |mut s, x| {
        let _: std::fmt::Result = write!(s, "{x:02x}");
        s
    });
    format!("{}-{}-{}-{}-{}", &hex[..8], &hex[8..12], &hex[12..16], &hex[16..20], &hex[20..])
}

/// 20 chars: letters, digits and symbols, picked without bias (bytes past the last whole set are redrawn).
fn password() -> String {
    const SET: &[u8] = b"ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz23456789!@#$%^&*-_=+?";
    let limit: usize = 256 - 256 % SET.len();
    let mut out: String = String::new();
    while out.len() < 20 {
        for b in random_bytes(32) {
            let b: usize = usize::from(b);
            if b < limit && out.len() < 20 {
                out.push(char::from(SET[b % SET.len()]));
            }
        }
    }
    out
}

const LOREM: &str = "Lorem ipsum dolor sit amet, consectetur adipiscing elit, sed do eiusmod tempor incididunt ut labore et \
dolore magna aliqua. Ut enim ad minim veniam, quis nostrud exercitation ullamco laboris nisi ut aliquip ex ea commodo \
consequat. Duis aute irure dolor in reprehenderit in voluptate velit esse cillum dolore eu fugiat nulla pariatur.";

impl Generated {
    pub fn text(self) -> String {
        let now: i64 = now();
        let local: String = datetime(now, term::utc_offset(now));
        match self {
            Self::Date => local[..10].to_owned(),
            Self::DateTime => local,
            Self::Uuid => uuid(),
            Self::Password => password(),
            Self::Lorem => LOREM.to_owned(),
        }
    }
}

// --- In the editor ------------------------------------------------------------------------------------------

/// A tool window's answer line: `= 42` or `! why`.
fn is_answer(line: &str) -> bool {
    line.starts_with("= ") || line.starts_with("! ")
}

impl Editor {
    /// `<Leader>xc`…: the tool's window, focused (split below the current one the first time), in insert mode on
    /// its last line.
    pub(super) fn open_tool(&mut self, tool: Tool) {
        let existing: Option<usize> =
            self.buffers.iter().position(|b| b.throwaway && b.title.as_deref() == Some(tool.title()));
        let index: usize = existing.unwrap_or_else(|| {
            let mut buf: Buffer = Buffer::scratch();
            buf.title = Some(tool.title().to_owned());
            buf.throwaway = true;
            self.buffers.push(buf);
            self.buffers.len() - 1
        });
        if let Some(window) = self.windows.iter().position(|w| w.buffer == index) {
            self.focus(window);
        } else {
            self.split(false);
            self.switch_to(index);
        }
        self.dashboard = false;
        let buf: &mut Buffer = self.buf_mut();
        let last: usize = buf.last_line();
        buf.cursor = Pos::new(last, buf.line_len(last));
        self.mode = super::Mode::Insert;
        self.info(format!("{}: {} · <CR> answers, on any line", tool.title(), tool.hint()));
    }

    /// The tool whose window holds the current buffer.
    fn current_tool(&self) -> Option<Tool> {
        let buf: &Buffer = self.buf();
        Tool::ALL.into_iter().find(|t| buf.throwaway && buf.title.as_deref() == Some(t.title()))
    }

    /// `<CR>` in a tool window: answers the line under the cursor on the line below it (`= …`, or `! …` for an
    /// error, replacing an older answer there), yanks the answer into `"`, and goes on at an empty last line.
    /// False outside tool windows. On an answer line it answers the question above.
    pub(super) fn tool_enter(&mut self) -> bool {
        let Some(tool): Option<Tool> = self.current_tool() else { return false };
        let mut line: usize = self.buf().cursor.line;
        while line > 0 && is_answer(self.buf().line(line)) {
            line -= 1;
        }
        let input: String = self.buf().line(line).trim().to_owned();
        if is_answer(&input) || (input.is_empty() && tool != Tool::Time) {
            return true;
        }
        let shown: String = match tool.run(&input, self.calc_ans) {
            Ok(answer) => {
                if tool == Tool::Calc {
                    self.calc_ans = calc(&input, self.calc_ans);
                }
                self.store_register(Register { text: answer.clone(), linewise: false, block: false }, true);
                format!("= {answer}")
            }
            Err(why) => format!("! {why}"),
        };
        let buf: &mut Buffer = self.buf_mut();
        buf.seal();
        let next: usize = line + 1;
        if next <= buf.last_line() && is_answer(buf.line(next)) {
            buf.set_line(next, shown);
        } else {
            let end: usize = buf.line_len(line);
            buf.insert(Pos::new(line, end), &format!("\n{shown}"));
        }
        let last: usize = buf.last_line();
        if !buf.line(last).is_empty() {
            let end: usize = buf.line_len(last);
            buf.insert(Pos::new(last, end), "\n");
        }
        buf.cursor = Pos::new(buf.last_line(), 0);
        true
    }

    pub(super) fn open_case_prompt(&mut self) {
        self.open_prompt("Case: snake camel pascal kebab constant title lower upper", "", PromptAction::Case);
    }

    /// `<Leader>xk`: the WORD under the cursor (blank-separated) in the typed case.
    pub(super) fn case_word(&mut self, input: &str) -> Result<String, String> {
        let case: WordCase = WordCase::parse(input).ok_or_else(|| format!("unknown case: {input}"))?;
        let buf: &mut Buffer = self.buf_mut();
        let Pos { line, col }: Pos = buf.cursor;
        let chars: Vec<char> = buf.line(line).chars().collect();
        if chars.get(col).is_none_or(|c| c.is_whitespace()) {
            return Err("no word under the cursor".into());
        }
        let start: usize = chars[..col].iter().rposition(|c| c.is_whitespace()).map_or(0, |i| i + 1);
        let end: usize = chars[col..].iter().position(|c| c.is_whitespace()).map_or(chars.len(), |i| col + i);
        let word: String = chars[start..end].iter().collect();
        let new: String = case.apply(&word);
        if new != word && !new.is_empty() {
            buf.seal();
            buf.remove(Pos::new(line, start), Pos::new(line, end));
            buf.insert(Pos::new(line, start), &new);
            buf.cursor = Pos::new(line, start);
        }
        Ok(new)
    }

    /// `<Leader>xd`, `xi`…: the text goes in at the cursor, as one undo step.
    pub(super) fn insert_generated(&mut self, what: Generated) {
        let text: String = what.text();
        let buf: &mut Buffer = self.buf_mut();
        let at: Pos = buf.cursor;
        buf.seal();
        buf.insert(at, &text);
        buf.cursor = Pos::new(at.line, at.col + text.chars().count().saturating_sub(1));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn eval(expr: &str) -> String {
        calc(expr, Some(10.0)).map_or_else(|| "none".into(), format_number)
    }

    #[test]
    fn calculator() {
        assert_eq!(eval("1 + 2 * 3"), "7");
        assert_eq!(eval("0.1 + 0.2"), "0.3");
        assert_eq!(eval("-2^2"), "-4");
        assert_eq!(eval("2^3^2"), "512");
        assert_eq!(eval("2pi"), eval("2 * pi"));
        assert_eq!(eval("3(4+1)"), "15");
        assert_eq!(eval("2pi(1+1)"), eval("4pi"), "a constant before ( multiplies");
        assert_eq!(eval("ans(2)"), "20");
        assert_eq!(eval("sqrt(16) + abs(-3)"), "7");
        assert_eq!(eval("log(8, 2)"), "3");
        assert_eq!(eval("max(1, 7, 3)"), "7");
        assert_eq!(eval("5!"), "120");
        assert_eq!(eval("200 * 15%"), "30");
        assert_eq!(eval("10 % 3"), "1");
        assert_eq!(eval("0xff + 0b11 + 0o10"), "266");
        assert_eq!(eval("1.5e3"), "1500");
        assert_eq!(eval("ans * 2"), "20");
        assert_eq!(eval("1e20"), "1e20");
        assert_eq!(eval("1/3"), "0.3333333333");
        assert_eq!(eval("10 / 0"), "none");
        assert_eq!(eval("2 +"), "none");
        assert_eq!(eval("foo"), "none");
        assert_eq!(eval(&"(".repeat(200)), "none");
    }

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

    #[test]
    fn bases_and_colors() {
        assert_eq!(bases("255", None).unwrap(), "255 · 0xff · 0o377 · 0b1111_1111 · 'ÿ' U+00FF");
        assert_eq!(bases("0x41", None).unwrap(), "65 · 0x41 · 0o101 · 0b100_0001 · 'A' U+0041");
        assert_eq!(bases("-0b101", None).unwrap(), "-5 · -0x5 · -0o5 · -0b101");
        assert_eq!(bases("2^40", None).unwrap().split(" · ").nth(1), Some("0x10000000000"));
        assert!(bases("1.5", None).is_err());
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

    #[test]
    fn dates() {
        assert_eq!(civil_from_days(0), (1970, 1, 1));
        assert_eq!(days_from_civil(2000, 2, 29), 11_016);
        for days in [-800_000, -1, 0, 59, 11_016, 20_000, 3_000_000] {
            let (y, m, d): (i64, i64, i64) = civil_from_days(days);
            assert_eq!(days_from_civil(y, m, d), days);
        }
        assert_eq!(datetime(1_700_000_000, 0), "2023-11-14 22:13:20");
        assert!(time("1700000000", 0).unwrap().contains("2023-11-14 22:13:20 UTC · Tuesday"));
        assert!(time("1700000000000", 0).unwrap().starts_with("1700000000000 ms · 1700000000"));
        assert!(time("2023-11-14T22:13:20Z", 0).unwrap().starts_with("1700000000 ·"));
        assert_eq!(time("2026-01-01Z to 2026-01-08T12:00Z", 0).unwrap(), "7 days 12 h 0 min · 648000 s");
        assert!(time("2026-13-01", 0).is_err());
        assert_eq!(time("2026-01-01Z 2026-01-02Z 2026-01-03Z", 0).unwrap_err(), "at most two dates");
        assert!(time("2026-01-01 25:00", 0).is_err());
        assert!(time("2026-01-01 1:2:3:4", 0).is_err());
        assert!(time("2026-01-01 x:00", 0).is_err());
        assert!(time("2026-01-02Z 2026-01-01Z", 0).unwrap().starts_with("-1 days"));
        assert!(time("now", 1_700_000_000).unwrap().starts_with("1700000000 ·"));
        assert!(time("", 1_700_000_000).unwrap().starts_with("1700000000 ·"));
        assert!(time("soon", 0).is_err());
    }

    /// Every syntax the calculator reads, and a clear message for every way it can fail.
    #[test]
    fn calculator_reads_everything_and_explains_errors() {
        let ok = |expr: &str| evaluate(expr, Some(10.0)).map(format_number);
        assert_eq!(ok("[1+2]*{3}"), Ok("9".into()), "other brackets");
        assert_eq!(ok("2×3·4∙1 ÷ 4 − 1"), Ok("5".into()), "typographic operators");
        assert_eq!(ok("2**3**2"), Ok("512".into()), "** is a power");
        assert_eq!(ok("√16 + √(9)²"), Ok("13".into()));
        assert_eq!(ok("2³ + π - pi"), Ok("8".into()));
        assert_eq!(ok("sqrt(2"), ok("sqrt(2)"), "open parentheses close at the end");
        assert_eq!(ok("((1+2"), Ok("3".into()));
        assert_eq!(ok("1,5 * 2"), Ok("3".into()), "decimal comma");
        assert_eq!(ok("max(1,5)"), Ok("5".into()), "a comma between arguments");
        assert_eq!(ok("1e5-3"), Ok("99997".into()), "the sign after e only");
        assert_eq!(ok("1E+2 + 2e"), ok("100 + 2e"));
        assert_eq!(ok("(1+1)(2)"), Ok("4".into()));
        assert_eq!(ok("tau/2 - phi + phi"), ok("pi"));
        let err = |expr: &str| evaluate(expr, None).unwrap_err();
        assert_eq!(err(""), "type an expression, e.g. 2+3*4");
        assert_eq!(err("  _ "), "type an expression, e.g. 2+3*4");
        assert_eq!(err("2+3)"), "unmatched ) at column 4");
        assert_eq!(err("2 + $"), "unexpected $ at column 5");
        assert_eq!(err("2 +"), "incomplete: something is missing after +");
        assert_eq!(err("()"), "empty parentheses at column 1");
        assert_eq!(err("foo + 1"), "unknown name foo at column 1");
        assert_eq!(err("foo(1)"), "unknown function foo");
        assert_eq!(err("sqrt(1, 2)"), "wrong number of arguments for sqrt");
        assert_eq!(err("hypot(1)"), "wrong number of arguments for hypot");
        assert_eq!(err("max()"), "unexpected ) at column 5");
        assert_eq!(err("ans"), "no previous answer for ans yet");
        assert_eq!(err("2.5!"), "! needs a whole number from 0 to 170");
        assert_eq!(err("171!"), "! needs a whole number from 0 to 170");
        assert_eq!(err("sqrt(-1)"), "undefined result (like sqrt(-1), log(0) or 0/0)");
        assert_eq!(err("1/0"), "infinite result (a division by zero or an overflow)");
        assert_eq!(err("1.2.3"), "bad number 1.2.3 at column 1");
        assert_eq!(err("0x"), "bad number at column 1");
        assert_eq!(err("0xffffffffffffffffff"), "bad number at column 1");
        assert_eq!(err(&"(".repeat(100)), "too deeply nested");
        assert_eq!(err(&"√".repeat(100)), "too deeply nested");
        assert_eq!(err(&format!("{}1", "sqrt(".repeat(100))), "too deeply nested");
        assert_eq!(err("sqrt(1+)"), "unexpected ) at column 8");
        assert_eq!(ok("(1 000 000"), Ok("1000000".into()), "blanks inside a number are thousands separators");
        for function in [
            "sqrt", "cbrt", "abs", "floor", "ceil", "round", "trunc", "sign", "exp", "ln", "log2", "log", "log10",
            "sin", "cos", "tan", "asin", "acos", "atan", "sinh", "cosh", "tanh", "rad", "deg",
        ] {
            assert!(evaluate(&format!("{function}(0.5)"), None).is_ok(), "{function}");
        }
        assert_eq!(ok("pow(2, 10) + hypot(3, 4) + min(4, 2)"), Ok("1031".into()));
    }

    #[test]
    fn cases_json_and_generated() {
        assert_eq!(words("parseHTTPServer_v2-x"), ["parse", "HTTP", "Server", "v2", "x"]);
        let all: Vec<String> = WordCase::NAMES.iter().map(|(_, c)| c.apply("fooBar baz")).collect();
        assert_eq!(
            all,
            [
                "foo_bar_baz",
                "fooBarBaz",
                "FooBarBaz",
                "foo-bar-baz",
                "FOO_BAR_BAZ",
                "Foo Bar Baz",
                "foo bar baz",
                "FOO BAR BAZ"
            ]
        );
        assert_eq!(WordCase::parse("k"), Some(WordCase::Kebab));
        assert_eq!(WordCase::parse(""), None);
        let text: &str = r#"{"a": [1, 2.50, {}], "b": {"c": "x,{y}"}, "d": []}"#;
        assert_eq!(json(text, true).unwrap(), r#"{"a":[1,2.50,{}],"b":{"c":"x,{y}"},"d":[]}"#);
        assert_eq!(
            json(text, false).unwrap(),
            "{\n  \"a\": [\n    1,\n    2.50,\n    {}\n  ],\n  \"b\": {\n    \"c\": \"x,{y}\"\n  },\n  \"d\": []\n}"
        );
        assert!(json("{\"a\": }", false).is_err());
        assert_eq!(json("{\"a\": 1 // c\n}", false).unwrap_err(), "comments are not JSON");
        assert_eq!(json(r#"["a\"b", "c\\"]"#, true).unwrap(), r#"["a\"b","c\\"]"#, "escapes inside strings");
        assert_eq!(random_bytes_from(20, |_| false).len(), 20, "falls back when the system source fails");
        assert_ne!(random_bytes_from(8, |_| false), random_bytes_from(8, |_| false));
        let id: String = uuid();
        assert_eq!((id.len(), &id[14..15]), (36, "4"));
        assert_ne!(uuid(), id);
        assert_eq!(password().len(), 20);
        assert_eq!(Generated::Date.text().len(), 10);
    }
}
