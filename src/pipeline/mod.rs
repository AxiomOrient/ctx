// 7-stage prompt pipeline modules
pub mod classifier;   // Stage 2: Classify
pub mod composer;     // Stage 4: MMR Select + Stage 5: Token Trim
pub mod dependency;   // Dependency resolution
pub mod scorer;       // Stage 3: Retrieve & Score  
pub mod transformer; // Stage 6: Template
pub mod mmr;         // MMR algorithm implementation

use crate::domain::{errors::Result, types::*};
use crate::util::hash;
use crate::knowledge::{ontology::OntologyRegistry, rules::RuleSet};

/// 7-Stage Prompt Pipeline with Knowledge Integration
/// 
/// PLAN.md specification:
/// 1. Normalize & Hash -> deterministic execution ID
/// 2. Classify -> facets extraction using ontology + rules
/// 3. Retrieve & Score -> candidate ranking
/// 4. MMR Select -> diversity optimization
/// 5. Token Trim -> budget enforcement  
/// 6. Template -> prompt assembly
/// 7. Send -> AI transport (handled by drivers/ai)
pub struct Pipeline {
    execution_id: String,
    ontology: Option<OntologyRegistry>,
    rules: Option<RuleSet>,
}

impl Pipeline {
    pub fn new() -> Self {
        Self {
            execution_id: String::new(),
            ontology: None,
            rules: None,
        }
    }

    /// Load ontology and rules from default paths
    pub fn with_knowledge(mut self, ontology_path: Option<&str>, rules_path: Option<&str>) -> Result<Self> {
        // Load ontology if path provided
        if let Some(onto_path) = ontology_path {
            if std::path::Path::new(onto_path).exists() {
                let ontology_yaml = std::fs::read_to_string(onto_path)
                    .map_err(|e| crate::domain::errors::ContextError::Io(e))?;
                self.ontology = Some(OntologyRegistry::from_yaml(&ontology_yaml)
                    .map_err(|e| crate::domain::errors::ContextError::Other(format!("Failed to load ontology: {}", e)))?);
            }
        }

        // Load rules if path provided  
        if let Some(rules_path) = rules_path {
            if std::path::Path::new(rules_path).exists() {
                let rules_yaml = std::fs::read_to_string(rules_path)
                    .map_err(|e| crate::domain::errors::ContextError::Io(e))?;
                self.rules = Some(crate::knowledge::rules::load_yaml_rules_from_string(&rules_yaml)
                    .map_err(|e| crate::domain::errors::ContextError::Other(format!("Failed to load rules: {}", e)))?);
            }
        }

        Ok(self)
    }

    /// Stage 1: Normalize & Hash
    /// Generate deterministic execution ID from normalized input
    pub fn stage_1_normalize_hash(&mut self, input: &ComposeInput) -> Result<String> {
        let normalized = self.normalize_input(input)?;
        self.execution_id = hash::deterministic_hash(&normalized);
        Ok(self.execution_id.clone())
    }

    /// Stage 2: Classify
    /// Extract facets using ontology and rules (PLAN.md knowledge integration)
    pub fn stage_2_classify(&self, query: &str) -> Result<Vec<String>> {
        let mut facets = Vec::new();
        
        // Use ontology-based classification if available (preserving original logic)
        if let Some(ref ontology) = self.ontology {
            // Use the existing sophisticated classification logic
            let query_lower = query.to_lowercase();
            
            // Iterate through ontology namespaces (preserving the original structure)
            for (namespace, namespace_data) in &ontology.ontology.namespaces {
                for (value_key, value_data) in &namespace_data.values {
                    // Check main value
                    if query_lower.contains(&value_key.to_lowercase()) {
                        facets.push(format!("{}:{}", namespace, value_key));
                    }
                    
                    // Check synonyms (this was a key feature of the original)
                    for synonym in &value_data.synonyms {
                        if query_lower.contains(&synonym.to_lowercase()) {
                            facets.push(format!("{}:{}", namespace, value_key));
                            break; // Avoid duplicates
                        }
                    }
                }
            }
        }
        
        // Apply rules if available (preserving sophisticated rule logic)
        if let Some(ref rules) = self.rules {
            let query_lower = query.to_lowercase();
            
            // Apply keyword rules (exact contains, case-insensitive)
            for (namespace, keywords) in &rules.keywords {
                for keyword in keywords {
                    if query_lower.contains(&keyword.to_lowercase()) {
                        facets.push(format!("{}:{}", namespace, keyword));
                    }
                }
            }
            
            // Apply regex rules (preserving the sophisticated pattern matching)
            for (namespace, patterns) in &rules.regexes {
                for pattern in patterns {
                    if let Ok(regex) = regex::Regex::new(pattern) {
                        if regex.is_match(query) {
                            facets.push(format!("{}:regex_match", namespace));
                        }
                    }
                }
            }
        }
        
        // Fallback to simple keyword extraction if no knowledge base or no matches
        if facets.is_empty() {
            facets = query.split_whitespace()
                .filter(|word| word.len() > 2)
                .filter(|word| !self.is_stop_word(word))
                .map(|word| word.to_lowercase())
                .collect();
        }
        
        // Remove duplicates and sort for determinism
        facets.sort();
        facets.dedup();
        
        Ok(facets)
    }

    /// Stage 3: Retrieve & Score
    /// Find and score candidate documents 
    pub fn stage_3_retrieve_score(&self, _facets: &[String], _input: &ComposeInput) -> Result<Vec<ScoredCandidate>> {
        // Placeholder - will be implemented with actual storage
        Ok(Vec::new())
    }

    /// Stage 4: MMR Select
    /// Apply Maximal Marginal Relevance for diversity
    pub fn stage_4_mmr_select(&self, candidates: Vec<ScoredCandidate>, max_count: usize) -> Result<Vec<ScoredCandidate>> {
        if candidates.len() <= max_count {
            return Ok(candidates);
        }

        // Simple top-k selection for now - MMR algorithm to be implemented
        let mut sorted = candidates;
        sorted.sort_by(|a, b| b.base_score.partial_cmp(&a.base_score).unwrap_or(std::cmp::Ordering::Equal));
        sorted.truncate(max_count);
        Ok(sorted)
    }

    /// Stage 5: Token Trim
    /// Enforce token budget constraints
    pub fn stage_5_token_trim(&self, candidates: Vec<ScoredCandidate>, max_tokens: u32) -> Result<Vec<ScoredCandidate>> {
        let mut total_tokens = 0;
        let mut selected = Vec::new();

        for candidate in candidates {
            let candidate_tokens = candidate.candidate.tokens as u32;
            if total_tokens + candidate_tokens <= max_tokens {
                total_tokens += candidate_tokens;
                selected.push(candidate);
            } else {
                break;
            }
        }

        Ok(selected)
    }

    /// Stage 6: Template
    /// Assemble final prompt from selected documents
    pub fn stage_6_template(&self, candidates: Vec<ScoredCandidate>, query: &str) -> Result<PromptBundle> {
        let mut context_parts = Vec::new();
        let mut sources = Vec::new();
        let mut total_tokens = 0;

        for candidate in &candidates {
            if let Some(content) = &candidate.candidate.content {
                context_parts.push(format!("## {}\n{}", candidate.candidate.title, content));
                sources.push(candidate.candidate.doc_id.clone());
                total_tokens += candidate.candidate.tokens as u32;
            }
        }

        let context = context_parts.join("\n\n");
        let prompt = format!("Context:\n{}\n\nQuery: {}", context, query);

        Ok(PromptBundle::new(prompt, context, total_tokens)
            .with_sources(sources)
            .with_confidence(self.calculate_confidence(&candidates)))
    }

    /// Execute complete 7-stage pipeline
    pub fn execute(&mut self, input: ComposeInput) -> Result<PromptBundle> {
        // Stage 1: Normalize & Hash
        let _execution_id = self.stage_1_normalize_hash(&input)?;

        // Stage 2: Classify  
        let facets = self.stage_2_classify(&input.query)?;

        // Stage 3: Retrieve & Score
        let candidates = self.stage_3_retrieve_score(&facets, &input)?;

        // Stage 4: MMR Select
        let max_count = 10; // configurable
        let selected = self.stage_4_mmr_select(candidates, max_count)?;

        // Stage 5: Token Trim
        let max_tokens = input.max_tokens.unwrap_or(4000);
        let trimmed = self.stage_5_token_trim(selected, max_tokens)?;

        // Stage 6: Template
        let bundle = self.stage_6_template(trimmed, &input.query)?;

        Ok(bundle)
    }

    // Helper methods
    fn normalize_input(&self, input: &ComposeInput) -> Result<String> {
        // Normalize for deterministic hashing
        let mut normalized = input.query.trim().to_lowercase();
        if let Some(tags) = &input.tags {
            let mut sorted_tags = tags.clone();
            sorted_tags.sort();
            normalized.push_str(&format!("|tags:{}", sorted_tags.join(",")));
        }
        if let Some(max_tokens) = input.max_tokens {
            normalized.push_str(&format!("|tokens:{}", max_tokens));
        }
        Ok(normalized)
    }

    fn calculate_confidence(&self, candidates: &[ScoredCandidate]) -> f32 {
        if candidates.is_empty() {
            return 0.0;
        }
        candidates.iter().map(|c| c.base_score).sum::<f32>() / candidates.len() as f32
    }

    fn is_stop_word(&self, word: &str) -> bool {
        matches!(word.to_lowercase().as_str(), 
            "the" | "a" | "an" | "and" | "or" | "but" | "in" | "on" | "at" | "to" | "for" | 
            "of" | "with" | "by" | "is" | "are" | "was" | "were" | "be" | "been" | "have" | 
            "has" | "had" | "do" | "does" | "did" | "will" | "would" | "could" | "should")
    }
}

impl Default for Pipeline {
    fn default() -> Self {
        Self::new()
    }
}