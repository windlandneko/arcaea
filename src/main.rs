use arcaea::{Editor, Error};
use crossterm::style::Stylize;

fn main() -> Result<(), Error> {
    std::panic::set_hook(Box::new(|panic_info| {
        let _ = crossterm::execute!(
            std::io::stdout(),
            crossterm::event::DisableFocusChange,
            crossterm::event::DisableBracketedPaste,
            crossterm::event::DisableMouseCapture,
            crossterm::terminal::EnableLineWrap,
            crossterm::terminal::LeaveAlternateScreen,
            crossterm::cursor::Show,
        );
        let _ = crossterm::terminal::disable_raw_mode();

        print!("\n{}: ", "Error".bold().red());
        if let Some(location) = panic_info.location() {
            println!("{}:{}:{}", location.file(), location.line(), location.column());
        } else {
            println!("at unknown location");
        }
        if let Some(s) = panic_info.payload().downcast_ref::<&str>() {
            println!("panic occurred: {s:?}");
        } else if let Some(s) = panic_info.payload().downcast_ref::<String>() {
            println!("panic occurred: {s:?}");
        } else {
            println!("panic occurred");
        }
    }));

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
