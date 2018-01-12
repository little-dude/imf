use errors::{parse_ok, Error, ErrorKind, ParseResult};
use std::io::Write;
use atom::{parse_atom, parse_dot_atom};
use whitespaces::{read_cfws, read_fws, replace_fws};
use quoted_string::{parse_quoted_string, read_quoted_string};
use common::parse_phrase as parse_display_name;
use common::{is_obs_no_ws_ctl, parse_word};

#[derive(Clone, Debug, Hash, PartialEq, Eq)]
pub struct Address {
    local_part: Vec<u8>,
    domain: Vec<u8>,
}

impl Address {
    pub fn parse(&mut self, buf: &[u8]) -> Result<Self, Error> {
        let mut local_part = Cursor::new(&mut self.local_part);
        let (_, read) = parse_new_local_part(buf, &mut local_part).or_else(|e| {
            match *e.kind() {
                ErrorKind::Parsing => {

                }
            }
    }
}

/// Parse the local part of an address as defined in
/// [RFC5322 section 3.4.1](https://tools.ietf.org/html/rfc5322#section-3.4.1).
///
///
/// ```no_rust
/// local-part      =   dot-atom / quoted-string / obs-local-part
/// dot-atom        =   [CFWS] dot-atom-text [CFWS]
/// dot-atom-text   =   1*atext *("." 1*atext)
/// atext           =   ALPHA / DIGIT /    ; Printable US-ASCII
///                     "!" / "#" /        ;  characters not including
///                     "$" / "%" /        ;  specials.  Used for atoms.
///                     "&" / "'" /
///                     "*" / "+" /
///                     "-" / "/" /
///                     "=" / "?" /
///                     "^" / "_" /
///                     "`" / "{" /
///                     "|" / "}" /
///                     "~"
/// obs-local-part  =   word *("." word)
/// word            =   atom / quoted-string
/// atom            =   [CFWS] 1*atext [CFWS]
/// ```
///
/// `obs-local-part` is more generic than `dot-atom/quoted-string`. It allows three more things:
///
/// - mixing atoms and quoted strings: `atom."string".atom`
/// - having whitespaces around dots `atom . another . atom`
/// - a combination of both: `atom . " string " . atom`
///
/// This parser parses the most laxist for (the `obs-local-part`), replacing any FWS or CFWS by a
/// single space.
fn parse_obsolete_local_part<'a, W: Write>(buf: &'a [u8], writer: &mut W) -> ParseResult<'a> {
    let (_, word) = parse_word(buf, writer)?;
    let mut i = word.len();
    while i < buf.len() {
        match parse_word(&buf[i..], writer) {
            Ok((_, word)) => i += word.len(),
            Err(e) => match *e.kind() {
                ErrorKind::Parsing => break,
                _ => return Err(e),
            },
        }
    }
    parse_ok(buf, i)
}

/// Parse the local part of an address as defined in
/// [RFC5322 section 3.4.1](https://tools.ietf.org/html/rfc5322#section-3.4.1).
///
/// ```no_rust
/// local-part      =   dot-atom / quoted-string / obs-local-part
/// dot-atom        =   [CFWS] dot-atom-text [CFWS]
/// dot-atom-text   =   1*atext *("." 1*atext)
/// atext           =   ALPHA / DIGIT /    ; Printable US-ASCII
///                     "!" / "#" /        ;  characters not including
///                     "$" / "%" /        ;  specials.  Used for atoms.
///                     "&" / "'" /
///                     "*" / "+" /
///                     "-" / "/" /
///                     "=" / "?" /
///                     "^" / "_" /
///                     "`" / "{" /
///                     "|" / "}" /
///                     "~"
/// obs-local-part  =   word *("." word)
/// word            =   atom / quoted-string
/// atom            =   [CFWS] 1*atext [CFWS]
/// ```
///
/// `obs-local-part` is more generic than `dot-atom/quoted-string`. It allows three more things:
///
/// - mixing atoms and quoted strings: `atom."string".atom`
/// - having whitespaces around dots `atom . another . atom`
/// - a combination of both: `atom . " string " . atom`
///
/// This parser parses the most strict form (the `dot-atom/quoted-string`), replacing any FWS or CFWS by a single space.
fn parse_new_local_part<'a, W: Write>(buf: &'a [u8], writer: &mut W) -> ParseResult<'a> {
    let (_, local_part) = parse_dot_atom(buf, writer).or_else(|e| match *e.kind() {
        ErrorKind::Parsing => parse_quoted_string(buf, writer),
        _ => Err(e),
    })?;
    parse_ok(buf, local_part.len())
}

// obs-domain      =   atom *("." atom)
// obs-dtext       =   obs-NO-WS-CTL / quoted-pair
// domain          =   dot-atom / domain-literal / obs-domain
// domain-literal  =   [CFWS] "[" *([FWS] dtext) [FWS] "]" [CFWS]
// dtext           =   %d33-90 /          ; Printable US-ASCII
//                     %d94-126 /         ;  characters not including
//                     obs-dtext          ;  "[", "]", or "\"
fn parse_obsolete_domain<'a, W: Write>(buf: &'a [u8], writer: &mut W) -> ParseResult<'a> {
    let (_, atom) = parse_atom(buf, writer)?;
    let mut i = atom.len();
    while i < buf.len() {
        let (_, atom) = parse_atom(&buf[i..], writer)?;
        i += atom.len();
    }
    parse_ok(buf, i)
}

// obs-domain      =   atom *("." atom)
// obs-dtext       =   obs-NO-WS-CTL / quoted-pair
// domain          =   dot-atom / domain-literal / obs-domain
// domain-literal  =   [CFWS] "[" *([FWS] dtext) [FWS] "]" [CFWS]
// dtext           =   %d33-90 /          ; Printable US-ASCII
//                     %d94-126 /         ;  characters not including
//                     obs-dtext          ;  "[", "]", or "\"
fn parse_new_domain<'a, W: Write>(buf: &'a [u8], writer: &mut W) -> ParseResult<'a> {
    match parse_dot_atom(buf, writer) {
        Ok((_, dot_atom)) => parse_ok(buf, dot_atom.len()),
        Err(e) => match *e.kind() {
            ErrorKind::Parsing => parse_domain_literal(buf, writer),
            _ => Err(e),
        },
    }
}

// obs-dtext       =   obs-NO-WS-CTL / quoted-pair
// domain-literal  =   [CFWS] "[" *([FWS] dtext) [FWS] "]" [CFWS]
// dtext           =   %d33-90 /          ; Printable US-ASCII
//                     %d94-126 /         ;  characters not including
//                     obs-dtext          ;  "[", "]", or "\"
fn parse_domain_literal<'a, W: Write>(buf: &'a [u8], writer: &mut W) -> ParseResult<'a> {
    // read [CFWS]
    let mut i = if let Ok((_, cfws)) = read_cfws(buf) {
        cfws.len()
    } else {
        0
    };

    // read "["
    if i >= buf.len() || buf[i] != b'[' {
        return Err(ErrorKind::Parsing.into());
    }
    i += 1;
    // read *([FWS] dtext)
    while i < buf.len() {
        // read [FWS]
        match replace_fws(&buf[i..], writer) {
            Ok((_, fws)) => i += fws.len(),
            Err(e) => match *e.kind() {
                ErrorKind::Parsing => {}
                _ => return Err(e),
            },
        }
        // read dtext
        match buf[i] {
            // regular character
            c if is_obs_no_ws_ctl(c) || (c >= 33 && c <= 90) || (c >= 94 && c <= 126) => {
                writer.write_all(&[c][..])?;
                i += 1;
            }
            // quoted-pair
            b'\\' => {
                if i + 1 < buf.len() && buf[i + 1] >= 0 && buf[i + 1] < 127 {
                    writer.write_all(&buf[i + 1..i + 2])?;
                    i += 2;
                } else {
                    return Err(ErrorKind::Parsing.into());
                }
            }
            // if we can't parse dtext, break out of the loop
            _ => break,
        }
    }
    // read [CFWS]
    if let Ok((_, cfws)) = read_cfws(&buf[i..]) {
        i += cfws.len();
    }

    // read "]" [CFWS]
    if i < buf.len() && buf[i] == b']' {
        if let Ok((_, cfws)) = read_cfws(&buf[i..]) {
            i += cfws.len();
        }
        parse_ok(buf, i)
    } else {
        Err(ErrorKind::Parsing.into())
    }
}

mod test {
    fn test_parse_domain
}


