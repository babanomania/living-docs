use serde::Serialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Severity {
    Error,
    Warning,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Finding {
    pub file: String,
    pub line: usize,
    pub rule: String,
    pub severity: Severity,
    pub message: String,
}

/// `<file>:<line>  <rule>  <message>`, matching the format shown in
/// CLAUDE.md's hero-feature example.
pub fn format_text(findings: &[Finding]) -> String {
    findings
        .iter()
        .map(|f| format!("{}:{}  {}  {}", f.file, f.line, f.rule, f.message))
        .collect::<Vec<_>>()
        .join("\n")
}

pub fn format_json(findings: &[Finding]) -> anyhow::Result<String> {
    Ok(serde_json::to_string_pretty(findings)?)
}
