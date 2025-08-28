//! CLI 명령어 모듈들
//!
//! 각 CLI 명령어를 독립적인 모듈로 분리하여 관리합니다.

pub mod classify;

pub mod import;
pub mod index;
pub mod mcp;
pub mod parse;
#[cfg(feature = "server")]
pub mod server;
pub mod validate;

// 명령어 실행 함수들을 re-export
pub use classify::{ClassifyArgs, run_classify};

pub use import::{ImportArgs, run_import};
pub use index::{IndexArgs, run_index};
pub use mcp::run_mcp;
pub use parse::{ParseArgs, run_parse};
#[cfg(feature = "server")]
pub use server::{ServerArgs, run_server};
pub use validate::{ValidateArgs, run_validate};

// IndexRepo는 제거됨 - 대신 server 명령어 사용
