# Backtracetk

Backtracetk is a command-line tool that prints colorized Rust backtraces without needing extra dependencies.
It works by capturing the output of a Rust binary, detecting anything that looks like a backtrace, and then printing it with colors to make it easier on the eyes.
Additionally, it displays code snippets if available in the filesystem and offers configurable options to hide specific frames.

Backtracetk is useful in situations where you can't or don't want to add runtime dependencies.
It is thus more "dynamic", allowing you to run the process many times (assuming it's cheap to do so) and adjust the output accordingly without the need to recompile your code.

If you're ok with adding dependencies, consider looking at [color-eyre](https://crates.io/crates/color-eyre) or [color-backtrace](https://crates.io/crates/color-backtrace).

I've only tested this on Linux and primarily within a single project.
If you try it and encounter any issues, please share the output of your process.

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
      --style <STYLE>                    Set the backtrace style to `short` (RUST_BACKTRACE=1) or `full`
                                         (RUST_BACKTRACE=full) [default: short] [possible values: short, full]
      --lib-backtrace <LIB_BACKTRACE>    Enable or disable `Backtrace::capture`. If this flag is set to `no`,
                                         backtracetk sets RUST_LIB_BACKTRACE=0, disabling
                                         `Backtrace::capture`. If the flag is set to `yes`, no changes are
                                         made, and the default behavior of capturing backtraces remains
                                         enabled [default: no] [possible values: yes, no]
      --clicolor-force <CLICOLOR_FORCE>  If this flag is `yes`, set CLICOLOR_FORCE=1. If the flag is `no`, no
                                         changes are made [default: yes] [possible values: yes, no]
      --hide-output                      By default, backtracetk prints each captured line as it reads it,
                                         providing immediate feedback. If this flag is set, this output is
                                         suppressed, and nothing will be printed until the program exits
  -h, --help                             Print help
```

### Configuration

Backtracetk will search for a configuration file named `backtrack.toml` or `.backtrack.toml` in the parent directories starting from the directory where the command is executed. Currently, the only supported configuration option is hide, which accepts an array of tables. Each table can take one of two forms:

* A table with the key `pattern` and a value containing a regex. Any frame matching this pattern will be hidden from the output.
* A table with the keys `start` and `end`, both containing regex values. Every frame between a frame matching the `start` regex and a frame matching the `end` regex will be hidden. The `end` pattern is optional; if omitted, every frame after matching `start` will be hidden.

For an example:

![Screenshot2](./screenshot2.png)
