#!/usr/bin/env python3
"""
hungarian_extract.py — Extract morphological paradigm tables from UD Hungarian-Szeged.

Mirrors paradigm_extract.py / turkish_extract.py but adapted for Hungarian.

Hungarian noun paradigms: Case(18) x Number(2) — 36 slots
  (18 productive cases attested in UD Hungarian-Szeged)

Hungarian verb paradigms (4D): Mood(4) x Tense(2) x Person(3) x Number(2) — 48 slots
Hungarian verb paradigms (5D): Mood(4) x Tense(2) x Person(3) x Number(2) x Definite(2) — 96 slots
  Hungarian has a unique definite/indefinite conjugation system (határozatlan/határozott):
  - Indefinite: "látok" (I see [something])
  - Definite: "látom" (I see [it/the thing])
  This is the key typological feature for cross-linguistic comparison.

Hungarian adjective paradigms: Case(18) x Number(2) x Degree(3) — 108 slots

Part of cross-linguistic TT-rank experiment for Paper-2 (SIGMORPHON).
"""

import json
import sys
from collections import defaultdict
from pathlib import Path


# ──────────────────────────────────────────────────────────────
# Hungarian morphological feature dimensions
# ──────────────────────────────────────────────────────────────

# Hungarian has 18 productive cases in UD (from most to least frequent in corpus)
CASES = [
    "Nom", "Acc", "Ine", "Sup", "Ins", "Sbl",  # 6 most common
    "Gen", "Dat", "Ill", "All", "Del", "Ela",    # medium frequency
    "Abl", "Abs", "Cau", "Ter", "Ade", "Tra",    # rarer cases
    # Tem (temporal) and Ess (essive-formal) are very rare in UD but exist
]

NUMBERS = ["Sing", "Plur"]

# Verb dimensions (finite forms only)
# Moods: 4 basic moods (compound moods like Cnd,Pot are very rare, excluded)
MOODS = ["Ind", "Cnd", "Imp", "Pot"]

# Tenses: Hungarian has 2 tenses (like Finnish, unlike Turkish's 4)
TENSES = ["Pres", "Past"]

# Person: 1, 2, 3
PERSONS = ["1", "2", "3"]

# Definiteness: Hungarian's unique feature — definite vs indefinite conjugation
DEFINITES = ["Ind", "Def"]

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
    Build Case(18) x Number(2) paradigm tables for Hungarian nouns.
    """
    lemma_slots = defaultdict(dict)
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

        form_lower = form.lower()
        if key not in lemma_slots[lemma]:
            lemma_slots[lemma][key] = form_lower
            lemma_coverage[lemma] += 1

    return lemma_slots, lemma_coverage


def extract_verb_paradigms(tokens):
    """
    Build Mood(4) x Tense(2) x Person(3) x Number(2) paradigm tables for
    Hungarian finite verb forms (without definiteness).

    This 4D tensor is directly comparable to Finnish verbs:
    Finnish: Mood(4) x Tense(2) x Person(4) x Number(2) = 64 slots
    Hungarian: Mood(4) x Tense(2) x Person(3) x Number(2) = 48 slots
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
        # Skip compound moods like "Cnd,Pot"
        if "," in (mood or ""):
            continue
        if mood not in MOODS or person not in PERSONS or number not in NUMBERS:
            continue

        # Tense: non-Indicative moods may lack tense
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


def extract_verb_paradigms_with_definiteness(tokens):
    """
    Build Mood(4) x Tense(2) x Person(3) x Number(2) x Definite(2) tables.

    This is the 5D tensor unique to Hungarian — the definite/indefinite
    conjugation split is the primary typological differentiator.

    Comparable to Turkish's 5D tensor with Polarity, but linguistically
    very different: Hungarian definiteness marks agreement with the object's
    definiteness, while Turkish polarity is a simple affixal negation.
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
        definite = feat_dict.get("Definite")
        if mood is None or person is None or number is None or definite is None:
            continue
        # Skip compound moods
        if "," in (mood or ""):
            continue
        if mood not in MOODS or person not in PERSONS or number not in NUMBERS:
            continue
        if definite not in DEFINITES:
            continue

        tense = feat_dict.get("Tense", "Pres")
        if tense not in TENSES:
            tense = "Pres"

        mi = MOODS.index(mood)
        ti = TENSES.index(tense)
        pi = PERSONS.index(person)
        ni = NUMBERS.index(number)
        di = DEFINITES.index(definite)
        key = (mi, ti, pi, ni, di)

        form_lower = form.lower()
        if key not in lemma_slots[lemma]:
            lemma_slots[lemma][key] = form_lower
            lemma_coverage[lemma] += 1

    return lemma_slots, lemma_coverage


def extract_adj_paradigms(tokens):
    """
    Build Case(18) x Number(2) x Degree(3) paradigm tables for adjectives.
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
    """Encode a surface form as a fixed-length vector of character indices."""
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
    sorted_chars = sorted(chars)
    char_to_idx = {ch: i + 2 for i, ch in enumerate(sorted_chars)}
    idx_to_char = {v: k for k, v in char_to_idx.items()}
    idx_to_char[0] = "<PAD>"
    idx_to_char[1] = "<UNK>"
    return char_to_idx, idx_to_char


def main():
    # Use both train and dev for maximum paradigm coverage
    base_dir = Path(sys.argv[1]) if len(sys.argv) > 1 else Path(
        "/Users/yongseok/oss/finnishNLP/ud-hungarian-szeged"
    )
    conllu_files = [
        base_dir / "hu_szeged-ud-train.conllu",
        base_dir / "hu_szeged-ud-dev.conllu",
    ]
    output_path = Path(__file__).parent / "hungarian_paradigms.json"

    noun_tokens = []
    verb_tokens = []
    adj_tokens = []

    for conllu_path in conllu_files:
        print(f"Parsing {conllu_path}...")
        for form, lemma, upos, feat_dict in parse_conllu(str(conllu_path)):
            if upos == "NOUN":
                noun_tokens.append((form, lemma, feat_dict))
            elif upos in ("VERB", "AUX"):
                verb_tokens.append((form, lemma, feat_dict))
            elif upos == "ADJ":
                adj_tokens.append((form, lemma, feat_dict))

    print(f"\nTotal tokens across train+dev:")
    print(f"  NOUN tokens: {len(noun_tokens)}")
    print(f"  VERB+AUX tokens: {len(verb_tokens)}")
    print(f"  ADJ tokens: {len(adj_tokens)}")

    # Extract paradigm tables
    noun_slots, noun_cov = extract_noun_paradigms(noun_tokens)
    verb_slots, verb_cov = extract_verb_paradigms(verb_tokens)
    verb_def_slots, verb_def_cov = extract_verb_paradigms_with_definiteness(verb_tokens)
    adj_slots, adj_cov = extract_adj_paradigms(adj_tokens)

    # Select top lemmas by coverage
    N_TOP = 100

    def top_lemmas(coverage_dict, min_coverage=4):
        items = [(lemma, cov) for lemma, cov in coverage_dict.items()
                 if cov >= min_coverage]
        items.sort(key=lambda x: -x[1])
        return items[:N_TOP]

    top_nouns = top_lemmas(noun_cov, min_coverage=4)
    top_verbs = top_lemmas(verb_cov, min_coverage=4)
    top_verbs_def = top_lemmas(verb_def_cov, min_coverage=4)
    top_adjs = top_lemmas(adj_cov, min_coverage=3)

    print(f"\nSelected paradigms:")
    print(f"  Nouns:       {len(top_nouns)} (coverage range: "
          f"{top_nouns[-1][1] if top_nouns else 0}-"
          f"{top_nouns[0][1] if top_nouns else 0})")
    print(f"  Verbs (4D):  {len(top_verbs)} (coverage range: "
          f"{top_verbs[-1][1] if top_verbs else 0}-"
          f"{top_verbs[0][1] if top_verbs else 0})")
    print(f"  Verbs (5D):  {len(top_verbs_def)} (coverage range: "
          f"{top_verbs_def[-1][1] if top_verbs_def else 0}-"
          f"{top_verbs_def[0][1] if top_verbs_def else 0})")
    print(f"  Adjectives:  {len(top_adjs)} (coverage range: "
          f"{top_adjs[-1][1] if top_adjs else 0}-"
          f"{top_adjs[0][1] if top_adjs else 0})")

    # Collect all forms for character vocabulary
    all_forms = set()
    for lemma, _ in top_nouns:
        all_forms.update(noun_slots[lemma].values())
    for lemma, _ in top_verbs:
        all_forms.update(verb_slots[lemma].values())
    for lemma, _ in top_verbs_def:
        all_forms.update(verb_def_slots[lemma].values())
    for lemma, _ in top_adjs:
        all_forms.update(adj_slots[lemma].values())

    char_to_idx, idx_to_char = build_char_vocabulary(all_forms)

    max_len = max((len(f) for f in all_forms), default=1)
    print(f"\nCharacter vocabulary: {len(char_to_idx)} chars, max_form_len: {max_len}")

    # Build output
    output = {
        "metadata": {
            "language": "Hungarian",
            "sources": [str(p) for p in conllu_files],
            "char_vocab_size": len(char_to_idx) + 2,
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
        "verbs_4d": {
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
        "verbs_5d": {
            "dimensions": ["Mood", "Tense", "Person", "Number", "Definite"],
            "dim_labels": {
                "Mood": MOODS,
                "Tense": TENSES,
                "Person": PERSONS,
                "Number": NUMBERS,
                "Definite": DEFINITES,
            },
            "shape": [len(MOODS), len(TENSES), len(PERSONS), len(NUMBERS),
                       len(DEFINITES)],
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
        total = len(CASES) * len(NUMBERS)
        paradigm = {
            "coverage": cov,
            "total_slots": total,
            "fill_rate": round(cov / total, 3),
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

    # Fill verb paradigms (4D — comparable to Finnish)
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
        output["verbs_4d"]["paradigms"][lemma] = paradigm

    # Fill verb paradigms (5D — with definiteness, Hungarian's unique feature)
    for lemma, cov in top_verbs_def:
        slots = verb_def_slots[lemma]
        total = (len(MOODS) * len(TENSES) * len(PERSONS)
                 * len(NUMBERS) * len(DEFINITES))
        paradigm = {
            "coverage": cov,
            "total_slots": total,
            "fill_rate": round(cov / total, 3),
            "forms": {},
            "char_encoded": {},
        }
        for (mi, ti, pi, ni, di), form in slots.items():
            slot_key = (f"{MOODS[mi]}|{TENSES[ti]}|{PERSONS[pi]}"
                        f"|{NUMBERS[ni]}|{DEFINITES[di]}")
            paradigm["forms"][slot_key] = form
            paradigm["char_encoded"][slot_key] = encode_form_as_char_vector(
                form, max_len, char_to_idx
            )
        output["verbs_5d"]["paradigms"][lemma] = paradigm

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
