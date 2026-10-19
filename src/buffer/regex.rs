//! Vim regular expressions for `/`, `?`, `*`, `:s` and `:g`, on one line at a time.
//!
//! Magic mode (Vim's default): `.` `*` `[]` `^` `$` are special; `\+` `\=` `\?` `\{n,m}` `\(` `\)` `\|` `\<` `\>`
//! need a backslash. `\v` (very magic) makes `+ = ? { ( ) | < >` special without it. Classes: `\d \w \s \a \l \u`
//! and their negations `\D \W \S \A \L \U`, plus `\t`. `\c` / `\C` force ignoring or matching case.
//! The pattern compiles to a small program run by a backtracking machine with its own stack, so repeated groups
//! never deepen the call stack; runs of single characters repeat in a loop, and a step budget stops runaway patterns.

use std::cell::Cell;
use std::ops::Range;

/// Steps one search may take before giving up (`(a*)*b` on a long line would otherwise never end).
const BUDGET: usize = 1_000_000;
/// Instructions one pattern may compile to: `\{n,m}` on a group copies it, so `\(ab\)\{99999}` is refused.
const MAX_PROGRAM: usize = 100_000;

#[derive(Clone, Debug, PartialEq, Eq)]
enum Node {
    Char(char),
    Any,
    Class {
        ranges: Vec<(char, char)>,
        negated: bool,
    },
    LineStart,
    LineEnd,
    WordStart,
    WordEnd,
    /// `\(…\)` with its capture number (1-based), holding alternatives.
    Group(Vec<Vec<Node>>, usize),
    Repeat {
        node: Box<Node>,
        min: usize,
        max: usize,
    },
}

/// One step of the compiled pattern.
#[derive(Clone, Debug, PartialEq, Eq)]
enum Inst {
    /// One char matching a `Char`, `Any` or `Class` node.
    One(Node),
    /// An anchor: `LineStart`, `LineEnd`, `WordStart` or `WordEnd`.
    Assert(Node),
    /// A greedy run of the one-char node, `min..=max` long, given back one char at a time.
    Run {
        node: Node,
        min: usize,
        max: usize,
    },
    /// Go on at the next instruction; on failure, come back and jump to the target instead.
    Split(usize),
    Jmp(usize),
    /// Record the position in capture slot n: `2 * group` for a group's start, `2 * group + 1` for its end.
    Save(usize),
    /// Record where a round of a repeated group starts, in register n.
    Mark(usize),
    /// Fail unless the round since `Mark(n)` consumed something (a group matching empty would loop forever).
    Moved(usize),
    Match,
}

/// What a failure goes back to: a choice to retry, or a write to undo on the way.
enum Back {
    Branch {
        pc: usize,
        pos: usize,
    },
    /// A `Run` that can give back more chars: resume after it with `count` chars from `start`.
    Fewer {
        pc: usize,
        start: usize,
        count: usize,
        min: usize,
    },
    Slot(usize, Option<usize>),
    Reg(usize, usize),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Regex {
    prog: Vec<Inst>,
    /// Registers the program's `Mark`s use.
    regs: usize,
    groups: usize,
    ignore_case: bool,
    /// Text every match contains (the longest run of plain chars): a line without it is skipped whole.
    required: Option<String>,
    /// The char every match starts with: other start positions are not tried.
    first: Option<char>,
}

/// One match: char ranges of the whole match and of each `\(…\)` group.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Match {
    pub range: Range<usize>,
    pub groups: Vec<Option<Range<usize>>>,
}

struct Parser<'a> {
    chars: &'a [char],
    at: usize,
    very_magic: bool,
    groups: usize,
    case: Option<bool>,
}

impl Regex {
    /// `ignore_case` is the `ignorecase`/`smartcase` verdict; `\c` and `\C` in the pattern override it.
    pub fn new(pattern: &str, ignore_case: bool) -> Result<Self, String> {
        let chars: Vec<char> = pattern.chars().collect();
        let mut parser: Parser<'_> = Parser { chars: &chars, at: 0, very_magic: false, groups: 0, case: None };
        let branches: Vec<Vec<Node>> = parser.alternatives()?;
        if parser.at < chars.len() {
            return Err("E54: Unmatched \\)".into());
        }
        let ignore_case: bool = parser.case.unwrap_or(ignore_case);
        // Shortcuts are exact only for a single branch, and for chars case folding cannot change.
        let plain = |node: &Node| match node {
            Node::Char(c) if !ignore_case || fold(*c) == upper(*c) => Some(*c),
            _ => None,
        };
        let (required, first): (Option<String>, Option<char>) = match &branches[..] {
            [only] => {
                let runs = only.split(|n| plain(n).is_none()).filter(|run| !run.is_empty());
                let longest: Option<&[Node]> = runs.max_by_key(|run| run.len());
                (longest.map(|run| run.iter().filter_map(plain).collect()), only.first().and_then(plain))
            }
            _ => (None, None),
        };
        let mut compiler: Compiler = Compiler { prog: Vec::new(), regs: 0 };
        compiler.alternatives(&branches)?;
        compiler.prog.push(Inst::Match);
        Ok(Self { prog: compiler.prog, regs: compiler.regs, groups: parser.groups, ignore_case, required, first })
    }

    /// Every non-overlapping match in `line`, left to right.
    pub fn find_all(&self, line: &str) -> Vec<Match> {
        if self.required.as_deref().is_some_and(|text| !line.contains(text)) {
            return Vec::new();
        }
        let chars: Vec<char> = line.chars().collect();
        let budget: Cell<usize> = Cell::new(BUDGET);
        let matcher: Matcher<'_> = Matcher { chars: &chars, ignore_case: self.ignore_case, budget: &budget };
        let mut found: Vec<Match> = Vec::new();
        let mut state: State =
            State { slots: vec![None; 2 * (self.groups + 1)], regs: vec![0; self.regs], stack: Vec::new() };
        let mut start: usize = 0;
        while start <= chars.len() {
            if let Some(c) = self.first {
                match chars[start..].iter().position(|&x| x == c) {
                    Some(skip) => start += skip,
                    None => break,
                }
            }
            let Some(m): Option<Match> = self.match_at(&matcher, start, &mut state) else {
                start += 1;
                continue;
            };
            // An empty match (`^`, `x*`) moves on by one char so the scan always advances; as in Vim, one
            // right where the previous match ended is dropped (`:s/.*/X/g` gives one X).
            start = if m.range.is_empty() { m.range.end + 1 } else { m.range.end };
            if !(m.range.is_empty() && found.last().is_some_and(|prev: &Match| prev.range.end == m.range.start)) {
                found.push(m);
            }
        }
        found
    }

    /// The same pattern scanning every position of every line, to check the shortcuts against.
    #[cfg(test)]
    pub fn without_shortcuts(&self) -> Self {
        Self { required: None, first: None, ..self.clone() }
    }

    /// The match starting at `start`, if any; `state` is scratch space reused across positions.
    fn match_at(&self, matcher: &Matcher<'_>, start: usize, state: &mut State) -> Option<Match> {
        state.slots.fill(None);
        state.stack.clear();
        let end: usize = self.run(matcher, start, state)?;
        let groups: Vec<Option<Range<usize>>> =
            (1..=self.groups).map(|g| state.slots[2 * g].zip(state.slots[2 * g + 1]).map(|(a, b)| a..b)).collect();
        Some(Match { range: start..end, groups })
    }

    /// Runs the program from `start`: the end of the first match in priority order (alternatives left to right,
    /// repetitions longest first), or None.
    fn run(&self, matcher: &Matcher<'_>, start: usize, state: &mut State) -> Option<usize> {
        let State { slots, regs, stack }: &mut State = state;
        let (mut pc, mut pos): (usize, usize) = (0, start);
        loop {
            if !matcher.step() {
                return None;
            }
            let ok: bool = match &self.prog[pc] {
                Inst::Match => return Some(pos),
                Inst::One(node) => {
                    let hit: bool = matcher.one(node, pos);
                    (pc, pos) = (pc + 1, pos + 1);
                    hit
                }
                Inst::Assert(node) => {
                    pc += 1;
                    matcher.holds(node, pos)
                }
                Inst::Run { node, min, max } => {
                    let mut count: usize = 0;
                    while count < *max && matcher.one(node, pos + count) {
                        count += 1;
                    }
                    if count > *min {
                        stack.push(Back::Fewer { pc: pc + 1, start: pos, count: count - 1, min: *min });
                    }
                    (pc, pos) = (pc + 1, pos + count);
                    count >= *min
                }
                Inst::Split(alt) => {
                    stack.push(Back::Branch { pc: *alt, pos });
                    pc += 1;
                    true
                }
                Inst::Jmp(to) => {
                    pc = *to;
                    true
                }
                Inst::Save(slot) => {
                    stack.push(Back::Slot(*slot, slots[*slot]));
                    slots[*slot] = Some(pos);
                    pc += 1;
                    true
                }
                Inst::Mark(reg) => {
                    stack.push(Back::Reg(*reg, regs[*reg]));
                    regs[*reg] = pos;
                    pc += 1;
                    true
                }
                Inst::Moved(reg) => {
                    pc += 1;
                    pos > regs[*reg]
                }
            };
            if ok {
                continue;
            }
            // Undo writes back to the latest choice, and take it.
            loop {
                match stack.pop()? {
                    Back::Slot(slot, old) => slots[slot] = old,
                    Back::Reg(reg, old) => regs[reg] = old,
                    Back::Branch { pc: to, pos: at } => {
                        (pc, pos) = (to, at);
                        break;
                    }
                    Back::Fewer { pc: to, start, count, min } => {
                        if count > min {
                            stack.push(Back::Fewer { pc: to, start, count: count - 1, min });
                        }
                        (pc, pos) = (to, start + count);
                        break;
                    }
                }
            }
        }
    }
}

/// The machine's scratch space: capture slots, round registers and the backtrack stack.
struct State {
    slots: Vec<Option<usize>>,
    regs: Vec<usize>,
    stack: Vec<Back>,
}

/// Turns parsed nodes into a program.
struct Compiler {
    prog: Vec<Inst>,
    regs: usize,
}

impl Compiler {
    fn emit(&mut self, inst: Inst) -> Result<usize, String> {
        if self.prog.len() >= MAX_PROGRAM {
            return Err("E363: pattern uses more memory than 'maxmempattern'".into());
        }
        self.prog.push(inst);
        Ok(self.prog.len() - 1)
    }

    /// Points the `Split` or `Jmp` at `at` to the next instruction.
    fn land(&mut self, at: usize) {
        let here: usize = self.prog.len();
        if let Some(Inst::Split(to) | Inst::Jmp(to)) = self.prog.get_mut(at) {
            *to = here;
        }
    }

    /// `a\|b\|c`: each branch is tried in order.
    fn alternatives(&mut self, branches: &[Vec<Node>]) -> Result<(), String> {
        let mut ends: Vec<usize> = Vec::new();
        for (i, branch) in branches.iter().enumerate() {
            let last: bool = i + 1 == branches.len();
            let split: Option<usize> = if last { None } else { Some(self.emit(Inst::Split(0))?) };
            for node in branch {
                self.node(node)?;
            }
            if let Some(split) = split {
                ends.push(self.emit(Inst::Jmp(0))?);
                self.land(split);
            }
        }
        for end in ends {
            self.land(end);
        }
        Ok(())
    }

    fn node(&mut self, node: &Node) -> Result<(), String> {
        match node {
            Node::Char(_) | Node::Any | Node::Class { .. } => self.emit(Inst::One(node.clone())).map(drop),
            Node::LineStart | Node::LineEnd | Node::WordStart | Node::WordEnd => {
                self.emit(Inst::Assert(node.clone())).map(drop)
            }
            Node::Group(branches, index) => {
                self.emit(Inst::Save(2 * index))?;
                self.alternatives(branches)?;
                self.emit(Inst::Save(2 * index + 1)).map(drop)
            }
            Node::Repeat { node, min, max } if is_single(node) => {
                self.emit(Inst::Run { node: (**node).clone(), min: *min, max: *max }).map(drop)
            }
            Node::Repeat { node, min, max } => self.repeat(node, *min, *max),
        }
    }

    /// A repeated group: `min` rounds, then optional ones, each tried before stopping and each consuming something.
    fn repeat(&mut self, node: &Node, min: usize, max: usize) -> Result<(), String> {
        let reg: usize = self.regs;
        self.regs += 1;
        let round = |me: &mut Self| -> Result<(), String> {
            me.emit(Inst::Mark(reg))?;
            me.node(node)?;
            me.emit(Inst::Moved(reg)).map(drop)
        };
        for _ in 0..min {
            round(self)?;
        }
        if max == usize::MAX {
            let top: usize = self.emit(Inst::Split(0))?;
            round(self)?;
            self.emit(Inst::Jmp(top))?;
            self.land(top);
            return Ok(());
        }
        let mut exits: Vec<usize> = Vec::new();
        for _ in min..max {
            exits.push(self.emit(Inst::Split(0))?);
            round(self)?;
        }
        for exit in exits {
            self.land(exit);
        }
        Ok(())
    }
}

impl Parser<'_> {
    fn peek(&self) -> Option<char> {
        self.chars.get(self.at).copied()
    }

    /// Whether the next token is the operator `op`: written `\op` in magic mode, or bare in very magic mode.
    fn at_operator(&self, op: char) -> bool {
        if self.very_magic {
            self.peek() == Some(op)
        } else {
            self.peek() == Some('\\') && self.chars.get(self.at + 1) == Some(&op)
        }
    }

    fn eat_operator(&mut self, op: char) -> bool {
        let found: bool = self.at_operator(op);
        if found {
            self.at += if self.very_magic { 1 } else { 2 };
        }
        found
    }

    fn alternatives(&mut self) -> Result<Vec<Vec<Node>>, String> {
        let mut branches: Vec<Vec<Node>> = vec![self.branch()?];
        while self.eat_operator('|') {
            branches.push(self.branch()?);
        }
        Ok(branches)
    }

    fn branch(&mut self) -> Result<Vec<Node>, String> {
        let mut nodes: Vec<Node> = Vec::new();
        while self.at < self.chars.len() && !self.at_operator('|') && !self.at_operator(')') {
            let Some(atom): Option<Node> = self.atom(nodes.is_empty())? else { continue };
            let atom: Node = self.quantified(atom)?;
            nodes.push(atom);
        }
        Ok(nodes)
    }

    /// `^` and `$` are anchors only at the edges of a branch, as in Vim; elsewhere they are literal.
    fn at_branch_end(&self) -> bool {
        self.at >= self.chars.len() || self.at_operator('|') || self.at_operator(')')
    }

    /// The next atom, or None for a flag that matches nothing (`\v`, `\c`).
    fn atom(&mut self, branch_start: bool) -> Result<Option<Node>, String> {
        let Some(c): Option<char> = self.peek() else { return Ok(None) };
        self.at += 1;
        let node: Node = match c {
            '.' => Node::Any,
            '[' => self.class()?,
            '^' if branch_start => Node::LineStart,
            '$' if self.at_branch_end() => Node::LineEnd,
            '(' if self.very_magic => self.group()?,
            '<' if self.very_magic => Node::WordStart,
            '>' if self.very_magic => Node::WordEnd,
            '*' if branch_start => Node::Char('*'),
            '\\' => {
                let Some(e): Option<char> = self.peek() else { return Ok(Some(Node::Char('\\'))) };
                self.at += 1;
                match e {
                    'v' => {
                        self.very_magic = true;
                        return Ok(None);
                    }
                    'm' | 'M' | 'V' => {
                        self.very_magic = false;
                        return Ok(None);
                    }
                    'c' | 'C' => {
                        self.case = Some(e == 'c');
                        return Ok(None);
                    }
                    '(' if !self.very_magic => self.group()?,
                    '<' if !self.very_magic => Node::WordStart,
                    '>' if !self.very_magic => Node::WordEnd,
                    't' => Node::Char('\t'),
                    'd' | 'D' | 'w' | 'W' | 's' | 'S' | 'a' | 'A' | 'l' | 'L' | 'u' | 'U' => class_escape(e),
                    other => Node::Char(other),
                }
            }
            c => Node::Char(c),
        };
        Ok(Some(node))
    }

    fn group(&mut self) -> Result<Node, String> {
        self.groups += 1;
        let index: usize = self.groups;
        let branches: Vec<Vec<Node>> = self.alternatives()?;
        if !self.eat_operator(')') {
            return Err("E54: Unmatched \\(".into());
        }
        Ok(Node::Group(branches, index))
    }

    /// `[abc]`, `[^a-z]`, `[\d_]`; a `]` right after `[` or `[^` is literal.
    fn class(&mut self) -> Result<Node, String> {
        let negated: bool = self.peek() == Some('^');
        if negated {
            self.at += 1;
        }
        let mut ranges: Vec<(char, char)> = Vec::new();
        let mut first: bool = true;
        loop {
            let Some(c): Option<char> = self.peek() else { return Err("E769: Missing ] after [".into()) };
            self.at += 1;
            if c == ']' && !first {
                break;
            }
            first = false;
            let low: char = if c == '\\' {
                let Some(e): Option<char> = self.peek() else { return Err("E769: Missing ] after [".into()) };
                self.at += 1;
                match class_escape(e) {
                    Node::Class { ranges: more, negated: false } => {
                        ranges.extend(more);
                        continue;
                    }
                    _ if e == 't' => '\t',
                    _ => e,
                }
            } else {
                c
            };
            if self.peek() == Some('-') && self.chars.get(self.at + 1).is_some_and(|&n| n != ']') {
                let high: char = self.chars[self.at + 1];
                self.at += 2;
                ranges.push((low, high));
            } else {
                ranges.push((low, low));
            }
        }
        Ok(Node::Class { ranges, negated })
    }

    fn quantified(&mut self, atom: Node) -> Result<Node, String> {
        let (min, max): (usize, usize) = if self.peek() == Some('*') {
            self.at += 1;
            (0, usize::MAX)
        } else if self.eat_operator('+') {
            (1, usize::MAX)
        } else if self.eat_operator('=') || self.eat_operator('?') {
            (0, 1)
        } else if self.eat_operator('{') {
            self.braces()?
        } else {
            return Ok(atom);
        };
        if matches!(atom, Node::LineStart | Node::LineEnd | Node::WordStart | Node::WordEnd) {
            return Err("E64: nothing to repeat".into());
        }
        Ok(Node::Repeat { node: Box::new(atom), min, max })
    }

    /// The inside of `\{n,m}`: `{n}`, `{n,}`, `{,m}`, `{}` (any count); Vim's closing `\}` or `}`.
    fn braces(&mut self) -> Result<(usize, usize), String> {
        let number = |p: &mut Self| -> Option<usize> {
            let start: usize = p.at;
            while p.peek().is_some_and(|c| c.is_ascii_digit()) {
                p.at += 1;
            }
            p.chars[start..p.at].iter().collect::<String>().parse().ok()
        };
        let min: Option<usize> = number(self);
        let max: Option<usize> = if self.peek() == Some(',') {
            self.at += 1;
            number(self).or(Some(usize::MAX))
        } else {
            min
        };
        if self.peek() == Some('\\') {
            self.at += 1;
        }
        if self.peek() != Some('}') {
            return Err("E554: Syntax error in \\{...}".into());
        }
        self.at += 1;
        Ok((min.unwrap_or(0), max.unwrap_or(usize::MAX)))
    }
}

fn class_escape(e: char) -> Node {
    let ranges: Vec<(char, char)> = match e.to_ascii_lowercase() {
        'd' => vec![('0', '9')],
        'w' => vec![('0', '9'), ('a', 'z'), ('A', 'Z'), ('_', '_')],
        's' => vec![(' ', ' '), ('\t', '\t')],
        'a' => vec![('a', 'z'), ('A', 'Z')],
        'l' => vec![('a', 'z')],
        _ => vec![('A', 'Z')],
    };
    Node::Class { ranges, negated: e.is_ascii_uppercase() }
}

fn is_word(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}

struct Matcher<'a> {
    chars: &'a [char],
    ignore_case: bool,
    /// Steps left for the whole line (shared by every start position).
    budget: &'a Cell<usize>,
}

impl Matcher<'_> {
    /// Takes one step; false once the budget is spent.
    fn step(&self) -> bool {
        let left: usize = self.budget.get();
        self.budget.set(left.saturating_sub(1));
        left > 0
    }

    /// Whether the one-char node `node` matches the char at `pos`.
    fn one(&self, node: &Node, pos: usize) -> bool {
        let Some(&c): Option<&char> = self.chars.get(pos) else { return false };
        match node {
            Node::Char(want) => c == *want || (self.ignore_case && fold(c) == fold(*want)),
            Node::Class { ranges, negated } => {
                let within = |c: char| ranges.iter().any(|&(lo, hi)| (lo..=hi).contains(&c));
                let hit: bool = within(c) || (self.ignore_case && (within(fold(c)) || within(upper(c))));
                hit != *negated
            }
            // `Any`, the only other node a single char is matched against.
            _ => true,
        }
    }

    /// Whether the anchor `node` (`^`, `$`, `\<` or, for anything else, `\>`) holds at `pos`.
    fn holds(&self, node: &Node, pos: usize) -> bool {
        let word_at = |i: usize| self.chars.get(i).is_some_and(|&c| is_word(c));
        let word_before: bool = pos > 0 && word_at(pos - 1);
        match node {
            Node::LineStart => pos == 0,
            Node::LineEnd => pos == self.chars.len(),
            Node::WordStart => !word_before && word_at(pos),
            _ => word_before && !word_at(pos),
        }
    }
}

fn is_single(node: &Node) -> bool {
    matches!(node, Node::Char(_) | Node::Any | Node::Class { .. })
}

fn fold(c: char) -> char {
    c.to_lowercase().next().unwrap_or(c)
}

fn upper(c: char) -> char {
    c.to_uppercase().next().unwrap_or(c)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn spans(pattern: &str, line: &str) -> Vec<Range<usize>> {
        Regex::new(pattern, false).unwrap().find_all(line).into_iter().map(|m| m.range).collect()
    }

    #[test]
    fn magic_syntax() {
        assert_eq!(spans("foo", "a foo foo"), vec![2..5, 6..9]);
        assert_eq!(spans("f.o", "fao fbo"), vec![0..3, 4..7]);
        assert_eq!(spans("ab*c", "ac abc abbbc"), vec![0..2, 3..6, 7..12]);
        assert_eq!(spans("ab\\+c", "ac abc"), vec![3..6]);
        assert_eq!(spans("colou\\=r", "color colour"), vec![0..5, 6..12]);
        assert_eq!(spans("a\\{2,3}", "a aa aaaa"), vec![2..4, 5..8]);
        assert_eq!(spans("^x", "xx"), vec![0..1]);
        assert_eq!(spans("x$", "xx"), vec![1..2]);
        assert_eq!(spans("a^b$c", "a^b$c"), vec![0..5], "anchors only at the edges");
        assert_eq!(spans("[0-9]\\+", "a12b3"), vec![1..3, 4..5]);
        assert_eq!(spans("[^a-z ]", "ab C d"), vec![3..4]);
        assert_eq!(spans("\\d\\+", "x 42 y7"), vec![2..4, 6..7]);
        assert_eq!(spans("\\<is\\>", "this is"), vec![5..7]);
        assert_eq!(spans("cat\\|dog", "dog cat"), vec![0..3, 4..7]);
        assert_eq!(spans("\\(ab\\)\\+", "ababx ab"), vec![0..4, 6..8]);
        assert_eq!(spans("a\\.b", "axb a.b"), vec![4..7]);
        assert_eq!(spans("é.", "é é!"), vec![0..2, 2..4]);
    }

    #[test]
    fn very_magic_case_and_groups() {
        assert_eq!(spans("\\v(ab)+|z{2}", "abab zz z"), vec![0..4, 5..7]);
        assert_eq!(spans("\\v<\\w+>", "hi there"), vec![0..2, 3..8]);
        assert_eq!(spans("\\cFOO", "foo"), vec![0..3]);
        assert_eq!(Regex::new("\\CFoo", true).unwrap().find_all("foo"), Vec::new());
        assert_eq!(Regex::new("[a-c]", true).unwrap().find_all("B").len(), 1);
        let m: Match = Regex::new("\\(\\w\\+\\)=\\(\\d*\\)", false).unwrap().find_all("x key=12").remove(0);
        assert_eq!((m.range, m.groups), (2..8, vec![Some(2..5), Some(6..8)]));
        assert!(Regex::new("\\(a", false).is_err());
        assert!(Regex::new("[a", false).is_err());
        assert!(Regex::new("a\\)", false).is_err());
    }

    #[test]
    fn shortcuts_keep_the_same_matches() {
        let re: Regex = Regex::new("TODO", false).unwrap();
        assert_eq!((re.required.as_deref(), re.first), (Some("TODO"), Some('T')));
        let re: Regex = Regex::new("\\v(\\w+): (\\w+)", false).unwrap();
        assert_eq!((re.required.as_deref(), re.first), (Some(": "), None));
        let re: Regex = Regex::new("ab\\|cd", false).unwrap();
        assert_eq!((re.required.as_deref(), re.first), (None, None), "no shortcut across alternatives");
        let re: Regex = Regex::new("abc", true).unwrap();
        assert_eq!((re.required.as_deref(), re.first), (None, None), "letters fold when ignoring case");
        assert_eq!(spans("x\\d", "a x1 xx x2"), vec![2..4, 8..10]);
        assert_eq!(Regex::new("Ab1", true).unwrap().find_all("aB1 ab1").len(), 2);
    }

    /// Nested repeats on a long line stop at the step budget instead of hanging.
    #[test]
    fn runaway_patterns_end() {
        let line: String = "a".repeat(5000);
        assert_eq!(spans("\\(a*\\)*b", &line), Vec::<Range<usize>>::new());
        assert_eq!(spans(".*", &line), vec![0..5000]);
        // Repeated groups run on the machine's own stack: any number of rounds, even on a small thread stack.
        let small_stack = std::thread::Builder::new().stack_size(128 * 1024).spawn(|| {
            let pairs: String = "ab".repeat(100_000);
            (spans("\\(ab\\)*c", &pairs).is_empty(), spans("\\(ab\\)\\+", &pairs).pop())
        });
        assert_eq!(small_stack.unwrap().join().unwrap(), (true, Some(0..200_000)));
        assert_eq!(spans("\\v(a|ab)(c|bcd)", "abcd"), vec![0..4], "alternatives backtrack in order");
        assert_eq!(spans("\\v(a*)+b", "aab"), vec![0..3], "a group matching empty does not loop");
        assert_eq!(spans("\\v(ab){2,3}", "abababab"), vec![0..6]);
        assert!(Regex::new("\\v(ab){99999}", false).unwrap_err().starts_with("E363"), "too big to compile");
        // The line holds the `b` the pattern requires, so it is searched: exponential, until the budget ends it.
        let line: String = "a".repeat(40) + "cb";
        let start: std::time::Instant = std::time::Instant::now();
        assert_eq!(spans("\\(a\\|a\\)*c\\(a\\|a\\)*\\(a\\|a\\)*d\\|\\(a\\|a\\)*xb", &line), Vec::<Range<usize>>::new());
        assert!(start.elapsed() < std::time::Duration::from_secs(2), "{:?}", start.elapsed());
        assert_eq!(spans("\\v(a)\\m\\(b\\)\\M.\\Vc", "abxc"), vec![0..4], "back to magic, nomagic, very nomagic");
        assert_eq!(spans("\\l\\+", "ABcdE"), vec![2..4]);
    }
}
