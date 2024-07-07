use std::collections::HashMap;
use std::fs;
use std::io::{BufRead, BufReader, Read};
use std::process::{Command, Stdio};

use backtracetk::{Frame, FrameFilter};
use clap::Parser;
use regex::Regex;
use serde::Deserialize;

/// Print colorized Rust backtraces by capturing the output of an external process.
#[derive(clap::Parser)]
#[command(max_term_width = 110, arg_required_else_help = true)]
struct Args {
    #[arg(trailing_var_arg(true))]
    cmd: Vec<String>,

    /// Set the backtrace style to `short` (RUST_BACKTRACE=1) or `full` (RUST_BACKTRACE=full)
    #[arg(long, default_value = "short")]
    style: BacktraceStyle,

    /// Enable or disable `Backtrace::capture`. If this flag is set to `no`, backtracetk sets
    /// RUST_LIB_BACKTRACE=0, disabling `Backtrace::capture`. If the flag is set to `yes`, no
    /// changes are made, and the default behavior of capturing backtraces remains enabled.
    #[arg(long, default_value = "no")]
    lib_backtrace: YesNo,

    /// If this flag is `yes`, set CLICOLOR_FORCE=1. If the flag is `no`, no changes are made.
    #[arg(long, default_value = "yes")]
    clicolor_force: YesNo,

    /// By default, backtracetk prints each captured line as it reads it, providing immediate feedback.
    /// If this flag is set, this output is suppressed, and nothing will be printed until the program
    /// exits.
    #[arg(long)]
    hide_output: bool,
}

#[derive(clap::ValueEnum, Copy, Clone)]
enum BacktraceStyle {
    Short,
    Full,
}

#[derive(clap::ValueEnum, Copy, Clone, Debug)]
enum YesNo {
    Yes,
    No,
}

impl YesNo {
    fn is_yes(&self) -> bool {
        matches!(self, Self::Yes)
    }

    fn is_no(&self) -> bool {
        matches!(self, Self::No)
    }
}

impl BacktraceStyle {
    fn env_var_str(&self) -> &'static str {
        match self {
            BacktraceStyle::Short => "1",
            BacktraceStyle::Full => "full",
        }
    }
}
fn main() -> anyhow::Result<()> {
    let mut args = Args::parse();

    let config = Config::read()?;
    let mut filters = config.to_filters()?;

    let mut env_vars = vec![("RUST_BACKTRACE", args.style.env_var_str())];
    if args.lib_backtrace.is_no() {
        env_vars.push(("RUST_LIB_BACKTRACE", "0"));
    }
    if args.clicolor_force.is_yes() {
        env_vars.push(("CLICOLOR_FORCE", "1"));
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
        if !args.hide_output {
            anstream::eprintln!("{line}");
        }
        parser.parse_line(line);
    }

    for backtrace in parser.into_backtraces() {
        backtrace.render(&mut filters)?;
    }

    Ok(())
}

#[derive(Default, Deserialize)]
struct Config {
    #[serde(default = "Default::default")]
    hide: Vec<HideConfig>,
}

impl Config {
    fn read() -> Result<Config, toml::de::Error> {
        let Some(path) = Config::find_file() else {
            return Ok(Config::default());
        };

        let mut contents = String::new();
        let mut file = fs::File::open(path).unwrap();
        file.read_to_string(&mut contents).unwrap();
        toml::from_str(&contents)
    }

    fn to_filters(&self) -> Result<Filters, regex::Error> {
        let mut filters = vec![];
        for filter in &self.hide {
            filters.push(filter.try_into()?)
        }
        Ok(Filters { filters })
    }

    fn find_file() -> Option<std::path::PathBuf> {
        let mut path = std::env::current_dir().unwrap();
        loop {
            for name in ["backtracetk.toml", ".backtracetk.toml"] {
                let file = path.join(name);
                if file.exists() {
                    return Some(file);
                }
            }
            if !path.pop() {
                return None;
            }
        }
    }
}

enum HideConfig {
    Pattern { pattern: String },
    Span { begin: String, end: Option<String> },
}

pub struct Filters {
    filters: Vec<Filter>,
}

impl FrameFilter for Filters {
    fn should_hide(&mut self, frame: &Frame) -> bool {
        self.filters
            .iter_mut()
            .any(|filter| filter.do_match(&frame.function))
    }
}

enum Filter {
    Pattern(Regex),
    Span {
        begin: Regex,
        end: Option<Regex>,
        inside: bool,
    },
}

impl Filter {
    fn do_match(&mut self, s: &str) -> bool {
        match self {
            Filter::Pattern(regex) => regex.is_match(s),
            Filter::Span {
                begin: start,
                end,
                inside,
            } => {
                if *inside {
                    let Some(end) = end else { return true };
                    *inside = !end.is_match(s);
                    true
                } else {
                    *inside = start.is_match(s);
                    *inside
                }
            }
        }
    }
}

impl TryFrom<&HideConfig> for Filter {
    type Error = regex::Error;

    fn try_from(value: &HideConfig) -> Result<Self, Self::Error> {
        let filter = match value {
            HideConfig::Pattern { pattern } => Filter::Pattern(pattern.as_str().try_into()?),
            HideConfig::Span { begin, end } => Filter::Span {
                begin: begin.as_str().try_into()?,
                end: end.as_deref().map(Regex::try_from).transpose()?,
                inside: false,
            },
        };
        Ok(filter)
    }
}

// Unfortunately we have to implement our own deserializer.
// See https://github.com/toml-rs/toml/issues/748 and https://github.com/toml-rs/toml/issues/535
impl<'de> Deserialize<'de> for HideConfig {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        const PATTERN: &str = "pattern";
        const BEGIN: &str = "begin";
        const END: &str = "end";

        use serde::de::Error;

        struct Visitor;
        impl<'de> serde::de::Visitor<'de> for Visitor {
            type Value = HideConfig;

            fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
                write!(
                    f,
                    "a map with wither the field `{PATTERN}`, or the fields `{BEGIN}` and optionally `{END}`"
                )
            }

            fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error>
            where
                A: serde::de::MapAccess<'de>,
            {
                let mut entries = HashMap::<String, String>::default();
                while let Some((k, v)) = map.next_entry::<String, String>()? {
                    entries.insert(k, v);
                }

                if entries.contains_key(PATTERN) && entries.contains_key(BEGIN) {
                    return Err(Error::custom(format!(
                        "cannot use `{PATTERN}` and `{BEGIN}` toghether"
                    )));
                }
                if let Some(pattern) = entries.remove(PATTERN) {
                    Ok(HideConfig::Pattern { pattern })
                } else if let Some(begin) = entries.remove(BEGIN) {
                    let end = entries.remove(END);
                    Ok(HideConfig::Span { begin, end })
                } else {
                    Err(Error::custom(format!(
                        "missing field `{PATTERN}` or `{BEGIN}`"
                    )))
                }
            }
        }
        deserializer.deserialize_map(Visitor)
    }
}
