//! `:Name` commands from the config: the lines go to a program's stdin, and its stdout replaces them
//! or shows as a message. Any language works, since lim only talks to the process. A command typed by hand runs in
//! the background while editing goes on; formatters on save run before the write.

use std::io::{self, Read, Write};
use std::path::Path;
use std::process::{Child, ChildStderr, ChildStdin, ChildStdout, Command, ExitStatus, Stdio};
use std::sync::mpsc::{self, Receiver, Sender, TryRecvError};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use super::vim::LineRange;
use super::{Editor, may_spawn};
use crate::buffer::Buffer;
use crate::config::{CommandOutput, UserCommand};
use crate::error::Error;

/// How long a save waits for a formatter, and a background command may run.
const SAVE_TIMEOUT: Duration = Duration::from_secs(10);
const JOB_TIMEOUT: Duration = Duration::from_secs(600);

/// What a program printed, or why it failed.
type Outcome = Result<String, String>;

/// Runs a program with its input and a time limit: `pipe_for`, or a stand-in in tests.
pub type Runner = fn(&[String], &str, Duration) -> Outcome;

/// A `:Name` command running in the background, applied when its output arrives.
pub struct Job {
    name: String,
    output: CommandOutput,
    lines: LineRange,
    /// The buffer's revision when the command started: replacing waits for text that has not changed since.
    revision: u64,
    result: Receiver<Outcome>,
}

impl Editor {
    /// Runs the `"on": "save"` commands that match `path` on the whole buffer, in load order; the first failure
    /// stops them and is returned.
    pub(super) fn run_on_save(&mut self, path: &Path) -> Option<Error> {
        let names: Vec<String> =
            self.config.commands.iter().filter(|c| c.runs_on_save(path)).map(|c| c.name.clone()).collect();
        names.iter().find_map(|name| self.run_user_command("", name, &[], false).err())
    }

    /// Without a range, the command gets the whole buffer. In the `background`, `poll_jobs` applies its result.
    pub(super) fn run_user_command(
        &mut self,
        range: &str,
        name: &str,
        args: &[String],
        background: bool,
    ) -> Result<(), Error> {
        let Some(command): Option<UserCommand> = self.config.commands.iter().find(|c| c.name == name).cloned() else {
            return Err(Error::NotACommand(name.into()));
        };
        let lines: LineRange = if range.is_empty() {
            LineRange { first: 0, last: self.buf().last_line() }
        } else {
            self.parse_range(range).ok_or_else(|| Error::NotACommand(range.into()))?
        };
        let fail = |why: String| Error::UserCommand(name.into(), why);
        let (runner, real): (Runner, bool) = self.runner.map_or((pipe_for, true), |fake| (fake, false));
        if real && !may_spawn() {
            return Err(fail("cannot run programs here".into()));
        }
        let file: String = self.buf().path.as_ref().map(|p| p.display().to_string()).unwrap_or_default();
        let line: String = (self.buf().cursor.line + 1).to_string();
        let words: Vec<String> =
            command.run.iter().chain(args).map(|a| a.replace("${file}", &file).replace("${line}", &line)).collect();
        let input: String = self.buf().lines()[lines.first..=lines.last].join("\n") + "\n";
        if background {
            let (send, result): (Sender<Outcome>, Receiver<Outcome>) = mpsc::channel();
            thread::spawn(move || send.send(runner(&words, &input, JOB_TIMEOUT)));
            self.start_job(name, command.output, lines, result);
            return Ok(());
        }
        let output: String = runner(&words, &input, SAVE_TIMEOUT).map_err(fail)?;
        self.apply_output(name, command.output, lines, &input, &output)
    }

    /// `:upgrade` / `<Leader>pu`: checks for a newer release and rebuilds lim in the background, as a job.
    pub(super) fn upgrade(&mut self) {
        if !may_spawn() {
            return self.error(&Error::UserCommand("upgrade".into(), "cannot run programs here".into()));
        }
        let (send, result): (Sender<Outcome>, Receiver<Outcome>) = mpsc::channel();
        thread::spawn(move || send.send(crate::update::run()));
        self.start_job("upgrade", CommandOutput::Message, LineRange { first: 0, last: 0 }, result);
    }

    /// Tracks a background command on the current buffer until `result` arrives.
    pub(super) fn start_job(&mut self, name: &str, output: CommandOutput, lines: LineRange, result: Receiver<Outcome>) {
        let revision: u64 = self.buf().revision();
        self.jobs.push(Job { name: name.into(), output, lines, revision, result });
        self.info(format!("{name}: running"));
    }

    /// Applies the finished background commands; called on every key and timer tick.
    pub(super) fn poll_jobs(&mut self) {
        let mut i: usize = 0;
        while i < self.jobs.len() {
            let result: Result<String, String> = match self.jobs[i].result.try_recv() {
                Ok(result) => result,
                Err(TryRecvError::Empty) => {
                    i += 1;
                    continue;
                }
                Err(TryRecvError::Disconnected) => Err("stopped".into()),
            };
            let job: Job = self.jobs.remove(i);
            if let Err(e) = self.finish_job(&job, result) {
                self.error(&e);
            }
        }
    }

    fn finish_job(&mut self, job: &Job, result: Result<String, String>) -> Result<(), Error> {
        let fail = |why: String| Error::UserCommand(job.name.clone(), why);
        let output: String = result.map_err(fail)?;
        if job.output == CommandOutput::Message {
            return self.apply_output(&job.name, job.output, job.lines, "", &output);
        }
        let Some(index): Option<usize> = self.buffers.iter().position(|b| b.revision() == job.revision) else {
            return Err(fail("the text changed while it ran, output dropped".into()));
        };
        let current: usize = self.current;
        self.current = index;
        let input: String = self.buf().lines()[job.lines.first..=job.lines.last].join("\n") + "\n";
        let applied: Result<(), Error> = self.apply_output(&job.name, job.output, job.lines, &input, &output);
        self.current = current;
        applied
    }

    /// Shows `output` as a message, or puts it in place of `lines` (whose text was `input`).
    fn apply_output(
        &mut self,
        name: &str,
        kind: CommandOutput,
        lines: LineRange,
        input: &str,
        output: &str,
    ) -> Result<(), Error> {
        let fail = |why: String| Error::UserCommand(name.into(), why);
        match kind {
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
    pipe_for(argv, input, SAVE_TIMEOUT)
}

/// `pipe`, killing the program after `timeout`.
fn pipe_for(argv: &[String], input: &str, timeout: Duration) -> Result<String, String> {
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
    let pipes: Option<((ChildStdin, ChildStdout), ChildStderr)> =
        child.stdin.take().zip(child.stdout.take()).zip(child.stderr.take());
    let Some(((mut stdin, stdout), stderr)) = pipes else { return Err("no pipes".into()) };
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
        if start.elapsed() > timeout {
            let _: io::Result<()> = child.kill();
            let _: io::Result<ExitStatus> = child.wait();
            return Err(format!("timed out after {}s", timeout.as_secs()));
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
    use super::*;

    #[test]
    fn pipes_through_a_program() {
        let filter: &str = if cfg!(windows) { "findstr" } else { "grep" };
        assert_eq!(pipe(&[filter.into(), "b".into()], "b\na\nxb\n").unwrap(), "b\nxb\n");
        assert!(pipe(&[filter.into(), "b".into()], "a\n").is_err(), "no match exits with 1");
        assert!(pipe(&["lim-no-such-program".into()], "").unwrap_err().starts_with("lim-no-such-program"));
        assert!(pipe(&[], "").is_err());
        let wait: Vec<String> = if cfg!(windows) {
            ["ping", "-n", "4", "127.0.0.1"].map(String::from).into()
        } else {
            ["sleep", "3"].map(String::from).into()
        };
        assert_eq!(pipe_for(&wait, "", Duration::from_millis(100)).unwrap_err(), "timed out after 0s");
    }
}
