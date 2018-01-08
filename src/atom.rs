use std::io::Write;

use errors::{ErrorKind, ParseResult, parse_ok};
use whitespaces::read_cfws;

/// Return true if the byte represents an alphabetical character (`a-zA-Z`)
fn is_alphabetical(c: u8) -> bool {
    // (c >= 65 && c <= 90) || (c >= 97 && c <= 122)
    (c >= b'A' && c <= b'Z') || (c >= b'a' && c <= b'z')
}

/// Return true if the byte represents a digit (`0-9`)
fn is_digit(c: u8) -> bool {
    // c >= 48 && c <= 57
    c >= b'0' && c <= b'9'
}

/// Return true if the byte represents an "atext" token.
///
/// ```no_rust
///  atext           =   ALPHA / DIGIT /    ; Printable US-ASCII
///                      "!" / "#" /        ;  characters not including
///                      "$" / "%" /        ;  specials.  Used for atoms.
///                      "&" / "'" /
///                      "*" / "+" /
///                      "-" / "/" /
///                      "=" / "?" /
///                      "^" / "_" /
///                      "`" / "{" /
///                      "|" / "}" /
///                      "~"
/// ```
///
/// See [RFC5322 section 3.2.3].
///
/// [RFC2822 section 3.2.3]: https://tools.ietf.org/html/rfc5322#section-3.2.3
fn is_atext(c: u8) -> bool {
    is_alphabetical(c) || is_digit(c) ||
        c == b'!' || c == b'#' ||
        c == b'$' || c == b'%' ||
        c == b'&' || c == b'\'' ||
        c == b'*' || c == b'+' ||
        c == b'-' || c == b'/' ||
        c == b'=' || c == b'?' ||
        c == b'^' || c == b'_' ||
        c == b'`' || c == b'{' ||
        c == b'|' || c == b'}' ||
        c == b'~'
}

fn is_dot_atom(c: u8) -> bool {
    is_atext(c) || c == b'.'
}

pub fn read_atom(buf: &[u8]) -> ParseResult {
    let mut i: usize = 0;
    if let Ok((_, cfws)) = read_cfws(buf) {
        i += cfws.len();
    }

    let (_, atom) = read_atom_text(&buf[i..])?;
    i += atom.len();

    if let Ok((_, cfws)) = read_cfws(&buf[i..]) {
        i += cfws.len();
    }
    parse_ok(buf, i)
}

pub fn parse_atom<'a, W: Write>(buf: &'a [u8], writer: &mut W) -> ParseResult<'a> {
    let mut i: usize = 0;
    if let Ok((_, cfws)) = read_cfws(buf) {
        i += cfws.len();
    }

    let (_, atom) = read_atom_text(&buf[i..])?;
    writer.write_all(atom)?;
    i += atom.len();

    if let Ok((_, cfws)) = read_cfws(&buf[i..]) {
        i += cfws.len();
    }
    parse_ok(buf, i)
}

fn read_dot_atom(buf: &[u8]) -> ParseResult {
    let mut i: usize = 0;
    if let Ok((_, cfws)) = read_cfws(buf) {
        i += cfws.len();
    }

    let (_, atom) = read_dot_atom_text(&buf[i..])?;
    i += atom.len();

    if let Ok((_, cfws)) = read_cfws(&buf[i..]) {
        i += cfws.len();
    }
    parse_ok(buf, i)
}

pub fn parse_dot_atom<'a, W: Write>(buf: &'a [u8], writer: &mut W) -> ParseResult<'a> {
    let mut i: usize = 0;
    if let Ok((_, cfws)) = read_cfws(buf) {
        i += cfws.len();
    }

    let (_, atom) = read_dot_atom_text(&buf[i..])?;
    i += atom.len();
    writer.write_all(atom)?;

    if let Ok((_, cfws)) = read_cfws(&buf[i..]) {
        i += cfws.len();
    }
    parse_ok(buf, i)
}

fn read_dot_atom_text(buf: &[u8]) -> ParseResult {
    let mut i: usize = 0;
    while i < buf.len() {
        if is_atext(buf[i]) {
            i += 1;
        } else if buf[i] == b'.' {
            if i + 1 < buf.len() && is_atext(buf[i+1]) {
                i += 2;
            } else {
                break
            }
        } else {
            break;
        }
    }
    if i == 0 {
        return Err(ErrorKind::Parsing.into());
    }
    parse_ok(buf, i)
}

fn read_atom_text(buf: &[u8]) -> ParseResult {
    let mut i: usize = 0;
    while i < buf.len() && is_atext(buf[i]) {
        i += 1;
    }
    if i == 0 {
        return Err(ErrorKind::Parsing.into());
    }
    parse_ok(buf, i)
}

mod test {
    use super::*;

    fn test_read<F>(f: F, input: &[u8], exp_left: &[u8], exp_read: &[u8])
        where F: Fn(&[u8]) -> ParseResult
    {
        let (left, read) = f(input).unwrap();
        assert_eq!(read, exp_read);
        assert_eq!(left, exp_left);
    }

    #[test]
    fn test_read_atom() {
        let f = read_atom;
        test_read(f, &b"a"[..], &b""[..], &b"a"[..]);
        test_read(f, &b"abc"[..], &b""[..], &b"abc"[..]);
        test_read(f, &b"\r\n\tabc "[..], &b""[..], &b"\r\n\tabc "[..]);
        test_read(f, &b"!#$%&'*+-/=?^_`{}|~."[..], &b"."[..], &b"!#$%&'*+-/=?^_`{}|~"[..]);
    }

    #[test]
    fn test_read_dot_atom() {
        let f = read_dot_atom;
        test_read(f, &b"a"[..], &b""[..], &b"a"[..]);
        test_read(f, &b"abc"[..], &b""[..], &b"abc"[..]);
        test_read(f, &b"\r\n\tabc "[..], &b""[..], &b"\r\n\tabc "[..]);
        test_read(f, &b"!#$%&'*+-/=?^_`{}|~."[..], &b"."[..], &b"!#$%&'*+-/=?^_`{}|~"[..]);

        test_read(f, &b"a.b"[..], &b""[..], &b"a.b"[..]);
        test_read(f, &b"abc.abc"[..], &b""[..], &b"abc.abc"[..]);
        test_read(f, &b"\r\n\tabc.abc "[..], &b""[..], &b"\r\n\tabc.abc "[..]);
        test_read(f, &b"!#$%&'*+-/=?^_`{}|~.abc"[..], &b""[..], &b"!#$%&'*+-/=?^_`{}|~.abc"[..]);
    }
}
