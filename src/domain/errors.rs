use thiserror::Error;

#[derive(Debug, Error)]
pub enum ContextError {
    #[error("Parse error at line {line}: {message}")]
    ParseError { line: usize, message: String },

    #[error("Section '{section_id}' not found. Available sections: {available:?}")]
    SectionNotFound {
        section_id: String,
        available: Vec<String>,
    },

    #[error("Marker '{marker}' not found in document")]
    MarkerNotFound { marker: String },

    #[error("Invalid frontmatter: {0}")]
    InvalidFrontmatter(String),

    #[error("Token budget {budget} exceeded by {excess} tokens")]
    TokenBudgetExceeded { budget: u32, excess: u32 },

    #[error("Optimization failed: {0}")]
    OptimizationError(String),

    #[error("Assembly failed: {0}")]
    AssemblyError(String),

    #[error("Configuration error: {0}")]
    ConfigError(String),

    #[error("Server error: {0}")]
    ServerError(String),

    #[error("Classification failed: {0}")]
    ClassificationError(String),

    #[error("Composition failed: {0}")]
    CompositionError(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("YAML error: {0}")]
    YamlError(#[from] serde_yaml::Error),

    #[error("JSON error: {0}")]
    JsonError(#[from] serde_json::Error),

    #[error("Database error: {0}")]
    DatabaseError(#[from] sqlx::Error),

    #[error("Other error: {0}")]
    Other(String),
}

pub type Result<T> = std::result::Result<T, ContextError>;

/// 로깅 정책: stdout은 결과/머신판독용, stderr은 로그용
pub fn log_info(msg: &str) {
    eprintln!("[info] {}", msg);
}

pub fn log_warn(msg: &str) {
    eprintln!("[warn] {}", msg);
}

pub fn log_error(msg: &str) {
    eprintln!("[error] {}", msg);
}

/// i18n 최소 적용: 영어 기본 메시지
pub fn user_msg(key: &str) -> &'static str {
    match key {
        "parse_failed" => "Failed to parse the document.",
        "schema_invalid" => "Schema validation failed.",
        "marker_not_found" => "Marker not found in document.",
        "token_budget_exceeded" => "Token budget exceeded.",
        "optimization_failed" => "Optimization process failed.",
        _ => "Unexpected error occurred.",
    }
}

impl ContextError {
    pub fn parse_error(line: usize, message: impl Into<String>) -> Self {
        Self::ParseError {
            line,
            message: message.into(),
        }
    }

    pub fn section_not_found(section_id: impl Into<String>, available: Vec<String>) -> Self {
        Self::SectionNotFound {
            section_id: section_id.into(),
            available,
        }
    }

    pub fn marker_not_found(marker: impl Into<String>) -> Self {
        Self::MarkerNotFound {
            marker: marker.into(),
        }
    }

    /// 사용자 친화적인 에러 메시지와 해결 방법 제공
    pub fn user_friendly_message(&self) -> String {
        match self {
            ContextError::ParseError { line, message } => {
                format!(
                    "❌ 파싱 오류 ({}번째 줄): {}\n\n💡 해결 방법:\n  • YAML 문법을 확인해주세요\n  • 들여쓰기가 올바른지 확인해주세요",
                    line, message
                )
            }
            ContextError::MarkerNotFound { marker } => {
                format!(
                    "❌ 마커를 찾을 수 없습니다: '{}'\n\n💡 해결 방법:\n  • 문서에 해당 헤딩이 있는지 확인해주세요\n  • 마커 형식이 정확한지 확인해주세요 (예: '## 제목')",
                    marker
                )
            }
            ContextError::InvalidFrontmatter(msg) => {
                format!(
                    "❌ 잘못된 frontmatter: {}\n\n💡 해결 방법:\n  • 문서가 '---'로 시작하고 끝나는지 확인해주세요\n  • YAML 형식이 올바른지 확인해주세요",
                    msg
                )
            }
            ContextError::TokenBudgetExceeded { budget, excess } => {
                format!(
                    "❌ 토큰 예산 초과: {}토큰 초과 (예산: {})\n\n💡 해결 방법:\n  • 토큰 예산을 늘려주세요\n  • 섹션 우선순위를 조정해주세요",
                    excess, budget
                )
            }
            ContextError::OptimizationError(msg) => {
                format!(
                    "❌ 최적화 오류: {}\n\n💡 해결 방법:\n  • 입력 데이터를 확인해주세요\n  • 토큰 예산이 0보다 큰지 확인해주세요",
                    msg
                )
            }
            _ => self.to_string(),
        }
    }
}
