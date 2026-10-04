use crate::domain::{
    CtxError, EvidenceInspection, EvidenceRef, EvidenceState, Result, Topology,
};
use sha2::{Digest, Sha256};
use std::fs;
use std::path::{Component, Path};

pub fn inspect_evidence(workspace: &Path, evidence: &EvidenceRef) -> EvidenceInspection {
    match resolve_evidence(workspace, evidence) {
        Ok((snippet, actual_digest)) => {
            let expected = evidence.digest.as_deref().map(normalize_digest);
            let state = match expected.as_deref() {
                None => EvidenceState::Unpinned,
                Some(value) if value.eq_ignore_ascii_case(&actual_digest) => EvidenceState::Valid,
                Some(_) => EvidenceState::Stale,
            };

            EvidenceInspection {
                state,
                path: evidence.path.clone(),
                lines: evidence.lines,
                expected_digest: evidence.digest.clone(),
                actual_digest: Some(format!("sha256:{actual_digest}")),
                snippet: Some(snippet),
                message: None,
            }
        }
        Err(error) => {
            let state = if error.0.starts_with("missing evidence file:") {
                EvidenceState::Missing
            } else {
                EvidenceState::Invalid
            };
            EvidenceInspection {
                state,
                path: evidence.path.clone(),
                lines: evidence.lines,
                expected_digest: evidence.digest.clone(),
                actual_digest: None,
                snippet: None,
                message: Some(error.0),
            }
        }
    }
}

pub fn pin_topology(workspace: &Path, topology_path: &Path, topology: &mut Topology) -> Result<usize> {
    let mut pinned = 0;
    for edge in &mut topology.edges {
        for evidence in &mut edge.evidence {
            let (_, digest) = resolve_evidence(workspace, evidence)?;
            evidence.digest = Some(format!("sha256:{digest}"));
            pinned += 1;
        }
    }

    let serialized = serde_yaml::to_string(topology)?;
    fs::write(topology_path, serialized)?;
    Ok(pinned)
}

fn resolve_evidence(workspace: &Path, evidence: &EvidenceRef) -> Result<(String, String)> {
    let relative = Path::new(&evidence.path);
    validate_relative_path(relative)?;

    let workspace = workspace.canonicalize()?;
    let joined = workspace.join(relative);
    if !joined.exists() {
        return Err(CtxError(format!(
            "missing evidence file: {}",
            joined.display()
        )));
    }

    let absolute = joined.canonicalize()?;
    if !absolute.starts_with(&workspace) {
        return Err(CtxError(format!(
            "evidence escapes workspace: {}",
            evidence.path
        )));
    }

    let content = fs::read_to_string(&absolute)?;
    let snippet = line_slice(&content, evidence.lines.start, evidence.lines.end)?;
    let digest = sha256_hex(snippet.as_bytes());
    Ok((snippet, digest))
}

fn validate_relative_path(path: &Path) -> Result<()> {
    if path.is_absolute() {
        return Err(CtxError(format!(
            "evidence path must be relative: {}",
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
            "evidence path may not escape workspace: {}",
            path.display()
        )));
    }
    Ok(())
}

fn line_slice(content: &str, start: usize, end: usize) -> Result<String> {
    if start == 0 || end < start {
        return Err(CtxError(format!("invalid line range: {start}..{end}")));
    }

    let lines: Vec<&str> = content.lines().collect();
    if end > lines.len() {
        return Err(CtxError(format!(
            "line range {start}..{end} exceeds source length {}",
            lines.len()
        )));
    }
    Ok(lines[start - 1..end].join("\n"))
}

fn normalize_digest(value: &str) -> String {
    value
        .strip_prefix("sha256:")
        .unwrap_or(value)
        .trim()
        .to_ascii_lowercase()
}

pub fn sha256_hex(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    let mut out = String::with_capacity(digest.len() * 2);
    for byte in digest {
        use std::fmt::Write as _;
        let _ = write!(&mut out, "{byte:02x}");
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn slices_lines_deterministically() {
        let source = "one\ntwo\nthree\n";
        assert_eq!(line_slice(source, 2, 3).unwrap(), "two\nthree");
    }

    #[test]
    fn rejects_parent_traversal() {
        let path = PathBuf::from("../secret.md");
        assert!(validate_relative_path(&path).is_err());
    }
}
