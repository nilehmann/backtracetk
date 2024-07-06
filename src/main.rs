use std::fs;
use std::io::{self, BufRead, BufReader, Read};
use std::process::{Command, Stdio};

use backtracetk::Frame;
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
fn main() -> io::Result<()> {
    let mut args = Args::parse();

    let config = read_config();

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

    let mut filter = |frame: &Frame| {
        for regex in &config.hide {
            if regex.is_match(&frame.function) {
                return true;
            }
        }
        false
    };
    for backtrace in parser.into_backtraces() {
        backtrace.render(&mut filter)?;
    }

    Ok(())
}

#[derive(Default, Deserialize)]
struct Config {
    #[serde(deserialize_with = "deserialize_regex_vec")]
    #[serde(default = "Default::default")]
    hide: Vec<Regex>,
}

fn read_config() -> Config {
    let Some(path) = find_config_file() else {
        return Config::default();
    };

    let mut contents = String::new();
    let mut file = fs::File::open(path).unwrap();
    file.read_to_string(&mut contents).unwrap();
    toml::from_str(&contents).unwrap()
}

fn find_config_file() -> Option<std::path::PathBuf> {
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

fn deserialize_regex_vec<'de, D>(deserializer: D) -> Result<Vec<Regex>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let strings: Vec<String> = Vec::deserialize(deserializer)?;
    strings
        .into_iter()
        .map(|s| Regex::try_from(s).map_err(serde::de::Error::custom))
        .collect()
}
