//! `:Name` commands from plugins and the config: the lines go to a program's stdin, and its stdout replaces them
//! or shows as a message. Any language works, since lim only talks to the process.

use std::io::{self, Read, Write};
use std::process::{Child, ChildStderr, ChildStdin, ChildStdout, Command, ExitStatus, Stdio};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use super::vim::LineRange;
use super::{Editor, may_spawn};
use crate::buffer::Buffer;
use crate::config::{CommandOutput, UserCommand};
use crate::error::Error;

/// shortcut: the editor waits for the program up to this long, run it in the background if plugins with long
/// jobs show up.
const TIMEOUT: Duration = Duration::from_secs(10);

impl Editor {
    /// Without a range, the command gets the whole buffer.
    pub(super) fn run_user_command(&mut self, range: &str, name: &str, args: &[String]) -> Result<(), Error> {
        let Some(command): Option<UserCommand> = self.config.commands.iter().find(|c| c.name == name).cloned() else {
            return Err(Error::NotACommand(name.into()));
        };
        let lines: LineRange = if range.is_empty() {
            LineRange { first: 0, last: self.buf().last_line() }
        } else {
            self.parse_range(range).ok_or_else(|| Error::NotACommand(range.into()))?
        };
        let fail = |why: String| Error::UserCommand(name.into(), why);
        if !may_spawn() {
            return Err(fail("cannot run programs here".into()));
        }
        let file: String = self.buf().path.as_ref().map(|p| p.display().to_string()).unwrap_or_default();
        let plugin: String = command.dir.display().to_string();
        let line: String = (self.buf().cursor.line + 1).to_string();
        let words: Vec<String> = command
            .run
            .iter()
            .chain(args)
            .map(|a| a.replace("${file}", &file).replace("${line}", &line).replace("${plugin}", &plugin))
            .collect();
        let input: String = self.buf().lines()[lines.first..=lines.last].join("\n") + "\n";
        let output: String = pipe(&words, &input).map_err(fail)?;
        match command.output {
            CommandOutput::Message if output.trim().is_empty() => self.info(format!("{name}: done")),
            CommandOutput::Message => self.info(output.trim_end().lines().collect::<Vec<&str>>().join(" | ")),
            CommandOutput::Replace => {
                let new: Vec<String> = output.lines().map(str::to_owned).collect();
                // A program that printed nothing most likely failed quietly; never wipe the text for it.
                if new.is_empty() && !input.trim().is_empty() {
                    return Err(fail("no output, text left as is".into()));
                }
                let buf: &mut Buffer = self.buf_mut();
                if buf.lines()[lines.first..=lines.last] != new[..] {
                    buf.seal();
                    buf.splice(lines.first..lines.last + 1, new);
                    buf.clamp(false);
                }
            }
        }
        Ok(())
    }
}

/// Runs `argv` with `input` on stdin. Its stdout with `\r\n` folded, or why it failed (its first stderr line).
pub fn pipe(argv: &[String], input: &str) -> Result<String, String> {
    let Some((program, rest)): Option<(&String, &[String])> = argv.split_first() else {
        return Err("nothing to run".into());
    };
    let mut child: Child = Command::new(program)
        .args(rest)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("{program}: {e}"))?;
    let (Some(mut stdin), Some(stdout), Some(stderr)): (Option<ChildStdin>, Option<ChildStdout>, Option<ChildStderr>) =
        (child.stdin.take(), child.stdout.take(), child.stderr.take())
    else {
        return Err("no pipes".into());
    };
    // One thread per pipe, so a program that writes before it has read all its input cannot deadlock.
    let input: String = input.to_owned();
    thread::spawn(move || {
        // A program that ignores its input closes the pipe early; that is fine.
        let _: io::Result<()> = stdin.write_all(input.as_bytes());
    });
    let out: JoinHandle<Vec<u8>> = thread::spawn(move || read_all(stdout));
    let err: JoinHandle<Vec<u8>> = thread::spawn(move || read_all(stderr));
    let start: Instant = Instant::now();
    let status: ExitStatus = loop {
        if let Some(status) = child.try_wait().map_err(|e| e.to_string())? {
            break status;
        }
        if start.elapsed() > TIMEOUT {
            let _: io::Result<()> = child.kill();
            let _: io::Result<ExitStatus> = child.wait();
            return Err(format!("timed out after {}s", TIMEOUT.as_secs()));
        }
        thread::sleep(Duration::from_millis(5));
    };
    if !status.success() {
        let err: String = String::from_utf8_lossy(&err.join().unwrap_or_default()).into_owned();
        return Err(err
            .lines()
            .map(str::trim)
            .find(|l| !l.is_empty())
            .map_or_else(|| status.to_string(), str::to_owned));
    }
    String::from_utf8(out.join().unwrap_or_default())
        .map(|s| s.replace("\r\n", "\n"))
        .map_err(|_| "the output is not UTF-8".into())
}

fn read_all(mut pipe: impl Read) -> Vec<u8> {
    let mut bytes: Vec<u8> = Vec::new();
    let _: io::Result<usize> = pipe.read_to_end(&mut bytes);
    bytes
}

#[cfg(test)]
mod tests {
    use std::path::{Path, PathBuf};

    use super::*;
    use crate::config::Config;

    #[test]
    fn pipes_through_a_program() {
        let filter: &str = if cfg!(windows) { "findstr" } else { "grep" };
        assert_eq!(pipe(&[filter.into(), "b".into()], "b\na\nxb\n").unwrap(), "b\nxb\n");
        assert!(pipe(&[filter.into(), "b".into()], "a\n").is_err(), "no match exits with 1");
        assert!(pipe(&["lim-no-such-program".into()], "").unwrap_err().starts_with("lim-no-such-program"));
        assert!(pipe(&[], "").is_err());
    }

    /// The shipped example plugins load cleanly, and the text and info ones give the same answers on every system
    /// (coreutils on Unix, `PowerShell` on Windows).
    #[test]
    fn example_plugins_work() {
        let dir: PathBuf = Path::new(env!("CARGO_MANIFEST_DIR")).join("examples").join("plugins");
        let mut config: Config = Config::default();
        let mut warnings: Vec<String> = Vec::new();
        config.read_plugins(&dir, &mut warnings);
        assert_eq!(warnings, Vec::<String>::new());
        assert_eq!(config.commands.len(), 23);
        let cases: [(&str, &str, &str); 9] = [
            ("Sort", "b\na\n", "a\nb\n"),
            ("Uniq", "a\na\nb\n", "a\nb\n"),
            ("Reverse", "a\nb\n", "b\na\n"),
            ("Upper", "ab\n", "AB\n"),
            ("Trim", "a  \n", "a\n"),
            ("B64", "hi\n", "aGkK\n"),
            ("B64d", "aGkK\n", "hi\n"),
            ("Wc", "a b\nc\n", "2 lines, 3 words, 6 chars\n"),
            ("Sha", "hi\n", "98ea6e4f216f2fb4b69fff9b3a44842c38686ca685f3f55dc48c5d3fb1107be4\n"),
        ];
        let run = |name: &str, input: &str| -> String {
            let command: &UserCommand = config.commands.iter().find(|c| c.name == name).unwrap();
            let plugin: String = command.dir.display().to_string();
            let words: Vec<String> = command.run.iter().map(|a| a.replace("${plugin}", &plugin)).collect();
            pipe(&words, input).unwrap_or_else(|e| panic!("{name}: {e}"))
        };
        // PowerShell takes a while to start, so the cases run side by side.
        thread::scope(|s| {
            for (name, input, expected) in cases {
                s.spawn(move || assert_eq!(run(name, input), expected, "{name}"));
            }
            s.spawn(|| assert_eq!(run("Calc", "2*(3+4)/7\n").trim().parse::<f64>().ok(), Some(2.0)));
        });
    }
}
