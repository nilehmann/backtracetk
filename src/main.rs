use std::io::{self, BufRead, BufReader};
use std::process::{Command, Stdio};

use clap::Parser;
use termcolor::{ColorChoice, StandardStream};

#[derive(clap::Parser)]
#[command(version, about, long_about = None)]
struct Args {
    #[arg(trailing_var_arg(true))]
    cmd: Vec<String>,

    #[arg(long, short, default_value = "short")]
    style: BacktraceStyle,

    #[arg(long, short)]
    no_lib_backtrace: bool,
}

#[derive(clap::ValueEnum, Copy, Clone)]
enum BacktraceStyle {
    Short,
    Full,
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

    if args.cmd.len() == 0 {
        std::process::exit(1);
    }

    let mut env_vars = vec![("RUST_BACKTRACE", args.style.env_var_str())];
    if args.no_lib_backtrace {
        env_vars.push(("RUST_LIB_BACKTRACE", "0"));
    }

    let child = Command::new(args.cmd.remove(0))
        .args(args.cmd)
        .stderr(Stdio::piped())
        .envs(env_vars)
        .spawn()?;

    let mut parser = backtracetk::Parser::new();
    let stderr = child.stderr.expect("failed to open stderr");
    for line in BufReader::new(stderr).lines() {
        let line = line?;
        eprintln!("{line}");
        parser.parse_line(line);
    }

    let mut stderr = StandardStream::stderr(ColorChoice::Auto);
    for backtrace in parser.into_backtraces() {
        backtrace.render(&mut stderr)?;
    }

    Ok(())
}
