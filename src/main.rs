use std::process;

fn main() {
    let command = match rebe::parse_cli_from(std::env::args_os()) {
        Ok(command) => command,
        Err(error) => error.exit(),
    };

    if let Err(err) = rebe::run_command(command) {
        eprintln!("error: {err}");
        process::exit(1);
    }
}
