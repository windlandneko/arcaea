use std::{fmt, path::PathBuf};

use terminal_clipboard::ClipboardError;

pub enum Error {
    UnrecognizedOption(String),
    TooManyArguments(usize),
    Io(std::io::Error),
    Fmt(std::fmt::Error),
    ClipboardError(String),
    FileError(PathBuf, usize, String),
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
            Error::ClipboardError(message) => write!(f, "Clipboard error: {}", message),
            Error::FileError(path, line, message) => write!(
                f,
                "File error: {} (line {}): {}",
                path.display(),
                line,
                message
            ),
            // _ => write!(f, "An unknown error occurred."),
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

impl From<ClipboardError> for Error {
    fn from(error: ClipboardError) -> Self {
        Self::ClipboardError(error.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn test_unrecognized_option_debug() {
        let error = Error::UnrecognizedOption("--invalid".to_string());
        assert_eq!(format!("{:?}", error), "Unrecognized option: --invalid");
    }

    #[test]
    fn test_too_many_arguments_debug() {
        let error = Error::TooManyArguments(3);
        assert_eq!(
            format!("{:?}", error),
            "Too many arguments! (3 arguments provided) This program needs no more than one argument."
        );
    }

    #[test]
    fn test_io_error_debug() {
        let io_error = std::io::Error::new(std::io::ErrorKind::NotFound, "file not found");
        let error = Error::Io(io_error);
        assert_eq!(format!("{:?}", error), "File IO error: file not found");
    }

    #[test]
    fn test_fmt_error_debug() {
        let fmt_error = std::fmt::Error::default();
        let error = Error::Fmt(fmt_error);
        assert_eq!(
            format!("{:?}", error),
            "Format error: an error occurred when formatting an argument"
        );
    }

    #[test]
    fn test_clipboard_error_debug() {
        let error = Error::ClipboardError("clipboard access denied".to_string());
        assert_eq!(
            format!("{:?}", error),
            "Clipboard error: clipboard access denied"
        );
    }

    #[test]
    fn test_file_error_debug() {
        let path = PathBuf::from("/test/file.txt");
        let error = Error::FileError(path, 42, "invalid syntax".to_string());
        assert_eq!(
            format!("{:?}", error),
            "File error: /test/file.txt (line 42): invalid syntax"
        );
    }

    #[test]
    fn test_from_io_error() {
        let io_error = std::io::Error::new(std::io::ErrorKind::Other, "test error");
        let error: Error = io_error.into();
        assert!(matches!(error, Error::Io(_)));
    }

    #[test]
    fn test_from_fmt_error() {
        let fmt_error = std::fmt::Error::default();
        let error: Error = fmt_error.into();
        assert!(matches!(error, Error::Fmt(_)));
    }
}
