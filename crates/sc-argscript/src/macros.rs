//! Local macro instantiation, following cCreateCommand::Execute (0x8F90F0),
//! InstantiateMacro (0x8FDAF0) and SubstituteParameters (0x8FD0E0).
use crate::{ParseError, split_arguments};
use std::collections::HashMap;
use std::sync::Arc;

#[derive(Debug, PartialEq)]
pub enum MacroError {
    Arguments(ParseError),
    Duplicate,
    Undefined,
    Arity { expected: usize, actual: usize },
    MissingName(usize),
    InvalidName(usize),
    MissingBrace(usize),
    UnknownParameter(usize),
    UnsupportedByte(usize),
}

#[derive(Debug, PartialEq)]
pub enum ReplayError<E> {
    Call(MacroError),
    Expansion { line: usize, error: MacroError },
    Command { line: usize, error: E },
}

pub(crate) struct Definition {
    parameters: Vec<String>,
    body: Vec<(usize, String)>,
}

#[derive(Default)]
pub struct MacroTable {
    definitions: HashMap<String, Arc<Definition>>,
}

impl MacroTable {
    pub(crate) fn contains(&self, name: &str) -> bool {
        self.definitions.contains_key(&name.to_ascii_lowercase())
    }
    /// Commit a completed definition. Supply physical line numbers and cleaned body text.
    pub fn define(
        &mut self,
        name: &str,
        parameters: &str,
        body: Vec<(usize, String)>,
        replace_existing: bool,
    ) -> Result<(), MacroError> {
        let key = name.to_ascii_lowercase();
        if !replace_existing && self.definitions.contains_key(&key) {
            return Err(MacroError::Duplicate);
        }
        let parameters = split_arguments(parameters)
            .map_err(MacroError::Arguments)?
            .into_iter()
            .map(|arg| arg.value.to_owned())
            .collect();
        self.definitions
            .insert(key, Arc::new(Definition { parameters, body }));
        Ok(())
    }

    /// Pass the contents of the call's argument group. The caller parses each
    /// generated line immediately; a failure leaves earlier callbacks in effect.
    pub fn create<E>(
        &self,
        name: &str,
        arguments: &str,
        parse_line: impl FnMut(usize, &str) -> Result<(), E>,
    ) -> Result<(), ReplayError<E>> {
        let (definition, values) = self.prepare(name, arguments).map_err(ReplayError::Call)?;
        definition.replay(&values, parse_line)
    }

    pub(crate) fn prepare<'a>(
        &self,
        name: &str,
        arguments: &'a str,
    ) -> Result<(Arc<Definition>, Vec<crate::Argument<'a>>), MacroError> {
        let values = split_arguments(arguments).map_err(MacroError::Arguments)?;
        let definition = self
            .definitions
            .get(&name.to_ascii_lowercase())
            .ok_or(MacroError::Undefined)?;
        if values.len() != definition.parameters.len() {
            return Err(MacroError::Arity {
                expected: definition.parameters.len(),
                actual: values.len(),
            });
        }
        Ok((Arc::clone(definition), values))
    }
}

impl Definition {
    pub(crate) fn replay<E>(
        &self,
        values: &[crate::Argument<'_>],
        mut parse_line: impl FnMut(usize, &str) -> Result<(), E>,
    ) -> Result<(), ReplayError<E>> {
        for (line, body) in &self.body {
            let expanded = expand(body, &self.parameters, values)
                .map_err(|error| ReplayError::Expansion { line: *line, error })?;
            parse_line(*line, &expanded)
                .map_err(|error| ReplayError::Command { line: *line, error })?;
        }
        Ok(())
    }
}

fn expand(
    body: &str,
    parameters: &[String],
    values: &[crate::Argument<'_>],
) -> Result<String, MacroError> {
    let bytes = body.as_bytes();
    if let Some(offset) = bytes.iter().position(|&b| b == 0 || !b.is_ascii()) {
        return Err(MacroError::UnsupportedByte(offset));
    }
    let mut output = String::with_capacity(body.len());
    let mut cursor = 0;
    while let Some(rel) = body[cursor..].find('&') {
        let amp = cursor + rel;
        output.push_str(&body[cursor..amp]);
        let raw = bytes.get(amp + 1) == Some(&b'{');
        let start = amp + 1 + usize::from(raw);
        let first = bytes.get(start).ok_or(MacroError::MissingName(amp))?;
        if !first.is_ascii_alphabetic() && *first != b'_' {
            return Err(MacroError::InvalidName(start));
        }
        let mut end = start + 1;
        while bytes
            .get(end)
            .is_some_and(|b| b.is_ascii_alphanumeric() || *b == b'_')
        {
            end += 1;
        }
        if raw && bytes.get(end) != Some(&b'}') {
            return Err(MacroError::MissingBrace(amp));
        }
        let index = parameters
            .iter()
            .position(|p| p == &body[start..end])
            .ok_or(MacroError::UnknownParameter(amp))?;
        if !raw {
            output.push('(');
        }
        output.push_str(values[index].value);
        if !raw {
            output.push(')');
        }
        cursor = end + usize::from(raw);
    }
    output.push_str(&body[cursor..]);
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn arguments_are_split_before_lookup() {
        let result = MacroTable::default().create("missing", "-", |_, _| Ok::<_, ()>(()));
        assert!(matches!(
            result,
            Err(ReplayError::Call(MacroError::Arguments(_)))
        ));
    }

    #[test]
    fn single_pass_replay_retains_prior_effects_and_first_duplicate() {
        let mut table = MacroTable::default();
        table
            .define(
                "Sample",
                "x x",
                vec![
                    (4, "emit &x &{x}_end".into()),
                    (6, "bad &X".into()),
                    (9, "unused".into()),
                ],
                false,
            )
            .unwrap();
        let mut seen = vec![];
        let result = table.create("sample", "\"a &b\" -flag", |line, text| {
            seen.push((line, text.to_owned()));
            Ok::<_, ()>(())
        });
        assert_eq!(seen, [(4, "emit (a &b) a &b_end".into())]);
        assert_eq!(
            result,
            Err(ReplayError::Expansion {
                line: 6,
                error: MacroError::UnknownParameter(4)
            })
        );
    }

    #[test]
    fn replay_stops_at_first_command_failure() {
        let mut table = MacroTable::default();
        table
            .define(
                "M",
                "",
                vec![(5, "bad".into()), (6, "&unknown".into())],
                false,
            )
            .unwrap();
        let result = table.create("m", "", |_, _| Err("command error"));
        assert_eq!(
            result,
            Err(ReplayError::Command {
                line: 5,
                error: "command error"
            })
        );
    }
}
