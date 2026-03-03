#!/usr/bin/env python3
"""Extract (form, UPOS) -> lemma dictionary from CoNLL-U training data.

For each unique (lowercase_form, UPOS) pair, keeps the most frequent lemma.
Outputs a sorted TSV file: form<TAB>UPOS<TAB>lemma

Usage:
    python3 extract_lemma_dict.py <train.conllu> <output.tsv>

Example:
    python3 scripts/extract_lemma_dict.py \
        ../ud-finnish-tdt/fi_tdt-ud-train.conllu \
        data/lemma_dict.tsv
"""

import collections
import sys


def extract_lemma_dict(conllu_path: str) -> dict[tuple[str, str], str]:
    """Parse CoNLL-U and return {(lowercase_form, UPOS): best_lemma}."""
    counts: dict[tuple[str, str, str], int] = collections.Counter()

    with open(conllu_path, encoding="utf-8") as f:
        for line in f:
            line = line.strip()
            if not line or line.startswith("#"):
                continue
            fields = line.split("\t")
            if len(fields) < 10:
                continue
            id_str = fields[0]
            # Skip multi-word tokens and empty nodes.
            if "-" in id_str or "." in id_str:
                continue

            form = fields[1].lower()
            lemma = fields[2]
            upos = fields[3]
            counts[(form, upos, lemma)] += 1

    # For each (form, upos), keep the most frequent lemma.
    best: dict[tuple[str, str], str] = {}
    for (form, upos, lemma), count in counts.items():
        key = (form, upos)
        if key not in best:
            best[key] = (lemma, count)
        elif count > best[key][1]:
            best[key] = (lemma, count)

    return {k: v[0] for k, v in best.items()}


def write_tsv(
    dictionary: dict[tuple[str, str], str],
    output_path: str,
    skip_identity: bool = True,
) -> int:
    """Write dictionary as sorted TSV: form<TAB>UPOS<TAB>lemma.

    If skip_identity is True, omit entries where lowercased form equals
    lowercased lemma (with # removed). The Rust loader will treat missing
    entries as identity (form -> form).
    """
    # Sort by (form, UPOS) for binary search compatibility and reproducibility.
    entries = sorted(dictionary.items())

    written = 0
    skipped = 0
    with open(output_path, "w", encoding="utf-8") as f:
        for (form, upos), lemma in entries:
            if skip_identity:
                lemma_clean = lemma.lower().replace("#", "")
                if form == lemma_clean:
                    skipped += 1
                    continue
            f.write(f"{form}\t{upos}\t{lemma}\n")
            written += 1

    if skipped:
        print(
            f"  Skipped {skipped} identity entries (form == lemma)", file=sys.stderr
        )

    return written


def main() -> None:
    if len(sys.argv) != 3:
        print(f"Usage: {sys.argv[0]} <train.conllu> <output.tsv>", file=sys.stderr)
        sys.exit(1)

    conllu_path = sys.argv[1]
    output_path = sys.argv[2]

    print(f"Extracting lemma dictionary from {conllu_path} ...", file=sys.stderr)
    dictionary = extract_lemma_dict(conllu_path)
    print(f"  {len(dictionary)} unique (form, UPOS) -> lemma entries", file=sys.stderr)

    written = write_tsv(dictionary, output_path, skip_identity=True)
    print(f"  {written} entries written (non-identity)", file=sys.stderr)

    # Report file size.
    import os

    size_kb = os.path.getsize(output_path) / 1024
    print(f"  Written to {output_path} ({size_kb:.1f} KB)", file=sys.stderr)


if __name__ == "__main__":
    main()
