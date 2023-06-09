use std::io::{self, BufRead, BufReader};
use std::process::{Command, Stdio};

use clap::Parser;
use termcolor::{ColorChoice, StandardStream};

#[derive(clap::Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    #[arg(trailing_var_arg(true))]
    cmd: Vec<String>,

    #[arg(long, short, default_value = "1")]
    backtrace: String,
}

fn main() -> io::Result<()> {
    let mut args = Args::parse();

    let Some(cmd) = args.cmd.pop() else {
        eprintln!("No command specified");
        std::process::exit(1);
    };

    let mut cmd = Command::new(cmd);
    for arg in args.cmd {
        cmd.arg(arg);
    }
    let child = cmd
        .stderr(Stdio::piped())
        .env("RUST_BACKTRACE", args.backtrace)
        .spawn()
        .expect("Failed to execute command");

    let mut parser = backtracetk::Parser::new();
    let stderr = child.stderr.expect("Failed to open stderr");
    for line in BufReader::new(stderr).lines() {
        let line = line?;
        if !parser.parse_line(&line) {
            eprintln!("{line}");
        }
    }
    let backtrace = parser.into_backtrace();

    let mut stderr = StandardStream::stderr(ColorChoice::Auto);
    backtrace.render(&mut stderr)?;

    Ok(())
}
