#!/usr/bin/env python3
"""
paradigm_extract.py — Extract morphological paradigm tables from UD Finnish-TDT.

For each lemma with sufficient paradigm coverage, extracts the surface forms
organized by their morphological feature combinations (Case×Number for nouns,
Mood×Tense×Person×Number for finite verbs, etc.) and encodes them as tensors
suitable for TT decomposition.

Output: paradigms.json with structured paradigm data.

Part of TT-rank experiment for Paper-2 (SIGMORPHON).
"""

import json
import sys
from collections import defaultdict
from pathlib import Path

# ──────────────────────────────────────────────────────────────
# Finnish morphological feature dimensions
# ──────────────────────────────────────────────────────────────

# 15 cases in Finnish (UD tagset). Instructive and Comitative are rare.
CASES = [
    "Nom", "Gen", "Par",       # grammatical
    "Ine", "Ela", "Ill",       # internal local
    "Ade", "Abl", "All",       # external local
    "Ess", "Tra",              # general local / state
    "Ins", "Com", "Abe",       # marginal
]

NUMBERS = ["Sing", "Plur"]

# Verb dimensions (finite forms only)
MOODS = ["Ind", "Cnd", "Imp", "Pot"]
TENSES = ["Pres", "Past"]   # only for Ind; others have no tense
PERSONS = ["1", "2", "3", "0"]   # 0 = impersonal/passive in some analyses
VOICES = ["Act", "Pass"]

# Adjective degrees
DEGREES = ["Pos", "Cmp", "Sup"]


def parse_conllu(path: str):
    """Parse CoNLL-U file, yield (form, lemma, upos, feat_dict) tuples."""
    with open(path, encoding="utf-8") as f:
        for line in f:
            line = line.strip()
            if not line or line.startswith("#"):
                continue
            cols = line.split("\t")
            if len(cols) < 10:
                continue
            tok_id = cols[0]
            # Skip multi-word tokens and empty nodes
            if "-" in tok_id or "." in tok_id:
                continue

            form = cols[1]
            lemma = cols[2]
            upos = cols[3]
            feats_str = cols[5]

            feat_dict = {}
            if feats_str != "_":
                for feat in feats_str.split("|"):
                    if "=" in feat:
                        name, val = feat.split("=", 1)
                        feat_dict[name] = val

            yield form, lemma, upos, feat_dict


def extract_noun_paradigms(tokens):
    """
    Build Case(15) × Number(2) paradigm tables for nouns.

    For each (lemma, NOUN), fill the 15×2 grid with observed surface forms.
    """
    # Group by lemma
    lemma_slots = defaultdict(dict)  # lemma -> {(case_idx, num_idx): form}
    lemma_coverage = defaultdict(int)

    for form, lemma, feat_dict in tokens:
        case = feat_dict.get("Case")
        number = feat_dict.get("Number")
        if case is None or number is None:
            continue
        if case not in CASES or number not in NUMBERS:
            continue

        ci = CASES.index(case)
        ni = NUMBERS.index(number)
        key = (ci, ni)

        # Use lowercase, first occurrence wins (avoid proper noun capitalization)
        form_lower = form.lower()
        if key not in lemma_slots[lemma]:
            lemma_slots[lemma][key] = form_lower
            lemma_coverage[lemma] += 1

    return lemma_slots, lemma_coverage


def extract_verb_paradigms(tokens):
    """
    Build Mood(4) × Tense(2) × Person(4) × Number(2) paradigm tables for
    finite verb forms.

    Non-finite forms (infinitives, participles) are excluded — they have
    different dimensional structure.
    """
    lemma_slots = defaultdict(dict)
    lemma_coverage = defaultdict(int)

    for form, lemma, feat_dict in tokens:
        verb_form = feat_dict.get("VerbForm")
        if verb_form != "Fin":
            continue

        mood = feat_dict.get("Mood")
        person = feat_dict.get("Person")
        number = feat_dict.get("Number")
        if mood is None or person is None or number is None:
            continue
        if mood not in MOODS or person not in PERSONS or number not in NUMBERS:
            continue

        # Tense: only Indicative has tense. For Cnd/Imp/Pot, use index 0.
        tense = feat_dict.get("Tense", "Pres")
        if tense not in TENSES:
            tense = "Pres"

        mi = MOODS.index(mood)
        ti = TENSES.index(tense)
        pi = PERSONS.index(person)
        ni = NUMBERS.index(number)
        key = (mi, ti, pi, ni)

        form_lower = form.lower()
        if key not in lemma_slots[lemma]:
            lemma_slots[lemma][key] = form_lower
            lemma_coverage[lemma] += 1

    return lemma_slots, lemma_coverage


def extract_adj_paradigms(tokens):
    """
    Build Case(15) × Number(2) × Degree(3) paradigm tables for adjectives.
    """
    lemma_slots = defaultdict(dict)
    lemma_coverage = defaultdict(int)

    for form, lemma, feat_dict in tokens:
        case = feat_dict.get("Case")
        number = feat_dict.get("Number")
        degree = feat_dict.get("Degree")
        if case is None or number is None:
            continue
        if case not in CASES or number not in NUMBERS:
            continue
        if degree is None:
            degree = "Pos"
        if degree not in DEGREES:
            continue

        ci = CASES.index(case)
        ni = NUMBERS.index(number)
        di = DEGREES.index(degree)
        key = (ci, ni, di)

        form_lower = form.lower()
        if key not in lemma_slots[lemma]:
            lemma_slots[lemma][key] = form_lower
            lemma_coverage[lemma] += 1

    return lemma_slots, lemma_coverage


def encode_form_as_char_vector(form: str, max_len: int, char_to_idx: dict):
    """
    Encode a surface form as a fixed-length vector of character indices.

    Uses 0 for padding, 1 for unknown chars.
    """
    vec = []
    for i in range(max_len):
        if i < len(form):
            ch = form[i]
            vec.append(char_to_idx.get(ch, 1))
        else:
            vec.append(0)  # padding
    return vec


def build_char_vocabulary(all_forms):
    """Build character vocabulary from all observed forms."""
    chars = set()
    for form in all_forms:
        chars.update(form)
    # Sort for deterministic ordering
    sorted_chars = sorted(chars)
    # 0 = padding, 1 = unknown, 2+ = actual characters
    char_to_idx = {ch: i + 2 for i, ch in enumerate(sorted_chars)}
    idx_to_char = {v: k for k, v in char_to_idx.items()}
    idx_to_char[0] = "<PAD>"
    idx_to_char[1] = "<UNK>"
    return char_to_idx, idx_to_char


def main():
    project_root = Path(__file__).resolve().parent.parent.parent
    default_conllu = str(project_root / "vendor" / "ud-finnish-tdt" / "fi_tdt-ud-train.conllu")
    conllu_path = sys.argv[1] if len(sys.argv) > 1 else default_conllu
    output_path = Path(__file__).parent / "paradigms.json"

    print(f"Parsing {conllu_path}...")

    # Collect tokens by POS
    noun_tokens = []
    verb_tokens = []
    adj_tokens = []

    for form, lemma, upos, feat_dict in parse_conllu(conllu_path):
        if upos == "NOUN":
            noun_tokens.append((form, lemma, feat_dict))
        elif upos == "VERB":
            verb_tokens.append((form, lemma, feat_dict))
        elif upos == "ADJ":
            adj_tokens.append((form, lemma, feat_dict))

    print(f"  NOUN tokens: {len(noun_tokens)}")
    print(f"  VERB tokens: {len(verb_tokens)}")
    print(f"  ADJ tokens: {len(adj_tokens)}")

    # Extract paradigm tables
    noun_slots, noun_cov = extract_noun_paradigms(noun_tokens)
    verb_slots, verb_cov = extract_verb_paradigms(verb_tokens)
    adj_slots, adj_cov = extract_adj_paradigms(adj_tokens)

    # Select top lemmas by coverage
    N_TOP = 100

    def top_lemmas(coverage_dict, min_coverage=5):
        items = [(lemma, cov) for lemma, cov in coverage_dict.items() if cov >= min_coverage]
        items.sort(key=lambda x: -x[1])
        return items[:N_TOP]

    top_nouns = top_lemmas(noun_cov, min_coverage=5)
    top_verbs = top_lemmas(verb_cov, min_coverage=5)
    top_adjs = top_lemmas(adj_cov, min_coverage=5)

    print(f"\nSelected paradigms:")
    print(f"  Nouns:  {len(top_nouns)} (coverage range: "
          f"{top_nouns[-1][1] if top_nouns else 0}-{top_nouns[0][1] if top_nouns else 0})")
    print(f"  Verbs:  {len(top_verbs)} (coverage range: "
          f"{top_verbs[-1][1] if top_verbs else 0}-{top_verbs[0][1] if top_verbs else 0})")
    print(f"  Adjs:   {len(top_adjs)} (coverage range: "
          f"{top_adjs[-1][1] if top_adjs else 0}-{top_adjs[0][1] if top_adjs else 0})")

    # Collect all forms for character vocabulary
    all_forms = set()
    for lemma, _ in top_nouns:
        all_forms.update(noun_slots[lemma].values())
    for lemma, _ in top_verbs:
        all_forms.update(verb_slots[lemma].values())
    for lemma, _ in top_adjs:
        all_forms.update(adj_slots[lemma].values())

    char_to_idx, idx_to_char = build_char_vocabulary(all_forms)

    # Determine max form length
    max_len = max((len(f) for f in all_forms), default=1)
    print(f"\nCharacter vocabulary: {len(char_to_idx)} chars, max_form_len: {max_len}")

    # Build output
    output = {
        "metadata": {
            "source": conllu_path,
            "char_vocab_size": len(char_to_idx) + 2,  # +2 for PAD and UNK
            "max_form_length": max_len,
            "char_to_idx": char_to_idx,
            "idx_to_char": {str(k): v for k, v in idx_to_char.items()},
        },
        "nouns": {
            "dimensions": ["Case", "Number"],
            "dim_labels": {"Case": CASES, "Number": NUMBERS},
            "shape": [len(CASES), len(NUMBERS)],
            "paradigms": {},
        },
        "verbs": {
            "dimensions": ["Mood", "Tense", "Person", "Number"],
            "dim_labels": {
                "Mood": MOODS,
                "Tense": TENSES,
                "Person": PERSONS,
                "Number": NUMBERS,
            },
            "shape": [len(MOODS), len(TENSES), len(PERSONS), len(NUMBERS)],
            "paradigms": {},
        },
        "adjectives": {
            "dimensions": ["Case", "Number", "Degree"],
            "dim_labels": {
                "Case": CASES,
                "Number": NUMBERS,
                "Degree": DEGREES,
            },
            "shape": [len(CASES), len(NUMBERS), len(DEGREES)],
            "paradigms": {},
        },
    }

    # Fill noun paradigms
    for lemma, cov in top_nouns:
        slots = noun_slots[lemma]
        paradigm = {
            "coverage": cov,
            "total_slots": len(CASES) * len(NUMBERS),
            "fill_rate": round(cov / (len(CASES) * len(NUMBERS)), 3),
            "forms": {},
            "char_encoded": {},
        }
        for (ci, ni), form in slots.items():
            slot_key = f"{CASES[ci]}|{NUMBERS[ni]}"
            paradigm["forms"][slot_key] = form
            paradigm["char_encoded"][slot_key] = encode_form_as_char_vector(
                form, max_len, char_to_idx
            )
        output["nouns"]["paradigms"][lemma] = paradigm

    # Fill verb paradigms
    for lemma, cov in top_verbs:
        slots = verb_slots[lemma]
        total = len(MOODS) * len(TENSES) * len(PERSONS) * len(NUMBERS)
        paradigm = {
            "coverage": cov,
            "total_slots": total,
            "fill_rate": round(cov / total, 3),
            "forms": {},
            "char_encoded": {},
        }
        for (mi, ti, pi, ni), form in slots.items():
            slot_key = f"{MOODS[mi]}|{TENSES[ti]}|{PERSONS[pi]}|{NUMBERS[ni]}"
            paradigm["forms"][slot_key] = form
            paradigm["char_encoded"][slot_key] = encode_form_as_char_vector(
                form, max_len, char_to_idx
            )
        output["verbs"]["paradigms"][lemma] = paradigm

    # Fill adjective paradigms
    for lemma, cov in top_adjs:
        slots = adj_slots[lemma]
        total = len(CASES) * len(NUMBERS) * len(DEGREES)
        paradigm = {
            "coverage": cov,
            "total_slots": total,
            "fill_rate": round(cov / total, 3),
            "forms": {},
            "char_encoded": {},
        }
        for (ci, ni, di), form in slots.items():
            slot_key = f"{CASES[ci]}|{NUMBERS[ni]}|{DEGREES[di]}"
            paradigm["forms"][slot_key] = form
            paradigm["char_encoded"][slot_key] = encode_form_as_char_vector(
                form, max_len, char_to_idx
            )
        output["adjectives"]["paradigms"][lemma] = paradigm

    # Write output
    with open(output_path, "w", encoding="utf-8") as f:
        json.dump(output, f, ensure_ascii=False, indent=2)

    print(f"\nOutput written to {output_path}")
    print(f"  File size: {output_path.stat().st_size / 1024:.1f} KB")


if __name__ == "__main__":
    main()
