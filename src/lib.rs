use std::{
    fs::File,
    io::{self, BufRead, Write},
    path::Path,
};

use regex::Regex;
use termcolor::{Color, ColorSpec, StandardStream, WriteColor};

#[derive(Debug)]
pub struct Backtrace {
    frames: Vec<Frame>,
}

#[derive(Debug)]
pub struct Frame {
    function: String,
    frameno: u32,
    source_info: Option<SourceInfo>,
}

#[derive(Debug)]
pub struct SourceInfo {
    file: String,
    lineno: usize,
    colno: usize,
}

impl Backtrace {
    pub fn render(&self, stdout: &mut StandardStream) -> io::Result<()> {
        if self.frames.is_empty() {
            return Ok(());
        }
        let frameno_width = self.frames.len().ilog10() as usize + 1;

        for frame in self.frames.iter().rev() {
            frame.render(stdout, frameno_width)?;
        }
        Ok(())
    }
}

impl Frame {
    fn render(&self, stdout: &mut StandardStream, width: usize) -> io::Result<()> {
        write!(stdout, "{:>width$}: ", self.frameno)?;

        stdout.set_color(ColorSpec::new().set_fg(Some(Color::Green)))?;
        writeln!(stdout, "{}", self.function)?;
        stdout.set_color(&ColorSpec::new())?;

        if let Some(source_info) = &self.source_info {
            source_info.render(stdout, width)?;
        }
        Ok(())
    }
}

impl SourceInfo {
    fn render(&self, stdout: &mut StandardStream, width: usize) -> io::Result<()> {
        write!(stdout, "{:width$}  at ", "")?;
        writeln!(stdout, "{}:{}:{}", self.file, self.lineno, self.colno)?;
        self.render_code(stdout, width)?;
        Ok(())
    }

    fn render_code(&self, stdout: &mut StandardStream, width: usize) -> io::Result<()> {
        let path = Path::new(&self.file);
        if path.exists() {
            let lineno = self.lineno - 1;
            let file = File::open(path)?;
            let reader = io::BufReader::new(file);
            let viewport = reader
                .lines()
                .enumerate()
                .skip(lineno.saturating_sub(2))
                .take(5);
            for (i, line) in viewport {
                if i == lineno {
                    stdout.set_color(ColorSpec::new().set_bold(true))?;
                }
                write!(stdout, "{:width$}    {} | ", "", i + 1)?;
                writeln!(stdout, "{}", line?)?;
                if i == lineno {
                    stdout.set_color(ColorSpec::new().set_bold(false))?;
                }
            }
        }
        Ok(())
    }
}

pub struct Parser {
    re1: Regex,
    re2: Regex,
    lines: Vec<BacktraceLine>,
}

enum BacktraceLine {
    Function { function: String, frameno: u32 },
    Source(SourceInfo),
}

impl Parser {
    pub fn new() -> Parser {
        let re1 = Regex::new(r"^\s+(?P<frameno>\d+):\s+((\w+)\s+-\s+)?(?P<function>.+)").unwrap();
        let re2 = Regex::new(r"^\s+at\s+(?P<file>[^:]+):(?P<lineno>\d+):(?P<colno>\d+)").unwrap();
        Parser {
            re1,
            re2,
            lines: vec![],
        }
    }

    pub fn parse_line(&mut self, line: &str) -> bool {
        if let Some(captures) = self.re1.captures(&line) {
            let frameno = captures.name("frameno").unwrap().as_str().to_string();
            let function = captures.name("function").unwrap().as_str().to_string();
            self.lines.push(BacktraceLine::Function {
                function,
                frameno: frameno.parse().unwrap(),
            });
            true
        } else if let Some(captures) = self.re2.captures(&line) {
            let file = captures.name("file").unwrap().as_str().to_string();
            let lineno = captures.name("lineno").unwrap().as_str();
            let colno = captures.name("colno").unwrap().as_str();
            self.lines.push(BacktraceLine::Source(SourceInfo {
                file,
                lineno: lineno.parse().unwrap(),
                colno: colno.parse().unwrap(),
            }));
            true
        } else {
            false
        }
    }

    pub fn into_backtrace(self) -> Backtrace {
        let mut frames = vec![];
        let mut lines = self.lines.into_iter().peekable();
        while let Some(line) = lines.next() {
            if let BacktraceLine::Function { function, frameno } = line {
                let source_info = lines
                    .next_if(|line| matches!(line, BacktraceLine::Source(..)))
                    .and_then(|line| {
                        if let BacktraceLine::Source(source_info) = line {
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
        }
        Backtrace { frames }
    }
}
