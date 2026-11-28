#![forbid(unsafe_code)]
//! Argument splitting for a single preprocessed ArgScript command.
//! Based on development cArguments::SplitIntoArguments (0x8A4740).
//! Spans are half-open byte ranges in the supplied line, including delimiters.

use std::ops::Range;

pub mod macros;
pub mod source;

#[derive(Debug, PartialEq, Eq)]
pub enum Kind {
    Word,
    Quoted,
    Group,
    Option,
}

#[derive(Debug, PartialEq, Eq)]
pub struct Argument<'a> {
    pub kind: Kind,
    pub value: &'a str,
    pub span: Range<usize>,
}

#[derive(Debug, PartialEq, Eq)]
pub enum ErrorKind {
    UnexpectedClose,
    MissingClose,
    MissingQuote,
    EmptyOption,
    UnsupportedByte,
}

#[derive(Debug, PartialEq, Eq)]
pub struct ParseError {
    pub kind: ErrorKind,
    pub offset: usize,
}

fn space(b: u8) -> bool {
    b == b' ' || (b'\t'..=b'\r').contains(&b)
}

fn after_quote(bytes: &[u8], opening: usize) -> Result<usize, ParseError> {
    bytes[opening + 1..]
        .iter()
        .position(|&b| b == b'"')
        .map(|n| opening + n + 2)
        .ok_or(ParseError {
            kind: ErrorKind::MissingQuote,
            offset: opening,
        })
}

/// Returns all arguments in order, including the command and repeated options.
/// ASCII input only. Macro expansion and comment stripping precede this entry point.
///
/// ```
/// use sc_argscript::{split_arguments, Kind};
/// let argv = split_arguments("example (a b) -flag").unwrap();
/// assert_eq!(argv[1].value, "a b");
/// assert_eq!(argv[2].kind, Kind::Option);
/// ```
pub fn split_arguments(line: &str) -> Result<Vec<Argument<'_>>, ParseError> {
    let bytes = line.as_bytes();
    if let Some(offset) = bytes.iter().position(|&b| b == 0 || !b.is_ascii()) {
        return Err(ParseError {
            kind: ErrorKind::UnsupportedByte,
            offset,
        });
    }
    let mut args = Vec::new();
    let mut i = 0;
    while i < bytes.len() {
        if space(bytes[i]) {
            i += 1;
            continue;
        }
        let start = i;
        let (kind, value) = match bytes[i] {
            b'"' => {
                i = after_quote(bytes, start)?;
                (Kind::Quoted, &line[start + 1..i - 1])
            }
            b'(' => {
                let mut depth = 1usize;
                i += 1;
                while depth > 0 {
                    match bytes.get(i) {
                        None => {
                            return Err(ParseError {
                                kind: ErrorKind::MissingClose,
                                offset: start,
                            });
                        }
                        Some(b'"') => {
                            i = after_quote(bytes, i)?;
                            continue;
                        }
                        Some(b'(') => depth += 1,
                        Some(b')') => depth -= 1,
                        _ => {}
                    }
                    i += 1;
                }
                (Kind::Group, &line[start + 1..i - 1])
            }
            _ => {
                while i < bytes.len() && !space(bytes[i]) && !matches!(bytes[i], b'(' | b'"') {
                    if bytes[i] == b')' {
                        return Err(ParseError {
                            kind: ErrorKind::UnexpectedClose,
                            offset: i,
                        });
                    }
                    i += 1;
                }
                let word = &line[start..i];
                if word == "-" {
                    return Err(ParseError {
                        kind: ErrorKind::EmptyOption,
                        offset: start,
                    });
                }
                let option = word
                    .strip_prefix('-')
                    .and_then(|s| s.as_bytes().first())
                    .is_some_and(u8::is_ascii_alphabetic);
                (if option { Kind::Option } else { Kind::Word }, word)
            }
        };
        args.push(Argument {
            kind,
            value,
            span: start..i,
        });
    }
    Ok(args)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn preserves_native_boundaries() {
        let line = "cmd(a(\"()\"))\"\" -x -1 -x \"-x\"\x0b--x";
        let args = split_arguments(line).unwrap();
        assert_eq!(
            args.iter().map(|a| (&a.kind, a.value)).collect::<Vec<_>>(),
            [
                (&Kind::Word, "cmd"),
                (&Kind::Group, "a(\"()\")"),
                (&Kind::Quoted, ""),
                (&Kind::Option, "-x"),
                (&Kind::Word, "-1"),
                (&Kind::Option, "-x"),
                (&Kind::Quoted, "-x"),
                (&Kind::Word, "--x")
            ]
        );
        assert_eq!(&line[args[1].span.clone()], "(a(\"()\"))");
    }

    #[test]
    fn reports_first_error_position() {
        for (s, kind, offset) in [
            ("word)", ErrorKind::UnexpectedClose, 4),
            (" (a", ErrorKind::MissingClose, 1),
            ("(x \"a)", ErrorKind::MissingQuote, 3),
            ("ok -", ErrorKind::EmptyOption, 3),
            ("a\0b", ErrorKind::UnsupportedByte, 1),
            ("x时间", ErrorKind::UnsupportedByte, 1),
        ] {
            assert_eq!(split_arguments(s), Err(ParseError { kind, offset }));
        }
    }
}
