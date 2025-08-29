// 7-stage prompt pipeline modules
pub mod classifier;   // Stage 2: Classify
pub mod composer;     // Stage 4: MMR Select + Stage 5: Token Trim
pub mod dependency;   // Dependency resolution
pub mod scorer;       // Stage 3: Retrieve & Score  
pub mod transformer; // Stage 6: Template
pub mod mmr;         // MMR algorithm implementation

use crate::domain::{errors::Result, types::*};
use tracing::instrument;
use crate::util::hash;
use crate::knowledge::{ontology::OntologyRegistry, rules::{RuleSet, schema::YamlRuleSet}};

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
    yaml_rules: Option<YamlRuleSet>,
    docs_root: std::path::PathBuf,
    mmr_lambda: f32,
}

impl Pipeline {
    pub fn new() -> Self {
        Self {
            execution_id: String::new(),
            ontology: None,
            rules: None,
            yaml_rules: None,
            docs_root: std::path::PathBuf::from("documents"),
            mmr_lambda: crate::domain::constants::scoring::DEFAULT_MMR_LAMBDA,
        }
    }

    /// Load ontology and rules from default paths
    pub fn with_knowledge(mut self, ontology_path: Option<&str>, rules_path: Option<&str>) -> Result<Self> {
        // Load ontology if path provided
        if let Some(onto_path) = ontology_path {
            if std::path::Path::new(onto_path).exists() {
                let ontology_yaml = std::fs::read_to_string(onto_path)
                    .map_err(crate::domain::errors::ContextError::Io)?;
                self.ontology = Some(OntologyRegistry::from_yaml(&ontology_yaml)
                    .map_err(|e| crate::domain::errors::ContextError::Other(format!("Failed to load ontology: {}", e)))?);
            }
        }

        // Load rules if path provided  
        if let Some(rules_path) = rules_path {
            if std::path::Path::new(rules_path).exists() {
                let rules_yaml = std::fs::read_to_string(rules_path)
                    .map_err(crate::domain::errors::ContextError::Io)?;
                // Prefer weighted YAML model when available
                if let Ok(yaml_rules) = serde_yaml::from_str::<YamlRuleSet>(&rules_yaml) {
                    self.yaml_rules = Some(yaml_rules.clone());
                    self.rules = Some(yaml_rules.to_rule_set());
                } else {
                    self.rules = Some(crate::knowledge::rules::load_yaml_rules_from_string(&rules_yaml)
                        .map_err(|e| crate::domain::errors::ContextError::Other(format!("Failed to load rules: {}", e)))?);
                }
            }
        }

        Ok(self)
    }

    /// Set documents root directory
    pub fn with_docs_root(mut self, root: impl Into<std::path::PathBuf>) -> Self {
        self.docs_root = root.into();
        self
    }

    /// Set MMR lambda parameter
    pub fn with_mmr_lambda(mut self, lambda: f32) -> Self {
        self.mmr_lambda = lambda;
        self
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
        use crate::pipeline::classifier::engine::ClassifierEngine;
        use crate::knowledge::rules::applier::{RuleApplier, ClassificationLike};

        if let Some(ref onto) = self.ontology {
            let classification = if let (Some(ref rules), Some(ref yaml)) = (&self.rules, &self.yaml_rules) {
                let engine = ClassifierEngine::new_with_yaml(onto, rules, yaml.clone());
                engine.classify(query, "")
            } else if let Some(ref rules) = self.rules {
                let engine = ClassifierEngine::new(onto, rules);
                engine.classify(query, "")
            } else {
                let empty_rules = RuleSet::new();
                let engine = ClassifierEngine::new(onto, &empty_rules);
                engine.classify(query, "")
            };

            // Extract facets
            let mut facets: Vec<String> = Vec::new();
            for (ns, values) in classification.facets().to_vec_map() {
                for v in values { facets.push(format!("{}:{}", ns, v)); }
            }

            // Apply rule applier (requires/prohibits → add required facets)
            if let Some(ref rules) = self.rules {
                struct Facade<'a>(&'a crate::pipeline::classifier::facet::Classification);
                impl<'a> ClassificationLike for Facade<'a> {
                    fn has_facet(&self, ns: &str, val: &str) -> bool { self.0.has_facet(ns, val) }
                }
                let applier = RuleApplier::new(rules);
                let result = applier.calculate_rule_applications(&Facade(&classification));
                for action in result.actions {
                    if let crate::domain::rule_application::RuleAction::AddFacet{namespace,value} = action {
                        facets.push(format!("{}:{}", namespace, value));
                    }
                }
            }

            facets.sort(); facets.dedup();
            return Ok(facets);
        }

        // Fallback simple extraction
        let mut facets: Vec<String> = query.split_whitespace()
            .filter(|w| w.len() > 2)
            .filter(|w| !self.is_stop_word(w))
            .map(|w| w.to_lowercase()).collect();
        facets.sort(); facets.dedup();
        Ok(facets)
    }

    /// Stage 3: Retrieve & Score
    /// Discover local markdown documents and score them against the query
    pub fn stage_3_retrieve_score(&self, facets: &[String], input: &ComposeInput) -> Result<Vec<ScoredCandidate>> {
        use crate::drivers::storage::{local::LocalFsStorage, Storage};
        use crate::pipeline::scorer::DocumentScorer;
        use crate::domain::types::{BuildQuery, DocumentCandidate};
        use crate::domain::constants::scoring::DEFAULT_CONFIDENCE_THRESHOLD;

        // Resolve documents root
        let storage = LocalFsStorage::new(std::path::PathBuf::from("."));
        let docs_root = &self.docs_root;
        let files = storage.list_files(docs_root, "*.md")?;

        // Build a query from input + facets
        let mut query = BuildQuery::new("local".into(), "main".into(), "latest".into())
            .with_confidence_threshold(DEFAULT_CONFIDENCE_THRESHOLD);
        // Populate query text and facets
        query.query_text = Some(input.query.clone());
        query.required_facets = self.parse_required_facets(facets);

        let mut candidates: Vec<DocumentCandidate> = Vec::new();
        for rel_path in files {
            match self.read_candidate(&storage, &rel_path) {
                Ok(c) => candidates.push(c),
                Err(e) => {
                    // Skip unreadable documents; keep pipeline robust
                    crate::domain::errors::log_warn(&format!(
                        "Skipping document {:?}: {}",
                        rel_path, e
                    ));
                }
            }
        }

        if candidates.is_empty() {
            return Ok(Vec::new());
        }

        // Score
        let scorer = DocumentScorer::new();
        let scored = scorer.score_documents(&candidates, &query)?;
        Ok(scored)
    }

    /// Stage 4: MMR Select
    /// Apply MMR selection within token budget to maximize diversity and relevance
    pub fn stage_4_mmr_select(&self, candidates: Vec<ScoredCandidate>, budget_tokens: usize) -> Result<Vec<ScoredCandidate>> {
        use crate::pipeline::mmr::DocumentSelector;
        let selector = DocumentSelector::with_mmr_lambda(self.mmr_lambda);
        let selected = selector.select_documents(candidates, budget_tokens, 0)?;
        Ok(selected)
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
    /// Assemble final prompt from selected documents with strict section-level budget
    pub fn stage_6_template(&self, candidates: Vec<ScoredCandidate>, query: &str, budget_tokens: u32) -> Result<PromptBundle> {
        use crate::doc::parse::section::SectionExtractor;
        let mut context_parts = Vec::new();
        let mut sources = Vec::new();
        let mut total_tokens: u32 = 0;

        // Determine requested section ids from query (explicit directive or inferred)
        let requested = self.parse_requested_sections(query);
        let extractor = SectionExtractor::new();

        for candidate in &candidates {
            if let Some(content) = &candidate.candidate.content {
                // Try to parse frontmatter → ContextDocument and body
                if let Some((fm_map, body)) = self.extract_frontmatter(content) {
                    // Attempt to parse to ContextDocument; if it fails, include full doc
                    match serde_yaml::from_value::<crate::domain::types::ContextDocument>(serde_yaml::Value::Mapping(fm_map.clone())) {
                        Ok(doc_meta) => {
                            // Use extractor to get sections
                            let sections = extractor.extract_sections(&doc_meta, &body).unwrap_or_default();
                            // Decide which to include
                            let mut selected = Vec::new();
                            if !requested.is_empty() {
                                for s in &sections {
                                    if requested.contains(&s.id) || requested.contains(&s.name.to_lowercase()) {
                                        selected.push(s);
                                    }
                                }
                            } else {
                                // Fallback: include all defined sections in priority order
                                selected = sections.iter().collect();
                            }

                            // Stable order: by priority desc then id
                            let mut selected_owned: Vec<_> = selected.iter().cloned().cloned().collect();
                            selected_owned.sort_by(|a, b| b.priority.cmp(&a.priority).then_with(|| a.id.cmp(&b.id)));

                            for s in selected_owned {
                                if total_tokens.saturating_add(s.tokens) > budget_tokens { break; }
                                context_parts.push(format!("### {} — {}\n{}", candidate.candidate.title, s.name, s.content));
                                sources.push(candidate.candidate.doc_id.clone());
                                total_tokens = total_tokens.saturating_add(s.tokens);
                            }
                        }
                        Err(_) => {
                            // No valid context.v1 frontmatter: include full content
                            if total_tokens.saturating_add(candidate.candidate.tokens as u32) <= budget_tokens {
                                context_parts.push(format!("## {}\n{}", candidate.candidate.title, body));
                                sources.push(candidate.candidate.doc_id.clone());
                                total_tokens = total_tokens.saturating_add(candidate.candidate.tokens as u32);
                            }
                        }
                    }
                } else {
                    // No frontmatter: include full content
                    if total_tokens.saturating_add(candidate.candidate.tokens as u32) <= budget_tokens {
                        context_parts.push(format!("## {}\n{}", candidate.candidate.title, content));
                        sources.push(candidate.candidate.doc_id.clone());
                        total_tokens = total_tokens.saturating_add(candidate.candidate.tokens as u32);
                    }
                }
            }
        }

        let context = context_parts.join("\n\n");
        let prompt = format!("Context:\n{}\n\nQuery: {}", context, query);

        Ok(PromptBundle::new(prompt, context, total_tokens)
            .with_sources(sources)
            .with_confidence(self.calculate_confidence(&candidates)))
    }

    /// Execute complete 7-stage pipeline
    #[instrument(skip(self), fields(query = %input.query))]
    pub fn execute(&mut self, input: ComposeInput) -> Result<PromptBundle> {
        // Stage 1: Normalize & Hash
        let _execution_id = self.stage_1_normalize_hash(&input)?;

        // Stage 2: Classify  
        let facets = self.stage_2_classify(&input.query)?;

        // Stage 3: Retrieve & Score
        let candidates = self.stage_3_retrieve_score(&facets, &input)?;

        // Stage 4: MMR Select
        let budget_tokens = input.max_tokens.map(|v| v as usize).unwrap_or(crate::domain::constants::composition::DEFAULT_TOKEN_BUDGET);
        let selected = self.stage_4_mmr_select(candidates, budget_tokens)?;

        // Stage 5: Token Trim
        let max_tokens = input.max_tokens.unwrap_or(4000);
        let trimmed = self.stage_5_token_trim(selected, max_tokens)?;

        // Stage 6: Template
        let bundle = self.stage_6_template(trimmed, &input.query, max_tokens)?;

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

    // Convert flat facet strings like "ns:val" into BuildQuery required_facets
    fn parse_required_facets(&self, facets: &[String]) -> Vec<(String, Vec<String>)> {
        use std::collections::BTreeMap;
        let mut map: BTreeMap<String, Vec<String>> = BTreeMap::new();
        for f in facets {
            if let Some((ns, val)) = f.split_once(':') {
                map.entry(ns.to_string()).or_default().push(val.to_string());
            }
        }
        map.into_iter().map(|(k, mut v)| { v.sort(); v.dedup(); (k, v) }).collect()
    }

    // Read a markdown file and build a DocumentCandidate
    fn read_candidate<S: crate::drivers::storage::Storage + ?Sized>(&self, storage: &S, rel_path: &std::path::Path) -> Result<crate::domain::types::DocumentCandidate> {
        use crate::domain::types::DocumentCandidate;
        use chrono::Utc;

        let content = storage.read(rel_path)?;
        let fm_body = self.extract_frontmatter(&content);

        // Title from frontmatter.title or first markdown heading
        let title = fm_body
            .as_ref()
            .and_then(|(map, _)| map.get("title").and_then(|v| v.as_str()).map(|s| s.to_string()))
            .or_else(|| self.first_heading(&content))
            .unwrap_or_else(|| rel_path.file_stem().and_then(|s| s.to_str()).unwrap_or("document").to_string());

        // Tags as facets if provided
        let mut facets: std::collections::HashMap<String, Vec<String>> = std::collections::HashMap::new();
        if let Some((map, _)) = &fm_body {
            if let Some(tags_val) = map.get("tags") {
                if let Some(arr) = tags_val.as_sequence() {
                    let vals: Vec<String> = arr.iter().filter_map(|v| v.as_str().map(|s| s.to_string())).collect();
                    if !vals.is_empty() {
                        // Always keep original tags
                        facets.insert("tag".to_string(), vals.clone());
                        // Try to map tags into ontology namespaces
                        for t in vals {
                            if let Some((ns, val)) = self.map_tag_to_ontology(&t) {
                                facets.entry(ns).or_default().push(val);
                            }
                        }
                    }
                }
            }
        }

        // Fallback: classify the document itself to populate facets if empty
        if facets.is_empty() {
            let body_text = fm_body.as_ref().map(|(_, b)| b.as_str()).unwrap_or("");
            let doc_text = format!("{}\n\n{}", title, body_text);
            let doc_facets = self.stage_2_classify(&doc_text)?;
            for f in doc_facets {
                if let Some((ns, val)) = f.split_once(':') {
                    facets.entry(ns.to_string()).or_default().push(val.to_string());
                }
            }
        }

        // tokens estimate
        let tokens = self.estimate_tokens(
            fm_body.as_ref().map(|(_, b)| b.as_str()).unwrap_or(&content)
        );

        // freshness: file modified time if possible
        let freshness = {
            let full = std::path::Path::new(".").join(rel_path);
            match std::fs::metadata(&full).and_then(|m| m.modified()) {
                Ok(mt) => chrono::DateTime::<Utc>::from(mt).to_rfc3339(),
                Err(_) => Utc::now().to_rfc3339(),
            }
        };

        let candidate = DocumentCandidate {
            doc_id: rel_path.to_string_lossy().to_string(),
            sha: "local".to_string(),
            title,
            locale: "ko".to_string(),
            trust: crate::domain::constants::defaults::DEFAULT_TRUST,
            freshness,
            confidence: crate::domain::constants::defaults::DEFAULT_CLASSIFICATION_CONFIDENCE,
            path: rel_path.to_string_lossy().to_string(),
            facets,
            tokens,
            content: Some(content),
        };
        Ok(candidate)
    }

    fn extract_frontmatter(&self, content: &str) -> Option<(serde_yaml::Mapping, String)> {
        let lines: Vec<&str> = content.lines().collect();
        if lines.first().map(|l| l.trim()) != Some("---") {
            return None;
        }
        let end = match lines[1..].iter().position(|l| l.trim() == "---") {
            Some(idx) => idx + 1,
            None => return None,
        };
        let front = lines[1..end].join("\n");
        let body = lines[end + 1..].join("\n");
        match serde_yaml::from_str::<serde_yaml::Mapping>(&front) {
            Ok(map) => Some((map, body)),
            Err(_) => None,
        }
    }

    fn first_heading(&self, content: &str) -> Option<String> {
        for line in content.lines() {
            let t = line.trim();
            if let Some(rest) = t.strip_prefix('#') {
                return Some(rest.trim().trim_start_matches('#').trim().to_string());
            }
        }
        None
    }

    fn estimate_tokens(&self, text: &str) -> usize {
        let per_char = crate::domain::constants::parsing::DEFAULT_TOKENS_PER_CHAR;
        ((text.len() as f32) * per_char).ceil() as usize
    }

    // Attempt to map a tag string to an ontology namespace/value; returns Some(ns,value)
    fn map_tag_to_ontology(&self, tag: &str) -> Option<(String, String)> {
        let onto = self.ontology.as_ref()?;
        let t = tag.to_lowercase();
        for (ns, nsdata) in &onto.ontology.namespaces {
            if nsdata.values.contains_key(&t) {
                return Some((ns.clone(), t.clone()));
            }
            for (canon, ov) in &nsdata.values {
                if ov.synonyms.iter().any(|s| s.to_lowercase() == t) {
                    return Some((ns.clone(), canon.clone()));
                }
            }
        }
        None
    }

    // Parse explicit requested sections from query text: e.g., "sections:intro,details" or "parts:constraints"
    fn parse_requested_sections(&self, query: &str) -> std::collections::HashSet<String> {
        let mut set = std::collections::HashSet::new();
        let lower = query.to_lowercase();
        for key in ["sections:", "parts:"] {
            if let Some(idx) = lower.find(key) {
                let tail = &lower[idx + key.len()..];
                for token in tail.split([' ', ',', ';', '\n']) {
                    let t = token.trim();
                    if t.is_empty() { continue; }
                    // stop at next directive-like token
                    if t.contains(':') && (t.starts_with("lang:") || t.starts_with("artifact:") || t.starts_with("phase:")) { break; }
                    set.insert(t.to_string());
                }
            }
        }
        // Also infer from facets like "artifact:..." or "doc_process:..."
        if let Ok(facets) = self.stage_2_classify(query) {
            for f in facets {
                if let Some((ns, val)) = f.split_once(':') {
                    if ns == "artifact" || ns == "doc_process" || ns == "section" {
                        set.insert(val.to_string());
                    }
                }
            }
        }
        set
    }
}

impl Default for Pipeline {
    fn default() -> Self {
        Self::new()
    }
}
