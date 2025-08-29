use serde::{Deserialize, Serialize};
#[cfg(feature = "mcp_sdk")]
use rmcp::schemars as schemars;
use std::collections::HashMap;
use std::path::PathBuf;

// As seen in `doc::schema::document` and `doc::parse::section`
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ContextDocument {
    pub id: String,
    pub title: String,
    pub r#type: String,
    pub version: String,
    pub schema: String,
    pub author: Option<String>,
    pub updated: Option<String>,
    pub domain: Option<String>,
    pub tags: Vec<String>,
    pub sections: Vec<SectionDef>,
    pub estimated_tokens: Option<u32>,
    #[serde(skip)]
    pub path: Option<PathBuf>,
    // Added missing fields
    pub facets: HashMap<String, Vec<String>>,
    pub locale: Option<String>,
    pub trust: Option<f32>,
    pub freshness: Option<String>,
    pub aliases: Option<Vec<String>>,
    pub conflicts: Option<Vec<String>>,
    pub dependencies: Option<Vec<String>>,
}

impl ContextDocument {
    // Helper for tests
    pub fn new_test(id: &str, title: &str) -> Self {
        Self {
            id: id.to_string(),
            title: title.to_string(),
            version: "1.0.0".to_string(),
            schema: "context.v1".to_string(),
            ..Default::default()
        }
    }

    pub fn new(id: String, title: String) -> Self {
        Self {
            id,
            title,
            version: "1.0.0".to_string(),
            schema: "context.v1".to_string(),
            ..Default::default()
        }
    }

    pub fn validate_schema(&self) -> Result<(), crate::domain::errors::ContextError> {
        // Basic validation logic can be added here if needed
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SectionDef {
    pub id: String,
    pub name: String,
    pub marker: String,
    pub priority: u8,
    pub tokens: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtractedSection {
    pub id: String,
    pub name: String,
    pub content: String,
    pub tokens: u32,
    pub priority: u8,
    pub score: f32,
}

// As seen in `doc::parse::frontmatter`
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContextMetadata {
    pub title: String,
    pub version: String,
    pub sections: Vec<SectionDef>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BuildQuery {
    pub repo: String,
    pub branch: String,
    pub commit_sha: String,
    pub lang: Option<String>,
    pub maturity: Option<String>,
    pub required_facets: Vec<(String, Vec<String>)>,
    pub axes: Option<Vec<String>>,
    pub query_text: Option<String>,
    pub include_sources: Option<Vec<String>>,
    pub confidence_threshold: f32,
}

impl BuildQuery {
    pub fn new(repo: String, branch: String, commit_sha: String) -> Self {
        Self {
            repo,
            branch,
            commit_sha,
            lang: None,
            maturity: None,
            required_facets: Vec::new(),
            axes: None,
            query_text: None,
            include_sources: None,
            confidence_threshold: 0.0,
        }
    }
    pub fn with_query_text(mut self, text: String) -> Self {
        self.query_text = Some(text);
        self
    }
    pub fn with_confidence_threshold(mut self, threshold: f32) -> Self {
        self.confidence_threshold = threshold;
        self
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DocumentCandidate {
    pub doc_id: String,
    pub sha: String,
    pub title: String,
    pub locale: String,
    pub trust: f32,
    pub freshness: String,
    pub confidence: f32,
    pub path: String,
    pub facets: std::collections::HashMap<String, Vec<String>>,
    pub tokens: usize,
    pub content: Option<String>,
}

impl DocumentCandidate {
    pub fn days_since_freshness(&self) -> Option<i64> {
        use chrono::{DateTime, Utc};
        
        // Try to parse the freshness string as an ISO 8601 datetime
        if let Ok(dt) = DateTime::parse_from_rfc3339(&self.freshness) {
            let now = Utc::now();
            let freshness_utc = dt.with_timezone(&Utc);
            let duration = now.signed_duration_since(freshness_utc);
            Some(duration.num_days())
        } else {
            // If parsing fails, return None
            None
        }
    }

    pub fn facet_set(&self) -> std::collections::HashSet<String> {
        let mut set = std::collections::HashSet::new();
        for (key, values) in &self.facets {
            for value in values {
                set.insert(format!("{}:{}", key, value));
            }
        }
        set
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct CompositionResult {
    pub merged_document: MergedDocument,
    pub execution_id: String,
    pub confidence_score: f32,
    pub selection_rationale: String,
    pub rejected_documents: Vec<DocumentCandidate>,
}

impl CompositionResult {
    pub fn new(merged_document: MergedDocument, execution_id: String, confidence_score: f32) -> Self {
        Self {
            merged_document,
            execution_id,
            confidence_score,
            ..Default::default()
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct MergedDocument {
    pub content: String,
    pub source_documents: Vec<String>,
    pub tokens: usize,
    pub sections: Vec<MergedSection>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MergedSection {
    pub title: String,
    pub content: String,
    pub source_doc_id: String,
    pub confidence: f32,
}

impl MergedDocument {
    pub fn empty() -> Self {
        Self {
            content: String::new(),
            source_documents: Vec::new(),
            tokens: 0,
            sections: Vec::new(),
        }
    }
}

// Types for scorer module
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ScoreBreakdown {
    pub keyword_score: f32,
    pub freshness_score: f32,
    pub trust_score: f32,
    pub confidence_score: f32,
    pub facet_coverage_score: f32,
    pub axes_match_score: f32,
}

#[derive(Debug, Clone)]
pub struct ScoredCandidate {
    pub candidate: DocumentCandidate,
    pub base_score: f32,
    pub score_breakdown: ScoreBreakdown,
    pub keyword_score: f32,
    pub freshness_score: f32,
    pub trust_score: f32,
    pub confidence_score: f32,
    pub facet_coverage_score: f32,
    pub axes_match_score: f32,
}

impl ScoredCandidate {
    pub fn new(candidate: DocumentCandidate) -> Self {
        Self {
            candidate,
            base_score: 0.0,
            score_breakdown: ScoreBreakdown::default(),
            keyword_score: 0.0,
            freshness_score: 0.0,
            trust_score: 0.0,
            confidence_score: 0.0,
            facet_coverage_score: 0.0,
            axes_match_score: 0.0,
        }
    }
    pub fn calculate_base_score(&mut self, weights: &ScoringWeights) {
        self.base_score = self.keyword_score * weights.keyword
            + self.freshness_score * weights.freshness
            + self.trust_score * weights.trust
            + self.confidence_score * weights.confidence
            + self.facet_coverage_score * weights.facet_coverage
            + self.axes_match_score * weights.axes_match;
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScoringWeights {
    pub keyword: f32,
    pub freshness: f32,
    pub trust: f32,
    pub confidence: f32,
    pub facet_coverage: f32,
    pub axes_match: f32,
}

impl Default for ScoringWeights {
    fn default() -> Self {
        Self {
            keyword: 0.4,
            freshness: 0.15,
            trust: 0.15,
            confidence: 0.1,
            facet_coverage: 0.1,
            axes_match: 0.1,
        }
    }
}

// ===== PLAN.md API Types =====

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComposeInput {
    pub query: String,
    pub tags: Option<Vec<String>>,
    pub max_tokens: Option<u32>,
    pub exclude_sources: Option<Vec<String>>,
    pub priority_sources: Option<Vec<String>>,
}

impl ComposeInput {
    pub fn new(query: String) -> Self {
        Self {
            query,
            tags: None,
            max_tokens: None,
            exclude_sources: None,
            priority_sources: None,
        }
    }

    pub fn with_max_tokens(mut self, max_tokens: u32) -> Self {
        self.max_tokens = Some(max_tokens);
        self
    }

    pub fn with_tags(mut self, tags: Vec<String>) -> Self {
        self.tags = Some(tags);
        self
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkInput {
    pub task: String,
    pub context: Option<String>,
    pub ai_provider: Option<String>,
    pub max_tokens: Option<u32>,
    pub temperature: Option<f32>,
}

impl WorkInput {
    pub fn new(task: String) -> Self {
        Self {
            task,
            context: None,
            ai_provider: None,
            max_tokens: None,
            temperature: None,
        }
    }

    pub fn with_context(mut self, context: String) -> Self {
        self.context = Some(context);
        self
    }

    pub fn with_ai_provider(mut self, provider: String) -> Self {
        self.ai_provider = Some(provider);
        self
    }
}

#[cfg_attr(feature = "mcp_sdk", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkOutput {
    pub response: AiResponse,
    pub context_used: PromptBundle,
    pub execution_id: String,
}

impl WorkOutput {
    pub fn new(response: AiResponse, context_used: PromptBundle, execution_id: String) -> Self {
        Self {
            response,
            context_used,
            execution_id,
        }
    }
}

#[cfg_attr(feature = "mcp_sdk", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PromptBundle {
    pub prompt: String,
    pub context: String,
    pub tokens: u32,
    pub sources: Vec<String>,
    pub confidence: f32,
}

impl PromptBundle {
    pub fn new(prompt: String, context: String, tokens: u32) -> Self {
        Self {
            prompt,
            context,
            tokens,
            sources: Vec::new(),
            confidence: 0.0,
        }
    }

    pub fn with_sources(mut self, sources: Vec<String>) -> Self {
        self.sources = sources;
        self
    }

    pub fn with_confidence(mut self, confidence: f32) -> Self {
        self.confidence = confidence;
        self
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiPrompt {
    pub content: String,
    pub role: String,
    pub timestamp: String,
}

impl AiPrompt {
    pub fn user(content: String) -> Self {
        Self {
            content,
            role: "user".to_string(),
            timestamp: chrono::Utc::now().to_rfc3339(),
        }
    }

    pub fn system(content: String) -> Self {
        Self {
            content,
            role: "system".to_string(),
            timestamp: chrono::Utc::now().to_rfc3339(),
        }
    }
}

#[cfg_attr(feature = "mcp_sdk", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiResponse {
    pub content: String,
    pub model: String,
    pub tokens_used: u32,
    pub finish_reason: String,
    pub timestamp: String,
}

impl AiResponse {
    pub fn new(content: String, model: String, tokens_used: u32) -> Self {
        Self {
            content,
            model,
            tokens_used,
            finish_reason: "stop".to_string(),
            timestamp: chrono::Utc::now().to_rfc3339(),
        }
    }

    pub fn with_finish_reason(mut self, reason: String) -> Self {
        self.finish_reason = reason;
        self
    }
}
