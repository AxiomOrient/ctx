use serde::Deserialize;
use std::{fs, path::Path};

#[derive(Debug, Deserialize, Clone)]
pub struct Defaults { pub budget: usize, pub min_confidence: f32, pub mmr_lambda: f32, pub reply_max: usize }

#[derive(Debug, Deserialize, Clone)]
pub struct Paths { pub index_dir: String, pub cache_dir: String, pub ontology: String, pub rules: String }

#[derive(Debug, Deserialize, Clone)]
pub struct AiCli { pub cmd: String, pub args: Vec<String>, pub timeout_ms: u64 }

#[derive(Debug, Deserialize, Clone)]
pub struct AiOpenAi { pub api_key_env: String, pub model: String, pub timeout_ms: u64 }

#[derive(Debug, Deserialize, Clone)]
pub struct Ai { pub order: Vec<String>, #[serde(default)] pub claude_cli: Option<AiCli>, #[serde(default)] pub gemini_cli: Option<AiCli>, #[serde(default)] pub openai: Option<AiOpenAi> }

#[derive(Debug, Deserialize, Clone)]
pub struct Cfg { pub defaults: Defaults, pub paths: Paths, pub ai: Ai }

impl Cfg {
  pub fn load(p: impl AsRef<Path>) -> anyhow::Result<Self> {
    let s = fs::read_to_string(p)?;
    Ok(toml::from_str(&s)?)
  }
}
