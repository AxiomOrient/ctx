//! Classify 명령어 구현
//!
//! 단일 파일을 온톨로지 기반으로 분류하여 패싯을 추출합니다.

use crate::app::cli::utils::format_output;
use crate::app::config::AppConfig;
use ctx_core::common::utils::extract_title_and_body;
use ctx_core::core::classifier::facet::Classification;
use ctx_core::core::classifier::service::ClassifierService;
use ctx_core::Result;
use colored::*;
use std::fs;
use std::path::PathBuf;

/// Classify 명령어 인자
#[derive(Debug, Clone)]
pub struct ClassifyArgs {
    pub file: PathBuf,
    pub ontology: Option<PathBuf>,
    pub format: String,
    pub confidence_threshold: f32,
}

/// Classify 명령어 실행
///
/// # Arguments
/// * `args` - classify 명령어 인자
///
/// # Returns
/// * 성공 시 Ok(()), 실패 시 ContextError
pub fn run_classify(args: ClassifyArgs) -> Result<()> {
    println!(
        "{}",
        format!("🎯 Classifying file: {}", args.file.display()).cyan()
    );

    // 2. 온톨로지와 룰 파일 로드
    // Allow config defaults from ctxset.toml
    let cfg = AppConfig::load_from_current_dir();
    let ontology_path = args
        .ontology
        .or_else(|| cfg.as_ref().and_then(|c| c.ontology.clone()))
        .unwrap_or_else(|| PathBuf::from("ontology.yaml"));
    let ontology_content = fs::read_to_string(&ontology_path)?;

    // 룰 파일 로드 (옵션)
    let rules_path = cfg
        .as_ref()
        .and_then(|c| c.rules.clone())
        .unwrap_or_else(|| PathBuf::from("rules.yaml"));
    let rules_content = if rules_path.exists() {
        Some(fs::read_to_string(rules_path)?)
    } else {
        None
    };

    // 3. 파일 읽기 및 메타데이터 추출
    let content = fs::read_to_string(&args.file)?;
    let (title, body) = extract_title_and_body(&content);

    // 4. ClassifierService를 사용한 실제 분류 수행
    let classification = ClassifierService::classify_with_yaml(
        &ontology_content,
        rules_content.as_deref(),
        &title.unwrap_or_default(),
        &body,
    )?;

    // 5. 결과 출력
    match args.format.as_str() {
        "human" => print_human_classification(&classification, args.confidence_threshold),
        "json" | "yaml" => {
            let output = format_output(&classification, &args.format)?;
            println!("{}", output);
        }
        _ => {
            return Err(ctx_core::ContextError::OptimizationError(format!(
                "Unsupported format: {}",
                args.format
            )));
        }
    }

    Ok(())
}

/// 사람이 읽기 쉬운 형태로 분류 결과 출력
fn print_human_classification(classification: &Classification, threshold: f32) {
    println!("\n🎯 분류 결과:");

    // 신뢰도 표시
    let confidence = classification.confidence();
    let _confidence_color = if confidence >= threshold {
        "green"
    } else if confidence >= 0.5 {
        "yellow"
    } else {
        "red"
    };

    println!(
        "신뢰도: {:.2} {}",
        confidence,
        if confidence >= threshold {
            "✅"
        } else {
            "⚠️"
        }
    );

    // 패싯 표시
    if !classification.facets().is_empty() {
        println!("\n📋 감지된 패싯:");
        for (namespace, values) in classification.facets().iter() {
            println!("  {}: {:?}", namespace.bold(), values);
        }
    } else {
        println!("\n📋 감지된 패싯: 없음");
    }

    // 경고 표시
    if !classification.warnings().is_empty() {
        println!("\n⚠️ 경고:");
        for warning in classification.warnings() {
            println!("  - {}", warning.yellow());
        }
    }

    // 오류 표시
    if !classification.errors().is_empty() {
        println!("\n❌ 오류:");
        for error in classification.errors() {
            println!("  - {}", error.red());
        }
    }

    // 빌드 가능 여부
    let is_buildable = classification.is_valid();
    println!(
        "\n🚀 빌드 가능: {}",
        if is_buildable {
            "예 ✅".green()
        } else {
            "아니오 ❌".red()
        }
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    #[test]
    fn test_run_classify_basic() -> Result<()> {
        // 임시 파일 생성
        let mut temp_file = NamedTempFile::new().map_err(ctx_core::ContextError::Io)?;

        writeln!(
            temp_file,
            "# SwiftUI Guide\n\nThis is about SwiftUI development for iOS."
        )
        .map_err(ctx_core::ContextError::Io)?;

        // 임시 온톨로지 파일 생성
        let mut ontology_file = NamedTempFile::new().map_err(ctx_core::ContextError::Io)?;

        writeln!(
            ontology_file,
            r#"
namespaces:
  tech:
    values:
      swiftui:
        synonyms: []
        parents: []
        deprecated: false
"#
        )
        .map_err(ctx_core::ContextError::Io)?;

        let args = ClassifyArgs {
            file: temp_file.path().to_path_buf(),
            ontology: Some(ontology_file.path().to_path_buf()),
            format: "json".to_string(),
            confidence_threshold: 0.5,
        };

        // 실행 (오류가 없어야 함)
        run_classify(args)?;

        Ok(())
    }
}
