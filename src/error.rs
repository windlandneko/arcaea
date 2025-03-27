use std::fmt;

pub enum Error {
    UnrecognizedOption(String),
    TooManyArguments(usize),
}

// Provides detailed and user-friendly error messages for debugging purposes.
impl fmt::Debug for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Error::UnrecognizedOption(option) => write!(f, "Unrecognized option: {}", option),
            Error::TooManyArguments(_) => {
                write!(f, "Too many arguments! This program needs no more than one argument.")
            }
        }
    }
}
