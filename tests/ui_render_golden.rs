#[cfg(feature = "ui_render_rust")]
mod golden {
    use std::fs;
    use std::path::{Path, PathBuf};

    fn cases_dir() -> PathBuf { PathBuf::from("tests/golden/ui_render") }

    fn golden_path_for(input: &Path) -> PathBuf {
        let stem = input.file_stem().unwrap().to_string_lossy();
        input.parent().unwrap().join(format!("{}.html.golden", stem))
    }

    fn write_pretty(path: &Path, s: &str) {
        if let Some(dir) = path.parent() { let _ = fs::create_dir_all(dir); }
        fs::write(path, s).expect("write golden");
    }

    #[test]
    fn render_markdown_golden() {
        let update = std::env::var("UPDATE_GOLDEN").ok().filter(|v| v == "1").is_some();
        let dir = cases_dir();
        let mut inputs: Vec<PathBuf> = fs::read_dir(&dir)
            .unwrap_or_else(|_| panic!("missing cases dir: {}", dir.display()))
            .filter_map(|e| e.ok())
            .map(|e| e.path())
            .filter(|p| p.extension().map(|e| e == "md").unwrap_or(false))
            .collect();
        inputs.sort();
        assert!(!inputs.is_empty());

    for input in inputs {
        let md = fs::read_to_string(&input).expect("read");
        let html = ctx::app::ui::render::render_markdown_to_safe_html(&md);
        // Sanitizer checks for malicious content case
        if input.file_name().unwrap().to_string_lossy().contains("links_and_sanitize") {
            assert!(!html.contains("<script"));
            assert!(!html.contains("onerror="));
            assert!(!html.contains("javascript:"));
        }
        let golden = golden_path_for(&input);
        if update || !golden.exists() {
            write_pretty(&golden, &html);
            eprintln!("updated golden {}", golden.display());
            continue;
        }
            let expected = fs::read_to_string(&golden).expect("read golden");
            assert_eq!(html, expected, "golden mismatch: {}", input.display());
        }
    }
}
