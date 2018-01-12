use std::io::Write;

use errors::{parse_ok, ErrorKind, ParseResult};
use whitespaces::{read_cfws, read_fws, replace_fws};

/// NULL character
pub static NULL: u8 = 0;

/// Delete character
pub static DEL: u8 = 127;

/// Some characters are reserved for special interpretation, such as delimiting lexical tokens.  To
/// permit use of these characters as uninterpreted data, a quoting mechanism is provided.
///
/// ```no_rust
/// quoted-pair     =   ("\" (VCHAR / WSP)) / obs-qp
/// ```
///
/// Where any quoted-pair appears, it is to be interpreted as the character alone.  That is to say,
/// the "\" character that appears as part of a quoted-pair is semantically "invisible".
///
/// this definition is a bit convoluted, but it means that a quoted pair is basically "\" followed
/// by any ASCII character between 0 and 127:
///
///   - `obs-qp = "\" (%d0 / obs-NO-WS-CTL / LF / CR)` this is 0-8 10-31 and 127
///   - `quoted-pair = ("\" (VCHAR / WSP)) / obs-qp` this is 9 and 32-126
///
/// See [RFC5322 section 3.2.1](https://tools.ietf.org/html/rfc5322#section-3.2.1)
// FIXME: UNUSED
pub fn read_quoted_pair(buf: &[u8]) -> ParseResult {
    if buf.len() < 2 || buf[0] != b'\\' || buf[1] > 127 {
        return Err(ErrorKind::Parsing.into());
    }
    parse_ok(buf, 2)
}

/// Return `true` is the character is a valid non-escaped character in quoted content
fn is_valid_qtext(c: u8) -> bool {
    // qtext is everything except (NULL, \, ") and any kind of whitespace
    c > NULL &&
        c <= DEL &&
        c != b'\t' &&
        c != b'\n' &&
        c != b'\r' &&
        c != b' ' &&
        c != b'"' &&
        c != b'\\'
}

/// Read quoted content, and write the content it into the provided writer, removing the
/// backslashes used to escape characters. See
/// [RFC5322 section 3.2.4](https://tools.ietf.org/html/rfc5322#section-3.2.4)
///
/// If you just need to read quoted content (as opposed to un-escape it and write the result into a
/// writer), use [`read_qcontent`](fn.read_qcontent.html) instead.
///
/// ```no_rust
/// qtext           =   %d33 /             ; Printable US-ASCII
///                     %d35-91 /          ;  characters not including
///                     %d93-126 /         ;  "\" or the quote character
///                     obs-qtext
/// obs-qtext       =   obs-NO-WS-CTL
/// obs-NO-WS-CTL   =   %d1-8 /            ; US-ASCII control
///                     %d11 /             ;  characters that do not
///                     %d12 /             ;  include the carriage
///                     %d14-31 /          ;  return, line feed, and
///                     %d127              ;  white space characters
/// qcontent        =   qtext / quoted-pair
/// ```
///
/// # Examples
///
/// ```
/// # extern crate imf;
/// # use imf::quoted_string::parse_qcontent;
/// # fn main() {
/// use std::io::{Write, Cursor};
/// let mut writer = Cursor::new(Vec::new());
/// let res = parse_qcontent(b"\\q\\u\\o\\t\\e\\d-pairs. Other content.", &mut writer).unwrap();
/// // the content stop at the first space. Escape characters got removed.
/// assert_eq!(writer.get_ref(), b"quoted-pairs.");
/// // remaining bytes
/// assert_eq!(res.0, b" Other content.");
/// // bytes read
/// assert_eq!(res.1, b"\\q\\u\\o\\t\\e\\d-pairs.");
/// }
/// ```
pub fn parse_qcontent<'a, W: Write>(buf: &'a [u8], writer: &mut W) -> ParseResult<'a> {
    if buf.is_empty() {
        return Err(ErrorKind::Parsing.into());
    }
    let mut i: usize = 0;
    let mut last_write: usize = 0;
    while i < buf.len() {
        match buf[i] {
            // read a normal character
            c if is_valid_qtext(c) => i += 1,
            b'\\' => {
                // write whatever we parsed up to here
                writer.write_all(&buf[last_write..i])?;
                if i == buf.len() || buf[i + 1] > 127 {
                    return Err(ErrorKind::Parsing.into());
                }
                last_write = i + 1; // buf[i] is \, we want to skip it next time we write
                i += 2;
            }
            // we expect the quoted content to be at least one valid character.
            _ if i == 0 => return Err(ErrorKind::Parsing.into()),
            _ => break,
        }
    }
    writer.write_all(&buf[last_write..i])?;
    parse_ok(buf, i)
}

/// Read quoted content.
///
///
/// ```no_rust
/// qtext           =   %d33 /             ; Printable US-ASCII
///                     %d35-91 /          ;  characters not including
///                     %d93-126 /         ;  "\" or the quote character
///                     obs-qtext
/// obs-qtext       =   obs-NO-WS-CTL
/// obs-NO-WS-CTL   =   %d1-8 /            ; US-ASCII control
///                     %d11 /             ;  characters that do not
///                     %d12 /             ;  include the carriage
///                     %d14-31 /          ;  return, line feed, and
///                     %d127              ;  white space characters
///
/// qcontent        =   qtext / quoted-pair
/// ```
///
/// See [RFC5322 section 3.2.4](https://tools.ietf.org/html/rfc5322#section-3.2.4)
pub fn read_qcontent(buf: &[u8]) -> ParseResult {
    if buf.is_empty() {
        return Err(ErrorKind::Parsing.into());
    }
    let mut i: usize = 0;
    while i < buf.len() {
        match buf[i] {
            // read a normal character
            c if is_valid_qtext(c) => i += 1,
            b'\\' => {
                // we expect a quoted character between 0 and 127
                if i == buf.len() || buf[i + 1] > 127 {
                    return Err(ErrorKind::Parsing.into());
                } else {
                    i += 2;
                }
            }
            // we expect the quoted content to be at least one valid character.
            _ if i == 0 => return Err(ErrorKind::Parsing.into()),
            _ => break,
        }
    }
    parse_ok(buf, i)
}

/// Parse a quoted string, performing unfolding on folding whitespaces, and un-escaping quoted-pair
/// in the quoted string, and writing the parsed content into the provided writer. See [RFC5322
/// section 3.2.4](https://tools.ietf.org/html/rfc5322#section-3.2.4)
///
/// ```no_rust
/// qtext           =   %d33 /             ; Printable US-ASCII
///                     %d35-91 /          ;  characters not including
///                     %d93-126 /         ;  "\" or the quote character
///                     obs-qtext
///
/// qcontent        =   qtext / quoted-pair
///
/// quoted-string   =   [CFWS]
///                     DQUOTE *([FWS] qcontent) [FWS] DQUOTE
///                     [CFWS]
/// obs-NO-WS-CTL   =   %d1-8 /            ; US-ASCII control
///                     %d11 /             ;  characters that do not
///                     %d12 /             ;  include the carriage
///                     %d14-31 /          ;  return, line feed, and
///                     %d127              ;  white space characters
///
/// obs-qtext       =   obs-NO-WS-CTL
/// ```
///
/// A quoted-string is treated as a unit. That is, quoted-string is identical to atom,
/// semantically. Since a quoted-string is allowed to contain FWS, folding is permitted. Also note
/// that since quoted-pair is allowed in a quoted-string, the quote and backslash characters may
/// appear in a quoted-string so long as they appear as a quoted-pair.
///
/// Semantically, neither the optional CFWS outside of the quote characters nor the quote
/// characters themselves are part of the quoted-string; the quoted-string is what is contained
/// between the two quote characters. As stated earlier, the "\\" in any quoted-pair and the CRLF in
/// any FWS/CFWS that appears within the quoted-string are semantically "invisible" and therefore
/// not part of the quoted-string either.
///
/// # Examples
///
/// ```
/// # extern crate imf;
/// # use imf::quoted_string::parse_quoted_string;
/// # fn main() {
/// use std::io::{Write, Cursor};
///
/// let mut writer = Cursor::new(Vec::new());
/// let input = b" \t\r\n (comment)\"simple\r\n string\\\n\" (comment)\t ";
/// parse_quoted_string(&input[..], &mut writer).unwrap();
/// assert_eq!(&writer.get_ref()[..], &b"simple string\n"[..]);
/// # }
pub fn parse_quoted_string<'a, W: Write>(buf: &'a [u8], writer: &mut W) -> ParseResult<'a> {
    // read [CFWS]
    let mut i = match read_cfws(buf) {
        Ok((_, cfws)) => cfws.len(),
        Err(_) => 0,
    };

    // read "
    if i >= buf.len() {
        return Err(ErrorKind::Parsing.into());
    }
    if buf[i] != b'"' {
        return Err(ErrorKind::Parsing.into());
    } else {
        i += 1;
    }

    // read *([FWS] qcontent)
    while i < buf.len() {
        match replace_fws(&buf[i..], writer) {
            Ok((_, fws)) => i += fws.len(),
            Err(e) => match *e.kind() {
                ErrorKind::Io(_) => return Err(e),
                ErrorKind::Parsing => {} // ignore
            },
        }
        match parse_qcontent(&buf[i..], writer) {
            Ok((_, qcontent)) => i += qcontent.len(),
            Err(e) => match *e.kind() {
                ErrorKind::Io(_) => return Err(e),
                ErrorKind::Parsing => break,
            },
        }
    }

    // read "
    if i >= buf.len() || buf[i] != b'"' {
        return Err(ErrorKind::Parsing.into());
    } else {
        i += 1;
    }

    // read [CFWS]
    match read_cfws(&buf[i..]) {
        Ok((_, cfws)) => parse_ok(buf, i + cfws.len()),
        Err(_) => parse_ok(buf, i),
    }
}

pub fn read_quoted_string(buf: &[u8]) -> ParseResult {
    // read [CFWS]
    let mut i = match read_cfws(buf) {
        Ok((_, cfws)) => cfws.len(),
        Err(_) => 0,
    };

    // read "
    if i >= buf.len() {
        return Err(ErrorKind::Parsing.into());
    }
    if buf[i] != b'"' {
        return Err(ErrorKind::Parsing.into());
    } else {
        i += 1;
    }

    // read *([FWS] qcontent)
    while i < buf.len() {
        match read_fws(&buf[i..]) {
            Ok((_, fws)) => i += fws.len(),
            Err(e) => match *e.kind() {
                ErrorKind::Io(_) => return Err(e),
                ErrorKind::Parsing => {} // ignore
            },
        }
        match read_qcontent(&buf[i..]) {
            Ok((_, qcontent)) => i += qcontent.len(),
            Err(e) => match *e.kind() {
                ErrorKind::Io(_) => return Err(e),
                ErrorKind::Parsing => break,
            },
        }
    }

    // read "
    if i >= buf.len() || buf[i] != b'"' {
        return Err(ErrorKind::Parsing.into());
    } else {
        i += 1;
    }

    // read [CFWS]
    match read_cfws(&buf[i..]) {
        Ok((_, cfws)) => parse_ok(buf, i + cfws.len()),
        Err(_) => parse_ok(buf, i),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    #[test]
    fn test_quoted_pair() {
        assert_eq!(
            read_quoted_pair(&b"\\a"[..]).unwrap(),
            (&b""[..], &b"\\a"[..])
        );
        assert_eq!(
            read_quoted_pair(&b"\\\\"[..]).unwrap(),
            (&b""[..], &b"\\\\"[..])
        );
    }

    fn assert_quoted_string(input: &[u8], exp_parsed: &[u8], exp_left: &[u8], exp_read: &[u8]) {
        let mut writer = Cursor::new(Vec::new());
        let (left, read) = parse_quoted_string(input, &mut writer).unwrap();
        assert_eq!(&writer.get_ref()[..], exp_parsed);
        assert_eq!(left, exp_left);
        assert_eq!(read, exp_read);
    }

    #[test]
    fn test_parse_quoted_string() {
        assert_quoted_string(
            b"\"simple string\"".as_ref(),
            b"simple string".as_ref(),
            b"".as_ref(),
            b"\"simple string\"".as_ref(),
        );

        assert_quoted_string(
            b" \t\r\n \r\n \"simple string\" (comment)\t ".as_ref(),
            b"simple string".as_ref(),
            b"".as_ref(),
            b" \t\r\n \r\n \"simple string\" (comment)\t ".as_ref(),
        );

        assert_quoted_string(
            b"\"\\\"simple\\\" string\"".as_ref(),
            b"\"simple\" string".as_ref(),
            b"".as_ref(),
            b"\"\\\"simple\\\" string\"".as_ref(),
        );

        assert_quoted_string(
            b"\"\\\"simple\\\"\r\n string\"".as_ref(),
            b"\"simple\" string".as_ref(),
            b"".as_ref(),
            b"\"\\\"simple\\\"\r\n string\"".as_ref(),
        );

        assert_quoted_string(
            b"\"simple\\\nstring\"".as_ref(),
            b"simple\nstring".as_ref(),
            b"".as_ref(),
            b"\"simple\\\nstring\"".as_ref(),
        );
    }
}
