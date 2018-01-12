use std::fmt;
use std::error::Error as StdError;
use std::io::Error as IoError;

pub type ParseResult<'a> = Result<(&'a [u8], &'a [u8]), Error>;

#[derive(Debug)]
pub struct Error {
    kind: ErrorKind,
    cause: Option<Box<Error>>,
}

impl Error {
    pub fn set_cause(&mut self, error: Error) {
        self.cause = Some(Box::new(error));
    }

    pub fn kind(&self) -> &ErrorKind {
        &self.kind
    }

    pub fn is_token(&self) -> bool {
        match self.kind {
            ErrorKind::Token { .. } => true,
            _ => false,
        }
    }
    pub fn is_io(&self) -> bool {
        match self.kind {
            ErrorKind::Io(_) => true,
            _ => false,
        }
    }
    pub fn is_eof(&self) -> bool {
        match self.kind {
            ErrorKind::Eof => true,
            _ => false,
        }
    }
}

impl From<ErrorKind> for Error {
    fn from(kind: ErrorKind) -> Self {
        Error { kind: kind, cause: None }
    }
}

#[derive(Clone, Debug, Copy, Eq, PartialEq, Hash)]
pub enum Token {
    /// ```no_rust
    /// FWS      = ([*WSP CRLF] 1*WSP) / obs-FWS
    /// obs-FWS  = 1*WSP *(CRLF 1*WSP)
    /// ```
    /// These two definitions are equivalent to:
    /// ```no_rust
    /// FWS = (1*WSP *(CRLF 1*WSP)) / 1*(CRLF 1*WSP)
    /// ```
    Fws,
    /// ```no_rust
    /// *([FWS] comment) (([FWS] comment) / FWS)
    /// ```
    Cfws,
    /// ```no_rust
    /// ccontent = ctext / quoted-pair / comment
    /// comment  = "(" *([FWS] ccontent) [FWS] ")"
    /// ; Non white space controls The rest of the US-ASCII characters not including "(", ")", or "\"
    /// ctext    = NO-WS-CTL / %d33-39 / %d42-91 / %d93-126
    Comment,
    /// ```no_rust
    /// "\" %d0-127
    /// ```
    QuotedPair,
    /// ```no_rust
    /// [CFWS] " *([FWS] (qtext / quote-pair)) [FWS] " [CFWS]
    /// ```
    QuotedString,
    /// Printable US-ASCII characters not including `\`, `"`, or space.
    /// ```no_rust
    ///  %d1-8 / %d11 / %d12 / %d14-31 / %d33 / %d35-91 / %d93-126 / %d127
    /// ```
    QuotedText,
    Address,
    Domain,
    /// ```no_rust
    /// [CFWS] 1*atext [CFWS]
    /// ```
    Atom,
    /// ```no_rust
    /// [CFWS] 1*atext *("." 1*atext) [CFWS]
    /// ```
    DotAtom,
    /// ```no_rust
    /// ALPHA / DIGIT / "!" / "#" / "$" / "%" / "&" / "'" / "*" / "+" / "-" / "/" / "=" / "?" / "^" / "_" / "`" / "{" / "|" / "}" / "~"
    /// ```
    Atext,
    /// ```no_rust
    /// atom / quoted-string
    /// ```
    Word,
}

#[derive(Debug)]
pub enum ErrorKind {
    Eof,
    /// An error occured trying to parse a token
    Token {
        /// the token that could not be parsed
        token: Token,
        /// byte that triggered the error
        byte: u8,
        /// index where the failure occured
        position: usize,
    },
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
            ErrorKind::Eof => "no more byte to read in the buffer",
            ErrorKind::Token { .. } => "failed to parse a byte sequence",
            ErrorKind::Io(_) => "IO error",
        }
    }
}

impl From<IoError> for Error {
    fn from(err: IoError) -> Self {
        From::from(ErrorKind::Io(err))
    }
}
