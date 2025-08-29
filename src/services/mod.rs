pub mod cli;

use crate::domain::{errors::Result, types::*};
use crate::pipeline::Pipeline;
use crate::drivers::ai;
use crate::config::Cfg;

/// Classify text using ontology and rules when available.
pub fn classify_text(text: &str) -> Result<Vec<String>> {
    let mut pipeline = Pipeline::new();

    // Knowledge paths
    let ontology_path = [
        "knowledge/ontology.yaml",
        "ontology.yaml",
        "src/knowledge/ontology.yaml",
    ]
    .into_iter()
    .find(|p| std::path::Path::new(p).exists());

    let rules_path = [
        "knowledge/rules.yaml",
        "rules.yaml",
        "src/knowledge/rules.yaml",
    ]
    .into_iter()
    .find(|p| std::path::Path::new(p).exists());

    if ontology_path.is_some() || rules_path.is_some() {
        pipeline = pipeline.with_knowledge(ontology_path, rules_path)?;
    }

    pipeline.stage_2_classify(text)
}

/// Compose prompt from query and available context
/// 
/// Executes the 7-stage pipeline with ontology and rules integration
pub fn compose_prompt(input: ComposeInput) -> Result<PromptBundle> {
    // Load config if present
    let cfg = std::path::Path::new("config.toml")
        .exists()
        .then(|| Cfg::load("config.toml").ok())
        .flatten();

    let mut pipeline = Pipeline::new();
    
    // Try to load knowledge base (ontology + rules) with robust search order
    let ontology_path = [
        "knowledge/ontology.yaml",
        "ontology.yaml",
        "src/knowledge/ontology.yaml",
    ]
    .into_iter()
    .find(|p| std::path::Path::new(p).exists());

    let rules_path = [
        "knowledge/rules.yaml",
        "rules.yaml",
        "src/knowledge/rules.yaml",
    ]
    .into_iter()
    .find(|p| std::path::Path::new(p).exists());
    
    // Load knowledge base if available
    if ontology_path.is_some() || rules_path.is_some() {
        pipeline = pipeline.with_knowledge(ontology_path, rules_path)?;
    }

    // Apply config: docs root and mmr lambda
    if let Some(c) = &cfg {
        pipeline = pipeline.with_docs_root(std::path::PathBuf::from(&c.paths.docs_dir));
        pipeline = pipeline.with_mmr_lambda(c.defaults.mmr_lambda);
    }
    
    // Fill defaults from config if not provided
    let input = if let Some(c) = &cfg {
        if input.max_tokens.is_none() {
            ComposeInput { max_tokens: Some(c.defaults.budget as u32), ..input }
        } else { input }
    } else { input };

    pipeline.execute(input)
}

/// Complete AI work pipeline: compose context + send to AI + return result
/// 
/// This is the highest-level API that combines context composition
/// with AI interaction to provide complete work completion.
pub async fn work(input: WorkInput) -> Result<WorkOutput> {
    // Stage 1-6: Compose context
    let compose_input = ComposeInput {
        query: input.task.clone(),
        tags: None,
        max_tokens: input.max_tokens,
        exclude_sources: None,
        priority_sources: None,
    };
    
    let context_bundle = compose_prompt(compose_input)?;
    
    // Stage 7: Send to AI
    let ai_prompt = if let Some(context) = input.context {
        format!("{}\n\nAdditional Context: {}\n\nTask: {}", 
                context_bundle.prompt, context, input.task)
    } else {
        format!("{}\n\nTask: {}", context_bundle.prompt, input.task)
    };
    
    let prompt = AiPrompt::user(ai_prompt);
    let response = ai::send_prompt(prompt, input.ai_provider, input.temperature).await?;
    
    let execution_id = crate::util::hash::deterministic_hash(&input.task);
    
    Ok(WorkOutput::new(response, context_bundle, execution_id))
}

// Helper function for classification
// No stop-word filtering at this layer; classification relies on knowledge/rules
