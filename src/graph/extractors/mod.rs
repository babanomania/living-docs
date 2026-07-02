use std::path::{Path, PathBuf};

use crate::parser::{Language, SourceRange};

pub mod express;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExtractedRoute {
    /// Uppercase HTTP method, e.g. "GET".
    pub method: String,
    /// The route path as written, e.g. "/users/:id".
    pub path: String,
    pub file: PathBuf,
    pub range: SourceRange,
    /// Best-effort handler name; `None` for inline/anonymous handlers.
    pub handler: Option<String>,
}

/// A pluggable route extractor for one web framework. Express is the only
/// MVP implementation (CLAUDE.md scope discipline); Fastify, NestJS, and
/// Spring arrive in Phase 8 behind this same trait.
pub trait RouteExtractor {
    /// Framework name, stored alongside each extracted route.
    fn name(&self) -> &'static str;

    fn extract(
        &self,
        file: &Path,
        source: &str,
        language: Language,
    ) -> anyhow::Result<Vec<ExtractedRoute>>;
}
