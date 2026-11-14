use serde::{Deserialize, Serialize};

/// Stable diagnostic severity. UI layers can translate `code` without parsing
/// human-readable text.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Severity {
    Error,
    Warning,
    Info,
}

/// A machine-readable diagnostic emitted by the M0 schema and planner layers.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Diagnostic {
    pub severity: Severity,
    pub code: String,
    pub path: Option<String>,
    pub detail: String,
}

impl Diagnostic {
    pub fn error(code: impl Into<String>, path: Option<String>, detail: impl Into<String>) -> Self {
        Self {
            severity: Severity::Error,
            code: code.into(),
            path,
            detail: detail.into(),
        }
    }

    pub fn warning(
        code: impl Into<String>,
        path: Option<String>,
        detail: impl Into<String>,
    ) -> Self {
        Self {
            severity: Severity::Warning,
            code: code.into(),
            path,
            detail: detail.into(),
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct DiagnosticSet {
    pub items: Vec<Diagnostic>,
}

impl DiagnosticSet {
    pub fn push(&mut self, diagnostic: Diagnostic) {
        self.items.push(diagnostic);
    }

    pub fn has_errors(&self) -> bool {
        self.items
            .iter()
            .any(|item| item.severity == Severity::Error)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn diagnostics_keep_stable_codes_and_severity() {
        let mut set = DiagnosticSet::default();
        set.push(Diagnostic::warning(
            "schema.future_field",
            Some("semantics".into()),
            "field will be handled by a later processor",
        ));
        assert!(!set.has_errors());
        assert_eq!(set.items[0].code, "schema.future_field");
        assert_eq!(set.items[0].severity, Severity::Warning);
    }
}
