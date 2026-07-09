#!/usr/bin/env python3
"""Look up one word in a local TSV glossary.

The glossary format is:

    word<TAB>definition

Blank lines and lines starting with "#" are ignored.
"""

import sys


def main() -> int:
    if len(sys.argv) != 3:
        print("usage: define-tsv.py <glossary.tsv> <word>", file=sys.stderr)
        return 2

    glossary_path = sys.argv[1]
    word = sys.argv[2].strip().lower()

    if not word:
        return 2

    try:
        definition = lookup(glossary_path, word)
    except OSError:
        return 1

    if not definition:
        return 1

    print(definition)
    return 0


def lookup(path: str, word: str) -> str:
    with open(path, "r", encoding="utf-8") as glossary:
        for line in glossary:
            stripped = line.strip()

            if not stripped or stripped.startswith("#"):
                continue

            key, separator, value = stripped.partition("\t")

            if not separator:
                continue

            if key.strip().lower() == word:
                return " ".join(value.split())

    return ""


if __name__ == "__main__":
    raise SystemExit(main())
