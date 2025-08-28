use crate::domain::errors::Result;
use std::path::{Path, PathBuf};

pub mod cache;
pub mod local;
pub mod sqlite;

pub trait Storage {
    fn read(&self, path: &Path) -> Result<String>;
    fn write(&self, path: &Path, content: &str) -> Result<()>;
    fn list_files(&self, folder: &Path, pattern: &str) -> Result<Vec<PathBuf>>;
    fn exists(&self, path: &Path) -> bool;
    fn create_dir_all(&self, path: &Path) -> Result<()>;
}
