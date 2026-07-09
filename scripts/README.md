# Definition Plugin Scripts

`rebe` can call external definition providers through `--define-command`.
This is the extension point for dictionary services or local resources that are not built into the binary.

## Contract

- The command must accept one looked-up word.
- Print the definition to `stdout`.
- Exit with status `0` when a definition is found.
- Exit with a non-zero status, or print nothing, when no definition is available.
- Keep output concise. `rebe` collapses whitespace and trims very long definitions before writing reports.

The command template supports these placeholders:

- `{word}`: shell-quoted word, safest for normal script arguments.
- `{word_raw}`: unquoted raw word.
- `{word_url}`: URL-encoded word.

Prefer `{word}` unless the target command specifically needs URL encoding.

## Examples

Network dictionary example:

```bash
cargo run -- analyze book.txt \
  --top 50 \
  --define-command 'scripts/define-dictionaryapi.py {word}' \
  --format csv \
  -o words.csv
```

Local TSV glossary example:

```bash
cargo run -- analyze book.txt \
  --top 50 \
  --define-command 'scripts/define-tsv.py glossary.tsv {word}' \
  --format csv \
  -o words.csv
```

`glossary.tsv` format:

```text
reader	a person who reads books or articles
vocabulary	the words used or known by a person
```

## Built-In Alternatives

Use `--define-mdx <PATH>` for local MDict `.mdx` dictionaries.
Use `--define-youdao` for the built-in Youdao API client.
