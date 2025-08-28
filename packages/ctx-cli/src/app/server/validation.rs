use super::dto::{ApiError, ClassifyRequest, ComposeRequest, ValidateRequest};
use std::collections::{BTreeMap, HashMap};

pub struct ApiValidator;

impl ApiValidator {
    pub fn validate_classify(req: &ClassifyRequest) -> Result<(), ApiError> {
        if req.title.trim().is_empty() && req.body.trim().is_empty() {
            return Err(ApiError::validation_error("title or body must be provided"));
        }
        if let Some(p) = &req.path {
            if p.contains("..") {
                return Err(ApiError::validation_error("path must not contain '..'"));
            }
        }
        Ok(())
    }

    pub fn validate_validate(req: &ValidateRequest) -> Result<(), ApiError> {
        Self::validate_facets(&req.facets)
    }

    pub fn validate_compose(req: &ComposeRequest) -> Result<(), ApiError> {
        // Basic repo/branch/sha checks
        if req.repo.trim().is_empty()
            || req.branch.trim().is_empty()
            || req.commit_sha.trim().is_empty()
        {
            return Err(ApiError::validation_error(
                "repo, branch, and commit_sha are required",
            ));
        }
        if req.commit_sha.len() < 6 || !req.commit_sha.chars().all(|c| c.is_ascii_hexdigit()) {
            return Err(ApiError::validation_error(
                "commit_sha must be hex and >= 6 chars",
            ));
        }

        // Facets schema
        Self::validate_facets(&req.facets)?;

        // Budget ranges (use centralized composition constants)
        let min_b = ctx_core::common::constants::composition::MIN_TOKEN_BUDGET;
        let max_b = ctx_core::common::constants::composition::MAX_TOKEN_BUDGET;
        if let Some(b) = req.budget {
            if b < min_b || b > max_b {
                return Err(ApiError::validation_error(format!(
                    "budget must be in [{}..={}]",
                    min_b, max_b
                )));
            }
        }
        if let Some(r) = req.reserve {
            if let Some(b) = req.budget {
                if r >= b {
                    return Err(ApiError::validation_error(
                        "reserve must be less than budget",
                    ));
                }
            }
        }
        if let Some(ct) = req.confidence_threshold {
            if !(0.0..=1.0).contains(&ct) {
                return Err(ApiError::validation_error(
                    "confidence_threshold must be in [0,1]",
                ));
            }
        }
        Ok(())
    }

    fn validate_facets(facets: &BTreeMap<String, Vec<String>>) -> Result<(), ApiError> {
        if facets.len() > 128 {
            return Err(ApiError::validation_error("too many facet namespaces"));
        }
        for (k, v) in facets {
            if k.trim().is_empty() {
                return Err(ApiError::validation_error(
                    "facet namespace must not be empty",
                ));
            }
            if v.is_empty() {
                return Err(ApiError::validation_error(format!(
                    "facet '{}' must have values",
                    k
                )));
            }
            if v.len() > 64 {
                return Err(ApiError::validation_error(format!(
                    "facet '{}' has too many values",
                    k
                )));
            }
            for val in v {
                if val.trim().is_empty() {
                    return Err(ApiError::validation_error(format!(
                        "facet '{}' contains empty value",
                        k
                    )));
                }
            }
        }
        Ok(())
    }

    // Intent gate: category ↔ action compatibility
    pub fn apply_intent_gate(
        facets: &BTreeMap<String, Vec<String>>,
        task_contract: &HashMap<String, Vec<String>>, // e.g., {"category:legacy_edit": ["refactor","migrate"]}
    ) -> Result<(), ApiError> {
        // Extract category/action
        let category = facets
            .get("category")
            .and_then(|v| v.first())
            .cloned()
            .unwrap_or_default();
        let actions = facets.get("action").cloned().unwrap_or_default();

        if category.is_empty() || actions.is_empty() {
            return Ok(()); // Nothing to gate
        }

        let key = format!("category:{}", category);
        if let Some(allowed) = task_contract.get(&key) {
            // If any action is not allowed, fail with suggestion list
            let mut disallowed: Vec<String> = Vec::new();
            for a in &actions {
                if !allowed.contains(a) {
                    disallowed.push(a.clone());
                }
            }
            if !disallowed.is_empty() {
                return Err(ApiError::new(
                    format!(
                        "Intent gate violation: disallowed actions: {:?}",
                        disallowed
                    ),
                    "INTENT_GATE_VIOLATION",
                ));
            }
        }
        Ok(())
    }
}
