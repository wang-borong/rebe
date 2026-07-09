use std::fs;
use std::path::{Path, PathBuf};

use crate::error::{RebeError, RebeResult};
use crate::text;

#[derive(Debug, Clone)]
pub struct Document {
    pub path: PathBuf,
    pub content: String,
    pub sentences: Vec<String>,
}

impl Document {
    pub fn load_txt(path: &Path) -> RebeResult<Self> {
        let content = fs::read_to_string(path)?;

        if content.trim().is_empty() {
            return Err(RebeError::InvalidArgument(format!(
                "input file is empty: {}",
                path.display()
            )));
        }

        let sentences = text::split_sentences(&content);

        Ok(Self {
            path: path.to_path_buf(),
            content,
            sentences,
        })
    }
}
