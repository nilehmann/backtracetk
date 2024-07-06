# Backtracetk

Backtracetk is a command line tool to print colorized Rust backtraces without the need to add extra
dependencies to your project.
It works by capturing the output of a process, detecting anything that looks like a backtrace, and then printing
it with colors to be easier on the eyes.
It also prints snippets of the code at each frame if it can find them in the file system.

## Installation

```bash
cargo install --git https://github.com/nilehmann/backtracetk
```

## Screenshot

![Screenshot](./screenshot1.png)

## Usage

```bash
$ backtracetk --help
Print colorized Rust backtraces by capturing the output of an external process

Usage: backtracetk [OPTIONS] [CMD]...

Arguments:
  [CMD]...

Options:
      --style <STYLE>         Set the backtrace style to short (RUST_BACKTRACE=1) or full
                              (RUST_BACKTRACE=full) [default: short] [possible values: short, full]
      --enable-lib-backtrace  By default, backtracetk sets RUST_LIB_BACKTRACE=0. Set this flag to revert this
                              behavior
      --hide-output           By default, backtracetk prints each captured line as it reads it, providing
                              immediate feedback. If this flag is set, this output is suppressed, and nothing
                              will be printed until the program exits
  -h, --help                  Print help

```

### Configuration

Backtracetk will attempt to locate a configuration file named `backtrack.toml` or `.backtrack.toml` in the parent directories starting from where the command is executed. Currently, the only supported configuration is `hide`, which accepts a list of regex patterns.
Any frame matching on of these patterns will be hidden from the output. For example:

![Screenshot2](./screenshot2.png)
