use crate::domain::{Ontology, Result, Topology};
use std::fs;
use std::path::Path;

pub fn load_ontology(path: &Path) -> Result<Ontology> {
    let content = fs::read_to_string(path)?;
    Ok(serde_yaml::from_str(&content)?)
}

pub fn load_topology(path: &Path) -> Result<Topology> {
    let content = fs::read_to_string(path)?;
    Ok(serde_yaml::from_str(&content)?)
}
