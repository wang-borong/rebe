use std::collections::HashMap;
use std::fs;
use std::io::{self, Cursor};
use std::path::{Path, PathBuf};

use boko::{Book, Format};
use docx_lite::extract_text;
use epub_parser::{Epub, TocEntry, ZipHandler};
use pdf_extract::extract_text_by_pages;
use quick_xml::events::Event;
use quick_xml::Reader;

use crate::error::{RebeError, RebeResult};
use crate::text;

#[derive(Debug, Clone)]
pub struct Document {
    pub path: PathBuf,
    pub content: String,
    pub sentences: Vec<String>,
}

#[derive(Debug)]
struct EpubManifestItem {
    href: String,
    media_type: String,
}

#[derive(Debug)]
struct EpubLayout {
    spine_paths: Vec<String>,
    ncx_path: Option<String>,
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
            "no supported text, EPUB, PDF, DOCX, or Kindle files found under directory: {}",
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
    } else if is_docx_file(path) {
        load_docx(path)
    } else if is_kindle_file(path) {
        load_kindle(path)
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
    let chapter_labels = load_epub_chapter_labels(path, &epub);
    let mut documents = Vec::new();

    for page in epub.pages {
        if page.content.trim().is_empty() {
            continue;
        }

        let chapter_label = chapter_labels
            .get(page.index)
            .and_then(|label| label.as_deref());
        let page_path = epub_page_path(path, page.index, chapter_label);
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

fn load_epub_chapter_labels(path: &Path, epub: &Epub) -> Vec<Option<String>> {
    let fallback = vec![None; epub.pages.len()];
    let Some(layout) = load_epub_layout(path) else {
        return fallback;
    };
    let Some(ncx_path) = layout.ncx_path else {
        return fallback;
    };

    if layout.spine_paths.len() != epub.pages.len() {
        return fallback;
    }

    let labels_by_path = toc_labels_by_path(&epub.toc, &ncx_path);

    layout
        .spine_paths
        .into_iter()
        .map(|spine_path| labels_by_path.get(&spine_path).cloned())
        .collect()
}

fn load_epub_layout(path: &Path) -> Option<EpubLayout> {
    let mut zip_handler = ZipHandler::new(path).ok()?;
    let opf_path = zip_handler.get_opf_path().ok()?;
    let opf_content = zip_handler.read_file(&opf_path).ok()?;

    parse_epub_layout(&opf_content, &opf_path)
}

fn parse_epub_layout(opf_content: &str, opf_path: &str) -> Option<EpubLayout> {
    let mut reader = Reader::from_str(opf_content);
    let mut manifest = HashMap::new();
    let mut spine_ids = Vec::new();
    let mut buffer = Vec::new();

    loop {
        match reader.read_event_into(&mut buffer) {
            Ok(Event::Empty(element)) | Ok(Event::Start(element)) => {
                let name = String::from_utf8_lossy(element.name().as_ref()).into_owned();

                if xml_element_name_matches(&name, "item") {
                    let mut id = None;
                    let mut href = None;
                    let mut media_type = None;

                    for attribute in element.attributes() {
                        let Ok(attribute) = attribute else {
                            continue;
                        };
                        let attribute_name = String::from_utf8_lossy(attribute.key.as_ref());
                        let Ok(value) = attribute.decode_and_unescape_value(reader.decoder())
                        else {
                            continue;
                        };

                        match xml_local_name(&attribute_name) {
                            "id" => id = Some(value.into_owned()),
                            "href" => href = Some(value.into_owned()),
                            "media-type" => media_type = Some(value.into_owned()),
                            _ => {}
                        }
                    }

                    if let (Some(id), Some(href), Some(media_type)) = (id, href, media_type) {
                        manifest.insert(id, EpubManifestItem { href, media_type });
                    }
                } else if xml_element_name_matches(&name, "itemref") {
                    for attribute in element.attributes() {
                        let Ok(attribute) = attribute else {
                            continue;
                        };
                        let attribute_name = String::from_utf8_lossy(attribute.key.as_ref());

                        if xml_local_name(&attribute_name) != "idref" {
                            continue;
                        }

                        if let Ok(value) = attribute.decode_and_unescape_value(reader.decoder()) {
                            spine_ids.push(value.into_owned());
                        }

                        break;
                    }
                }
            }
            Ok(Event::Eof) => break,
            Err(_) => return None,
            _ => {}
        }

        buffer.clear();
    }

    let spine_paths = spine_ids
        .iter()
        .filter_map(|id| manifest.get(id))
        .filter_map(|item| resolve_epub_archive_path(opf_path, &item.href))
        .collect::<Vec<_>>();
    let ncx_path = manifest
        .values()
        .find(|item| {
            item.media_type
                .eq_ignore_ascii_case("application/x-dtbncx+xml")
        })
        .and_then(|item| resolve_epub_archive_path(opf_path, &item.href));

    if spine_paths.is_empty() {
        return None;
    }

    Some(EpubLayout {
        spine_paths,
        ncx_path,
    })
}

fn toc_labels_by_path(toc: &[TocEntry], ncx_path: &str) -> HashMap<String, String> {
    let mut labels_by_path = HashMap::new();

    collect_toc_labels(toc, ncx_path, &mut labels_by_path);

    labels_by_path
}

fn collect_toc_labels(
    entries: &[TocEntry],
    ncx_path: &str,
    labels_by_path: &mut HashMap<String, String>,
) {
    for entry in entries {
        if let (Some(path), Some(label)) = (
            resolve_epub_archive_path(ncx_path, &entry.href),
            normalize_epub_chapter_label(&entry.label),
        ) {
            labels_by_path.entry(path).or_insert(label);
        }

        collect_toc_labels(&entry.children, ncx_path, labels_by_path);
    }
}

fn resolve_epub_archive_path(base_path: &str, href: &str) -> Option<String> {
    let href = href.split('#').next()?.trim();

    if href.is_empty() {
        return None;
    }

    let mut components = if href.starts_with('/') {
        Vec::new()
    } else {
        base_path
            .split('/')
            .filter(|component| !component.is_empty())
            .collect::<Vec<_>>()
    };

    if !href.starts_with('/') {
        components.pop();
    }

    for component in href.split('/') {
        match component {
            "" | "." => {}
            ".." => {
                components.pop();
            }
            _ => components.push(component),
        }
    }

    if components.is_empty() {
        return None;
    }

    Some(components.join("/"))
}

fn normalize_epub_chapter_label(label: &str) -> Option<String> {
    let label = label.split_whitespace().collect::<Vec<_>>().join(" ");

    if label.is_empty() {
        None
    } else {
        Some(label)
    }
}

fn epub_page_path(path: &Path, page_index: usize, chapter_label: Option<&str>) -> PathBuf {
    match chapter_label {
        Some(label) => PathBuf::from(format!(
            "{}#chapter-{}-{label}",
            path.display(),
            page_index + 1
        )),
        None => PathBuf::from(format!("{}#page-{}", path.display(), page_index + 1)),
    }
}

fn xml_element_name_matches(name: &str, expected: &str) -> bool {
    xml_local_name(name) == expected
}

fn xml_local_name(name: &str) -> &str {
    name.rsplit(':').next().unwrap_or(name)
}

fn load_docx(path: &Path) -> RebeResult<Vec<Document>> {
    let content = extract_text(path).map_err(|err| {
        RebeError::InvalidArgument(format!(
            "failed to extract DOCX text {}: {err}",
            path.display()
        ))
    })?;

    Ok(vec![Document::from_content(path.to_path_buf(), content)?])
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

fn load_kindle(path: &Path) -> RebeResult<Vec<Document>> {
    let mut errors = Vec::new();

    for format in kindle_formats(path) {
        match extract_kindle_markdown(path, format) {
            Ok(content) if !content.trim().is_empty() => {
                return Ok(vec![Document::from_content(path.to_path_buf(), content)?]);
            }
            Ok(_) => {
                errors.push(format!("{format:?}: no readable text"));
            }
            Err(err) => {
                errors.push(format!("{format:?}: {err}"));
            }
        }
    }

    let details = if errors.is_empty() {
        "no matching Kindle parser was selected".to_string()
    } else {
        errors.join("; ")
    };

    Err(RebeError::InvalidArgument(format!(
        "failed to extract Kindle text {}: {details}. Only non-DRM AZW3/AZW/MOBI/KFX files are supported",
        path.display()
    )))
}

fn extract_kindle_markdown(path: &Path, format: Format) -> io::Result<String> {
    let mut book = Book::open_format(path, format)?;
    let mut output = Cursor::new(Vec::new());
    book.export(Format::Markdown, &mut output)?;
    String::from_utf8(output.into_inner()).map_err(|err| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            format!("exported Markdown is not valid UTF-8: {err}"),
        )
    })
}

fn kindle_formats(path: &Path) -> Vec<Format> {
    match normalized_extension(path).as_deref() {
        Some("azw3") => vec![Format::Azw3],
        Some("azw") => vec![Format::Azw3, Format::Mobi],
        Some("mobi") => vec![Format::Mobi],
        Some("kfx") => vec![Format::Kfx],
        _ => Vec::new(),
    }
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
    is_text_file(path)
        || is_epub_file(path)
        || is_pdf_file(path)
        || is_docx_file(path)
        || is_kindle_file(path)
}

fn is_text_file(path: &Path) -> bool {
    matches!(
        normalized_extension(path).as_deref(),
        Some("txt" | "md" | "markdown")
    )
}

fn is_epub_file(path: &Path) -> bool {
    has_extension(path, "epub")
}

fn is_pdf_file(path: &Path) -> bool {
    has_extension(path, "pdf")
}

fn is_docx_file(path: &Path) -> bool {
    has_extension(path, "docx")
}

fn is_kindle_file(path: &Path) -> bool {
    matches!(
        normalized_extension(path).as_deref(),
        Some("azw3" | "azw" | "mobi" | "kfx")
    )
}

fn has_extension(path: &Path, expected: &str) -> bool {
    normalized_extension(path)
        .map(|extension| extension == expected)
        .unwrap_or(false)
}

fn normalized_extension(path: &Path) -> Option<String> {
    path.extension()
        .and_then(|extension| extension.to_str())
        .map(|extension| extension.to_ascii_lowercase())
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
        let epub_path = dir_path.join("book.epub");

        write_minimal_epub(&epub_path, "Readers read EPUB books.").expect("write epub fixture");

        let documents = load_documents(&epub_path).expect("load epub");

        assert_eq!(documents.len(), 1);
        assert!(documents[0].content.contains("Readers read EPUB books"));

        fs::remove_dir_all(dir_path).ok();
    }

    #[test]
    fn labels_epub_documents_with_ncx_chapter_titles() {
        if Command::new("zip").arg("--version").output().is_err() {
            return;
        }

        let dir_path = temp_dir_path("epub_chapters");
        let epub_path = dir_path.join("book.epub");
        let chapters = [
            EpubFixtureChapter {
                filename: "opening.xhtml",
                label: "Opening",
                content: "Readers enter the opening chapter.",
            },
            EpubFixtureChapter {
                filename: "journey.xhtml",
                label: "The Journey",
                content: "Readers continue the journey.",
            },
        ];

        write_epub_with_toc(&epub_path, &chapters).expect("write epub fixture");

        let documents = load_documents(&epub_path).expect("load epub");

        assert_eq!(documents.len(), 2);
        assert_eq!(
            documents[0].path,
            PathBuf::from(format!("{}#chapter-1-Opening", epub_path.display()))
        );
        assert_eq!(
            documents[1].path,
            PathBuf::from(format!("{}#chapter-2-The Journey", epub_path.display()))
        );

        fs::remove_dir_all(dir_path).ok();
    }

    #[test]
    fn loads_generated_azw3_document() {
        if Command::new("zip").arg("--version").output().is_err() {
            return;
        }

        let dir_path = temp_dir_path("azw3");
        let epub_path = dir_path.join("book.epub");
        let azw3_path = dir_path.join("book.azw3");

        write_minimal_epub(&epub_path, "Readers read AZW3 books.").expect("write epub fixture");

        let mut book = Book::open(&epub_path).expect("open epub with boko");
        let mut output = Cursor::new(Vec::new());
        book.export(Format::Azw3, &mut output)
            .expect("export azw3 fixture");
        fs::write(&azw3_path, output.into_inner()).expect("write azw3");

        let documents = load_documents(&azw3_path).expect("load azw3");

        assert_eq!(documents.len(), 1);
        assert!(documents[0].content.contains("Readers read AZW3 books"));

        fs::remove_dir_all(dir_path).ok();
    }

    #[test]
    fn recognizes_kindle_document_extensions() {
        assert!(is_supported_document_file(Path::new("book.AZW3")));
        assert!(is_supported_document_file(Path::new("book.azw")));
        assert!(is_supported_document_file(Path::new("book.mobi")));
        assert!(is_supported_document_file(Path::new("book.kfx")));
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

    #[test]
    fn loads_minimal_docx_document() {
        if Command::new("zip").arg("--version").output().is_err() {
            return;
        }

        let dir_path = temp_dir_path("docx");
        let staging_path = dir_path.join("staging");
        let docx_path = dir_path.join("book.docx");

        fs::create_dir_all(staging_path.join("_rels")).expect("create rels dir");
        fs::create_dir_all(staging_path.join("word")).expect("create word dir");
        fs::write(
            staging_path.join("[Content_Types].xml"),
            r#"<?xml version="1.0" encoding="UTF-8"?>
<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types">
  <Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/>
  <Default Extension="xml" ContentType="application/xml"/>
  <Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/>
</Types>"#,
        )
        .expect("write content types");
        fs::write(
            staging_path.join("_rels").join(".rels"),
            r#"<?xml version="1.0" encoding="UTF-8"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/>
</Relationships>"#,
        )
        .expect("write rels");
        fs::write(
            staging_path.join("word").join("document.xml"),
            r#"<?xml version="1.0" encoding="UTF-8"?>
<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
  <w:body>
    <w:p><w:r><w:t>Readers read DOCX books.</w:t></w:r></w:p>
  </w:body>
</w:document>"#,
        )
        .expect("write document");

        let status = Command::new("zip")
            .arg("-q")
            .arg("-r")
            .arg(&docx_path)
            .arg("[Content_Types].xml")
            .arg("_rels")
            .arg("word")
            .current_dir(&staging_path)
            .status()
            .expect("zip docx");

        assert!(status.success());

        let documents = load_documents(&docx_path).expect("load docx");

        assert_eq!(documents.len(), 1);
        assert!(documents[0].content.contains("Readers read DOCX books"));

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

    struct EpubFixtureChapter<'a> {
        filename: &'a str,
        label: &'a str,
        content: &'a str,
    }

    fn write_minimal_epub(path: &Path, text: &str) -> io::Result<()> {
        let chapters = [EpubFixtureChapter {
            filename: "chapter1.xhtml",
            label: "Chapter 1",
            content: text,
        }];

        write_epub_fixture(path, &chapters, false)
    }

    fn write_epub_with_toc(path: &Path, chapters: &[EpubFixtureChapter<'_>]) -> io::Result<()> {
        write_epub_fixture(path, chapters, true)
    }

    fn write_epub_fixture(
        path: &Path,
        chapters: &[EpubFixtureChapter<'_>],
        include_toc: bool,
    ) -> io::Result<()> {
        let staging_path = path
            .parent()
            .expect("epub parent")
            .join("staging")
            .join(path.file_stem().expect("epub file stem"));

        fs::create_dir_all(staging_path.join("META-INF"))?;
        fs::create_dir_all(staging_path.join("OEBPS"))?;
        fs::write(staging_path.join("mimetype"), "application/epub+zip")?;
        fs::write(
            staging_path.join("META-INF").join("container.xml"),
            r#"<?xml version="1.0" encoding="UTF-8"?>
<container version="1.0" xmlns="urn:oasis:names:tc:opendocument:xmlns:container">
  <rootfiles>
    <rootfile full-path="OEBPS/content.opf" media-type="application/oebps-package+xml"/>
  </rootfiles>
</container>"#,
        )?;
        fs::write(
            staging_path.join("OEBPS").join("content.opf"),
            format!(
                r#"<?xml version="1.0" encoding="UTF-8"?>
<package version="3.0" unique-identifier="bookid" xmlns="http://www.idpf.org/2007/opf">
  <metadata xmlns:dc="http://purl.org/dc/elements/1.1/">
    <dc:identifier id="bookid">urn:uuid:rebe-fixture</dc:identifier>
    <dc:title>Fixture</dc:title>
    <dc:language>en</dc:language>
  </metadata>
  <manifest>
    {}
    {}
  </manifest>
  <spine{}>
    {}
  </spine>
</package>"#,
                chapters
                    .iter()
                    .enumerate()
                    .map(|(index, chapter)| format!(
                        r#"<item id="chap{}" href="{}" media-type="application/xhtml+xml"/>"#,
                        index + 1,
                        chapter.filename
                    ))
                    .collect::<Vec<_>>()
                    .join("\n    "),
                if include_toc {
                    r#"<item id="toc" href="toc.ncx" media-type="application/x-dtbncx+xml"/>"#
                } else {
                    ""
                },
                if include_toc { r#" toc="toc""# } else { "" },
                chapters
                    .iter()
                    .enumerate()
                    .map(|(index, _)| format!(r#"<itemref idref="chap{}"/>"#, index + 1))
                    .collect::<Vec<_>>()
                    .join("\n    "),
            ),
        )?;
        for chapter in chapters {
            fs::write(
                staging_path.join("OEBPS").join(chapter.filename),
                format!(
                    r#"<?xml version="1.0" encoding="UTF-8"?>
<html xmlns="http://www.w3.org/1999/xhtml">
  <body><h1>{}</h1><p>{}</p></body>
</html>"#,
                    chapter.label, chapter.content
                ),
            )?;
        }

        if include_toc {
            fs::write(
                staging_path.join("OEBPS").join("toc.ncx"),
                format!(
                    r#"<?xml version="1.0" encoding="UTF-8"?>
<ncx xmlns="http://www.daisy.org/z3986/2005/ncx/" version="2005-1">
  <navMap>
    {}
  </navMap>
</ncx>"#,
                    chapters
                        .iter()
                        .enumerate()
                        .map(|(index, chapter)| format!(
                            r#"<navPoint id="chapter{}" playOrder="{}"><navLabel><text>{}</text></navLabel><content src="{}"/></navPoint>"#,
                            index + 1,
                            index + 1,
                            chapter.label,
                            chapter.filename
                        ))
                        .collect::<Vec<_>>()
                        .join("\n    "),
                ),
            )?;
        }

        let status = Command::new("zip")
            .arg("-q")
            .arg("-r")
            .arg(path)
            .arg("mimetype")
            .arg("META-INF")
            .arg("OEBPS")
            .current_dir(&staging_path)
            .status()?;

        if !status.success() {
            return Err(io::Error::other("failed to zip EPUB fixture"));
        }

        Ok(())
    }

    fn temp_dir_path(name: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time")
            .as_nanos();

        std::env::temp_dir().join(format!("rebe_document_{name}_{nanos}"))
    }
}
