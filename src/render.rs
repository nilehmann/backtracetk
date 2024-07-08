use std::{
    fmt,
    fs::File,
    io::{self, BufRead},
    path::Path,
};

use anstyle::{AnsiColor, Color, Reset, Style};

use crate::{config::Config, Backtrace, Frame, FrameFilter, PanicInfo, SourceInfo};

const GREEN: Style = Style::new().fg_color(Some(Color::Ansi(AnsiColor::Green)));
const CYAN: Style = Style::new().fg_color(Some(Color::Ansi(AnsiColor::Cyan)));
const RED: Style = Style::new().fg_color(Some(Color::Ansi(AnsiColor::Red)));
const BOLD: Style = Style::new().bold();
const RESET: Reset = Reset;

impl Backtrace {
    pub fn render(&self, config: &Config, filter: &mut impl FrameFilter) {
        let frameno_width = self.compute_frameno_width();
        let lineno_width = self.compute_lineno_width();
        let total_width = self.compute_width(frameno_width);
        let cx = RenderCtxt {
            config,
            frameno_width,
            lineno_width,
            total_width,
        };
        cx.render_backtrace(self, filter)
    }
}

struct RenderCtxt<'a> {
    config: &'a Config,
    frameno_width: usize,
    lineno_width: usize,
    total_width: usize,
}

impl<'a> RenderCtxt<'a> {
    fn render_backtrace(&self, backtrace: &Backtrace, filter: &mut impl FrameFilter) {
        if backtrace.frames.is_empty() {
            return;
        }
        anstream::eprintln!("\n{:━^width$}", " BACKTRACE ", width = self.total_width);

        let mut hidden = 0;
        for frame in backtrace.frames.iter().rev() {
            if filter.should_hide(frame) {
                hidden += 1;
            } else {
                self.print_hidden_frames_message(hidden);
                self.render_frame(frame);
                hidden = 0;
            }
        }
        self.print_hidden_frames_message(hidden);

        if let Some(panic_info) = &backtrace.panic_info {
            self.render_panic_info(panic_info);
        }

        eprintln!();
    }

    fn print_hidden_frames_message(&self, hidden: u32) {
        let msg = match hidden {
            0 => return,
            1 => format!(" ({hidden} frame hidden) "),
            _ => format!(" ({hidden} frames hidden) "),
        };
        anstream::eprintln!("{CYAN}{msg:┄^width$}{RESET}", width = self.total_width);
    }

    fn render_frame(&self, frame: &Frame) {
        anstream::eprintln!(
            "{:>width$}: {GREEN}{}{RESET}",
            frame.frameno,
            frame.function,
            width = self.frameno_width
        );

        if let Some(source_info) = &frame.source_info {
            self.render_source_info(source_info);
            let _ = self.render_code_snippet(source_info);
        }
    }

    fn render_source_info(&self, source_info: &SourceInfo) {
        let text = format!(
            "{}:{}:{}",
            source_info.file, source_info.lineno, source_info.colno
        );
        if self.config.hyperlinks.enabled {
            if let Some(encoded) = encode_file_path_for_url(&source_info.file) {
                let url =
                    self.config
                        .hyperlinks
                        .render(&encoded, source_info.lineno, source_info.colno);
                anstream::eprintln!("{}  at {}", self.frameno_padding(), Link::new(text, url));
                return;
            }
        }
        anstream::eprintln!("{}  at {text}", self.frameno_padding())
    }

    fn render_code_snippet(&self, source_info: &SourceInfo) -> io::Result<()> {
        let path = Path::new(&source_info.file);
        if path.exists() {
            let file = File::open(path)?;
            let reader = io::BufReader::new(file);
            for (i, line) in viewport(reader, source_info)? {
                if i == source_info.lineno {
                    anstream::eprint!("{BOLD}");
                }
                anstream::eprintln!(
                    "{}    {i:>width$} | {line}",
                    self.frameno_padding(),
                    width = self.lineno_width
                );
                if i == source_info.lineno {
                    anstream::eprint!("{RESET}");
                }
            }
        }
        Ok(())
    }

    fn frameno_padding(&self) -> Padding {
        Padding(self.frameno_width)
    }

    fn render_panic_info(&self, panic_info: &PanicInfo) {
        anstream::eprint!("{RED}");
        anstream::eprintln!(
            "thread '{}' panickd at {}",
            panic_info.thread,
            panic_info.at
        );
        for line in &panic_info.message {
            anstream::eprintln!("{line}");
        }
        anstream::eprint!("{RESET}");
    }
}

fn viewport(
    reader: io::BufReader<File>,
    source_info: &SourceInfo,
) -> io::Result<Vec<(usize, String)>> {
    reader
        .lines()
        .enumerate()
        .skip(source_info.lineno.saturating_sub(2))
        .take(5)
        .map(|(i, line)| Ok((i + 1, line?)))
        .collect()
}

impl Backtrace {
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
    /// Width without considering the source code snippet
    fn width(&self, frameno_width: usize) -> usize {
        frameno_width + self.file.len() + (self.lineno.ilog10() + self.colno.ilog10()) as usize + 9
    }
}

struct Padding(usize);

impl std::fmt::Display for Padding {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        for _ in 0..self.0 {
            write!(f, " ")?;
        }
        Ok(())
    }
}

struct Link {
    text: String,
    url: String,
}

impl Link {
    fn new(text: String, url: String) -> Self {
        Self { text, url }
    }
}

impl fmt::Display for Link {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "\u{1b}]8;;{}\u{1b}\\{}\u{1b}]8;;\u{1b}\\",
            self.url, self.text
        )
    }
}

fn encode_file_path_for_url(path: &str) -> Option<String> {
    println!("{path:?}");
    let path = Path::new(path).canonicalize().ok()?;
    println!("{path:?}");
    Some(format!("{}", path.display()))
}
