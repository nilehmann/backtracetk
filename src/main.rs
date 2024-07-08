use std::io::{BufRead, BufReader};
use std::process::{Command, Stdio};

use backtracetk::config::{self, Config};
use backtracetk::{Frame, FrameFilter};
use clap::Parser;
use regex::Regex;

/// Print colorized Rust backtraces by capturing the output of an external process.
#[derive(clap::Parser)]
#[command(max_term_width = 110, arg_required_else_help = true)]
struct Args {
    #[arg(trailing_var_arg(true))]
    cmd: Vec<String>,

    #[arg(long)]
    style: Option<config::BacktraceStyle>,

    #[arg(long)]
    clicolor_force: Option<config::ColorChoice>,

    /// By default, backtracetk prints each captured line as it reads it, providing immediate feedback.
    /// If this flag is set, this output is suppressed, and nothing will be printed until the program
    /// exits.
    #[arg(long)]
    hide_output: bool,
}

impl Args {
    fn override_config(&self, config: &mut Config) {
        if let Some(style) = self.style {
            config.style = style;
        }
        if let Some(choice) = self.clicolor_force {
            config.clicolor_force = choice;
        }
        if self.hide_output {
            config.hide_output = true;
        }
    }
}

fn main() -> anyhow::Result<()> {
    let mut args = Args::parse();

    let mut config = Config::read()?;
    args.override_config(&mut config);

    let mut env_vars = vec![("RUST_BACKTRACE", config.style.env_var_str())];

    if config.should_set_clicolor_force() {
        env_vars.push(("CLICOLOR_FORCE", "1"));
    }

    for (k, v) in &config.env {
        env_vars.push((k, v));
    }

    println!("$ {}", args.cmd.join(" "));

    let child = match Command::new(args.cmd.remove(0))
        .args(args.cmd)
        .stderr(Stdio::piped())
        .envs(env_vars)
        .spawn()
    {
        Ok(child) => child,
        Err(err) => {
            eprintln!("Error: command exited with non-zero code: `{err}`");
            std::process::exit(2);
        }
    };

    let mut parser = backtracetk::Parser::new();
    let stderr = child.stderr.expect("failed to open stderr");
    for line in BufReader::new(stderr).lines() {
        let line = line?;
        if !config.hide_output {
            anstream::eprintln!("{line}");
        }
        parser.parse_line(line);
    }

    for backtrace in parser.into_backtraces() {
        backtrace.render(&config, &mut Filters::new(&config));
    }

    Ok(())
}

pub struct Filters<'a> {
    filters: Vec<Filter<'a>>,
}

impl<'a> Filters<'a> {
    fn new(config: &'a Config) -> Self {
        let mut filters = vec![];
        for filter in &config.hide {
            filters.push(filter.into())
        }
        Self { filters }
    }
}

impl FrameFilter for Filters<'_> {
    fn should_hide(&mut self, frame: &Frame) -> bool {
        self.filters
            .iter_mut()
            .any(|filter| filter.do_match(&frame.function))
    }
}

enum Filter<'a> {
    Pattern(&'a Regex),
    Span {
        begin: &'a Regex,
        end: Option<&'a Regex>,
        inside: bool,
    },
}

impl Filter<'_> {
    fn do_match(&mut self, s: &str) -> bool {
        match self {
            Filter::Pattern(regex) => regex.is_match(s),
            Filter::Span { begin, end, inside } => {
                if *inside {
                    let Some(end) = end else { return true };
                    *inside = !end.is_match(s);
                    true
                } else {
                    *inside = begin.is_match(s);
                    *inside
                }
            }
        }
    }
}

impl<'a> From<&'a config::Hide> for Filter<'a> {
    fn from(value: &'a config::Hide) -> Self {
        match value {
            config::Hide::Pattern { pattern } => Filter::Pattern(pattern),
            config::Hide::Span { begin, end } => Filter::Span {
                begin,
                end: end.as_ref(),
                inside: false,
            },
        }
    }
}
