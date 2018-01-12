use std::io::Write;
use Buffer;

use errors::{ErrorKind, Error, Token};
use whitespaces::{skip_cfws, replace_fws};

/// NULL character
pub static NULL: u8 = 0;

/// Delete character
pub static DEL: u8 = 127;

/// Return `true` is the character is a valid non-escaped character in quoted content
/// See [RFC5322 section 3.2.1](https://tools.ietf.org/html/rfc5322#section-3.2.1)
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
pub fn parse_qcontent<W: Write>(input: &Buffer, writer: &mut W) -> Result<usize, Error> {
    let bytes = input.remaining();
    let pos = input.position();
    if bytes.is_empty() {
        return Err(ErrorKind::Eof.into());
    }

    let mut i: usize = 0;
    let mut last_write: usize = 0;
    while i < bytes.len() {
        let c = bytes[i];

        if is_valid_qtext(c) {
            i += 1;
        } else if c == b'\\' {
            // write whatever we parsed up to here
            writer.write_all(&bytes[last_write..i])?;
            // TODO: this may be optimisable by first checking the positive case, ie:
            //
            // if i + 1 < bytes.len() && bytes[i + 1] <= 127 {
            //      // handle positive case, which is the most common
            //      continue;
            // }
            //  
            // if i + 1 == bytes.len() {
            //      // return EOF error
            // } else {
            //      // return invalid token error
            // }
            //
            // but that looks a bit weird and it's not said that the compiler does not optimize
            // this away for us anyway, so we need to benchmark that.
            if i + 1 == bytes.len() {
                // if there nothing after the \ whereas we're expecting an escaped character
                return Err(ErrorKind::Eof.into());
            } else if bytes[i + 1] > 127 {
                // this is not a valid escaped character
                return Err(ErrorKind::Token {
                    token: Token::QuotedString,
                    byte: bytes[i + 1],
                    position: pos + i + 1,
                }.into());
            } else {
                // bytes[i] is \, we want to skip it next time we write
                last_write = i + 1;
                i += 2;
                continue;
            }
        } else {
            break;
        }
    }

    if i == 0 {
        // we expect the quoted content to be at least one valid character
        return Err(ErrorKind::Token {
            token: Token::QuotedString,
            byte: bytes[0],
            position: pos,
        }.into());
    }

    writer.write_all(&bytes[last_write..i])?;
    Ok(i)
}

/// Skip quoted content.
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
pub fn skip_qcontent(input: &Buffer) -> Result<usize, Error> {
    let bytes = input.remaining();
    let pos = input.position();

    if bytes.is_empty() {
        return Err(ErrorKind::Eof.into());
    }

    let mut i: usize = 0;
    while i < bytes.len() {
        match bytes[i] {
            // read a normal character
            c if is_valid_qtext(c) => i += 1,
            b'\\' => {
                // we expect a quoted character between 0 and 127
                if i + 1 == bytes.len() {
                    // if there nothing after the \ whereas we're expecting an escaped character
                    return Err(ErrorKind::Eof.into());
                } else if bytes[i + 1] > 127 {
                    // this is not a valid escaped character
                    return Err(ErrorKind::Token {
                        token: Token::QuotedString,
                        byte: bytes[i + 1],
                        position: pos + i + 1,
                    }.into());
                } else {
                    i += 2;
                }
            }
            // we expect the quoted content to be at least one valid character.
            _ if i == 0 => return Err(ErrorKind::Token {
                token: Token::QuotedString,
                byte: bytes[0],
                position: pos,
            }.into()),
            _ => break,
        }
    }
    Ok(i)
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
pub fn parse_quoted_string<W: Write>(input: &Buffer, writer: &mut W) -> Result<usize, Error> {

    let bytes = input.remaining();
    let pos = input.position();
    let mut buffer = input.clone();

    // read [CFWS]
    let mut i = skip_cfws(&buffer).unwrap_or(0);

    // read "
    if i >= bytes.len() {
        return Err(ErrorKind::Eof.into());
    }

    if bytes[i] != b'"' {
        return Err(ErrorKind::Token {
            token: Token::QuotedString,
            byte: bytes[i],
            position: pos + i,
        }.into());
    }
    i += 1;
    buffer.set_position(pos + i);

    // read *([FWS] qcontent)
    while i < bytes.len() {
        match replace_fws(&buffer, writer) {
            Ok(len) => {
                i += len;
                buffer.set_position(pos + i);
            }
            Err(e) => if e.is_io() {
                return Err(e);
            }
        }
        match parse_qcontent(&buffer, writer) {
            Ok(len) => {
                i += len;
                buffer.set_position(pos + i);
            }
            Err(e) => {
                if e.is_io() {
                    return Err(e);
                } else {
                    break;
                }
            }
        }
    }

    // read "
    if i >= bytes.len() {
        return Err(ErrorKind::Eof.into());
    } else if bytes[i] != b'"' {
        return Err(ErrorKind::Token {
            token: Token::QuotedString,
            byte: bytes[i],
            position: pos + i,
        }.into());
    }

    i += 1;
    buffer.set_position(pos + i);

    // read [CFWS]
    if let Ok(len) = skip_cfws(&buffer) {
        Ok(i + len)
    } else {
        Ok(i)
    }
}

// #[cfg(test)]
// mod tests {
//     use super::*;
//     use std::io::Cursor;
// 
//     fn assert_quoted_string(input: &[u8], exp_parsed: &[u8], exp_left: &[u8], exp_read: &[u8]) {
//         let mut writer = Cursor::new(Vec::new());
//         let (left, read) = parse_quoted_string(input, &mut writer).unwrap();
//         assert_eq!(&writer.get_ref()[..], exp_parsed);
//         assert_eq!(left, exp_left);
//         assert_eq!(read, exp_read);
//     }
// 
//     #[test]
//     fn test_parse_quoted_string() {
//         assert_quoted_string(
//             b"\"simple string\"".as_ref(),
//             b"simple string".as_ref(),
//             b"".as_ref(),
//             b"\"simple string\"".as_ref(),
//         );
// 
//         assert_quoted_string(
//             b" \t\r\n \r\n \"simple string\" (comment)\t ".as_ref(),
//             b"simple string".as_ref(),
//             b"".as_ref(),
//             b" \t\r\n \r\n \"simple string\" (comment)\t ".as_ref(),
//         );
// 
//         assert_quoted_string(
//             b"\"\\\"simple\\\" string\"".as_ref(),
//             b"\"simple\" string".as_ref(),
//             b"".as_ref(),
//             b"\"\\\"simple\\\" string\"".as_ref(),
//         );
// 
//         assert_quoted_string(
//             b"\"\\\"simple\\\"\r\n string\"".as_ref(),
//             b"\"simple\" string".as_ref(),
//             b"".as_ref(),
//             b"\"\\\"simple\\\"\r\n string\"".as_ref(),
//         );
// 
//         assert_quoted_string(
//             b"\"simple\\\nstring\"".as_ref(),
//             b"simple\nstring".as_ref(),
//             b"".as_ref(),
//             b"\"simple\\\nstring\"".as_ref(),
//         );
//     }
// }
