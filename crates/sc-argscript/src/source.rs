//! Single-source macro frontend. The caller handles domain commands. All line
//! numbers are one-based; calls run from the outermost to the innermost site.
use crate::macros::{MacroError, MacroTable, ReplayError};
use crate::{Argument, Kind, ParseError, space, split_arguments};
use std::path::Path;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Site {
    pub line: usize,
    pub calls: Vec<usize>,
}

#[derive(Debug, PartialEq)]
pub struct Diagnostic<E> {
    pub site: Site,
    pub cause: Cause<E>,
}

#[derive(Debug, PartialEq)]
pub enum Cause<E> {
    /// Offset in the cleaned command.
    Syntax(ParseError),
    Macro(MacroError),
    Command(E),
    UnmatchedComment,
    UnclosedComment,
    UnclosedDefinition,
    UnexpectedEnddef,
    ExpectedNameAndArguments,
    UnsupportedVariable,
    UnsupportedDirective(String),
    UnsupportedByte,
    ExpansionLimit,
}

#[derive(Debug)]
pub enum FileError<E> {
    Read(std::io::Error),
    Parse(Diagnostic<E>),
}

/// Read a UTF-8 .er2 file; the supplied path identifies all diagnostic sites.
pub fn parse_file<E>(
    path: &Path,
    command: impl FnMut(&Site, &[Argument<'_>]) -> Result<(), E>,
) -> Result<(), FileError<E>> {
    let source = std::fs::read_to_string(path).map_err(FileError::Read)?;
    parse_source(&source, command).map_err(FileError::Parse)
}

struct Pending {
    name: String,
    params: String,
    body: Vec<(usize, String)>,
    opening: Site,
    replace: bool,
}

#[derive(Default)]
struct Reader {
    macros: MacroTable,
    pending: Option<Pending>,
    comments: Vec<Site>,
    replays: usize,
}

/// Handles define/redefine/enddef/create in one source, retaining callback
/// effects on failure. Other commands go to the caller for validation.
pub fn parse_source<E>(
    source: &str,
    mut command: impl FnMut(&Site, &[Argument<'_>]) -> Result<(), E>,
) -> Result<(), Diagnostic<E>> {
    let mut reader = Reader::default();
    let mut rest = source.strip_prefix('\u{feff}').unwrap_or(source);
    let mut line = 1;
    while !rest.is_empty() {
        let end = rest.find(['\r', '\n']).unwrap_or(rest.len());
        reader.line(
            &rest[..end],
            Site {
                line,
                calls: vec![],
            },
            &mut command,
        )?;
        rest = &rest[end..];
        if let Some(next) = rest.strip_prefix("\r\n") {
            rest = next;
        } else if !rest.is_empty() {
            rest = &rest[1..];
        }
        line += 1;
    }
    if let Some(site) = reader.comments.pop() {
        return Err(Diagnostic {
            site,
            cause: Cause::UnclosedComment,
        });
    }
    if let Some(pending) = reader.pending {
        return Err(Diagnostic {
            site: pending.opening,
            cause: Cause::UnclosedDefinition,
        });
    }
    Ok(())
}

impl Reader {
    // StripCommentsAndWhiteSpace (0x8A5200) shields # until the next
    // matching quote/backslash, including while scanning a block comment.
    fn clean<E>(&mut self, raw: &str, site: &Site) -> Result<String, Cause<E>> {
        let b = raw.as_bytes();
        if b.contains(&0) {
            return Err(Cause::UnsupportedByte);
        }
        let mut out = String::new();
        let mut i = 0;
        while i < b.len() {
            let start = i;
            match b[i] {
                b'#' => {
                    match b.get(i + 1) {
                        Some(b'<') => self.comments.push(site.clone()),
                        Some(b'>') => {
                            self.comments.pop().ok_or(Cause::UnmatchedComment)?;
                        }
                        _ => break,
                    }
                    i += 2;
                    continue;
                }
                b'"' | b'\'' | b'\\' => {
                    i = b[i + 1..]
                        .iter()
                        .position(|&v| v == b[start])
                        .map_or(b.len(), |n| start + n + 2);
                }
                _ => {
                    i += raw[i..].chars().next().unwrap().len_utf8();
                }
            }
            if self.comments.is_empty() {
                out.push_str(&raw[start..i]);
            }
        }
        Ok(out
            .trim_matches(|c: char| c.is_ascii() && space(c as u8))
            .to_owned())
    }

    fn line<E>(
        &mut self,
        raw: &str,
        site: Site,
        command: &mut impl FnMut(&Site, &[Argument<'_>]) -> Result<(), E>,
    ) -> Result<(), Diagnostic<E>> {
        let fail = |cause| Diagnostic {
            site: site.clone(),
            cause,
        };
        let clean = self.clean(raw, &site).map_err(fail)?;
        if clean.is_empty() {
            return Ok(());
        }
        if let Some(pending) = &mut self.pending {
            if clean.eq_ignore_ascii_case("enddef") {
                let p = self.pending.take().unwrap();
                self.macros
                    .define(&p.name, &p.params, p.body, p.replace)
                    .map_err(|e| fail(Cause::Macro(e)))?;
            } else {
                pending.body.push((site.line, clean));
            }
            return Ok(());
        }
        if clean.contains('$') {
            return Err(fail(Cause::UnsupportedVariable));
        }
        let args = split_arguments(&clean).map_err(|e| fail(Cause::Syntax(e)))?;
        let name = args[0].value.to_ascii_lowercase();
        match name.as_str() {
            "if" | "else" | "elseif" | "endif" | "include" | "sinclude" | "screate" | "set"
            | "seti" | "setu" | "setb" | "setf" | "namespace" | "undefine" | "sundefine"
            | "pushdir" | "popdir" => Err(fail(Cause::UnsupportedDirective(name))),
            "enddef" => Err(fail(Cause::UnexpectedEnddef)),
            "define" | "redefine" | "create" => {
                let main = &args[..args
                    .iter()
                    .position(|a| a.kind == Kind::Option)
                    .unwrap_or(args.len())];
                let [_, target, group] = main else {
                    return Err(fail(Cause::ExpectedNameAndArguments));
                };
                if name == "create" {
                    let (definition, values) = self
                        .macros
                        .prepare(target.value, group.value)
                        .map_err(|e| fail(Cause::Macro(e)))?;
                    let mut calls = site.calls.clone();
                    calls.push(site.line);
                    let body_site = |line| Site {
                        line,
                        calls: calls.clone(),
                    };
                    return definition
                        .replay(&values, |line, expanded| {
                            self.replays += 1;
                            if calls.len() > 32 || self.replays > 10_000 {
                                return Err(Diagnostic {
                                    site: body_site(line),
                                    cause: Cause::ExpansionLimit,
                                });
                            }
                            self.line(expanded, body_site(line), command)
                        })
                        .map_err(|error| match error {
                            ReplayError::Command { error, .. } => error,
                            ReplayError::Expansion { line, error } => Diagnostic {
                                site: body_site(line),
                                cause: Cause::Macro(error),
                            },
                            ReplayError::Call(error) => fail(Cause::Macro(error)),
                        });
                }
                if name == "define" && self.macros.contains(target.value) {
                    return Err(fail(Cause::Macro(MacroError::Duplicate)));
                }
                split_arguments(group.value)
                    .map_err(|e| fail(Cause::Macro(MacroError::Arguments(e))))?;
                self.pending = Some(Pending {
                    name: target.value.into(),
                    params: group.value.into(),
                    opening: site,
                    body: vec![],
                    replace: name == "redefine",
                });
                Ok(())
            }
            _ => command(&site, &args).map_err(|e| fail(Cause::Command(e))),
        }
    }
}
