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

![Screenshot](./screenshot.png)
