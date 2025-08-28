pub mod cli;

use crate::domain::{errors::Result, types::*};
use crate::pipeline::Pipeline;
use crate::drivers::ai;

/// Classify text to extract facets and context requirements
/// 
/// This function implements a simple text classification to identify
/// relevant facets and context requirements from the input text.
pub fn classify_text(text: &str) -> Result<Vec<String>> {
    // Simple keyword-based classification
    // Can be enhanced with ML models later
    let facets = text
        .split_whitespace()
        .filter(|word| word.len() > 2)
        .filter(|word| !is_stop_word(word))
        .map(|word| word.to_lowercase())
        .collect();
    
    Ok(facets)
}

/// Compose prompt from query and available context
/// 
/// Executes the 7-stage pipeline with ontology and rules integration
pub fn compose_prompt(input: ComposeInput) -> Result<PromptBundle> {
    let mut pipeline = Pipeline::new();
    
    // Try to load knowledge base (ontology + rules)
    let ontology_path = if std::path::Path::new("knowledge/ontology.yaml").exists() {
        Some("knowledge/ontology.yaml")
    } else if std::path::Path::new("ontology.yaml").exists() {
        Some("ontology.yaml")
    } else {
        None
    };
    
    let rules_path = if std::path::Path::new("knowledge/rules.yaml").exists() {
        Some("knowledge/rules.yaml")
    } else if std::path::Path::new("rules.yaml").exists() {
        Some("rules.yaml")
    } else {
        None
    };
    
    // Load knowledge base if available
    if ontology_path.is_some() || rules_path.is_some() {
        pipeline = pipeline.with_knowledge(ontology_path, rules_path)?;
    }
    
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
fn is_stop_word(word: &str) -> bool {
    matches!(word.to_lowercase().as_str(), 
        "the" | "a" | "an" | "and" | "or" | "but" | "in" | "on" | "at" | "to" | "for" | 
        "of" | "with" | "by" | "is" | "are" | "was" | "were" | "be" | "been" | "have" | 
        "has" | "had" | "do" | "does" | "did" | "will" | "would" | "could" | "should")
}