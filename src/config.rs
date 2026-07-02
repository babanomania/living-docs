use std::fs;
use std::path::Path;

use anyhow::Context;
use serde::{Deserialize, Serialize};

/// `livingdocs.config.json`. Every field has a default, so a missing key
/// falls back to it rather than failing to parse.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    pub include: Vec<String>,
    pub exclude: Vec<String>,
    pub docs: String,
    pub model: ModelConfig,
    pub budget: BudgetConfig,
    pub output: OutputConfig,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            include: vec!["src/**".to_string()],
            exclude: vec!["**/*.test.ts".to_string(), "dist/**".to_string()],
            docs: "docs/".to_string(),
            model: ModelConfig::default(),
            budget: BudgetConfig::default(),
            output: OutputConfig::default(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ModelConfig {
    pub default: String,
    pub bulk: String,
}

impl Default for ModelConfig {
    fn default() -> Self {
        Self {
            default: "gpt-4.1".to_string(),
            bulk: "gpt-4o-mini".to_string(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct BudgetConfig {
    pub max_tokens: u32,
    pub max_findings_per_run: u32,
}

impl Default for BudgetConfig {
    fn default() -> Self {
        Self {
            max_tokens: 200_000,
            max_findings_per_run: 50,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct OutputConfig {
    pub mode: String,
    pub branch: String,
}

impl Default for OutputConfig {
    fn default() -> Self {
        Self {
            mode: "pr".to_string(),
            branch: "livingdocs/update".to_string(),
        }
    }
}

impl Config {
    pub fn load(path: &Path) -> anyhow::Result<Config> {
        let text = fs::read_to_string(path)
            .with_context(|| format!("failed to read config file at {}", path.display()))?;
        serde_json::from_str(&text)
            .with_context(|| format!("failed to parse config file at {}", path.display()))
    }

    pub fn write_default(path: &Path) -> anyhow::Result<()> {
        let json = serde_json::to_string_pretty(&Config::default())?;
        fs::write(path, format!("{json}\n"))
            .with_context(|| format!("failed to write config file at {}", path.display()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_match_spec_example() {
        let config = Config::default();
        assert_eq!(config.include, vec!["src/**"]);
        assert_eq!(config.docs, "docs/");
        assert_eq!(config.model.default, "gpt-4.1");
        assert_eq!(config.budget.max_tokens, 200_000);
        assert_eq!(config.output.branch, "livingdocs/update");
    }

    #[test]
    fn missing_fields_fall_back_to_defaults() {
        let config: Config = serde_json::from_str(r#"{"docs": "documentation/"}"#).unwrap();
        assert_eq!(config.docs, "documentation/");
        assert_eq!(config.include, Config::default().include);
        assert_eq!(config.model, ModelConfig::default());
    }
}
