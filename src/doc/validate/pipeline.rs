use crate::{domain::errors::Result, domain::types::ContextDocument};

/// 검증 스테이지 트레이트
pub trait ValidationStage {
    fn name(&self) -> &'static str;
    fn validate(&self, doc: &ContextDocument, body: &str) -> Result<()>;
}

/// 검증 파이프라인
pub struct ValidationPipeline {
    stages: Vec<Box<dyn ValidationStage>>,
}

impl ValidationPipeline {
    pub fn new() -> Self {
        Self { stages: Vec::new() }
    }

    /// 스테이지 추가
    pub fn add_stage<S: ValidationStage + 'static>(&mut self, stage: S) {
        self.stages.push(Box::new(stage));
    }

    /// 파이프라인 실행
    pub fn run(&self, doc: &ContextDocument, body: &str) -> Result<ValidationReport> {
        let mut report = ValidationReport::new();

        for stage in &self.stages {
            match stage.validate(doc, body) {
                Ok(()) => {
                    report.passed_stages.push(stage.name().to_string());
                }
                Err(e) => {
                    report.failed_stages.push(ValidationFailure {
                        stage_name: stage.name().to_string(),
                        error: e.to_string(),
                    });
                }
            }
        }

        report.is_valid = report.failed_stages.is_empty();
        Ok(report)
    }

    /// 스테이지 개수
    pub fn stage_count(&self) -> usize {
        self.stages.len()
    }
}

impl Default for ValidationPipeline {
    fn default() -> Self {
        Self::new()
    }
}

/// 검증 보고서
#[derive(Debug, Clone)]
pub struct ValidationReport {
    pub is_valid: bool,
    pub passed_stages: Vec<String>,
    pub failed_stages: Vec<ValidationFailure>,
}

impl ValidationReport {
    pub fn new() -> Self {
        Self {
            is_valid: true,
            passed_stages: Vec::new(),
            failed_stages: Vec::new(),
        }
    }

    /// 성공한 스테이지 수
    pub fn passed_count(&self) -> usize {
        self.passed_stages.len()
    }

    /// 실패한 스테이지 수
    pub fn failed_count(&self) -> usize {
        self.failed_stages.len()
    }

    /// 전체 스테이지 수
    pub fn total_count(&self) -> usize {
        self.passed_count() + self.failed_count()
    }
}

impl Default for ValidationReport {
    fn default() -> Self {
        Self::new()
    }
}

/// 검증 실패 정보
#[derive(Debug, Clone)]
pub struct ValidationFailure {
    pub stage_name: String,
    pub error: String,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ContextDocument;

    struct TestStage {
        name: &'static str,
        should_fail: bool,
    }

    impl ValidationStage for TestStage {
        fn name(&self) -> &'static str {
            self.name
        }

        fn validate(&self, _doc: &ContextDocument, _body: &str) -> Result<()> {
            if self.should_fail {
                Err(crate::domain::errors::ContextError::InvalidFrontmatter(
                    "Test failure".to_string(),
                ))
            } else {
                Ok(())
            }
        }
    }

    #[test]
    #[allow(clippy::unwrap_used)] // 테스트 코드에서는 허용
    fn test_pipeline_all_pass() {
        let mut pipeline = ValidationPipeline::new();
        pipeline.add_stage(TestStage {
            name: "stage1",
            should_fail: false,
        });
        pipeline.add_stage(TestStage {
            name: "stage2",
            should_fail: false,
        });

        let doc = ContextDocument::new_test("test", "Test Document");
        let report = pipeline.run(&doc, "test body").unwrap();

        assert!(report.is_valid);
        assert_eq!(report.passed_count(), 2);
        assert_eq!(report.failed_count(), 0);
    }

    #[test]
    #[allow(clippy::unwrap_used)] // 테스트 코드에서는 허용
    fn test_pipeline_some_fail() {
        let mut pipeline = ValidationPipeline::new();
        pipeline.add_stage(TestStage {
            name: "stage1",
            should_fail: false,
        });
        pipeline.add_stage(TestStage {
            name: "stage2",
            should_fail: true,
        });
        pipeline.add_stage(TestStage {
            name: "stage3",
            should_fail: false,
        });

        let doc = ContextDocument::new_test("test", "Test Document");
        let report = pipeline.run(&doc, "test body").unwrap();

        assert!(!report.is_valid);
        assert_eq!(report.passed_count(), 2);
        assert_eq!(report.failed_count(), 1);
        assert_eq!(report.failed_stages[0].stage_name, "stage2");
    }
}
