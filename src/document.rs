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

pub fn load_documents(input: &Path) -> RebeResult<Vec<Document>> {
    if input.is_file() {
        return Ok(vec![Document::load_txt(input)?]);
    }

    if !input.is_dir() {
        return Err(RebeError::InvalidArgument(format!(
            "input path does not exist or is not readable: {}",
            input.display()
        )));
    }

    let mut paths = Vec::new();
    collect_text_paths(input, &mut paths)?;
    paths.sort();

    if paths.is_empty() {
        return Err(RebeError::InvalidArgument(format!(
            "no text files found under directory: {}",
            input.display()
        )));
    }

    paths
        .iter()
        .map(|path| Document::load_txt(path))
        .collect::<RebeResult<Vec<_>>>()
}

fn collect_text_paths(dir: &Path, paths: &mut Vec<PathBuf>) -> RebeResult<()> {
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();

        if path.is_dir() {
            collect_text_paths(&path, paths)?;
        } else if is_text_file(&path) {
            paths.push(path);
        }
    }

    Ok(())
}

fn is_text_file(path: &Path) -> bool {
    path.extension()
        .and_then(|extension| extension.to_str())
        .map(|extension| {
            matches!(
                extension.to_ascii_lowercase().as_str(),
                "txt" | "md" | "markdown"
            )
        })
        .unwrap_or(false)
}
