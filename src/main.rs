use std::io::{BufRead, BufReader};
use std::process::{Command, Stdio};

use backtracetk::config::{self, Config, Echo};
use backtracetk::{Frame, FrameFilter};
use clap::Parser;
use regex::Regex;

/// Print colorized Rust backtraces by capturing the output of an external process.
#[derive(clap::Parser)]
#[command(max_term_width = 110, arg_required_else_help = true)]
struct Args {
    #[arg(trailing_var_arg(true))]
    cmd: Vec<String>,

    /// Print the current detected configuration
    #[arg(long)]
    print_config: bool,

    /// Print the default configuration used when no configuratoin files are detected
    #[arg(long)]
    print_default_config: bool,
}

fn main() -> anyhow::Result<()> {
    let mut args = Args::parse();

    if args.print_default_config {
        println!("{}", Config::default());
        std::process::exit(0);
    }

    let config = Config::read()?;

    if args.print_config {
        println!("{config}");
        std::process::exit(0);
    }

    let mut env_vars = vec![("RUST_BACKTRACE", config.style.env_var_str())];

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
        if let Echo::True = config.echo {
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
    Range {
        begin: &'a Regex,
        end: Option<&'a Regex>,
        inside: bool,
    },
}

impl Filter<'_> {
    fn do_match(&mut self, s: &str) -> bool {
        match self {
            Filter::Pattern(regex) => regex.is_match(s),
            Filter::Range { begin, end, inside } => {
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
            config::Hide::Range { begin, end } => Filter::Range {
                begin,
                end: end.as_ref(),
                inside: false,
            },
        }
    }
}
