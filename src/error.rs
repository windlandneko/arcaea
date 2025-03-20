#[derive(Debug)]
pub enum Error {
    UnrecognizedOption(String),
    TooManyArguments(usize),
}
