use errors::{ErrorKind, ParseResult, parse_ok};
use std::io::Write;


/// CRLF sequence (`\r\n`)
pub static CRLF: [u8; 2] = *b"\r\n";

/// Return true if the byte represents a whitespace or an horizontal tab.
/// See [RFC2822 section 2.2.2].
///
/// [RFC2822 section 2.2.2]: https://tools.ietf.org/html/rfc2822#section-2.2.2
///
fn is_wsp(c: u8) -> bool {
    c == b' ' || c == b'\t'
}

/// Read a folding whitespace (FWS). See [RFC5322 section 2.2.3].
///
/// ```no_rust
/// FWS             =   ([*WSP CRLF] 1*WSP) /   ; Folding white space
///                     obs-FWS
/// obs-FWS         =   1*WSP *(CRLF 1*WSP)
/// ```
///
/// [RFC5322 section 2.2.3]: https://tools.ietf.org/html/rfc5322#section-2.2.3
pub fn read_fws(buf: &[u8]) -> ParseResult {
    // the two definitions above are equivalent to
    // FWS = ([*WSP CRLF] 1*WSP) / (1*WSP *(CRLF 1*WSP))
    if buf.is_empty() {
        return Err(ErrorKind::Parsing.into());
    }
    let mut i: usize = 0;
    while i < buf.len() {
        match buf[i] {
            // whitespace
            c if is_wsp(c) => i += 1,
            // CRLF
            b'\r' => {
                // we need to match LF and then a space
                if i + 2 < buf.len() && buf[i+1] == b'\n' && is_wsp(buf[i+2]) {
                    i += 3;
                } else {
                    break;
                }
            },
            _ => break,
        }
    }
    if i == 0 {
        return Err(ErrorKind::Parsing.into());
    }
    parse_ok(buf, i)
}

/// Parse a folding whitespace (FWS): read a folding whitespace, unfold if (i.e. remove any CRLF
/// (`\r\n`)), and write it to the provided writer. See [RFC5322 section 2.2.3].
///
///
/// ```no_rust
/// FWS     =   ([*WSP CRLF] 1*WSP) / (1*WSP *(CRLF 1*WSP))
/// ```
///
/// [RFC5322 section 2.2.3]: https://tools.ietf.org/html/rfc5322#section-2.2.3
pub fn unfold_fws<'a, W: Write>(buf: &'a[u8], writer: &mut W) -> ParseResult<'a> {
    if buf.is_empty() {
        return Err(ErrorKind::Parsing.into());
    }
    let mut i: usize = 0;
    let mut next_write: usize = 0;
    while i < buf.len() {
        match buf[i] {
            // whitespace
            c if is_wsp(c) => i += 1,
            // CRLF
            b'\r' => {
                writer.write_all(&buf[next_write..i])?;
                // we need to match LF and then a space
                if i + 2 < buf.len() && buf[i+1] == b'\n' && is_wsp(buf[i+2]) {
                    next_write = i + 2;
                    i += 3;
                } else {
                    break;
                }
            },
            _ => break,
        }
    }
    if i == 0 {
        return Err(ErrorKind::Parsing.into());
    }
    writer.write_all(&buf[next_write..i])?;
    parse_ok(buf, i)
}

/// Read comments. See [RFC5322 section 3.2.3]
///
/// ```no_rust
/// FWS             =       ([*WSP CRLF] 1*WSP) /   ; Folding white space
///                         obs-FWS
///
/// ctext           =       NO-WS-CTL /     ; Non white space controls
///
///                         %d33-39 /       ; The rest of the US-ASCII
///                         %d42-91 /       ;  characters not including "(",
///                         %d93-126        ;  ")", or "\"
///
/// ccontent        =       ctext / quoted-pair / comment
///
/// comment         =       "(" *([FWS] ccontent) [FWS] ")"
/// ```
///
/// [RFC5322 section 3.2.3]: https://tools.ietf.org/html/rfc5322#section-3.2.3
fn read_comment(buf: &[u8]) -> ParseResult {
    if buf.len() < 2 {
        return Err(ErrorKind::Parsing.into());
    }
    if buf[0] != b'(' {
        return Err(ErrorKind::Parsing.into());
    }
    // comments can be nested. Since we already found an opening parenthesis, we start at 1.
    let mut nested_level = 1;

    let mut i: usize = 1;
    while i < buf.len() {
        match buf[i] {
            b'\\' => {
                // we want to ignore the next character, since it's escaped
                i += 2;
                continue;
            }
            b')' => {
                nested_level -= 1;
                if nested_level == 0 {
                    return parse_ok(buf, i+1);
                }
            }
            b'(' => nested_level += 1,
            // ignore any other character
            _ => {}
        }

        i += 1
    }
    Err(ErrorKind::Parsing.into())
}

/// Read CFWS. See [RFC5322 section 3.2.3].
///
/// ```no_rust
/// CFWS            =       *([FWS] comment) (([FWS] comment) / FWS)
/// ```
///
/// [RFC5322 section 3.2.3]: https://tools.ietf.org/html/rfc5322#section-3.2.3
pub fn read_cfws(buf: &[u8]) -> ParseResult {
    if buf.is_empty() {
        return Err(ErrorKind::Parsing.into());
    }

    let mut i: usize = 0;
    while i < buf.len() {
        if let Ok((_, fws)) = read_fws(&buf[i..]) {
            i += fws.len();
        }
        match read_comment(&buf[i..]) {
            Ok((_, comment)) => i += comment.len(),
            Err(_) => {
                if i == 0 {
                    // We're supposed to read at least one comment or one FWS
                    // So it's an error not to read anything
                    return Err(ErrorKind::Parsing.into());
                } else {
                    return parse_ok(buf, i);
                }
            }
        }
    }
    if i > 0 {
        parse_ok(buf, i)
    } else {
        Err(ErrorKind::Parsing.into())
    }
}

pub fn replace_cfws<'a, W: Write>(buf: &'a[u8], writer: &mut W) -> ParseResult<'a> {
    let res = read_cfws(buf)?;
    writer.write_all(&b" "[..])?;
    Ok(res)
}

pub fn replace_fws<'a, W: Write>(buf: &'a[u8], writer: &mut W) -> ParseResult<'a> {
    let res = read_fws(buf)?;
    writer.write_all(&b" "[..])?;
    Ok(res)
}

/// Unfold CFWS. See [RFC5322 section 3.2.3].
///
/// ```no_rust
/// CFWS            =       *([FWS] comment) (([FWS] comment) / FWS)
/// ```
///
/// [RFC5322 section 3.2.3]: https://tools.ietf.org/html/rfc5322#section-3.2.3
pub fn unfold_cfws<'a, W: Write>(buf: &'a[u8], writer: &mut W) -> ParseResult<'a> {
    if buf.is_empty() {
        return Err(ErrorKind::Parsing.into());
    }

    let mut i: usize = 0;
    while i < buf.len() {
        match unfold_fws(&buf[i..], writer) {
            Ok((_, fws)) => i += fws.len(),
            Err(e) => {
                match *e.kind() {
                    // ignore parsing errors, since this token is not mandatory
                    ErrorKind::Parsing => {},
                    // propagate the other errors
                    _ => return Err(e),
                }
            }
        }
        match read_comment(&buf[i..]) {
            Ok((_, comment)) => i += comment.len(),
            Err(_) => {
                if i == 0 {
                    // We're supposed to read at least one comment or one FWS
                    // So it's an error not to read anything
                    return Err(ErrorKind::Parsing.into());
                } else {
                    return parse_ok(buf, i);
                }
            }
        }
    }
    if i > 0 {
        parse_ok(buf, i)
    } else {
        Err(ErrorKind::Parsing.into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_comment() {
        assert_eq!(
            read_comment(&b"()"[..]).unwrap(),
            (&b""[..], &b"()"[..])
        );
        assert_eq!(
            read_comment(&b"(abc)"[..]).unwrap(),
            (&b""[..], &b"(abc)"[..])
        );
        assert_eq!(
            read_comment(&b"(a comment)abc"[..]).unwrap(),
            (&b"abc"[..], &b"(a comment)"[..])
        );
        assert_eq!(
            read_comment(&b"(a (nested (comment)))abc"[..]).unwrap(),
            (&b"abc"[..], &b"(a (nested (comment)))"[..])
        );
        assert_eq!(
            read_comment(&b"(a (nested \\((comment)\\)\\)))abc"[..]).unwrap(),
            (&b"abc"[..], &b"(a (nested \\((comment)\\)\\)))"[..])
        );
    }

    #[test]
    fn test_folding_whitespace() {
        assert_eq!(read_fws(&b" "[..]).unwrap(), (&b""[..], &b" "[..]));
        assert_eq!(read_fws(&b" \t"[..]).unwrap(), (&b""[..], &b" \t"[..]));
        assert_eq!(read_fws(&b" abc"[..]).unwrap(), (&b"abc"[..], &b" "[..]));
        assert_eq!(read_fws(&b"\tabc"[..]).unwrap(), (&b"abc"[..], &b"\t"[..]));
        assert_eq!(
            read_fws(&b"\t abc"[..]).unwrap(),
            (&b"abc"[..], &b"\t "[..])
        );
        assert_eq!(
            read_fws(&b" \r\n abc"[..]).unwrap(),
            (&b"abc"[..], &b" \r\n "[..])
        );
        assert_eq!(
            read_fws(&b" \r\n \r\n\tabc"[..]).unwrap(),
            (&b"abc"[..], &b" \r\n \r\n\t"[..])
        );
        assert_eq!(
            read_fws(&b" \r\nabc"[..]).unwrap(),
            (&b"\r\nabc"[..], &b" "[..])
        );
        assert_eq!(
            read_fws(&b" \r\n  \r\n \r\nabc"[..]).unwrap(),
            (&b"\r\nabc"[..], &b" \r\n  \r\n "[..])
        );

        // new folding whitespaces
        assert_eq!(
            read_fws(&b"\r\n   abc"[..]).unwrap(),
            (&b"abc"[..], &b"\r\n   "[..])
        );
        assert_eq!(
            read_fws(&b"\r\n \t  abc"[..]).unwrap(),
            (&b"abc"[..], &b"\r\n \t  "[..])
        );

        // failing cases
        assert!(read_fws(&b"\r\nabc"[..]).is_err());
        assert!(read_fws(&b"\r abc"[..]).is_err());
        assert!(read_fws(&b"\n\r abc"[..]).is_err());
    }

    #[test]
    fn test_cfws() {
        // fws test cases
        assert_eq!(read_cfws(&b" "[..]).unwrap(), (&b""[..], &b" "[..]));
        assert_eq!(read_cfws(&b" \t"[..]).unwrap(), (&b""[..], &b" \t"[..]));
        assert_eq!(read_cfws(&b" abc"[..]).unwrap(), (&b"abc"[..], &b" "[..]));
        assert_eq!(read_cfws(&b"\tabc"[..]).unwrap(), (&b"abc"[..], &b"\t"[..]));
        assert_eq!(
            read_cfws(&b"\t abc"[..]).unwrap(),
            (&b"abc"[..], &b"\t "[..])
        );
        assert_eq!(
            read_cfws(&b" \r\n abc"[..]).unwrap(),
            (&b"abc"[..], &b" \r\n "[..])
        );
        assert_eq!(
            read_cfws(&b" \r\n \r\n\tabc"[..]).unwrap(),
            (&b"abc"[..], &b" \r\n \r\n\t"[..])
        );
        assert_eq!(
            read_cfws(&b" \r\nabc"[..]).unwrap(),
            (&b"\r\nabc"[..], &b" "[..])
        );
        assert_eq!(
            read_cfws(&b" \r\n  \r\n \r\nabc"[..]).unwrap(),
            (&b"\r\nabc"[..], &b" \r\n  \r\n "[..])
        );
        assert_eq!(
            read_cfws(&b"\r\n   abc"[..]).unwrap(),
            (&b"abc"[..], &b"\r\n   "[..])
        );
        assert_eq!(
            read_cfws(&b"\r\n \t  abc"[..]).unwrap(),
            (&b"abc"[..], &b"\r\n \t  "[..])
        );
        assert!(read_cfws(&b"\r\nabc"[..]).is_err());
        assert!(read_cfws(&b"\r abc"[..]).is_err());
        assert!(read_cfws(&b"\n\r abc"[..]).is_err());

        // comment test cases
        assert_eq!(
            read_cfws(&b"(a comment)abc"[..]).unwrap(),
            (&b"abc"[..], &b"(a comment)"[..])
        );
        assert_eq!(
            read_cfws(&b"(a (nested (comment)))abc"[..]).unwrap(),
            (&b"abc"[..], &b"(a (nested (comment)))"[..])
        );
        assert_eq!(
            read_cfws(&b"(a (nested \\((comment)\\)\\)))abc"[..]).unwrap(),
            (&b"abc"[..], &b"(a (nested \\((comment)\\)\\)))"[..])
        );

        // mixed test cases
        assert_eq!(
            read_cfws(&b"  (a comment)  abc"[..]).unwrap(),
            (&b"abc"[..], &b"  (a comment)  "[..])
        );
        assert_eq!(
            read_cfws(&b"(a comment)  abc"[..]).unwrap(),
            (&b"abc"[..], &b"(a comment)  "[..])
        );
        assert_eq!(
            read_cfws(&b"  (a comment)abc"[..]).unwrap(),
            (&b"abc"[..], &b"  (a comment)"[..])
        );
        assert_eq!(
            read_cfws(&b"  (  a comment ( ) ()\r\n)  abc"[..]).unwrap(),
            (&b"abc"[..], &b"  (  a comment ( ) ()\r\n)  "[..])
        );
        assert_eq!(
            read_cfws(&b"(a comment)  () ()abc"[..]).unwrap(),
            (&b"abc"[..], &b"(a comment)  () ()"[..])
        );
    }
}
