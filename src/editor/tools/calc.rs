//! The calculator (`<Leader>xc`, `:calc`): an expression reader with functions, constants and `ans`.

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
}
