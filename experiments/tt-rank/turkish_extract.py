#!/usr/bin/env python3
"""
turkish_extract.py — Extract morphological paradigm tables from UD Turkish-IMST.

Mirrors paradigm_extract.py but adapted for Turkish morphological features.

Turkish noun paradigms: Case(6) x Number(2) — 12 slots
Turkish verb paradigms: Mood(4) x Tense(4) x Person(3) x Number(2) x Polarity(2) — 192 slots
  (Simplified: use most common moods/tenses to keep tensor small enough)

Part of cross-linguistic TT-rank experiment for Paper-2 (SIGMORPHON).
"""

import json
import sys
from collections import defaultdict
from pathlib import Path


# ──────────────────────────────────────────────────────────────
# Turkish morphological feature dimensions
# ──────────────────────────────────────────────────────────────

# Turkish has 6 productive cases (+ rare Equ=7, but very sparse)
CASES = ["Nom", "Acc", "Gen", "Dat", "Loc", "Abl"]

NUMBERS = ["Sing", "Plur"]

# Turkish verb dimensions (finite forms)
# Moods: Ind is dominant; keep the 4 most common
MOODS = ["Ind", "Pot", "Imp", "Cnd"]
# Tenses: 4 tenses attested in UD
TENSES = ["Pres", "Past", "Fut", "Pqp"]
# Person: 1, 2, 3
PERSONS = ["1", "2", "3"]
# Polarity: Pos, Neg
POLARITIES = ["Pos", "Neg"]


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
    Build Case(6) x Number(2) paradigm tables for Turkish nouns.
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
    Build Mood(4) x Tense(4) x Person(3) x Number(2) paradigm tables for
    Turkish finite verb forms.

    Note: Turkish verb morphology is richer than Finnish — we include
    only the 4 most common moods and tenses to keep the tensor manageable.
    Non-finite forms (participles, converbs, verbal nouns) are excluded.
    """
    lemma_slots = defaultdict(dict)
    lemma_coverage = defaultdict(int)

    for form, lemma, feat_dict in tokens:
        # Only finite forms (exclude Part, Vnoun, Conv)
        verb_form = feat_dict.get("VerbForm")
        if verb_form is not None:
            continue  # Non-finite form

        mood = feat_dict.get("Mood")
        person = feat_dict.get("Person")
        number = feat_dict.get("Number")
        if mood is None or person is None or number is None:
            continue
        if mood not in MOODS or person not in PERSONS or number not in NUMBERS:
            continue

        # Tense: Imperative and Conditional may lack tense
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


def extract_verb_paradigms_with_polarity(tokens):
    """
    Build Mood(4) x Tense(4) x Person(3) x Number(2) x Polarity(2) tables.

    This is the full 5-dimensional tensor for Turkish verbs.
    Polarity (Pos/Neg) is an explicit morphological dimension in Turkish,
    unlike Finnish where negation is a separate verb.
    """
    lemma_slots = defaultdict(dict)
    lemma_coverage = defaultdict(int)

    for form, lemma, feat_dict in tokens:
        verb_form = feat_dict.get("VerbForm")
        if verb_form is not None:
            continue

        mood = feat_dict.get("Mood")
        person = feat_dict.get("Person")
        number = feat_dict.get("Number")
        polarity = feat_dict.get("Polarity")
        if mood is None or person is None or number is None or polarity is None:
            continue
        if mood not in MOODS or person not in PERSONS or number not in NUMBERS:
            continue
        if polarity not in POLARITIES:
            continue

        tense = feat_dict.get("Tense", "Pres")
        if tense not in TENSES:
            tense = "Pres"

        mi = MOODS.index(mood)
        ti = TENSES.index(tense)
        pi = PERSONS.index(person)
        ni = NUMBERS.index(number)
        poli = POLARITIES.index(polarity)
        key = (mi, ti, pi, ni, poli)

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
    if len(sys.argv) < 2:
        print(f"Usage: {sys.argv[0]} <path-to-tr_imst-ud-train.conllu>", file=sys.stderr)
        sys.exit(1)
    conllu_path = sys.argv[1]
    output_path = Path(__file__).parent / "cross-linguistic" / "turkish_paradigms.json"

    print(f"Parsing {conllu_path}...")

    noun_tokens = []
    verb_tokens = []

    for form, lemma, upos, feat_dict in parse_conllu(conllu_path):
        if upos == "NOUN":
            noun_tokens.append((form, lemma, feat_dict))
        elif upos in ("VERB", "AUX"):
            verb_tokens.append((form, lemma, feat_dict))

    print(f"  NOUN tokens: {len(noun_tokens)}")
    print(f"  VERB+AUX tokens: {len(verb_tokens)}")

    # Extract paradigm tables
    noun_slots, noun_cov = extract_noun_paradigms(noun_tokens)
    verb_slots, verb_cov = extract_verb_paradigms(verb_tokens)
    verb_pol_slots, verb_pol_cov = extract_verb_paradigms_with_polarity(verb_tokens)

    # Select top lemmas by coverage
    N_TOP = 100

    def top_lemmas(coverage_dict, min_coverage=4):
        items = [(lemma, cov) for lemma, cov in coverage_dict.items()
                 if cov >= min_coverage]
        items.sort(key=lambda x: -x[1])
        return items[:N_TOP]

    top_nouns = top_lemmas(noun_cov, min_coverage=4)
    top_verbs = top_lemmas(verb_cov, min_coverage=4)
    top_verbs_pol = top_lemmas(verb_pol_cov, min_coverage=4)

    print(f"\nSelected paradigms:")
    print(f"  Nouns:      {len(top_nouns)} (coverage range: "
          f"{top_nouns[-1][1] if top_nouns else 0}-{top_nouns[0][1] if top_nouns else 0})")
    print(f"  Verbs (4D): {len(top_verbs)} (coverage range: "
          f"{top_verbs[-1][1] if top_verbs else 0}-{top_verbs[0][1] if top_verbs else 0})")
    print(f"  Verbs (5D): {len(top_verbs_pol)} (coverage range: "
          f"{top_verbs_pol[-1][1] if top_verbs_pol else 0}-{top_verbs_pol[0][1] if top_verbs_pol else 0})")

    # Collect all forms for character vocabulary
    all_forms = set()
    for lemma, _ in top_nouns:
        all_forms.update(noun_slots[lemma].values())
    for lemma, _ in top_verbs:
        all_forms.update(verb_slots[lemma].values())
    for lemma, _ in top_verbs_pol:
        all_forms.update(verb_pol_slots[lemma].values())

    char_to_idx, idx_to_char = build_char_vocabulary(all_forms)

    max_len = max((len(f) for f in all_forms), default=1)
    print(f"\nCharacter vocabulary: {len(char_to_idx)} chars, max_form_len: {max_len}")

    # Build output
    output = {
        "metadata": {
            "language": "Turkish",
            "source": conllu_path,
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
            "dimensions": ["Mood", "Tense", "Person", "Number", "Polarity"],
            "dim_labels": {
                "Mood": MOODS,
                "Tense": TENSES,
                "Person": PERSONS,
                "Number": NUMBERS,
                "Polarity": POLARITIES,
            },
            "shape": [len(MOODS), len(TENSES), len(PERSONS), len(NUMBERS), len(POLARITIES)],
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

    # Fill verb paradigms (4D)
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

    # Fill verb paradigms (5D)
    for lemma, cov in top_verbs_pol:
        slots = verb_pol_slots[lemma]
        total = (len(MOODS) * len(TENSES) * len(PERSONS)
                 * len(NUMBERS) * len(POLARITIES))
        paradigm = {
            "coverage": cov,
            "total_slots": total,
            "fill_rate": round(cov / total, 3),
            "forms": {},
            "char_encoded": {},
        }
        for (mi, ti, pi, ni, poli), form in slots.items():
            slot_key = (f"{MOODS[mi]}|{TENSES[ti]}|{PERSONS[pi]}"
                        f"|{NUMBERS[ni]}|{POLARITIES[poli]}")
            paradigm["forms"][slot_key] = form
            paradigm["char_encoded"][slot_key] = encode_form_as_char_vector(
                form, max_len, char_to_idx
            )
        output["verbs_5d"]["paradigms"][lemma] = paradigm

    # Write output
    with open(output_path, "w", encoding="utf-8") as f:
        json.dump(output, f, ensure_ascii=False, indent=2)

    print(f"\nOutput written to {output_path}")
    print(f"  File size: {output_path.stat().st_size / 1024:.1f} KB")


if __name__ == "__main__":
    main()
