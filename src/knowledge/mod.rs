pub mod ontology;
pub mod rules;

// Public API re-exports for convenience at crate::knowledge::*
pub use ontology::{Ontology, OntologyRegistry};
pub use rules::schema::RuleSet;
pub use rules::loader::{
    builtin_rules,
    load_from_config_dir as load_rules_from_config_dir,
    load_from_file as load_rules_from_file,
    load_from_string as load_rules_from_string,
    load_yaml_rules_from_string,
};
