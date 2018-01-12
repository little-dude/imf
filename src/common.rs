use errors::{parse_ok, Error, ErrorKind, ParseResult};
use std::io::Write;
use whitespaces::{read_cfws, read_fws, replace_cfws, replace_fws};
use quoted_string::{parse_quoted_string, read_quoted_string, DEL};
use atom::{parse_atom, parse_dot_atom, read_atom};

/// If the given byte is an upper case alphabetical character, return the same character as lowercase. Otherwise, return the byte.
pub fn lowercase(c: u8) -> u8 {
    match c {
        b'A'...b'Z' => c + 32,
        _ => c,
    }
}

/// Return `true` if the byte represents a non-whitespace control character.
/// See [RFC5322 section 4.1](https://tools.ietf.org/html/rfc5322#section-4.1)
///
/// ```no_rust
///  obs-NO-WS-CTL   =   %d1-8 /            ; US-ASCII control
///                      %d11 /             ;  characters that do not
///                      %d12 /             ;  include the carriage
///                      %d14-31 /          ;  return, line feed, and
///                      %d127              ;  white space characters
/// ```
pub fn is_obs_no_ws_ctl(c: u8) -> bool {
    (c >= 1 && c <= 8) || c == 11 || c == 12 || (c >= 14 && c <= 31) || c == DEL
}

/// Return `true` is the byte represents an "obs-ctext" character as defined in
/// [RFC5322 section 4.1](https://tools.ietf.org/html/rfc5322#section-4.1)
///
/// ```no_rust
///  obs-ctext       =   obs-NO-WS-CTL
/// ```
pub fn is_obs_ctext(c: u8) -> bool {
    is_obs_no_ws_ctl(c)
}

/// Return `true` is the byte represents an "obs-qtext" character as defined in
/// [RFC5322 section 4.1](https://tools.ietf.org/html/rfc5322#section-4.1)
///
/// ```no_rust
///  obs-qtext       =   obs-NO-WS-CTL
/// ```
pub fn is_obs_qtext(c: u8) -> bool {
    is_obs_no_ws_ctl(c)
}

/// Return true if the byte represents a "special" primitive token
///
/// ```no_rust
/// specials        =   "(" / ")" /        ; Special characters that do
///                     "<" / ">" /        ;  not appear in atext
///                     "[" / "]" /
///                     ":" / ";" /
///                     "@" / "\" /
///                     "," / "." /
///                     DQUOTE
/// ```
///
/// See [RFC5322 section 3.2.3].
///
/// [RFC2822 section 3.2.3]: https://tools.ietf.org/html/rfc5322#section-3.2.3
pub fn is_special(c: u8) -> bool {
    c == b'(' || c == b')' ||
        c == b'<' || c == b'>' ||
        c == b'[' || c == b']' ||
        c == b':' || c == b';' ||
        c == b'@' || c == b'\\' ||
        c == b',' || c == b'.' ||
        c == b'"'
}

/// Return `true` is the given byte represents a visible and printable (i.e. not a space)
/// character.
///
/// ```no_rust
/// VCHAR          =  %x21-7E
///                        ; visible (printing) characters
/// ```
///
/// See [RFC2234 section 6.1](https://tools.ietf.org/html/rfc2234#section6.1)
pub fn is_vchar(c: u8) -> bool {
    c >= 33 && c <= 126
}

pub fn read_word(buf: &[u8]) -> ParseResult {
    read_atom(buf).or_else(|e| match *e.kind() {
        ErrorKind::Parsing => read_quoted_string(buf),
        _ => Err(e),
    })
}

pub fn parse_word<'a, W: Write>(buf: &'a [u8], writer: &mut W) -> ParseResult<'a> {
    parse_atom(buf, writer).or_else(|e| match *e.kind() {
        ErrorKind::Parsing => parse_quoted_string(buf, writer),
        _ => Err(e),
    })
}

/// obs-phrase      =   word *(word / "." / CFWS)
/// phrase = 1*word / obs-phrase
pub fn read_phrase(buf: &[u8]) -> ParseResult {
    let (_, word) = read_word(buf)?;
    let mut i = word.len();
    while i < buf.len() {
        if let Ok((_, word)) = read_word(&buf[i..]) {
            i += word.len();
        } else if let Ok((_, cfws)) = read_cfws(&buf[i..]) {
            i += cfws.len();
        } else if buf[i] == b'.' {
            i += 1;
        } else {
            break;
        }
    }
    parse_ok(buf, i)
}

pub fn parse_phrase<'a, W: Write>(buf: &'a [u8], writer: &mut W) -> ParseResult<'a> {
    let (_, word) = parse_word(buf, writer)?;
    let mut i = word.len();
    while i < buf.len() {
        match parse_word(&buf[i..], writer) {
            Ok((_, word)) => {
                i += word.len();
                continue;
            }
            Err(e) => match *e.kind() {
                ErrorKind::Parsing => return Err(e),
                _ => {}
            },
        }

        match replace_cfws(&buf[i..], writer) {
            Ok((_, cfws)) => {
                i += cfws.len();
                continue;
            }
            Err(e) => match *e.kind() {
                ErrorKind::Parsing => return Err(e),
                _ => {}
            },
        }

        if buf[i] == b'.' {
            writer.write_all(&b"."[..])?;
            i += 1;
        } else {
            break;
        }
    }
    parse_ok(buf, i)
}

// unstructured = (*([FWS] VCHAR) *WSP) / obs-unstruct
// obs-utext    = %d0 / obs-NO-WS-CTL / VCHAR
// obs-unstruct = *((*LF *CR *(obs-utext *LF *CR)) / FWS)
