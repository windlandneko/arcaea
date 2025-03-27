use arcaea::Error;
use crossterm::style::Stylize;

fn main() -> Result<(), Error> {
    let mut arguments = std::env::args();

    match (arguments.nth(1), arguments.len()) {
        (Some(arg), 0) if arg == "-v" || arg == "--version" => {
            println!("arcaea {}", env!("VERSION_INFO"));
        }
        (Some(arg), 0) if arg == "-h" || arg == "--help" => print_help_message(),
        (Some(arg), 0) if arg.starts_with('-') => return Err(Error::UnrecognizedOption(arg)),

        (None, 0) => todo!("Implement functionality to create a new file"),
        (Some(filename), 0) => todo!(
            "Implement file parsing for the provided filename: {:?}",
            filename
        ),

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
        "  {}, {}{}",
        "-v".bold().cyan(),
        format!("{:<12}", "--version").bold().cyan(),
        "Print version info and exit"
    );
    println!(
        "  {}, {}{}",
        "-h".bold().cyan(),
        format!("{:<12}", "--help").bold().cyan(),
        "Print help"
    );
    println!();
}
