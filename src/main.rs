use std::env;
use std::process;

fn main() {
    if let Err(err) = rebe::run(env::args()) {
        eprintln!("error: {err}");
        process::exit(1);
    }
}
