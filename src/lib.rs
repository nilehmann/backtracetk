pub mod config;
mod partial;
mod render;

use regex::Regex;

pub struct Backtrace {
    pub frames: Vec<Frame>,
    pub panic_info: Option<PanicInfo>,
}

pub struct PanicInfo {
    pub thread: String,
    pub at: String,
    pub message: Vec<String>,
}

pub struct Frame {
    pub function: String,
    pub frameno: u32,
    pub source_info: Option<SourceInfo>,
}

pub struct SourceInfo {
    pub file: String,
    pub lineno: usize,
    pub colno: usize,
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
    /// A line that doesn't match any of the previous patterns
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

pub trait FrameFilter {
    fn should_hide(&mut self, frame: &Frame) -> bool;
}
