# Test Fixtures

`frankenstein-1818-excerpt.txt` is a short excerpt from Mary Shelley's
*Frankenstein; or, the Modern Prometheus* (1818), sourced from Project
Gutenberg eBook #84. Project Gutenberg identifies that eBook as public domain
in the United States. The document-format integration tests use the same text
to exercise text, EPUB, PDF, DOCX, and Kindle extraction paths.

`portable-test.mdx.base64` is the repository's small generated MDX structural
fixture, stored as Base64 so the binary data remains reviewable in source
control. It covers MDX lookup and HTML-to-text cleanup without embedding a
third-party dictionary.
