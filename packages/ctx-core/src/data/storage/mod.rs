use crate::Result;
use std::path::{Path, PathBuf};

/// 파일 저장/로딩을 위한 추상화 트레이트
pub trait Storage {
    /// 파일 내용 읽기
    fn read(&self, path: &Path) -> Result<String>;

    /// 파일 내용 쓰기
    fn write(&self, path: &Path, content: &str) -> Result<()>;

    /// 패턴에 맞는 파일 목록 조회
    fn list_files(&self, folder: &Path, pattern: &str) -> Result<Vec<PathBuf>>;

    /// 파일 존재 여부 확인
    fn exists(&self, path: &Path) -> bool;

    /// 디렉토리 생성
    fn create_dir_all(&self, path: &Path) -> Result<()>;
}

pub mod local;

pub use local::LocalFsStorage;
