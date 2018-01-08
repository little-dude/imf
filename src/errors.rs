use std::fmt;
use std::error::Error as StdError;
use std::io::Error as IoError;

pub type ParseResult<'a> = Result<(&'a[u8], &'a[u8]), Error>;

pub fn parse_ok(buf: &[u8], i: usize) -> ParseResult {
    Ok((&buf[i..], &buf[0..i]))
}

#[derive(Debug)]
pub struct Error {
    kind: ErrorKind,
}

impl Error {
    pub fn kind(&self) -> &ErrorKind {
        &self.kind
    }
}

impl From<ErrorKind> for Error {
    fn from(kind: ErrorKind) -> Self {
        Error { kind: kind }
    }
}

#[derive(Debug)]
pub enum ErrorKind {
    Parsing,
    Io(IoError),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str(self.description())
    }
}

impl StdError for Error {
    fn cause(&self) -> Option<&StdError> {
        match *self.kind() {
            ErrorKind::Io(ref err) => Some(err),
            _ => None,
        }
    }

    fn description(&self) -> &str {
        match self.kind {
            ErrorKind::Parsing => "failed to parse a byte sequence",
            ErrorKind::Io(_) => "IO error",
        }
    }
}

impl From<IoError> for Error {
    fn from(err: IoError) -> Self {
        From::from(ErrorKind::Io(err))
    }
}
