#!/usr/bin/env python3
"""Extract (form, UPOS) -> lemma dictionary from CoNLL-U data.

For each unique (lowercase_form, UPOS) pair, keeps the most frequent lemma.
When multiple input files are given, earlier files take priority on conflicts.
Outputs a sorted TSV file: form<TAB>UPOS<TAB>lemma

Usage:
    python3 extract_lemma_dict.py -o <output.tsv> <file1.conllu> [file2.conllu ...]

Example (single file, legacy):
    python3 scripts/extract_lemma_dict.py \
        vendor/ud-finnish-tdt/fi_tdt-ud-train.conllu \
        data/lemma_dict.tsv

Example (multi-source merge with TDT priority):
    python3 scripts/extract_lemma_dict.py \
        -o data/lemma_dict.tsv \
        vendor/ud-finnish-tdt/fi_tdt-ud-train.conllu \
        vendor/ud-finnish-tdt/fi_tdt-ud-dev.conllu \
        vendor/ud-finnish-tdt/fi_tdt-ud-test.conllu \
        vendor/ud-finnish-ood/fi_ood-ud-test.conllu \
        vendor/ud-finnish-pud/fi_pud-ud-test.conllu
"""

import collections
import os
import sys


def extract_lemma_dict(conllu_path: str) -> dict[tuple[str, str, str], int]:
    """Parse CoNLL-U and return {(lowercase_form, UPOS, lemma): count}."""
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

    return counts


def merge_counts(
    all_counts: list[tuple[str, dict[tuple[str, str, str], int]]],
) -> dict[tuple[str, str], str]:
    """Merge counts from multiple sources. Earlier sources take priority.

    Priority means: if (form, UPOS) already has a best lemma from an earlier
    source, later sources cannot override it even if they have higher counts.
    """
    # First pass: collect best lemma per (form, upos) per source.
    # Then merge with priority.
    merged: dict[tuple[str, str], tuple[str, int, int]] = {}
    # value: (lemma, count, source_priority) where lower priority = higher rank

    for priority, (source_name, counts) in enumerate(all_counts):
        # Build per-source best lemma
        source_best: dict[tuple[str, str], tuple[str, int]] = {}
        for (form, upos, lemma), count in counts.items():
            key = (form, upos)
            if key not in source_best:
                source_best[key] = (lemma, count)
            elif count > source_best[key][1]:
                source_best[key] = (lemma, count)

        # Merge into global dict with priority
        new_from_source = 0
        for key, (lemma, count) in source_best.items():
            if key not in merged:
                merged[key] = (lemma, count, priority)
                new_from_source += 1
            # If same priority (same source group), pick higher count
            elif merged[key][2] == priority and count > merged[key][1]:
                merged[key] = (lemma, count, priority)
            # Lower priority sources never override higher priority ones

        print(
            f"  {source_name}: {len(source_best)} unique pairs, "
            f"{new_from_source} new entries added",
            file=sys.stderr,
        )

    return {k: v[0] for k, v in merged.items()}


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
    # Support both legacy (2 positional args) and new (-o flag) syntax.
    if len(sys.argv) == 3 and not sys.argv[1].startswith("-"):
        # Legacy: extract_lemma_dict.py <input.conllu> <output.tsv>
        conllu_path = sys.argv[1]
        output_path = sys.argv[2]

        print(f"Extracting lemma dictionary from {conllu_path} ...", file=sys.stderr)
        counts = extract_lemma_dict(conllu_path)
        all_counts = [(os.path.basename(conllu_path), counts)]
        dictionary = merge_counts(all_counts)

    elif "-o" in sys.argv:
        # New: extract_lemma_dict.py -o <output.tsv> <file1> [file2] ...
        o_idx = sys.argv.index("-o")
        if o_idx + 1 >= len(sys.argv):
            print("Error: -o requires an output path", file=sys.stderr)
            sys.exit(1)
        output_path = sys.argv[o_idx + 1]
        # Input files are all args except the script name, -o, and output path
        input_files = [
            a for i, a in enumerate(sys.argv[1:], 1) if i != o_idx and i != o_idx + 1
        ]
        if not input_files:
            print("Error: no input files specified", file=sys.stderr)
            sys.exit(1)

        print(
            f"Extracting lemma dictionary from {len(input_files)} files ...",
            file=sys.stderr,
        )
        all_counts = []
        for path in input_files:
            print(f"  Reading {os.path.basename(path)} ...", file=sys.stderr)
            counts = extract_lemma_dict(path)
            all_counts.append((os.path.basename(path), counts))

        print(f"\nMerging (earlier files take priority) ...", file=sys.stderr)
        dictionary = merge_counts(all_counts)

    else:
        print(
            f"Usage: {sys.argv[0]} -o <output.tsv> <file1.conllu> [file2.conllu ...]\n"
            f"       {sys.argv[0]} <input.conllu> <output.tsv>  (legacy mode)",
            file=sys.stderr,
        )
        sys.exit(1)

    print(
        f"\n  {len(dictionary)} unique (form, UPOS) -> lemma entries", file=sys.stderr
    )

    written = write_tsv(dictionary, output_path, skip_identity=True)
    print(f"  {written} entries written (non-identity)", file=sys.stderr)

    size_kb = os.path.getsize(output_path) / 1024
    print(f"  Written to {output_path} ({size_kb:.1f} KB)", file=sys.stderr)


if __name__ == "__main__":
    main()
