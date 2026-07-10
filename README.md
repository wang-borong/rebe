# Rebe

[![CI](https://github.com/wang-borong/rebe/actions/workflows/CICD.yml/badge.svg)](https://github.com/wang-borong/rebe/actions/workflows/CICD.yml)
[![License: GPL-3.0-or-later](https://img.shields.io/badge/license-GPL--3.0--or--later-blue.svg)](LICENSE)
[![Rust](https://img.shields.io/badge/Rust-2021-orange.svg)](https://www.rust-lang.org/)

> Read first. Learn the words that matter before you start.

Rebe is a Rust command-line tool for preparing English books and article corpora.
It extracts vocabulary from the material you plan to read, removes words you
already know, ranks what remains by frequency and coverage, and keeps each
result connected to its source sentence and document.

It is built for readers who want a focused preparation list instead of a
generic word list detached from the book in front of them.

## Highlights

- Analyze `.txt`, Markdown, EPUB, PDF, DOCX, and non-DRM Kindle files
  (`.azw3`, `.azw`, `.mobi`, `.kfx`), including recursive directories.
- Normalize English word forms with WordNet lemmatization and project-specific
  lemma overrides.
- Filter by occurrence count, frequency, corpus coverage, document coverage,
  word length, common words, and probable proper nouns.
- Persist known words, ignore lists, lemma maps, and defaults in one profile.
- Preserve source distribution, source examples, surface forms, and optional
  definitions in text, CSV, and JSON reports.
- Look up definitions through a local MDict `.mdx` file, Youdao, or an external
  script.

## Quick Start

Build and run from a checkout:

```bash
cargo run -- analyze book.epub --top 80 --format csv -o vocabulary.csv
```

Install the local checkout as a command:

```bash
cargo install --path . --locked
rebe --help
```

Analyze a directory of articles with a reader profile:

```bash
rebe analyze articles/ \
  --profile ~/.config/rebe/profile.ini \
  --min-doc-frequency 25% \
  --coverage 80% \
  --format csv \
  --output article-vocabulary.csv
```

## A Practical Reading Workflow

1. Create a profile once.

   ```bash
   rebe profile init ~/.config/rebe/profile.ini
   ```

2. Analyze the book, then study the highest-value unknown words.

   ```bash
   rebe analyze book.epub \
     --profile ~/.config/rebe/profile.ini \
     --top 100 \
     --coverage 85% \
     --format csv \
     -o book-vocabulary.csv
   ```

3. Record words you have learned and names you never need in the list.

   ```bash
   rebe profile add-known ~/.config/rebe/profile.ini reader vocabulary
   rebe profile add-ignore ~/.config/rebe/profile.ini "Sherlock Holmes"
   ```

4. Re-run the same command. Rebe excludes the newly known and ignored words.

## Input and Source Context

Directory input is recursive. Supported files are `.txt`, `.md`, `.markdown`,
`.epub`, `.pdf`, `.docx`, `.azw3`, `.azw`, `.mobi`, and `.kfx`.

Every result includes its source files and examples. EPUB source labels use NCX
chapter titles when their table of contents maps cleanly to the reading spine;
otherwise Rebe falls back to page numbers. This makes a vocabulary report useful
while you are actually reading, not only while you are exporting it.

## Reader Profiles

Profiles collect reader-specific settings in an INI-like file:

```ini
[known]
reader
written

[ignore]
alice
project_name

[lemma]
mice = mouse
went = go

[defaults]
min-count = 2
format = csv
definition-max-chars = 600
```

`--known`, `--ignore`, and `--lemma-map` can still be supplied for a single run.
Command-line values override profile defaults. Lemma maps override the built-in
WordNet lemmatization when a book needs a project-specific form.

## Definitions

Use a local MDict dictionary:

```bash
rebe analyze book.txt \
  --top 50 \
  --define-mdx /path/to/dictionary.mdx \
  --format csv \
  -o vocabulary.csv
```

Or use a script for a local glossary or another dictionary service:

```bash
rebe analyze book.txt \
  --top 50 \
  --define-command 'scripts/define-tsv.py glossary.tsv {word}' \
  --format json \
  -o vocabulary.json
```

See [scripts/README.md](scripts/README.md) for the external definition-provider
contract. `--define-youdao` is available when you provide official Youdao API
credentials through options or environment variables.

## Useful Commands

```bash
# Show all options.
rebe --help

# Keep common function words and probable proper nouns.
rebe analyze book.txt --include-common --include-proper-nouns --top 50

# Analyze a corpus and retain words that appear in at least half the documents.
rebe analyze articles/ --min-doc-frequency 50% --format json -o corpus.json

# Preserve raw MDX HTML in JSON output.
rebe analyze book.txt --define-mdx /path/to/dictionary.mdx \
  --mdx-definition-format html --format json -o vocabulary.json
```

## Shell Completion

Generate a completion script with the built-in `completions` command. The
supported shells are `bash`, `elvish`, `fish`, `powershell`, and `zsh`.

```bash
# Load Bash completion in the current shell.
source <(rebe completions bash)

# Install Fish completion for the current user.
rebe completions fish > ~/.config/fish/completions/rebe.fish

# Generate a Zsh completion file.
rebe completions zsh > _rebe
```

## Limitations

- DRM-protected Kindle books are intentionally unsupported.
- WordNet lemmatization does not perform full sentence-level part-of-speech
  tagging. Use a lemma map to resolve important ambiguous forms.
- EPUB3 navigation documents and incomplete NCX/spine mappings use page-number
  source labels.
- MDX media resources from companion `.mdd` files are not embedded in output.

## Development

```bash
cargo fmt --check
cargo check
cargo test
cargo clippy --locked --all-targets -- -D warnings
```

The GitHub Actions workflow runs formatting, Clippy, tests, documentation, and
native release builds for Linux, macOS, and Windows. A tag beginning with `v`
publishes the built binaries, checksums, README, and license as a GitHub Release.

## License

Rebe is licensed under [GPL-3.0-or-later](LICENSE).

The optional MDX integration links `mdict-rs`, which is licensed
AGPL-3.0-only. Distributions that include this integration must also comply with
that dependency's license terms.
