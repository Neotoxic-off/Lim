//! `<Leader>x` tools: a calculator, a unit converter, number bases, timestamps, colors, word case, JSON layout and
//! generated text (dates, UUIDs, passwords, lorem ipsum). Each tool is a pure function of the typed text, in its own
//! module; this one shows them in a popup, writes each answer under its question and yanks it into `"`.

mod bases;
mod calc;
mod case;
mod color;
mod generated;
mod json;
mod time;
mod units;

pub use calc::{calc, evaluate, format_number};
pub use case::WordCase;
pub use generated::Generated;
pub use json::json;

use bases::bases;
use color::color;
use time::{now, time};
use units::convert;

use super::Editor;
use super::prompt::PromptAction;
use super::registers::Register;
use crate::buffer::{Buffer, Pos};

/// The tools that answer a line of input, shown in a popup over a notebook buffer that keeps every answer.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tool {
    Calc,
    Convert,
    Base,
    Time,
    Color,
}

impl Tool {
    pub const ALL: [Self; 5] = [Self::Calc, Self::Convert, Self::Base, Self::Time, Self::Color];

    /// The popup's buffer name.
    pub fn title(self) -> &'static str {
        match self {
            Self::Calc => "[Calculator]",
            Self::Convert => "[Convert]",
            Self::Base => "[Bases]",
            Self::Time => "[Time]",
            Self::Color => "[Color]",
        }
    }

    /// What to type, shown in the popup's bottom border.
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

/// A tool window's answer line: `= 42` or `! why`.
fn is_answer(line: &str) -> bool {
    line.starts_with("= ") || line.starts_with("! ")
}

impl Editor {
    /// `<Leader>xc`…: the tool's popup over the windows (one popup, shared by the tools), focused, in insert mode
    /// on its last line.
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
        } else if let Some(popup) = self.windows.iter().position(|w| w.floating) {
            self.focus(popup);
            self.switch_to(index);
        } else {
            let mut popup: super::Window =
                super::Window::new(self.next_window_id, index, super::View::of(&self.buffers[index]));
            popup.floating = true;
            self.next_window_id += 1;
            self.windows.push(popup);
            self.focus(self.windows.len() - 1);
        }
        self.dashboard = false;
        let buf: &mut Buffer = self.buf_mut();
        let last: usize = buf.last_line();
        buf.cursor = Pos::new(last, buf.line_len(last));
        self.mode = super::Mode::Insert;
    }

    /// The help a tool's popup shows in its bottom border, when `buffer` is a tool's.
    pub fn popup_hint(&self, buffer: usize) -> Option<String> {
        let buf: &Buffer = self.buffers.get(buffer)?;
        let tool: Tool = Tool::ALL.into_iter().find(|t| buf.throwaway && buf.title.as_deref() == Some(t.title()))?;
        Some(format!("{} · <CR> answers", tool.hint()))
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
