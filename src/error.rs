use std::fmt;

pub enum Error {
    UnrecognizedOption(String),
    TooManyArguments(usize),
    Io(std::io::Error),
    Fmt(std::fmt::Error),
    Config(std::path::PathBuf, usize, String),
}

// Provides detailed and user-friendly error messages for debugging purposes.
impl fmt::Debug for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Error::UnrecognizedOption(option) => write!(f, "Unrecognized option: {}", option),
            Error::TooManyArguments(count) => {
                write!(f, "Too many arguments! ({count} arguments provided) This program needs no more than one argument.")
            }
            Error::Io(error) => write!(f, "File IO error: {}", error),
            Error::Fmt(error) => write!(f, "Format error: {}", error),
            Error::Config(msg, count, context) => write!(
                f,
                "Config error: {} (count: {}, context: {})",
                msg.display(), count, context
            ),
        }
    }
}

impl From<std::io::Error> for Error {
    fn from(error: std::io::Error) -> Self {
        Self::Io(error)
    }
}

impl From<std::fmt::Error> for Error {
    fn from(error: std::fmt::Error) -> Self {
        Self::Fmt(error)
    }
}
