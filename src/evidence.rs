use crate::domain::{
    CtxError, EvidenceMatch, EvidenceSelector, EvidenceState, Result,
};
use std::fs;
use std::path::{Component, Path};
use unicode_normalization::UnicodeNormalization;

pub fn resolve_evidence(
    workspace: &Path,
    declaration: &str,
    selector: &EvidenceSelector,
) -> EvidenceMatch {
    let source = selector.source.as_deref().unwrap_or(declaration).to_string();

    match resolve(workspace, &source, selector) {
        Ok(result) => result,
        Err(error) => EvidenceMatch {
            state: EvidenceState::Invalid,
            source,
            exact: selector.exact.clone(),
            start_line: None,
            end_line: None,
            message: Some(error.0),
        },
    }
}

fn resolve(
    workspace: &Path,
    source: &str,
    selector: &EvidenceSelector,
) -> Result<EvidenceMatch> {
    if selector.exact.is_empty() {
        return Err(CtxError("evidence exact quote must not be empty".into()));
    }
    if selector
        .hint
        .is_some_and(|hint| hint.start == 0 || hint.end < hint.start)
    {
        return Err(CtxError(
            "evidence hint must be a valid one-based inclusive line range".into(),
        ));
    }

    let relative = Path::new(source);
    validate_relative_path(relative)?;

    let workspace = workspace.canonicalize()?;
    let joined = workspace.join(relative);
    if !joined.exists() {
        return Ok(EvidenceMatch {
            state: EvidenceState::Missing,
            source: source.to_string(),
            exact: selector.exact.clone(),
            start_line: None,
            end_line: None,
            message: Some("evidence source does not exist".into()),
        });
    }

    let absolute = joined.canonicalize()?;
    if !absolute.starts_with(&workspace) {
        return Err(CtxError(format!(
            "evidence source escapes workspace: {source}"
        )));
    }

    let content = normalize_newlines(&fs::read_to_string(&absolute)?);
    let (region, region_offset) = searchable_region(&content);
    let exact = normalize_newlines(&selector.exact);
    let prefix = selector.prefix.as_deref().map(normalize_newlines);
    let suffix = selector.suffix.as_deref().map(normalize_newlines);

    let raw_matches: Vec<usize> = region
        .match_indices(&exact)
        .map(|(index, _)| index)
        .collect();

    if raw_matches.is_empty() {
        return Ok(EvidenceMatch {
            state: EvidenceState::Stale,
            source: source.to_string(),
            exact: selector.exact.clone(),
            start_line: None,
            end_line: None,
            message: Some("exact quote is no longer present".into()),
        });
    }

    let has_context = prefix.is_some() || suffix.is_some();
    let selected = if has_context {
        let contextual: Vec<usize> = raw_matches
            .iter()
            .copied()
            .filter(|start| {
                context_matches(
                    region,
                    *start,
                    &exact,
                    prefix.as_deref(),
                    suffix.as_deref(),
                )
            })
            .collect();

        match contextual.as_slice() {
            [only] => *only,
            [] => {
                return Ok(EvidenceMatch {
                    state: EvidenceState::Stale,
                    source: source.to_string(),
                    exact: selector.exact.clone(),
                    start_line: None,
                    end_line: None,
                    message: Some(
                        "exact quote exists but declared prefix/suffix context no longer matches"
                            .into(),
                    ),
                });
            }
            _ => {
                return Ok(EvidenceMatch {
                    state: EvidenceState::Ambiguous,
                    source: source.to_string(),
                    exact: selector.exact.clone(),
                    start_line: None,
                    end_line: None,
                    message: Some(format!(
                        "selector matches {} locations; provide more distinguishing context",
                        contextual.len()
                    )),
                });
            }
        }
    } else {
        match raw_matches.as_slice() {
            [only] => *only,
            _ => {
                return Ok(EvidenceMatch {
                    state: EvidenceState::Ambiguous,
                    source: source.to_string(),
                    exact: selector.exact.clone(),
                    start_line: None,
                    end_line: None,
                    message: Some(format!(
                        "exact quote appears {} times; add prefix/suffix context",
                        raw_matches.len()
                    )),
                });
            }
        }
    };

    let start = region_offset + selected;
    let start_line = line_at(&content, start);
    let end_line = start_line + exact.bytes().filter(|byte| *byte == b'\n').count();
    let relocated = selector
        .hint
        .is_some_and(|hint| hint.start != start_line || hint.end != end_line);

    Ok(EvidenceMatch {
        state: if relocated {
            EvidenceState::Relocated
        } else {
            EvidenceState::Valid
        },
        source: source.to_string(),
        exact: selector.exact.clone(),
        start_line: Some(start_line),
        end_line: Some(end_line),
        message: relocated.then(|| {
            "quote resolved uniquely at a different line range; the hint is stale but evidence is valid"
                .to_string()
        }),
    })
}

fn searchable_region(content: &str) -> (&str, usize) {
    let Some(rest) = content.strip_prefix("---\n") else {
        return (content, 0);
    };
    let Some(end) = rest.find("\n---\n") else {
        return (content, 0);
    };
    let offset = 4 + end + 5;
    (&content[offset..], offset)
}

fn context_matches(
    content: &str,
    start: usize,
    exact: &str,
    prefix: Option<&str>,
    suffix: Option<&str>,
) -> bool {
    let end = start + exact.len();
    let prefix_ok = prefix.is_none_or(|value| content[..start].ends_with(value));
    let suffix_ok = suffix.is_none_or(|value| content[end..].starts_with(value));
    prefix_ok && suffix_ok
}

fn line_at(content: &str, byte_index: usize) -> usize {
    1 + content[..byte_index]
        .bytes()
        .filter(|byte| *byte == b'\n')
        .count()
}

fn validate_relative_path(path: &Path) -> Result<()> {
    if path.is_absolute() {
        return Err(CtxError(format!(
            "evidence source must be relative: {}",
            path.display()
        )));
    }
    if path.components().any(|component| {
        matches!(
            component,
            Component::ParentDir | Component::RootDir | Component::Prefix(_)
        )
    }) {
        return Err(CtxError(format!(
            "evidence source may not escape workspace: {}",
            path.display()
        )));
    }
    Ok(())
}

fn normalize_newlines(value: &str) -> String {
    value
        .replace("\r\n", "\n")
        .replace('\r', "\n")
        .nfc()
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn context_disambiguates_repeated_quote() {
        let content = "A same Z\nB same Y";
        let exact = "same";
        let starts: Vec<usize> = content
            .match_indices(exact)
            .map(|(index, _)| index)
            .filter(|start| context_matches(content, *start, exact, Some("B "), Some(" Y")))
            .collect();
        assert_eq!(starts.len(), 1);
    }

    #[test]
    fn markdown_frontmatter_is_not_searchable_evidence() {
        let input = "---\nexact: same\n---\n# Body\nsame\n";
        let (region, offset) = searchable_region(input);
        assert_eq!(region.matches("same").count(), 1);
        assert!(offset > 0);
    }

    #[test]
    fn line_numbers_are_one_based() {
        assert_eq!(line_at("a\nb\nc", 4), 3);
    }
}
