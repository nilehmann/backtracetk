use std::{
    fs::File,
    io::{self, BufRead},
    path::Path,
};

use crate::{Backtrace, Frame, FrameFilter, PanicInfo, SourceInfo};

const GREEN: anstyle::Style =
    anstyle::Style::new().fg_color(Some(anstyle::Color::Ansi(anstyle::AnsiColor::Green)));
const CYAN: anstyle::Style =
    anstyle::Style::new().fg_color(Some(anstyle::Color::Ansi(anstyle::AnsiColor::Cyan)));
const RED: anstyle::Style =
    anstyle::Style::new().fg_color(Some(anstyle::Color::Ansi(anstyle::AnsiColor::Red)));
const BOLD: anstyle::Style = anstyle::Style::new().bold();
const RESET: anstyle::Reset = anstyle::Reset;

impl Backtrace {
    pub fn render(&self, filter: &mut impl FrameFilter) -> io::Result<()> {
        if self.frames.is_empty() {
            return Ok(());
        }
        let framnow = self.compute_frameno_width();
        let linenow = self.compute_lineno_width();
        let width = self.compute_width(framnow);
        anstream::eprintln!("\n{:━^width$}", " BACKTRACE ");

        let mut hidden = 0;
        for frame in self.frames.iter().rev() {
            if filter.should_hide(frame) {
                hidden += 1;
            } else {
                print_hidden_frames_message(hidden, width)?;
                frame.render(framnow, linenow)?;
                hidden = 0;
            }
        }
        print_hidden_frames_message(hidden, width)?;

        if let Some(panic_info) = &self.panic_info {
            panic_info.render()?;
        }

        eprintln!();
        Ok(())
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
    fn render(&self, framenow: usize, linenow: usize) -> io::Result<()> {
        anstream::eprintln!(
            "{:>framenow$}: {GREEN}{}{RESET}",
            self.frameno,
            self.function
        );

        if let Some(source_info) = &self.source_info {
            let padding = Padding(framenow);
            anstream::eprintln!("{padding}  at {source_info}");
            source_info.render_code(padding, linenow)?;
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

fn print_hidden_frames_message(hidden: u32, width: usize) -> io::Result<()> {
    let msg = match hidden {
        0 => return Ok(()),
        1 => format!(" ({hidden} frame hidden) "),
        _ => format!(" ({hidden} frames hidden) "),
    };
    anstream::eprintln!("{CYAN}{msg:┄^width$}{RESET}");
    Ok(())
}

impl SourceInfo {
    fn render_code(&self, padding: Padding, linenow: usize) -> io::Result<()> {
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
                    anstream::eprint!("{BOLD}");
                }
                anstream::eprintln!("{padding}    {:>linenow$} | {}", i + 1, line?);
                if i == lineno {
                    anstream::eprint!("{RESET}");
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

impl PanicInfo {
    fn render(&self) -> io::Result<()> {
        anstream::eprint!("{RED}");
        anstream::eprintln!("thread '{}' panickd at {}", self.thread, self.at);
        for line in &self.message {
            anstream::eprintln!("{line}");
        }
        anstream::eprint!("{RESET}");
        Ok(())
    }
}

impl std::fmt::Display for SourceInfo {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}:{}:{}", self.file, self.lineno, self.colno)
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
