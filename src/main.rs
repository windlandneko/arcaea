use arcaea::Error;

fn main() -> Result<(), Error> {
    let mut arguments = std::env::args();

    match (arguments.nth(1), arguments.len()) {
        (Some(arg), 0) if arg == "-v" || arg == "--version" => {
            println!("arcaea {}", env!("VERSION_INFO"))
        }
        (Some(arg), 0) if arg.starts_with('-') => return Err(Error::UnrecognizedOption(arg)),

        (None, 0) => todo!("new file"),
        (Some(filename), 0) => todo!("parse file: {:?}", filename),

        (_, n_remaining_args) => return Err(Error::TooManyArguments(n_remaining_args + 1)),
    }
    Ok(())
}
