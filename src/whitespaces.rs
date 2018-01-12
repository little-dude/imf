use errors::{ErrorKind, Error, Token};
use Buffer;
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
/// These definitions are equivalent to this simpler one:
///
/// ```no_rust
/// FWS = (1*WSP *(CRLF 1*WSP)) / 1*(CRLF 1*WSP)
/// ```
///
/// [RFC5322 section 2.2.3]: https://tools.ietf.org/html/rfc5322#section-2.2.3
pub fn skip_fws(input: &Buffer) -> Result<usize, Error> {
    let bytes = input.remaining();
    if bytes.is_empty() {
        return Err(ErrorKind::Eof.into());
    }

    let mut i: usize = 0;
    while i < bytes.len() {
        match bytes[i] {
            // whitespace
            c if is_wsp(c) => i += 1,
            // CRLF
            b'\r' => {
                // we need to match LF and then a space
                if i + 2 < bytes.len() && bytes[i + 1] == b'\n' && is_wsp(bytes[i + 2]) {
                    i += 3;
                } else {
                    break;
                }
            }
            _ => break,
        }
    }
    if i == 0 {
        return Err(From::from(ErrorKind::Token {
            token: Token::Fws,
            byte: bytes[0],
            position: input.position(),
        }));
    }
    Ok(i)
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
pub fn unfold_fws<W: Write>(input: &Buffer, writer: &mut W) -> Result<usize, Error> {
    let bytes = input.remaining();
    if bytes.is_empty() {
        return Err(ErrorKind::Eof.into());
    }
    let mut i: usize = 0;
    let mut next_write: usize = 0;
    while i < bytes.len() {
        match bytes[i] {
            // whitespace
            c if is_wsp(c) => i += 1,
            // CRLF
            b'\r' => {
                writer.write_all(&bytes[next_write..i])?;
                // we need to match LF and then a space
                if i + 2 < bytes.len() && bytes[i + 1] == b'\n' && is_wsp(bytes[i + 2]) {
                    next_write = i + 2;
                    i += 3;
                } else {
                    break;
                }
            }
            _ => break,
        }
    }
    if i == 0 {
        return Err(From::from(ErrorKind::Token {
            token: Token::Fws,
            byte: bytes[0],
            position: input.position(),
        }));
    }
    writer.write_all(&bytes[next_write..i])?;
    Ok(i)
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
pub fn skip_comment(input: &Buffer) -> Result<usize, Error> {
    let bytes = input.remaining();
    if bytes.is_empty() {
        return Err(ErrorKind::Eof.into());
    }
    if bytes[0] != b'(' {
        return Err(From::from(ErrorKind::Token {
            token: Token::Comment,
            byte: bytes[0],
            position: input.position(),
        }));
    }
    // comments can be nested. Since we already found an opening parenthesis, we start at 1.
    let mut nested_level = 1;

    let mut i: usize = 1;
    while i < bytes.len() {
        match bytes[i] {
            b'\\' => {
                // we want to ignore the next character, since it's escaped
                i += 2;
                continue;
            }
            b')' => {
                nested_level -= 1;
                if nested_level == 0 {
                    return Ok(i+1);
                }
            }
            b'(' => nested_level += 1,
            // ignore any other character
            _ => {}
        }
        i += 1
    }
    // we reached the end of the buffer without seeing the closing parenthesis
    Err(ErrorKind::Eof.into())
}

/// Read CFWS. See [RFC5322 section 3.2.3].
///
/// ```no_rust
/// CFWS            =       *([FWS] comment) (([FWS] comment) / FWS)
/// ```
///
/// [RFC5322 section 3.2.3]: https://tools.ietf.org/html/rfc5322#section-3.2.3
pub fn skip_cfws(input: &Buffer) -> Result<usize, Error> {
    let bytes = input.remaining();
    if bytes.is_empty() {
        return Err(ErrorKind::Eof.into());
    }

    let pos = input.position();
    let mut buffer = input.clone();
    let mut i: usize = 0;

    while i < bytes.len() {
        // read a FWS
        buffer.set_position(pos + i);
        if let Ok(len) = skip_fws(&buffer) {
            i += len;
        }

        // read a comment
        buffer.set_position(pos + i);
        match skip_comment(&buffer) {
            Ok(len) => i += len,
            Err(_) => {
                if i == 0 {
                    // We're supposed to read at least one comment or one FWS
                    // So it's an error not to read anything
                    return Err(From::from(ErrorKind::Token {
                        token: Token::Cfws,
                        byte: bytes[0],
                        position: input.position(),
                    }));
                } else {
                    return Ok(i);
                }
            }
        }
    }

    assert!(i == bytes.len());
    Err(ErrorKind::Eof.into())
}

pub fn replace_cfws<W: Write>(input: &Buffer, writer: &mut W) -> Result<usize, Error> {
    let len = skip_cfws(input)?;
    // If we're here, then we read a CFWS. Let's replace it by a single space.
    writer.write_all(&b" "[..])?;
    Ok(len)
}

pub fn replace_fws<W: Write>(input: &Buffer, writer: &mut W) -> Result<usize, Error> {
    let len = skip_fws(input)?;
    // If we're here, then we read a CFWS. Let's replace it by a single space.
    writer.write_all(&b" "[..])?;
    Ok(len)
}

/// Unfold CFWS. See [RFC5322 section 3.2.3].
///
/// ```no_rust
/// CFWS            =       *([FWS] comment) (([FWS] comment) / FWS)
/// ```
///
/// [RFC5322 section 3.2.3]: https://tools.ietf.org/html/rfc5322#section-3.2.3
pub fn unfold_cfws<W: Write>(input: &Buffer, writer: &mut W) -> Result<usize, Error> {
    let bytes = input.remaining();
    if bytes.is_empty() {
        return Err(ErrorKind::Eof.into());
    }

    let pos = input.position();
    let mut buffer = input.clone();
    let mut i: usize = 0;

    while i < bytes.len() {
        buffer.set_position(pos + i);
        match unfold_fws(&buffer, writer) {
            Ok(len) => i += len,
            Err(e) => {
                match *e.kind() {
                    // ignore parsing errors, since this token is not mandatory
                    ErrorKind::Token { .. } | ErrorKind::Eof => {}
                    // propagate the other errors
                    _ => return Err(e),
                }
            }
        }

        buffer.set_position(pos + i);
        match skip_comment(&buffer) {
            Ok(len) => i += len,
            Err(_) => {
                if i == 0 {
                    // We're supposed to read at least one comment or one FWS
                    // So it's an error not to read anything
                    return Err(From::from(ErrorKind::Token {
                        token: Token::Cfws,
                        byte: bytes[0],
                        position: pos,
                    }));
                } else {
                    return Ok(i);
                }
            }
        }
    }

    assert!(i > 0);
    Ok(i)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_comment() {
        ok!(skip_comment, b"()", 2);
        ok!(skip_comment, b"(abc)", 5);
        ok!(skip_comment, b"(a comment)abc", 11);
        ok!(skip_comment, b"(a (nested (comment)))abc", 22);
        ok!(skip_comment, b"(a (nested \\((comment)\\)\\)))abc", 28);
        // negative tests
        eof!(skip_comment, b"(");
        eof!(skip_comment, b"(comment\\)");
        eof!(skip_comment, b"(comment()comment");
        tok!(skip_comment, b"fail", Token::Comment, b'f', 0);
        tok!(skip_comment, b"\\(", Token::Comment, b'\\', 0);
    }

    #[test]
    fn test_folding_whitespace() {
        ok!(skip_fws, b" ", 1);
        ok!(skip_fws, b" \t", 2);
        ok!(skip_fws, b" abc", 1);
        ok!(skip_fws, b"\tabc", 1);
        ok!(skip_fws, b"\t abc", 2);
        ok!(skip_fws, b" \r\n abc", 4);
        ok!(skip_fws, b" \r\n \r\n\tabc", 7);
        ok!(skip_fws, b" \r\nabc", 1);
        ok!(skip_fws, b" \r\n  \r\n \r\nabc", 8);
        ok!(skip_fws, b"\r\n   abc", 5);
        ok!(skip_fws, b"\r\n \t  abc", 6);
        // ideally it should fail at index 2 on the "a"
        tok!(skip_fws, b"\r\nabc", Token::Fws, b'\r', 0);
        // ideally it should fail at index 1 on the " "
        tok!(skip_fws, b"\r abc", Token::Fws, b'\r', 0);
        tok!(skip_fws, b"\n\r abc", Token::Fws, b'\n', 0);
    }

    #[test]
    fn test_cfws() {
        ok!(skip_cfws, b" ", 1);
        ok!(skip_cfws, b" \t", 2);
        ok!(skip_cfws, b" abc", 1);
        ok!(skip_cfws, b"\tabc", 1);
        ok!(skip_cfws, b"\t abc", 2);
        ok!(skip_cfws, b" \r\n abc", 4);
        ok!(skip_cfws, b" \r\n \r\n\tabc", 7);
        ok!(skip_cfws, b" \r\nabc", 1);
        ok!(skip_cfws, b" \r\n  \r\n \r\nabc", 8);
        ok!(skip_cfws, b"\r\n   abc", 5);
        ok!(skip_cfws, b"\r\n \t  abc", 6);
        tok!(skip_cfws, b"\r\nabc", Token::Cfws, b'\r', 0);
        tok!(skip_cfws, b"\r abc", Token::Cfws, b'\r', 0);
        tok!(skip_cfws, b"\n\r abc", Token::Cfws, b'\n', 0);
        ok!(skip_cfws, b"(a comment)abc", 11);
        ok!(skip_cfws, b"(a (nested (comment)))abc", b"(a (nested (comment)))".len());
        ok!(skip_cfws, b"(a (nested \\((comment)\\)\\)))abc", 28);
        ok!(skip_cfws, b"  (a comment)  abc", 15);
        ok!(skip_cfws, b"(a comment)  abc", 13);
        ok!(skip_cfws, b"  (a comment)abc", 13);
        ok!(skip_cfws, b"  (  a comment ( ) ()\r\n)  abc", 26);
        ok!(skip_cfws, b"(a comment)  () ()abc", 18);
    }
}
