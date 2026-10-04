use crate::domain::{
    ConceptMeta, CtxError, Edge, Entity, GraphSnapshot, Result, Schema,
};
use std::fs;
use std::path::{Path, PathBuf};

pub struct Workspace {
    pub root: PathBuf,
    pub schema: Schema,
    pub graph: GraphSnapshot,
}

pub fn compile_workspace(
    root: &Path,
    schema_path: &Path,
    knowledge_dir: &Path,
) -> Result<Workspace> {
    let root = root.canonicalize()?;
    let schema_path = resolve_inside(&root, schema_path)?;
    let knowledge_dir = resolve_inside(&root, knowledge_dir)?;

    if !knowledge_dir.is_dir() {
        return Err(CtxError(format!(
            "knowledge path is not a directory: {}",
            knowledge_dir.display()
        )));
    }

    let schema: Schema = serde_yaml::from_str(&fs::read_to_string(&schema_path)?)?;

    let mut documents = Vec::new();
    collect_markdown(&root, &knowledge_dir, &mut documents)?;
    documents.sort();

    let mut graph = GraphSnapshot::default();
    for path in documents {
        let relative = relative_string(&root, &path)?;
        let content = fs::read_to_string(&path)?;
        let meta = parse_frontmatter(&relative, &content)?;

        graph.entities.push(Entity {
            id: meta.id.clone(),
            kind: meta.kind.clone(),
            title: meta.title.clone().unwrap_or_else(|| meta.id.clone()),
            aliases: meta.aliases.clone(),
            document: relative.clone(),
        });

        for relation in meta.relations {
            graph.edges.push(Edge {
                from: meta.id.clone(),
                relation: relation.relation,
                to: relation.target,
                evidence: relation.evidence,
                declaration: relative.clone(),
            });
        }
    }

    graph.entities.sort_by(|a, b| a.id.cmp(&b.id).then_with(|| a.document.cmp(&b.document)));
    graph.edges.sort_by(|a, b| {
        a.from
            .cmp(&b.from)
            .then_with(|| a.relation.cmp(&b.relation))
            .then_with(|| a.to.cmp(&b.to))
            .then_with(|| a.declaration.cmp(&b.declaration))
    });

    Ok(Workspace {
        root,
        schema,
        graph,
    })
}

fn collect_markdown(root: &Path, dir: &Path, out: &mut Vec<PathBuf>) -> Result<()> {
    let mut entries = fs::read_dir(dir)?
        .collect::<std::result::Result<Vec<_>, _>>()?;
    entries.sort_by_key(|entry| entry.path());

    for entry in entries {
        let path = entry.path();
        let metadata = fs::symlink_metadata(&path)?;
        if metadata.file_type().is_symlink() {
            return Err(CtxError(format!(
                "symlinks are not allowed in knowledge tree: {}",
                relative_string(root, &path)?
            )));
        }
        if metadata.is_dir() {
            collect_markdown(root, &path, out)?;
        } else if metadata.is_file()
            && path
                .extension()
                .and_then(|ext| ext.to_str())
                .is_some_and(|ext| ext.eq_ignore_ascii_case("md"))
        {
            out.push(path);
        }
    }
    Ok(())
}

fn parse_frontmatter(path: &str, content: &str) -> Result<ConceptMeta> {
    let normalized = normalize_newlines(content);
    let Some(rest) = normalized.strip_prefix("---\n") else {
        return Err(CtxError(format!(
            "{path}: knowledge document must start with YAML frontmatter"
        )));
    };

    let Some(end) = rest.find("\n---\n") else {
        return Err(CtxError(format!(
            "{path}: YAML frontmatter is missing closing delimiter"
        )));
    };

    let yaml = &rest[..end];
    let meta: ConceptMeta = serde_yaml::from_str(yaml)
        .map_err(|error| CtxError(format!("{path}: invalid frontmatter: {error}")))?;

    if meta.id.trim().is_empty() {
        return Err(CtxError(format!("{path}: id must not be empty")));
    }
    if meta.kind.trim().is_empty() {
        return Err(CtxError(format!("{path}: type must not be empty")));
    }
    Ok(meta)
}

fn resolve_inside(root: &Path, path: &Path) -> Result<PathBuf> {
    let joined = if path.is_absolute() {
        path.to_path_buf()
    } else {
        root.join(path)
    };
    let canonical = joined.canonicalize()?;
    if !canonical.starts_with(root) {
        return Err(CtxError(format!(
            "path escapes workspace: {}",
            path.display()
        )));
    }
    Ok(canonical)
}

fn relative_string(root: &Path, path: &Path) -> Result<String> {
    let relative = path
        .strip_prefix(root)
        .map_err(|_| CtxError(format!("path is outside workspace: {}", path.display())))?;
    Ok(relative.to_string_lossy().replace('\\', "/"))
}

fn normalize_newlines(value: &str) -> String {
    value.replace("\r\n", "\n").replace('\r', "\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_strict_frontmatter() {
        let input = "---\nid: x\ntype: Thing\n---\n# X\n";
        let parsed = parse_frontmatter("x.md", input).unwrap();
        assert_eq!(parsed.id, "x");
        assert_eq!(parsed.kind, "Thing");
    }

    #[test]
    fn rejects_missing_frontmatter() {
        assert!(parse_frontmatter("x.md", "# X").is_err());
    }
}
