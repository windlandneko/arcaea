use arcaea::{Editor, Error};
use crossterm::style::Stylize;

fn main() -> Result<(), Error> {
    let mut arguments = std::env::args();

    match (arguments.nth(1), arguments.len()) {
        (Some(arg), 0) if arg == "-v" || arg == "--version" => {
            println!("arcaea {}", env!("VERSION_INFO"));
        }
        (Some(arg), 0) if arg == "-h" || arg == "--help" => print_help_message(),
        (Some(arg), 0) if arg.starts_with('-') => return Err(Error::UnrecognizedOption(arg)),

        (filename, 0) => Editor::new().init(&filename)?,

        (_, n_remaining_args) => return Err(Error::TooManyArguments(n_remaining_args + 1)),
    }
    Ok(())
}

/// Prints the help message for the application, including usage instructions and available options.
fn print_help_message() {
    println!("A Rust Console Ascii Editor App");
    println!();
    println!(
        "{} {} {}",
        "Usage:".bold().green(),
        "arcaea".bold().cyan(),
        "[filename]".cyan()
    );
    println!();
    println!("{}", "Options:".bold().green());
    println!(
        "  {}, {}Print version info and exit",
        "-v".bold().cyan(),
        format!("{:<12}", "--version").bold().cyan()
    );
    println!(
        "  {}, {}Print help",
        "-h".bold().cyan(),
        format!("{:<12}", "--help").bold().cyan()
    );
    println!();
}
