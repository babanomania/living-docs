use std::path::{Path, PathBuf};

use serde::Serialize;

pub mod typescript;

/// A node's location in its source file. Lines are 1-based to match
/// editors and CI annotations; bytes are 0-based, useful for slicing.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SourceRange {
    pub start_line: usize,
    pub end_line: usize,
    pub start_byte: usize,
    pub end_byte: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ClassNode {
    pub name: String,
    pub file: PathBuf,
    pub range: SourceRange,
    pub methods: Vec<String>,
    pub exported: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct FunctionNode {
    pub name: String,
    pub file: PathBuf,
    pub range: SourceRange,
    pub exported: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct InterfaceNode {
    pub name: String,
    pub file: PathBuf,
    pub range: SourceRange,
    pub exported: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ImportNode {
    pub file: PathBuf,
    pub range: SourceRange,
    /// The module specifier, e.g. `"./user-service"`, quotes stripped.
    pub source: String,
    /// Locally-bound names this import introduces (alias preferred over
    /// original name), plus `"* as x"` for namespace imports.
    pub specifiers: Vec<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct ParsedFile {
    pub classes: Vec<ClassNode>,
    pub functions: Vec<FunctionNode>,
    pub interfaces: Vec<InterfaceNode>,
    pub imports: Vec<ImportNode>,
}

/// Which grammar applies to a file, inferred from its extension.
/// `None` means the file is outside MVP language scope (TS/JS only).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Language {
    TypeScript,
    Tsx,
    JavaScript,
}

impl Language {
    pub fn from_path(path: &Path) -> Option<Language> {
        match path.extension().and_then(|e| e.to_str()) {
            Some("ts") => Some(Language::TypeScript),
            Some("tsx") => Some(Language::Tsx),
            Some("js") | Some("jsx") | Some("mjs") | Some("cjs") => Some(Language::JavaScript),
            _ => None,
        }
    }
}

/// Parse one file's source into its extracted symbols. `Ok(None)` means
/// the file's extension isn't a supported language, not an error.
pub fn parse_file(file: &Path, source: &str) -> anyhow::Result<Option<ParsedFile>> {
    let Some(language) = Language::from_path(file) else {
        return Ok(None);
    };
    typescript::parse(file, source, language).map(Some)
}
