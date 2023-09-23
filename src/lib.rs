use std::{
    fs::File,
    io::{self, BufRead, Write},
    path::Path,
};

use regex::Regex;
use termcolor::{Color, ColorSpec, StandardStream, WriteColor};

pub struct Backtrace {
    frames: Vec<Frame>,
    panic_info: Option<PanicInfo>,
}

struct PanicInfo {
    thread: String,
    at: String,
    message: Vec<String>,
}

pub struct Frame {
    function: String,
    frameno: u32,
    source_info: Option<SourceInfo>,
}

pub struct SourceInfo {
    file: String,
    lineno: usize,
    colno: usize,
}

impl Backtrace {
    pub fn render(&self, out: &mut StandardStream) -> io::Result<()> {
        if self.frames.is_empty() {
            return Ok(());
        }
        let framnow = self.compute_frameno_width();
        let linenow = self.compute_lineno_width();
        let width = self.compute_width(framnow);
        writeln!(out, "\n{:━^width$}", " BACKTRACE ")?;

        for frame in self.frames.iter().rev() {
            frame.render(out, framnow, linenow)?;
        }

        if let Some(panic_info) = &self.panic_info {
            panic_info.render(out)?;
        }

        writeln!(out)
    }

    fn compute_lineno_width(&self) -> usize {
        // This is assuming we have 2 more lines in the file, if we don't, in the worst case we will
        // print an unnecesary extra space for each line number.
        self.frames
            .iter()
            .flat_map(|f| &f.source_info)
            .map(|source_info| source_info.lineno + 3)
            .max()
            .unwrap_or(1)
            .ilog10() as usize
    }

    fn compute_frameno_width(&self) -> usize {
        self.frames.len().ilog10() as usize + 1
    }

    fn compute_width(&self, frameno_width: usize) -> usize {
        let term_size = termion::terminal_size().unwrap_or((80, 0)).0 as usize;
        self.frames
            .iter()
            .map(|f| f.width(frameno_width))
            .max()
            .unwrap_or(80)
            .min(term_size)
    }
}

impl Frame {
    fn render(&self, out: &mut StandardStream, framenow: usize, linenow: usize) -> io::Result<()> {
        write!(out, "{:>framenow$}: ", self.frameno)?;

        out.set_color(ColorSpec::new().set_fg(Some(Color::Green)))?;
        writeln!(out, "{}", self.function)?;
        out.set_color(&ColorSpec::new())?;

        if let Some(source_info) = &self.source_info {
            source_info.render(out, framenow, linenow)?;
        }
        Ok(())
    }

    fn width(&self, frameno_width: usize) -> usize {
        usize::max(
            frameno_width + 2 + self.function.len(),
            self.source_info
                .as_ref()
                .map(|s| s.width(frameno_width))
                .unwrap_or(0),
        )
    }
}

impl SourceInfo {
    fn render(&self, out: &mut StandardStream, framenow: usize, linenow: usize) -> io::Result<()> {
        write!(out, "{:framenow$}  at ", "")?;
        writeln!(out, "{self}")?;
        self.render_code(out, framenow, linenow)?;
        Ok(())
    }

    fn render_code(
        &self,
        out: &mut StandardStream,
        framenow: usize,
        linenow: usize,
    ) -> io::Result<()> {
        let path = Path::new(&self.file);
        if path.exists() {
            let lineno = self.lineno - 1;
            let file = File::open(path)?;
            let reader = io::BufReader::new(file);
            let viewport: Vec<_> = reader
                .lines()
                .enumerate()
                .skip(lineno.saturating_sub(2))
                .take(5)
                .collect();

            for (i, line) in viewport {
                if i == lineno {
                    out.set_color(ColorSpec::new().set_bold(true))?;
                }
                write!(out, "{:framenow$}    {:>linenow$} | ", "", i + 1)?;
                writeln!(out, "{}", line?)?;
                if i == lineno {
                    out.set_color(ColorSpec::new().set_bold(false))?;
                }
            }
        }
        Ok(())
    }

    /// Width without considering the source code snippet
    fn width(&self, framenow: usize) -> usize {
        framenow + self.file.len() + (self.lineno.ilog10() + self.colno.ilog10()) as usize + 9
    }
}

impl std::fmt::Display for SourceInfo {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}:{}:{}", self.file, self.lineno, self.colno)
    }
}

impl PanicInfo {
    fn render(&self, out: &mut StandardStream) -> io::Result<()> {
        out.set_color(ColorSpec::new().set_fg(Some(Color::Red)))?;
        writeln!(out, "thread '{}' panicked at {}", self.thread, self.at)?;
        for line in &self.message {
            writeln!(out, "{}", line)?;
        }
        out.set_color(&ColorSpec::new())?;
        Ok(())
    }
}

pub struct Parser {
    panic_regex: Regex,
    function_regex: Regex,
    source_regex: Regex,
    lines: Vec<ParsedLine>,
}

enum ParsedLine {
    /// A line reporting a panic, e.g.,
    /// ```ignore
    /// thread 'rustc' panicked at /rustc/b3aa8e7168a3d940122db3561289ffbf3f587262/compiler/rustc_errors/src/lib.rs:1651:9:
    /// ```
    ThreadPanic { thread: String, at: String },
    /// The begining of a trace starts with `stack backtrace:`
    BacktraceStart,
    /// The "header" of a frame containing the frame number and the function's name, e.g.,
    /// ```ignore
    ///   28: rustc_middle::ty::context::tls::enter_context`
    /// ```
    BacktraceHeader { function: String, frameno: u32 },
    /// Line containing source information about a frame, e.g.,
    /// ```ignore
    ///              at /rustc/b3aa8e7168a3d940122db3561289ffbf3f587262/compiler/rustc_middle/src/ty/context/tls.rs:79:9
    /// ```
    BacktraceSource(SourceInfo),
    /// A line that doesn't match any pattern
    Other(String),
}

impl Parser {
    pub fn new() -> Parser {
        let panic_regex =
            Regex::new(r"^thread\s+'(?P<thread>[^']+)'\spanicked\s+at\s+(?P<at>.+)").unwrap();
        let function_regex =
            Regex::new(r"^\s+(?P<frameno>\d+):\s+((\w+)\s+-\s+)?(?P<function>.+)").unwrap();
        let source_regex =
            Regex::new(r"^\s+at\s+(?P<file>[^:]+):(?P<lineno>\d+):(?P<colno>\d+)").unwrap();
        Parser {
            panic_regex,
            function_regex,
            source_regex,
            lines: vec![],
        }
    }

    pub fn parse_line(&mut self, line: String) {
        let parsed = if line.eq_ignore_ascii_case("stack backtrace:") {
            ParsedLine::BacktraceStart
        } else if let Some(captures) = self.panic_regex.captures(&line) {
            let thread = captures.name("thread").unwrap().as_str().to_string();
            let at = captures.name("at").unwrap().as_str().to_string();
            ParsedLine::ThreadPanic { thread, at }
        } else if let Some(captures) = self.function_regex.captures(&line) {
            let frameno = captures.name("frameno").unwrap().as_str().to_string();
            let function = captures.name("function").unwrap().as_str().to_string();
            ParsedLine::BacktraceHeader {
                function,
                frameno: frameno.parse().unwrap(),
            }
        } else if let Some(captures) = self.source_regex.captures(&line) {
            let file = captures.name("file").unwrap().as_str().to_string();
            let lineno = captures.name("lineno").unwrap().as_str();
            let colno = captures.name("colno").unwrap().as_str();
            ParsedLine::BacktraceSource(SourceInfo {
                file,
                lineno: lineno.parse().unwrap(),
                colno: colno.parse().unwrap(),
            })
        } else {
            ParsedLine::Other(line)
        };
        self.lines.push(parsed)
    }

    pub fn into_backtraces(self) -> Vec<Backtrace> {
        let mut backtraces = vec![];
        let mut frames = vec![];
        let mut lines = self.lines.into_iter().peekable();
        let mut panic_info = None;
        let mut in_panic_info = false;
        while let Some(line) = lines.next() {
            match line {
                ParsedLine::ThreadPanic { thread, at } => {
                    in_panic_info = true;
                    panic_info = Some(PanicInfo {
                        thread,
                        at,
                        message: vec![],
                    });
                }
                ParsedLine::Other(line) => {
                    if let Some(panic_info) = &mut panic_info {
                        if in_panic_info {
                            panic_info.message.push(line);
                        }
                    }
                }
                ParsedLine::BacktraceStart => {
                    in_panic_info = false;
                    if !frames.is_empty() {
                        backtraces.push(Backtrace {
                            frames: std::mem::take(&mut frames),
                            panic_info: std::mem::take(&mut panic_info),
                        });
                    }
                }
                ParsedLine::BacktraceHeader { function, frameno } => {
                    in_panic_info = false;
                    let source_info = lines
                        .next_if(|line| matches!(line, ParsedLine::BacktraceSource(..)))
                        .and_then(|line| {
                            if let ParsedLine::BacktraceSource(source_info) = line {
                                Some(source_info)
                            } else {
                                None
                            }
                        });
                    frames.push(Frame {
                        function,
                        frameno,
                        source_info,
                    })
                }
                ParsedLine::BacktraceSource(..) => {
                    // This case is in theory never reached because source lines should be consumed
                    // in the `BacktraceHeader` case.
                    in_panic_info = false;
                }
            }
        }
        if !frames.is_empty() {
            backtraces.push(Backtrace { frames, panic_info });
        }
        backtraces
    }
}
