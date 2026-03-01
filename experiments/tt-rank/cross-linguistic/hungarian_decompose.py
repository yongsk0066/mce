#!/usr/bin/env python3
"""
hungarian_decompose.py — TT decomposition and rank analysis of Hungarian paradigm tensors.

Cross-linguistic extension of tt_decompose.py, following the same structure
as turkish_decompose.py.

Key questions:
1. Does the bond-rank = feature interaction dimensionality finding hold for Hungarian?
2. How does Hungarian's definite/indefinite conjugation show up in TT-rank?
   (Compare to Turkish's Polarity dimension — both are 5D verb tensors)
3. Are Hungarian irregular verbs (van/lenni, megy, tesz, vesz, jön) highest-ranked?
4. Does the Mood-Tense bond in Hungarian match Finnish (both have 2 tenses)?

Part of cross-linguistic TT-rank experiment for Paper-2 (SIGMORPHON).
"""

import json
import sys
from pathlib import Path

import numpy as np

# Reuse TT-SVD implementation from parent directory
sys.path.insert(0, str(Path(__file__).parent.parent))
from tt_decompose import (
    NumpyEncoder,
    tt_svd,
    tt_to_full,
    tt_storage,
    compression_ratio,
    analyze_singular_values,
)

sys.path.insert(0, str(Path(__file__).parent))
from hungarian_extract import (
    CASES, NUMBERS, MOODS, TENSES, PERSONS, DEFINITES, DEGREES,
)


# ──────────────────────────────────────────────────────────────
# Tensor builders for Hungarian
# ──────────────────────────────────────────────────────────────

def build_hungarian_noun_tensor(paradigm_data, shape, char_to_idx, max_len):
    """
    Build a Case(18) x Number(2) x CharPos(max_len) tensor for a Hungarian noun.
    """
    n_cases, n_numbers = shape[0], shape[1]
    tensor = np.zeros((n_cases, n_numbers, max_len), dtype=np.float64)

    for slot_key, char_vec in paradigm_data["char_encoded"].items():
        parts = slot_key.split("|")
        case, number = parts[0], parts[1]
        ci = CASES.index(case)
        ni = NUMBERS.index(number)
        tensor[ci, ni, :] = char_vec

    return tensor


def build_hungarian_verb_4d_tensor(paradigm_data, shape, char_to_idx, max_len):
    """
    Build Mood(4) x Tense(2) x Person(3) x Number(2) x CharPos(max_len) tensor.

    Directly comparable to Finnish (Mood(4) x Tense(2) x Person(4) x Number(2)).
    """
    tensor = np.zeros((*shape, max_len), dtype=np.float64)

    for slot_key, char_vec in paradigm_data["char_encoded"].items():
        parts = slot_key.split("|")
        mood, tense, person, number = parts
        mi = MOODS.index(mood)
        ti = TENSES.index(tense)
        pi = PERSONS.index(person)
        ni = NUMBERS.index(number)
        tensor[mi, ti, pi, ni, :] = char_vec

    return tensor


def build_hungarian_verb_5d_tensor(paradigm_data, shape, char_to_idx, max_len):
    """
    Build Mood(4) x Tense(2) x Person(3) x Number(2) x Definite(2) x CharPos tensor.

    The 5D tensor with Definiteness is Hungarian's unique feature.
    Compare to Turkish's Mood(4) x Tense(4) x Person(3) x Number(2) x Polarity(2).
    """
    tensor = np.zeros((*shape, max_len), dtype=np.float64)

    for slot_key, char_vec in paradigm_data["char_encoded"].items():
        parts = slot_key.split("|")
        mood, tense, person, number, definite = parts
        mi = MOODS.index(mood)
        ti = TENSES.index(tense)
        pi = PERSONS.index(person)
        ni = NUMBERS.index(number)
        di = DEFINITES.index(definite)
        tensor[mi, ti, pi, ni, di, :] = char_vec

    return tensor


def build_hungarian_adj_tensor(paradigm_data, shape, char_to_idx, max_len):
    """
    Build Case(18) x Number(2) x Degree(3) x CharPos(max_len) tensor.
    """
    tensor = np.zeros((*shape, max_len), dtype=np.float64)

    for slot_key, char_vec in paradigm_data["char_encoded"].items():
        parts = slot_key.split("|")
        case, number, degree = parts
        ci = CASES.index(case)
        ni = NUMBERS.index(number)
        di = DEGREES.index(degree)
        tensor[ci, ni, di, :] = char_vec

    return tensor


def suffix_diff_encoding(form, lemma):
    """Encode a surface form as its difference from the lemma."""
    shared = 0
    for i in range(min(len(form), len(lemma))):
        if form[i] == lemma[i]:
            shared += 1
        else:
            break
    suffix = form[shared:]
    stem_change = lemma[shared:]
    return shared, stem_change, suffix


def build_suffix_tensor_hungarian_noun(paradigm_data, lemma, max_suffix_len=12):
    """
    Build Case(18) x Number(2) x SuffixPos(max_suffix_len) tensor
    using suffix-difference encoding.

    Hungarian suffixes can be longer than Finnish/Turkish due to the
    extensive case system and vowel harmony variants, so we use
    max_suffix_len=12.
    """
    all_suffixes = []
    for slot_key, form in paradigm_data["forms"].items():
        _, _, suffix = suffix_diff_encoding(form, lemma.lower())
        all_suffixes.append(suffix)

    chars = sorted(set("".join(all_suffixes)))
    char_map = {ch: i + 2 for i, ch in enumerate(chars)}

    tensor = np.zeros((len(CASES), len(NUMBERS), max_suffix_len), dtype=np.float64)

    for slot_key, form in paradigm_data["forms"].items():
        parts = slot_key.split("|")
        case, number = parts[0], parts[1]
        ci = CASES.index(case)
        ni = NUMBERS.index(number)

        _, _, suffix = suffix_diff_encoding(form, lemma.lower())
        for j, ch in enumerate(suffix[:max_suffix_len]):
            tensor[ci, ni, j] = char_map.get(ch, 1)

    return tensor, char_map


# ──────────────────────────────────────────────────────────────
# Load Finnish and Turkish results for comparison
# ──────────────────────────────────────────────────────────────

def load_comparison_data():
    """Load Finnish and Turkish results for cross-linguistic comparison."""
    base = Path(__file__).parent.parent
    comparison = {}

    # Finnish results
    finnish_path = base / "results.json"
    if finnish_path.exists():
        with open(finnish_path, encoding="utf-8") as f:
            comparison["Finnish"] = json.load(f)

    # Turkish results
    turkish_path = base / "cross-linguistic" / "turkish_results.json"
    if turkish_path.exists():
        with open(turkish_path, encoding="utf-8") as f:
            comparison["Turkish"] = json.load(f)

    return comparison


# ──────────────────────────────────────────────────────────────
# Main experiment
# ──────────────────────────────────────────────────────────────

def run_experiment():
    """Run the full Hungarian TT-rank experiment."""
    paradigm_path = Path(__file__).parent / "hungarian_paradigms.json"
    results_path = Path(__file__).parent / "hungarian_results.json"

    if not paradigm_path.exists():
        print("ERROR: hungarian_paradigms.json not found. "
              "Run hungarian_extract.py first.")
        sys.exit(1)

    print("Loading Hungarian paradigm data...")
    with open(paradigm_path, encoding="utf-8") as f:
        data = json.load(f)

    char_to_idx = data["metadata"]["char_to_idx"]
    max_len = data["metadata"]["max_form_length"]
    print(f"  Char vocab: {data['metadata']['char_vocab_size']}, max_len: {max_len}")

    results = {
        "experiment": "TT-rank of Hungarian morphological paradigms",
        "language": "Hungarian",
        "encoding": "character-level (PAD=0, UNK=1, chars=2+)",
        "algorithm": "TT-SVD (Oseledets 2011)",
        "relative_epsilon": 1e-6,
        "nouns": {"paradigms": {}, "summary": {}},
        "suffix_nouns": {"paradigms": {}, "summary": {}},
        "verbs_4d": {"paradigms": {}, "summary": {}},
        "verbs_5d": {"paradigms": {}, "summary": {}},
        "adjectives": {"paradigms": {}, "summary": {}},
    }

    # ── NOUNS ──
    print("\n" + "=" * 60)
    print("HUNGARIAN NOUN PARADIGMS: Case(18) x Number(2) x CharPos")
    print("=" * 60)

    noun_ranks_all = []
    noun_max_ranks = []
    noun_compressions = []

    for lemma, pdata in data["nouns"]["paradigms"].items():
        tensor = build_hungarian_noun_tensor(
            pdata, data["nouns"]["shape"], char_to_idx, max_len
        )

        cores, ranks = tt_svd(tensor, relative_epsilon=1e-6)
        cr = compression_ratio(tensor.shape, cores)

        recon = tt_to_full(cores)
        recon_error = np.linalg.norm(tensor - recon) / max(
            np.linalg.norm(tensor), 1e-15
        )

        max_rank = max(ranks[1:-1])
        noun_ranks_all.append(ranks)
        noun_max_ranks.append(max_rank)
        noun_compressions.append(cr)

        spectra = analyze_singular_values(tensor)
        sv_info = {}
        for k, svs in spectra:
            if len(svs) > 0 and svs[0] > 1e-15:
                significant = int(np.sum(svs > 0.01 * svs[0]))
                sv_info[f"bond_{k}"] = {
                    "rank": ranks[k],
                    "top_5_svs": svs[:5].tolist(),
                    "significant_svs": significant,
                    "sv_ratio_1_2": (
                        float(svs[0] / svs[1])
                        if len(svs) > 1 and svs[1] > 0
                        else float("inf")
                    ),
                }

        results["nouns"]["paradigms"][lemma] = {
            "coverage": pdata["coverage"],
            "fill_rate": pdata["fill_rate"],
            "tensor_shape": list(tensor.shape),
            "tt_ranks": ranks,
            "max_tt_rank": max_rank,
            "compression_ratio": round(cr, 2),
            "recon_error": float(recon_error),
            "sv_analysis": sv_info,
        }

    if noun_max_ranks:
        results["nouns"]["summary"] = {
            "n_paradigms": len(noun_max_ranks),
            "mean_max_rank": round(float(np.mean(noun_max_ranks)), 2),
            "median_max_rank": round(float(np.median(noun_max_ranks)), 2),
            "std_max_rank": round(float(np.std(noun_max_ranks)), 2),
            "min_max_rank": int(np.min(noun_max_ranks)),
            "max_max_rank": int(np.max(noun_max_ranks)),
            "rank_histogram": {
                str(r): int(c)
                for r, c in zip(*np.unique(noun_max_ranks, return_counts=True))
            },
            "mean_compression": round(float(np.mean(noun_compressions)), 2),
        }

        print(f"\n  Paradigms analyzed: {len(noun_max_ranks)}")
        print(
            f"  Max TT-rank: mean={np.mean(noun_max_ranks):.2f}"
            f" +/- {np.std(noun_max_ranks):.2f}"
        )
        print(
            f"  Max TT-rank range: [{np.min(noun_max_ranks)},"
            f" {np.max(noun_max_ranks)}]"
        )
        print(f"  Mean compression ratio: {np.mean(noun_compressions):.2f}x")

    # ── SUFFIX NOUNS ──
    print("\n" + "=" * 60)
    print("HUNGARIAN NOUNS (SUFFIX ENCODING): Case(18) x Number(2) x SuffixPos")
    print("=" * 60)

    suffix_max_ranks = []
    suffix_compressions = []
    max_suffix_len = 12

    for lemma, pdata in data["nouns"]["paradigms"].items():
        tensor, _ = build_suffix_tensor_hungarian_noun(
            pdata, lemma, max_suffix_len
        )

        cores, ranks = tt_svd(tensor, relative_epsilon=1e-6)
        cr = compression_ratio(tensor.shape, cores)
        recon = tt_to_full(cores)
        recon_error = np.linalg.norm(tensor - recon) / max(
            np.linalg.norm(tensor), 1e-15
        )

        max_rank = max(ranks[1:-1])
        suffix_max_ranks.append(max_rank)
        suffix_compressions.append(cr)

        spectra = analyze_singular_values(tensor)
        sv_info = {}
        for k, svs in spectra:
            if len(svs) > 0 and svs[0] > 1e-15:
                significant = int(np.sum(svs > 0.01 * svs[0]))
                sv_info[f"bond_{k}"] = {
                    "rank": ranks[k],
                    "top_5_svs": svs[:5].tolist(),
                    "significant_svs": significant,
                }

        results["suffix_nouns"]["paradigms"][lemma] = {
            "coverage": pdata["coverage"],
            "fill_rate": pdata["fill_rate"],
            "tensor_shape": list(tensor.shape),
            "tt_ranks": ranks,
            "max_tt_rank": max_rank,
            "compression_ratio": round(cr, 2),
            "recon_error": float(recon_error),
            "sv_analysis": sv_info,
        }

    if suffix_max_ranks:
        results["suffix_nouns"]["summary"] = {
            "n_paradigms": len(suffix_max_ranks),
            "encoding": "suffix-difference from lemma",
            "max_suffix_len": max_suffix_len,
            "mean_max_rank": round(float(np.mean(suffix_max_ranks)), 2),
            "median_max_rank": round(float(np.median(suffix_max_ranks)), 2),
            "std_max_rank": round(float(np.std(suffix_max_ranks)), 2),
            "min_max_rank": int(np.min(suffix_max_ranks)),
            "max_max_rank": int(np.max(suffix_max_ranks)),
            "rank_histogram": {
                str(r): int(c)
                for r, c in zip(*np.unique(suffix_max_ranks, return_counts=True))
            },
            "mean_compression": round(float(np.mean(suffix_compressions)), 2),
        }

        print(f"\n  Paradigms analyzed: {len(suffix_max_ranks)}")
        print(
            f"  Max TT-rank: mean={np.mean(suffix_max_ranks):.2f}"
            f" +/- {np.std(suffix_max_ranks):.2f}"
        )
        print(
            f"  Max TT-rank range: [{np.min(suffix_max_ranks)},"
            f" {np.max(suffix_max_ranks)}]"
        )
        print(f"  Mean compression ratio: {np.mean(suffix_compressions):.2f}x")

    # ── VERBS (4D) ──
    print("\n" + "=" * 60)
    print("HUNGARIAN VERB PARADIGMS (4D): Mood(4) x Tense(2) x Person(3) "
          "x Number(2) x CharPos")
    print("=" * 60)

    verb4_ranks_all = []
    verb4_max_ranks = []
    verb4_compressions = []
    verb4_bond_ranks = {1: [], 2: [], 3: [], 4: []}

    for lemma, pdata in data["verbs_4d"]["paradigms"].items():
        tensor = build_hungarian_verb_4d_tensor(
            pdata, data["verbs_4d"]["shape"], char_to_idx, max_len
        )

        cores, ranks = tt_svd(tensor, relative_epsilon=1e-6)
        cr = compression_ratio(tensor.shape, cores)
        recon = tt_to_full(cores)
        recon_error = np.linalg.norm(tensor - recon) / max(
            np.linalg.norm(tensor), 1e-15
        )

        max_rank = max(ranks[1:-1]) if len(ranks) > 2 else 1
        verb4_ranks_all.append(ranks)
        verb4_max_ranks.append(max_rank)
        verb4_compressions.append(cr)

        # Collect bond-specific ranks
        for b in range(1, min(5, len(ranks) - 1)):
            verb4_bond_ranks[b].append(ranks[b])

        spectra = analyze_singular_values(tensor)
        sv_info = {}
        for k, svs in spectra:
            if len(svs) > 0 and svs[0] > 1e-15:
                significant = int(np.sum(svs > 0.01 * svs[0]))
                sv_info[f"bond_{k}"] = {
                    "rank": ranks[k],
                    "top_5_svs": svs[: min(5, len(svs))].tolist(),
                    "significant_svs": significant,
                }

        results["verbs_4d"]["paradigms"][lemma] = {
            "coverage": pdata["coverage"],
            "fill_rate": pdata["fill_rate"],
            "tensor_shape": list(tensor.shape),
            "tt_ranks": ranks,
            "max_tt_rank": max_rank,
            "compression_ratio": round(cr, 2),
            "recon_error": float(recon_error),
            "sv_analysis": sv_info,
        }

    if verb4_max_ranks:
        bond_summary = {}
        for b in range(1, 5):
            if verb4_bond_ranks[b]:
                bond_summary[f"bond_{b}"] = {
                    "mean_rank": round(float(np.mean(verb4_bond_ranks[b])), 2),
                    "std_rank": round(float(np.std(verb4_bond_ranks[b])), 2),
                    "min_rank": int(np.min(verb4_bond_ranks[b])),
                    "max_rank": int(np.max(verb4_bond_ranks[b])),
                    "rank_histogram": {
                        str(r): int(c)
                        for r, c in zip(
                            *np.unique(verb4_bond_ranks[b], return_counts=True)
                        )
                    },
                }

        results["verbs_4d"]["summary"] = {
            "n_paradigms": len(verb4_max_ranks),
            "mean_max_rank": round(float(np.mean(verb4_max_ranks)), 2),
            "median_max_rank": round(float(np.median(verb4_max_ranks)), 2),
            "std_max_rank": round(float(np.std(verb4_max_ranks)), 2),
            "min_max_rank": int(np.min(verb4_max_ranks)),
            "max_max_rank": int(np.max(verb4_max_ranks)),
            "rank_histogram": {
                str(r): int(c)
                for r, c in zip(
                    *np.unique(verb4_max_ranks, return_counts=True)
                )
            },
            "mean_compression": round(float(np.mean(verb4_compressions)), 2),
            "bond_analysis": bond_summary,
        }

        print(f"\n  Paradigms analyzed: {len(verb4_max_ranks)}")
        print(
            f"  Max TT-rank: mean={np.mean(verb4_max_ranks):.2f}"
            f" +/- {np.std(verb4_max_ranks):.2f}"
        )
        print(
            f"  Max TT-rank range: [{np.min(verb4_max_ranks)},"
            f" {np.max(verb4_max_ranks)}]"
        )
        print(f"  Mean compression ratio: {np.mean(verb4_compressions):.2f}x")

        print("\n  Bond-specific analysis:")
        for b in range(1, 5):
            if verb4_bond_ranks[b]:
                dims_left = ["Mood", "Tense", "Person", "Number"][:b]
                dims_right = ["Mood", "Tense", "Person", "Number"][b:] + ["Char"]
                print(
                    f"    Bond {b}: {'+'.join(dims_left)} | "
                    f"{'+'.join(dims_right)}"
                    f"  mean={np.mean(verb4_bond_ranks[b]):.2f}"
                    f" +/- {np.std(verb4_bond_ranks[b]):.2f}"
                    f"  range=[{np.min(verb4_bond_ranks[b])},"
                    f" {np.max(verb4_bond_ranks[b])}]"
                )

    # ── VERBS (5D with Definiteness) ──
    print("\n" + "=" * 60)
    print("HUNGARIAN VERB PARADIGMS (5D): Mood x Tense x Person x Number "
          "x Definite x CharPos")
    print("=" * 60)

    verb5_ranks_all = []
    verb5_max_ranks = []
    verb5_compressions = []
    verb5_bond_ranks = {1: [], 2: [], 3: [], 4: [], 5: []}

    for lemma, pdata in data["verbs_5d"]["paradigms"].items():
        tensor = build_hungarian_verb_5d_tensor(
            pdata, data["verbs_5d"]["shape"], char_to_idx, max_len
        )

        cores, ranks = tt_svd(tensor, relative_epsilon=1e-6)
        cr = compression_ratio(tensor.shape, cores)
        recon = tt_to_full(cores)
        recon_error = np.linalg.norm(tensor - recon) / max(
            np.linalg.norm(tensor), 1e-15
        )

        max_rank = max(ranks[1:-1]) if len(ranks) > 2 else 1
        verb5_ranks_all.append(ranks)
        verb5_max_ranks.append(max_rank)
        verb5_compressions.append(cr)

        for b in range(1, min(6, len(ranks) - 1)):
            verb5_bond_ranks[b].append(ranks[b])

        spectra = analyze_singular_values(tensor)
        sv_info = {}
        for k, svs in spectra:
            if len(svs) > 0 and svs[0] > 1e-15:
                significant = int(np.sum(svs > 0.01 * svs[0]))
                sv_info[f"bond_{k}"] = {
                    "rank": ranks[k],
                    "top_5_svs": svs[: min(5, len(svs))].tolist(),
                    "significant_svs": significant,
                }

        results["verbs_5d"]["paradigms"][lemma] = {
            "coverage": pdata["coverage"],
            "fill_rate": pdata["fill_rate"],
            "tensor_shape": list(tensor.shape),
            "tt_ranks": ranks,
            "max_tt_rank": max_rank,
            "compression_ratio": round(cr, 2),
            "recon_error": float(recon_error),
            "sv_analysis": sv_info,
        }

    if verb5_max_ranks:
        bond_summary_5d = {}
        for b in range(1, 6):
            if verb5_bond_ranks[b]:
                bond_summary_5d[f"bond_{b}"] = {
                    "mean_rank": round(
                        float(np.mean(verb5_bond_ranks[b])), 2
                    ),
                    "std_rank": round(
                        float(np.std(verb5_bond_ranks[b])), 2
                    ),
                    "min_rank": int(np.min(verb5_bond_ranks[b])),
                    "max_rank": int(np.max(verb5_bond_ranks[b])),
                    "rank_histogram": {
                        str(r): int(c)
                        for r, c in zip(
                            *np.unique(verb5_bond_ranks[b], return_counts=True)
                        )
                    },
                }

        results["verbs_5d"]["summary"] = {
            "n_paradigms": len(verb5_max_ranks),
            "mean_max_rank": round(float(np.mean(verb5_max_ranks)), 2),
            "median_max_rank": round(float(np.median(verb5_max_ranks)), 2),
            "std_max_rank": round(float(np.std(verb5_max_ranks)), 2),
            "min_max_rank": int(np.min(verb5_max_ranks)),
            "max_max_rank": int(np.max(verb5_max_ranks)),
            "mean_compression": round(float(np.mean(verb5_compressions)), 2),
            "bond_analysis": bond_summary_5d,
        }

        print(f"\n  Paradigms analyzed: {len(verb5_max_ranks)}")
        print(
            f"  Max TT-rank: mean={np.mean(verb5_max_ranks):.2f}"
            f" +/- {np.std(verb5_max_ranks):.2f}"
        )
        print(
            f"  Max TT-rank range: [{np.min(verb5_max_ranks)},"
            f" {np.max(verb5_max_ranks)}]"
        )
        print(f"  Mean compression ratio: {np.mean(verb5_compressions):.2f}x")

        print("\n  Bond-specific analysis:")
        dims_5d = ["Mood", "Tense", "Person", "Number", "Definite"]
        for b in range(1, 6):
            if verb5_bond_ranks[b]:
                dims_left = dims_5d[:b]
                dims_right = dims_5d[b:] + ["Char"]
                print(
                    f"    Bond {b}: {'+'.join(dims_left)} | "
                    f"{'+'.join(dims_right)}"
                    f"  mean={np.mean(verb5_bond_ranks[b]):.2f}"
                    f" +/- {np.std(verb5_bond_ranks[b]):.2f}"
                    f"  range=[{np.min(verb5_bond_ranks[b])},"
                    f" {np.max(verb5_bond_ranks[b])}]"
                )

    # ── ADJECTIVES ──
    print("\n" + "=" * 60)
    print("HUNGARIAN ADJ PARADIGMS: Case(18) x Number(2) x Degree(3) x CharPos")
    print("=" * 60)

    adj_ranks_all = []
    adj_max_ranks = []
    adj_compressions = []

    for lemma, pdata in data["adjectives"]["paradigms"].items():
        tensor = build_hungarian_adj_tensor(
            pdata, data["adjectives"]["shape"], char_to_idx, max_len
        )

        cores, ranks = tt_svd(tensor, relative_epsilon=1e-6)
        cr = compression_ratio(tensor.shape, cores)
        recon = tt_to_full(cores)
        recon_error = np.linalg.norm(tensor - recon) / max(
            np.linalg.norm(tensor), 1e-15
        )

        max_rank = max(ranks[1:-1]) if len(ranks) > 2 else 1
        adj_ranks_all.append(ranks)
        adj_max_ranks.append(max_rank)
        adj_compressions.append(cr)

        spectra = analyze_singular_values(tensor)
        sv_info = {}
        for k, svs in spectra:
            if len(svs) > 0 and svs[0] > 1e-15:
                significant = int(np.sum(svs > 0.01 * svs[0]))
                sv_info[f"bond_{k}"] = {
                    "rank": ranks[k],
                    "top_5_svs": svs[: min(5, len(svs))].tolist(),
                    "significant_svs": significant,
                }

        results["adjectives"]["paradigms"][lemma] = {
            "coverage": pdata["coverage"],
            "fill_rate": pdata["fill_rate"],
            "tensor_shape": list(tensor.shape),
            "tt_ranks": ranks,
            "max_tt_rank": max_rank,
            "compression_ratio": round(cr, 2),
            "recon_error": float(recon_error),
            "sv_analysis": sv_info,
        }

    if adj_max_ranks:
        results["adjectives"]["summary"] = {
            "n_paradigms": len(adj_max_ranks),
            "mean_max_rank": round(float(np.mean(adj_max_ranks)), 2),
            "median_max_rank": round(float(np.median(adj_max_ranks)), 2),
            "std_max_rank": round(float(np.std(adj_max_ranks)), 2),
            "min_max_rank": int(np.min(adj_max_ranks)),
            "max_max_rank": int(np.max(adj_max_ranks)),
            "rank_histogram": {
                str(r): int(c)
                for r, c in zip(
                    *np.unique(adj_max_ranks, return_counts=True)
                )
            },
            "mean_compression": round(float(np.mean(adj_compressions)), 2),
        }

        print(f"\n  Paradigms analyzed: {len(adj_max_ranks)}")
        print(
            f"  Max TT-rank: mean={np.mean(adj_max_ranks):.2f}"
            f" +/- {np.std(adj_max_ranks):.2f}"
        )
        print(
            f"  Max TT-rank range: [{np.min(adj_max_ranks)},"
            f" {np.max(adj_max_ranks)}]"
        )
        print(f"  Mean compression ratio: {np.mean(adj_compressions):.2f}x")

    # ── FILL RATE vs TT-RANK CORRELATION ──
    print("\n" + "=" * 60)
    print("FILL RATE vs TT-RANK CORRELATION")
    print("=" * 60)

    for pos_name, pos_data in [
        ("nouns", results["nouns"]),
        ("verbs_4d", results["verbs_4d"]),
        ("verbs_5d", results["verbs_5d"]),
        ("adjectives", results["adjectives"]),
    ]:
        fills = []
        ranks_list = []
        for lemma, pdata in pos_data["paradigms"].items():
            fills.append(pdata["fill_rate"])
            ranks_list.append(pdata["max_tt_rank"])
        if len(fills) > 2:
            corr = np.corrcoef(fills, ranks_list)[0, 1]
            print(f"  {pos_name}: Pearson r = {corr:.4f} (n={len(fills)})")
            if "summary" in pos_data and pos_data["summary"]:
                pos_data["summary"]["fill_rank_correlation"] = round(
                    float(corr), 4
                )

    # ── NOTABLE PARADIGMS ──
    print("\n" + "=" * 60)
    print("NOTABLE PARADIGMS")
    print("=" * 60)

    # Nouns
    if results["nouns"]["paradigms"]:
        noun_items = [
            (l, r["max_tt_rank"], r["fill_rate"])
            for l, r in results["nouns"]["paradigms"].items()
        ]

        noun_items.sort(key=lambda x: (x[1], -x[2]))
        print("\n  Most regular nouns (lowest TT-rank):")
        for lemma, rank, fill in noun_items[:10]:
            print(f"    {lemma:<25s} max_rank={rank}, fill={fill:.3f}")

        noun_items.sort(key=lambda x: (-x[1], -x[2]))
        print("\n  Most irregular nouns (highest TT-rank):")
        for lemma, rank, fill in noun_items[:10]:
            print(f"    {lemma:<25s} max_rank={rank}, fill={fill:.3f}")

    # Verbs (4D)
    if results["verbs_4d"]["paradigms"]:
        verb_items = [
            (l, r["max_tt_rank"], r["fill_rate"])
            for l, r in results["verbs_4d"]["paradigms"].items()
        ]

        verb_items.sort(key=lambda x: (x[1], -x[2]))
        print("\n  Most regular verbs (lowest TT-rank):")
        for lemma, rank, fill in verb_items[:10]:
            print(f"    {lemma:<25s} max_rank={rank}, fill={fill:.3f}")

        verb_items.sort(key=lambda x: (-x[1], -x[2]))
        print("\n  Most irregular verbs (highest TT-rank):")
        for lemma, rank, fill in verb_items[:10]:
            print(f"    {lemma:<25s} max_rank={rank}, fill={fill:.3f}")

    results["notable"] = {}
    if results["nouns"]["paradigms"]:
        noun_items_sorted = sorted(
            [(l, r["max_tt_rank"])
             for l, r in results["nouns"]["paradigms"].items()],
            key=lambda x: x[1],
        )
        results["notable"]["lowest_rank_nouns"] = noun_items_sorted[:10]
        results["notable"]["highest_rank_nouns"] = noun_items_sorted[-10:][
            ::-1
        ]

    if results["verbs_4d"]["paradigms"]:
        verb_items_sorted = sorted(
            [(l, r["max_tt_rank"])
             for l, r in results["verbs_4d"]["paradigms"].items()],
            key=lambda x: x[1],
        )
        results["notable"]["lowest_rank_verbs"] = verb_items_sorted[:10]
        results["notable"]["highest_rank_verbs"] = verb_items_sorted[-10:][
            ::-1
        ]

    # ── KNOWN IRREGULAR HUNGARIAN VERBS ──
    print("\n" + "=" * 60)
    print("KNOWN IRREGULAR HUNGARIAN VERBS")
    print("=" * 60)

    # Hungarian irregular verbs:
    # van/lenni (to be) - most irregular, suppletive
    # megy (to go) - stem change men-/me-
    # tesz (to do/put) - stem change te-/tev-/tet-
    # vesz (to take/buy) - stem change ve-/vev-/vet-
    # lesz (to become) - stem change le-/lev-/let-
    # visz (to carry) - stem change vi-/viv-/vit-
    # hisz (to believe) - stem change hi-/hiv-/hit-
    # eszik (to eat) - ik-verb + irregular
    # iszik (to drink) - ik-verb + irregular
    # jön (to come) - stem change jö-/jöv-/jöt-
    # alszik (to sleep) - consonant cluster changes
    irregular_verbs = [
        "van", "lesz", "megy", "tesz", "vesz", "visz", "hisz",
        "eszik", "iszik", "jön", "alszik", "ad", "mond", "tud", "lát",
        "kap", "kell", "fog", "akar", "marad",
    ]
    print(f"\n  Checking: {irregular_verbs}")
    for v in irregular_verbs:
        found = False
        for tensor_type in ["verbs_4d", "verbs_5d"]:
            if v in results[tensor_type]["paradigms"]:
                r = results[tensor_type]["paradigms"][v]
                print(
                    f"    {v:<15s} ({tensor_type}) max_rank={r['max_tt_rank']}, "
                    f"fill={r['fill_rate']:.3f}, "
                    f"ranks={r['tt_ranks']}"
                )
                found = True
        if not found:
            print(f"    {v:<15s} (not in top paradigms)")

    # ── CROSS-LINGUISTIC COMPARISON ──
    print("\n" + "=" * 60)
    print("CROSS-LINGUISTIC COMPARISON: Finnish vs Turkish vs Hungarian")
    print("=" * 60)

    comparison_data = load_comparison_data()

    cross_ling = {}

    # Gather verb 4D bond data from all three languages
    for lang_name, lang_results in [
        ("Hungarian", results),
        ("Finnish", comparison_data.get("Finnish")),
        ("Turkish", comparison_data.get("Turkish")),
    ]:
        if lang_results is None:
            continue

        # Get verb bond data
        verb_key = "verbs" if lang_name == "Finnish" else "verbs_4d"
        if verb_key in lang_results and "summary" in lang_results[verb_key]:
            summary = lang_results[verb_key]["summary"]
            cross_ling[lang_name] = {
                "n_verb_paradigms": summary.get("n_paradigms", 0),
                "mean_max_rank": summary.get("mean_max_rank", 0),
                "fill_rank_corr": summary.get("fill_rank_correlation", 0),
            }

            if "bond_analysis" in summary:
                for bond_key, bond_data in summary["bond_analysis"].items():
                    cross_ling[lang_name][bond_key] = bond_data.get(
                        "mean_rank", 0
                    )

    if cross_ling:
        print("\n  Verb 4D bond means:")
        print(f"  {'Language':<12s} {'Bond 1':>8s} {'Bond 2':>8s} "
              f"{'Bond 3':>8s} {'Bond 4':>8s} {'MaxRank':>8s}")
        print("  " + "-" * 52)
        for lang in ["Finnish", "Turkish", "Hungarian"]:
            if lang in cross_ling:
                d = cross_ling[lang]
                b1 = d.get("bond_1", "-")
                b2 = d.get("bond_2", "-")
                b3 = d.get("bond_3", "-")
                b4 = d.get("bond_4", "-")
                mr = d.get("mean_max_rank", "-")
                print(f"  {lang:<12s} {b1:>8} {b2:>8} {b3:>8} {b4:>8} {mr:>8}")

    results["cross_linguistic_comparison"] = cross_ling

    # Write results
    with open(results_path, "w", encoding="utf-8") as f:
        json.dump(results, f, ensure_ascii=False, indent=2, cls=NumpyEncoder)
    print(f"\n\nResults written to {results_path}")
    print(f"  File size: {results_path.stat().st_size / 1024:.1f} KB")


if __name__ == "__main__":
    run_experiment()
