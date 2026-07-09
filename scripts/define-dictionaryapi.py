#!/usr/bin/env python3
"""Print a short English definition for one word using dictionaryapi.dev."""

import json
import sys
import urllib.error
import urllib.parse
import urllib.request


def main() -> int:
    if len(sys.argv) != 2:
        print("usage: define-dictionaryapi.py <word>", file=sys.stderr)
        return 2

    word = sys.argv[1].strip()
    if not word:
        return 2

    url = "https://api.dictionaryapi.dev/api/v2/entries/en/" + urllib.parse.quote(word)

    try:
        with urllib.request.urlopen(url, timeout=8) as response:
            payload = json.loads(response.read().decode("utf-8"))
    except (OSError, urllib.error.URLError, json.JSONDecodeError):
        return 1

    definition = first_definition(payload)
    if not definition:
        return 1

    print(definition)
    return 0


def first_definition(payload) -> str:
    if not isinstance(payload, list):
        return ""

    for entry in payload:
        for meaning in entry.get("meanings", []):
            for definition in meaning.get("definitions", []):
                value = definition.get("definition", "")
                if value:
                    return " ".join(value.split())

    return ""


if __name__ == "__main__":
    raise SystemExit(main())
