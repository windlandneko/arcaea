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

impl Error {
    pub fn get_error_message(err: &std::io::Error) -> &str {
        use std::io::ErrorKind::*;
        match err.kind() {
            AddrInUse => "地址被占用",
            AddrNotAvailable => "地址不可用",
            AlreadyExists => "文件已存在",
            ArgumentListTooLong => "参数列表过长",
            BrokenPipe => "管道已断开",
            ConnectionAborted => "连接已中止",
            ConnectionRefused => "连接被拒绝",
            ConnectionReset => "连接已重置",
            CrossesDevices => "不能跨设备进行链接或重命名",
            Deadlock => "检测到死锁",
            DirectoryNotEmpty => "文件夹不是空的，里面还有东西",
            ExecutableFileBusy => "可执行文件正在使用中",
            FileTooLarge => "文件太大",
            HostUnreachable => "主机不可达",
            Interrupted => "操作被中断",
            InvalidData => "数据无效",
            InvalidInput => "输入参数无效",
            IsADirectory => "该路径是一个目录",
            NetworkDown => "网络连接已断开",
            NetworkUnreachable => "网络不可达",
            NotADirectory => "不是一个目录",
            NotConnected => "未连接",
            NotFound => "未找到文件",
            NotSeekable => "文件不支持查找",
            Other => "发生未知错误",
            OutOfMemory => "内存不足（OOM）",
            PermissionDenied => "需要管理员权限",
            ReadOnlyFilesystem => "文件系统为只读",
            ResourceBusy => "资源正忙",
            StaleNetworkFileHandle => "网络文件句柄已失效",
            StorageFull => "存储空间不足",
            TimedOut => "操作超时",
            UnexpectedEof => "遇到意外 EOF 结束符，拼尽全力无法战胜",
            _ => "未知错误",
        }
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
