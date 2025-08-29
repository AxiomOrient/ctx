use std::fs;
use std::path::{Path, PathBuf};

use ctx::domain::types::ComposeInput;
use ctx::services;

fn cases_dir() -> PathBuf {
    PathBuf::from("tests/golden/cases/compose")
}

fn golden_path_for(input: &Path) -> PathBuf {
    let stem = input.file_stem().unwrap().to_string_lossy();
    let mut out = input.parent().unwrap().to_path_buf();
    out.push(format!("{}.out.golden.json", stem));
    out
}

fn normalize_json(mut v: serde_json::Value) -> serde_json::Value {
    // Sort sources array if present to stabilize output
    if let Some(obj) = v.as_object_mut() {
        if let Some(sources) = obj.get_mut("sources").and_then(|v| v.as_array_mut()) {
            sources.sort_by_key(|a| a.to_string());
        }
    }
    v
}

fn write_pretty_json(path: &Path, v: &serde_json::Value) {
    let pretty = serde_json::to_string_pretty(v).expect("serialize json");
    if let Some(dir) = path.parent() { fs::create_dir_all(dir).ok(); }
    fs::write(path, pretty + "\n").expect("write golden");
}

#[test]
fn golden_compose_cases() {
    let update = std::env::var("UPDATE_GOLDEN").ok().filter(|v| v == "1").is_some();

    let dir = cases_dir();
    let mut inputs: Vec<PathBuf> = fs::read_dir(&dir)
        .unwrap_or_else(|_| panic!("missing test cases dir: {}", dir.display()))
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.extension().map(|e| e == "json").unwrap_or(false))
        .filter(|p| p.file_name().unwrap().to_string_lossy().ends_with(".in.json"))
        .collect();
    inputs.sort();

    assert!(!inputs.is_empty(), "no golden compose input cases found under {}", dir.display());

    for input_path in inputs {
        let text = fs::read_to_string(&input_path)
            .unwrap_or_else(|e| panic!("failed to read {}: {}", input_path.display(), e));
        let ci: ComposeInput = serde_json::from_str(&text)
            .unwrap_or_else(|e| panic!("invalid JSON {}: {}", input_path.display(), e));

        let result = services::compose_prompt(ci).expect("compose prompt should succeed");
        let mut val = serde_json::to_value(&result).expect("serialize result");
        val = normalize_json(val);

        let golden_path = golden_path_for(&input_path);
        if update || !golden_path.exists() {
            write_pretty_json(&golden_path, &val);
            eprintln!("updated golden: {}", golden_path.display());
            continue;
        }

        let expected_text = fs::read_to_string(&golden_path)
            .unwrap_or_else(|e| panic!("failed to read golden {}: {}", golden_path.display(), e));
        let expected: serde_json::Value = serde_json::from_str(&expected_text)
            .unwrap_or_else(|e| panic!("invalid golden JSON {}: {}", golden_path.display(), e));

        if val != expected {
            let actual_pretty = serde_json::to_string_pretty(&val).unwrap();
            panic!(
                "golden mismatch for {}\nEXPECTED:\n{}\nACTUAL:\n{}\n\nTo update goldens: UPDATE_GOLDEN=1 cargo test -q",
                input_path.display(), expected_text, actual_pretty
            );
        }
    }
}

