use std::fs;
use std::path::{Path, PathBuf};

use epub_parser::Epub;
use pdf_extract::extract_text_by_pages;

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
        Self::from_content(path.to_path_buf(), content)
    }

    pub fn from_content(path: PathBuf, content: String) -> RebeResult<Self> {
        if content.trim().is_empty() {
            return Err(RebeError::InvalidArgument(format!(
                "input document is empty: {}",
                path.display()
            )));
        }

        let sentences = text::split_sentences(&content);

        Ok(Self {
            path,
            content,
            sentences,
        })
    }
}

pub fn load_documents(input: &Path) -> RebeResult<Vec<Document>> {
    if input.is_file() {
        return load_document_file(input);
    }

    if !input.is_dir() {
        return Err(RebeError::InvalidArgument(format!(
            "input path does not exist or is not readable: {}",
            input.display()
        )));
    }

    let mut paths = Vec::new();
    collect_document_paths(input, &mut paths)?;
    paths.sort();

    if paths.is_empty() {
        return Err(RebeError::InvalidArgument(format!(
            "no supported text, EPUB, or PDF files found under directory: {}",
            input.display()
        )));
    }

    let mut documents = Vec::new();

    for path in paths {
        documents.extend(load_document_file(&path)?);
    }

    Ok(documents)
}

fn load_document_file(path: &Path) -> RebeResult<Vec<Document>> {
    if is_epub_file(path) {
        load_epub(path)
    } else if is_pdf_file(path) {
        load_pdf(path)
    } else if is_text_file(path) {
        Ok(vec![Document::load_txt(path)?])
    } else {
        Err(RebeError::InvalidArgument(format!(
            "unsupported input file format: {}",
            path.display()
        )))
    }
}

fn load_epub(path: &Path) -> RebeResult<Vec<Document>> {
    let epub = Epub::parse(path).map_err(|err| {
        RebeError::InvalidArgument(format!("failed to parse EPUB {}: {err}", path.display()))
    })?;
    let mut documents = Vec::new();

    for page in epub.pages {
        if page.content.trim().is_empty() {
            continue;
        }

        let page_path = PathBuf::from(format!("{}#page-{}", path.display(), page.index + 1));
        documents.push(Document::from_content(page_path, page.content)?);
    }

    if documents.is_empty() {
        return Err(RebeError::InvalidArgument(format!(
            "EPUB contains no readable text pages: {}",
            path.display()
        )));
    }

    Ok(documents)
}

fn load_pdf(path: &Path) -> RebeResult<Vec<Document>> {
    let pages = extract_text_by_pages(path).map_err(|err| {
        RebeError::InvalidArgument(format!(
            "failed to extract PDF text {}: {err}",
            path.display()
        ))
    })?;
    let mut documents = Vec::new();

    for (page_index, content) in pages.into_iter().enumerate() {
        if content.trim().is_empty() {
            continue;
        }

        let page_path = PathBuf::from(format!("{}#page-{}", path.display(), page_index + 1));
        documents.push(Document::from_content(page_path, content)?);
    }

    if documents.is_empty() {
        return Err(RebeError::InvalidArgument(format!(
            "PDF contains no extractable text pages: {}",
            path.display()
        )));
    }

    Ok(documents)
}

fn collect_document_paths(dir: &Path, paths: &mut Vec<PathBuf>) -> RebeResult<()> {
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();

        if path.is_dir() {
            collect_document_paths(&path, paths)?;
        } else if is_supported_document_file(&path) {
            paths.push(path);
        }
    }

    Ok(())
}

fn is_supported_document_file(path: &Path) -> bool {
    is_text_file(path) || is_epub_file(path) || is_pdf_file(path)
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

fn is_epub_file(path: &Path) -> bool {
    has_extension(path, "epub")
}

fn is_pdf_file(path: &Path) -> bool {
    has_extension(path, "pdf")
}

fn has_extension(path: &Path, expected: &str) -> bool {
    path.extension()
        .and_then(|extension| extension.to_str())
        .map(|extension| extension.eq_ignore_ascii_case(expected))
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::process::Command;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn loads_plain_text_document() {
        let path = temp_dir_path("text").join("book.txt");
        fs::create_dir_all(path.parent().expect("parent")).expect("create dir");
        fs::write(&path, "Readers read books.").expect("write text");

        let documents = load_documents(&path).expect("load text");

        assert_eq!(documents.len(), 1);
        assert_eq!(documents[0].sentences.len(), 1);

        fs::remove_dir_all(path.parent().expect("parent")).ok();
    }

    #[test]
    fn loads_minimal_epub_document() {
        if Command::new("zip").arg("--version").output().is_err() {
            return;
        }

        let dir_path = temp_dir_path("epub");
        let staging_path = dir_path.join("staging");
        let epub_path = dir_path.join("book.epub");

        fs::create_dir_all(staging_path.join("META-INF")).expect("create meta dir");
        fs::create_dir_all(staging_path.join("OEBPS")).expect("create oebps dir");
        fs::write(staging_path.join("mimetype"), "application/epub+zip").expect("write mimetype");
        fs::write(
            staging_path.join("META-INF").join("container.xml"),
            r#"<?xml version="1.0" encoding="UTF-8"?>
<container version="1.0" xmlns="urn:oasis:names:tc:opendocument:xmlns:container">
  <rootfiles>
    <rootfile full-path="OEBPS/content.opf" media-type="application/oebps-package+xml"/>
  </rootfiles>
</container>"#,
        )
        .expect("write container");
        fs::write(
            staging_path.join("OEBPS").join("content.opf"),
            r#"<?xml version="1.0" encoding="UTF-8"?>
<package version="3.0" xmlns="http://www.idpf.org/2007/opf">
  <metadata xmlns:dc="http://purl.org/dc/elements/1.1/">
    <dc:title>Fixture</dc:title>
    <dc:language>en</dc:language>
  </metadata>
  <manifest>
    <item id="chap1" href="chapter1.xhtml" media-type="application/xhtml+xml"/>
  </manifest>
  <spine>
    <itemref idref="chap1"/>
  </spine>
</package>"#,
        )
        .expect("write opf");
        fs::write(
            staging_path.join("OEBPS").join("chapter1.xhtml"),
            r#"<?xml version="1.0" encoding="UTF-8"?>
<html xmlns="http://www.w3.org/1999/xhtml">
  <body><p>Readers read EPUB books.</p></body>
</html>"#,
        )
        .expect("write chapter");

        let status = Command::new("zip")
            .arg("-q")
            .arg("-r")
            .arg(&epub_path)
            .arg("mimetype")
            .arg("META-INF")
            .arg("OEBPS")
            .current_dir(&staging_path)
            .status()
            .expect("zip epub");

        assert!(status.success());

        let documents = load_documents(&epub_path).expect("load epub");

        assert_eq!(documents.len(), 1);
        assert!(documents[0].content.contains("Readers read EPUB books"));

        fs::remove_dir_all(dir_path).ok();
    }

    #[test]
    fn loads_minimal_pdf_document() {
        let dir_path = temp_dir_path("pdf");
        let pdf_path = dir_path.join("book.pdf");

        fs::create_dir_all(&dir_path).expect("create dir");
        fs::write(&pdf_path, minimal_pdf_bytes("Readers read PDF books.")).expect("write pdf");

        let documents = load_documents(&pdf_path).expect("load pdf");

        assert_eq!(documents.len(), 1);
        assert!(documents[0].content.contains("Readers read PDF books"));

        fs::remove_dir_all(dir_path).ok();
    }

    fn minimal_pdf_bytes(text: &str) -> Vec<u8> {
        let escaped_text = escape_pdf_text(text);
        let stream = format!("BT\n/F1 24 Tf\n72 720 Td\n({escaped_text}) Tj\nET\n");
        let objects = vec![
            "<< /Type /Catalog /Pages 2 0 R >>".to_string(),
            "<< /Type /Pages /Kids [3 0 R] /Count 1 >>".to_string(),
            "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 612 792] /Resources << /Font << /F1 4 0 R >> >> /Contents 5 0 R >>".to_string(),
            "<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>".to_string(),
            format!(
                "<< /Length {} >>\nstream\n{}endstream",
                stream.len(),
                stream
            ),
        ];
        let mut pdf = String::from("%PDF-1.4\n");
        let mut offsets = Vec::new();

        for (index, object) in objects.iter().enumerate() {
            offsets.push(pdf.len());
            pdf.push_str(&format!("{} 0 obj\n{}\nendobj\n", index + 1, object));
        }

        let xref_offset = pdf.len();
        pdf.push_str(&format!("xref\n0 {}\n", objects.len() + 1));
        pdf.push_str("0000000000 65535 f \n");

        for offset in offsets {
            pdf.push_str(&format!("{offset:010} 00000 n \n"));
        }

        pdf.push_str(&format!(
            "trailer\n<< /Size {} /Root 1 0 R >>\nstartxref\n{}\n%%EOF\n",
            objects.len() + 1,
            xref_offset
        ));

        pdf.into_bytes()
    }

    fn escape_pdf_text(text: &str) -> String {
        text.replace('\\', "\\\\")
            .replace('(', "\\(")
            .replace(')', "\\)")
    }

    fn temp_dir_path(name: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time")
            .as_nanos();

        std::env::temp_dir().join(format!("rebe_document_{name}_{nanos}"))
    }
}
