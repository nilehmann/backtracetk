use std::io::{self, Read};

fn main() -> io::Result<()> {
    let mut input = String::new();
    io::stdin().read_to_string(&mut input)?;

    let backtrace = backtracetk::parse(&input);

    backtrace.render()?;

    Ok(())
}
