//! Custom error type that includes the filename of the input file that caused the error.

use std::fmt;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub struct FileError<E: fmt::Display> {
    pub file: PathBuf,
    pub error: E
}

impl<E: fmt::Display> fmt::Display for FileError<E> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.file.display(), self.error)
    }
}

// Order file errors by their Display representation, i.e., filename first, then contained error.
// We need to manually implement PartialOrd/Ord/PartialEq/Eq such that we don't have a bound on the
// contained error E (which could be an anyhow::Error, that does not implement Ord etc.).
impl<E: fmt::Display> PartialOrd for FileError<E> {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        self.to_string().partial_cmp(&other.to_string())
    }
}

impl<E: fmt::Display> Ord for FileError<E> {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.to_string().cmp(&other.to_string())
    }
}

impl<E: fmt::Display> PartialEq for FileError<E> {
    fn eq(&self, other: &Self) -> bool {
        self.to_string().eq(&other.to_string())
    }
}

impl<E: fmt::Display> Eq for FileError<E> {}


impl<E: fmt::Display + fmt::Debug> std::error::Error for FileError<E> {}

/// Extension trait to attach the file to errors.
/// Due to the `E: fmt::Display` bound, this will never be overlapping/ambigious with the Result
/// extension trait below, since `Result` does not implement `fmt::Display`.
pub trait ErrorWithFile: fmt::Display + Sized {
    fn with_file(self, file: impl AsRef<Path>) -> FileError<Self> {
        FileError { 
            file: file.as_ref().to_owned(), 
            error: self 
        }
    }
}

impl<E: fmt::Display> ErrorWithFile for E {}

/// Extension trait to attach the file to the error in a `Result`.
pub trait ResultWithFile<T, E: fmt::Display> {
    fn with_file(self, file: impl AsRef<Path>) -> Result<T, FileError<E>>;
}

impl<T, E: ErrorWithFile> ResultWithFile<T, E> for Result<T, E> {
    fn with_file(self, file: impl AsRef<Path>) -> Result<T, FileError<E>> {
        self.map_err(|err| err.with_file(file))
    }
}
