#!/usr/bin/env python3
"""
turkish_decompose.py — TT decomposition and rank analysis of Turkish paradigm tensors.

Cross-linguistic extension of tt_decompose.py.

Reuses the TT-SVD implementation from tt_decompose.py and applies it
to Turkish morphological paradigms extracted by turkish_extract.py.

Key questions:
1. Does Turkish also show the bond-rank = feature interaction pattern?
2. Do Turkish irregular verbs (olmak, etmek, gelmek) have higher TT-rank?
3. How do Turkish bond ranks compare to Finnish bond ranks?

Part of cross-linguistic TT-rank experiment for Paper-2 (SIGMORPHON).
"""

import json
import sys
from pathlib import Path

import numpy as np

# Reuse TT-SVD implementation
sys.path.insert(0, str(Path(__file__).parent))
from tt_decompose import (
    NumpyEncoder,
    tt_svd,
    tt_to_full,
    tt_storage,
    compression_ratio,
    analyze_singular_values,
)
from turkish_extract import CASES, NUMBERS, MOODS, TENSES, PERSONS, POLARITIES


# ──────────────────────────────────────────────────────────────
# Tensor builders for Turkish
# ──────────────────────────────────────────────────────────────

def build_turkish_noun_tensor(paradigm_data, shape, char_to_idx, max_len):
    """
    Build a Case(6) x Number(2) x CharPos(max_len) tensor for a Turkish noun.
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


def build_turkish_verb_4d_tensor(paradigm_data, shape, char_to_idx, max_len):
    """
    Build Mood(4) x Tense(4) x Person(3) x Number(2) x CharPos(max_len) tensor.
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


def build_turkish_verb_5d_tensor(paradigm_data, shape, char_to_idx, max_len):
    """
    Build Mood(4) x Tense(4) x Person(3) x Number(2) x Polarity(2) x CharPos tensor.
    """
    tensor = np.zeros((*shape, max_len), dtype=np.float64)

    for slot_key, char_vec in paradigm_data["char_encoded"].items():
        parts = slot_key.split("|")
        mood, tense, person, number, polarity = parts
        mi = MOODS.index(mood)
        ti = TENSES.index(tense)
        pi = PERSONS.index(person)
        ni = NUMBERS.index(number)
        poli = POLARITIES.index(polarity)
        tensor[mi, ti, pi, ni, poli, :] = char_vec

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


def build_suffix_tensor_turkish_noun(paradigm_data, lemma, max_suffix_len=10):
    """
    Build Case(6) x Number(2) x SuffixPos(max_suffix_len) tensor
    using suffix-difference encoding.
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
# Main experiment
# ──────────────────────────────────────────────────────────────

def run_experiment():
    """Run the full Turkish TT-rank experiment."""
    paradigm_path = Path(__file__).parent / "cross-linguistic" / "turkish_paradigms.json"
    results_path = Path(__file__).parent / "cross-linguistic" / "turkish_results.json"

    if not paradigm_path.exists():
        print("ERROR: turkish_paradigms.json not found. Run turkish_extract.py first.")
        sys.exit(1)

    print("Loading Turkish paradigm data...")
    with open(paradigm_path, encoding="utf-8") as f:
        data = json.load(f)

    char_to_idx = data["metadata"]["char_to_idx"]
    max_len = data["metadata"]["max_form_length"]
    print(f"  Char vocab: {data['metadata']['char_vocab_size']}, max_len: {max_len}")

    results = {
        "experiment": "TT-rank of Turkish morphological paradigms",
        "language": "Turkish",
        "encoding": "character-level (PAD=0, UNK=1, chars=2+)",
        "algorithm": "TT-SVD (Oseledets 2011)",
        "relative_epsilon": 1e-6,
        "nouns": {"paradigms": {}, "summary": {}},
        "suffix_nouns": {"paradigms": {}, "summary": {}},
        "verbs_4d": {"paradigms": {}, "summary": {}},
        "verbs_5d": {"paradigms": {}, "summary": {}},
    }

    # ── NOUNS ──
    print("\n" + "=" * 60)
    print("TURKISH NOUN PARADIGMS: Case(6) x Number(2) x CharPos")
    print("=" * 60)

    noun_ranks_all = []
    noun_max_ranks = []
    noun_compressions = []

    for lemma, pdata in data["nouns"]["paradigms"].items():
        tensor = build_turkish_noun_tensor(
            pdata, data["nouns"]["shape"], char_to_idx, max_len
        )

        cores, ranks = tt_svd(tensor, relative_epsilon=1e-6)
        cr = compression_ratio(tensor.shape, cores)

        recon = tt_to_full(cores)
        recon_error = np.linalg.norm(tensor - recon) / max(np.linalg.norm(tensor), 1e-15)

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
                    "sv_ratio_1_2": (float(svs[0] / svs[1])
                                     if len(svs) > 1 and svs[1] > 0
                                     else float("inf")),
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
            "rank_histogram": {str(r): int(c) for r, c in
                              zip(*np.unique(noun_max_ranks, return_counts=True))},
            "mean_compression": round(float(np.mean(noun_compressions)), 2),
        }

        print(f"\n  Paradigms analyzed: {len(noun_max_ranks)}")
        print(f"  Max TT-rank: mean={np.mean(noun_max_ranks):.2f}"
              f" +/- {np.std(noun_max_ranks):.2f}")
        print(f"  Max TT-rank range: [{np.min(noun_max_ranks)},"
              f" {np.max(noun_max_ranks)}]")
        print(f"  Mean compression ratio: {np.mean(noun_compressions):.2f}x")

    # ── SUFFIX NOUNS ──
    print("\n" + "=" * 60)
    print("TURKISH NOUNS (SUFFIX ENCODING): Case(6) x Number(2) x SuffixPos")
    print("=" * 60)

    suffix_max_ranks = []
    suffix_compressions = []
    max_suffix_len = 10

    for lemma, pdata in data["nouns"]["paradigms"].items():
        tensor, _ = build_suffix_tensor_turkish_noun(pdata, lemma, max_suffix_len)

        cores, ranks = tt_svd(tensor, relative_epsilon=1e-6)
        cr = compression_ratio(tensor.shape, cores)
        recon = tt_to_full(cores)
        recon_error = np.linalg.norm(tensor - recon) / max(np.linalg.norm(tensor), 1e-15)

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
            "rank_histogram": {str(r): int(c) for r, c in
                              zip(*np.unique(suffix_max_ranks, return_counts=True))},
            "mean_compression": round(float(np.mean(suffix_compressions)), 2),
        }

        print(f"\n  Paradigms analyzed: {len(suffix_max_ranks)}")
        print(f"  Max TT-rank: mean={np.mean(suffix_max_ranks):.2f}"
              f" +/- {np.std(suffix_max_ranks):.2f}")
        print(f"  Max TT-rank range: [{np.min(suffix_max_ranks)},"
              f" {np.max(suffix_max_ranks)}]")
        print(f"  Mean compression ratio: {np.mean(suffix_compressions):.2f}x")

    # ── VERBS (4D) ──
    print("\n" + "=" * 60)
    print("TURKISH VERB PARADIGMS (4D): Mood(4) x Tense(4) x Person(3) x Number(2) x CharPos")
    print("=" * 60)

    verb4_ranks_all = []
    verb4_max_ranks = []
    verb4_compressions = []
    verb4_bond_ranks = {1: [], 2: [], 3: [], 4: []}

    for lemma, pdata in data["verbs_4d"]["paradigms"].items():
        tensor = build_turkish_verb_4d_tensor(
            pdata, data["verbs_4d"]["shape"], char_to_idx, max_len
        )

        cores, ranks = tt_svd(tensor, relative_epsilon=1e-6)
        cr = compression_ratio(tensor.shape, cores)
        recon = tt_to_full(cores)
        recon_error = np.linalg.norm(tensor - recon) / max(np.linalg.norm(tensor), 1e-15)

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
                    "top_5_svs": svs[:min(5, len(svs))].tolist(),
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
                        str(r): int(c) for r, c in
                        zip(*np.unique(verb4_bond_ranks[b], return_counts=True))
                    },
                }

        results["verbs_4d"]["summary"] = {
            "n_paradigms": len(verb4_max_ranks),
            "mean_max_rank": round(float(np.mean(verb4_max_ranks)), 2),
            "median_max_rank": round(float(np.median(verb4_max_ranks)), 2),
            "std_max_rank": round(float(np.std(verb4_max_ranks)), 2),
            "min_max_rank": int(np.min(verb4_max_ranks)),
            "max_max_rank": int(np.max(verb4_max_ranks)),
            "rank_histogram": {str(r): int(c) for r, c in
                              zip(*np.unique(verb4_max_ranks, return_counts=True))},
            "mean_compression": round(float(np.mean(verb4_compressions)), 2),
            "bond_analysis": bond_summary,
        }

        print(f"\n  Paradigms analyzed: {len(verb4_max_ranks)}")
        print(f"  Max TT-rank: mean={np.mean(verb4_max_ranks):.2f}"
              f" +/- {np.std(verb4_max_ranks):.2f}")
        print(f"  Max TT-rank range: [{np.min(verb4_max_ranks)},"
              f" {np.max(verb4_max_ranks)}]")
        print(f"  Mean compression ratio: {np.mean(verb4_compressions):.2f}x")

        print("\n  Bond-specific analysis:")
        for b in range(1, 5):
            if verb4_bond_ranks[b]:
                dims_left = ["Mood", "Tense", "Person", "Number"][:b]
                dims_right = ["Mood", "Tense", "Person", "Number"][b:] + ["Char"]
                print(f"    Bond {b}: {'+'.join(dims_left)} | {'+'.join(dims_right)}"
                      f"  mean={np.mean(verb4_bond_ranks[b]):.2f}"
                      f" +/- {np.std(verb4_bond_ranks[b]):.2f}"
                      f"  range=[{np.min(verb4_bond_ranks[b])},"
                      f" {np.max(verb4_bond_ranks[b])}]")

    # ── VERBS (5D with Polarity) ──
    print("\n" + "=" * 60)
    print("TURKISH VERB PARADIGMS (5D): Mood x Tense x Person x Number x Polarity x CharPos")
    print("=" * 60)

    verb5_ranks_all = []
    verb5_max_ranks = []
    verb5_compressions = []
    verb5_bond_ranks = {1: [], 2: [], 3: [], 4: [], 5: []}

    for lemma, pdata in data["verbs_5d"]["paradigms"].items():
        tensor = build_turkish_verb_5d_tensor(
            pdata, data["verbs_5d"]["shape"], char_to_idx, max_len
        )

        cores, ranks = tt_svd(tensor, relative_epsilon=1e-6)
        cr = compression_ratio(tensor.shape, cores)
        recon = tt_to_full(cores)
        recon_error = np.linalg.norm(tensor - recon) / max(np.linalg.norm(tensor), 1e-15)

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
                    "top_5_svs": svs[:min(5, len(svs))].tolist(),
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
                    "mean_rank": round(float(np.mean(verb5_bond_ranks[b])), 2),
                    "std_rank": round(float(np.std(verb5_bond_ranks[b])), 2),
                    "min_rank": int(np.min(verb5_bond_ranks[b])),
                    "max_rank": int(np.max(verb5_bond_ranks[b])),
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
        print(f"  Max TT-rank: mean={np.mean(verb5_max_ranks):.2f}"
              f" +/- {np.std(verb5_max_ranks):.2f}")
        print(f"  Max TT-rank range: [{np.min(verb5_max_ranks)},"
              f" {np.max(verb5_max_ranks)}]")
        print(f"  Mean compression ratio: {np.mean(verb5_compressions):.2f}x")

        print("\n  Bond-specific analysis:")
        dims_5d = ["Mood", "Tense", "Person", "Number", "Polarity"]
        for b in range(1, 6):
            if verb5_bond_ranks[b]:
                dims_left = dims_5d[:b]
                dims_right = dims_5d[b:] + ["Char"]
                print(f"    Bond {b}: {'+'.join(dims_left)} | {'+'.join(dims_right)}"
                      f"  mean={np.mean(verb5_bond_ranks[b]):.2f}"
                      f" +/- {np.std(verb5_bond_ranks[b]):.2f}"
                      f"  range=[{np.min(verb5_bond_ranks[b])},"
                      f" {np.max(verb5_bond_ranks[b])}]")

    # ── FILL RATE vs TT-RANK CORRELATION ──
    print("\n" + "=" * 60)
    print("FILL RATE vs TT-RANK CORRELATION")
    print("=" * 60)

    for pos_name, pos_data in [("nouns", results["nouns"]),
                                ("verbs_4d", results["verbs_4d"]),
                                ("verbs_5d", results["verbs_5d"])]:
        fills = []
        ranks_list = []
        for lemma, pdata in pos_data["paradigms"].items():
            fills.append(pdata["fill_rate"])
            ranks_list.append(pdata["max_tt_rank"])
        if len(fills) > 2:
            corr = np.corrcoef(fills, ranks_list)[0, 1]
            print(f"  {pos_name}: Pearson r = {corr:.4f} (n={len(fills)})")
            if "summary" in pos_data:
                pos_data["summary"]["fill_rank_correlation"] = round(float(corr), 4)

    # ── NOTABLE PARADIGMS ──
    print("\n" + "=" * 60)
    print("NOTABLE PARADIGMS")
    print("=" * 60)

    # Nouns
    if results["nouns"]["paradigms"]:
        noun_items = [(l, r["max_tt_rank"], r["fill_rate"])
                      for l, r in results["nouns"]["paradigms"].items()]

        noun_items.sort(key=lambda x: (x[1], -x[2]))
        print("\n  Most regular nouns (lowest TT-rank):")
        for lemma, rank, fill in noun_items[:10]:
            print(f"    {lemma:<20s} max_rank={rank}, fill={fill:.3f}")

        noun_items.sort(key=lambda x: (-x[1], -x[2]))
        print("\n  Most irregular nouns (highest TT-rank):")
        for lemma, rank, fill in noun_items[:10]:
            print(f"    {lemma:<20s} max_rank={rank}, fill={fill:.3f}")

    # Verbs (4D)
    if results["verbs_4d"]["paradigms"]:
        verb_items = [(l, r["max_tt_rank"], r["fill_rate"])
                      for l, r in results["verbs_4d"]["paradigms"].items()]

        verb_items.sort(key=lambda x: (x[1], -x[2]))
        print("\n  Most regular verbs (lowest TT-rank):")
        for lemma, rank, fill in verb_items[:10]:
            print(f"    {lemma:<20s} max_rank={rank}, fill={fill:.3f}")

        verb_items.sort(key=lambda x: (-x[1], -x[2]))
        print("\n  Most irregular verbs (highest TT-rank):")
        for lemma, rank, fill in verb_items[:10]:
            print(f"    {lemma:<20s} max_rank={rank}, fill={fill:.3f}")

    results["notable"] = {}
    if results["nouns"]["paradigms"]:
        noun_items_sorted = sorted(
            [(l, r["max_tt_rank"]) for l, r in results["nouns"]["paradigms"].items()],
            key=lambda x: x[1]
        )
        results["notable"]["lowest_rank_nouns"] = noun_items_sorted[:10]
        results["notable"]["highest_rank_nouns"] = noun_items_sorted[-10:][::-1]

    if results["verbs_4d"]["paradigms"]:
        verb_items_sorted = sorted(
            [(l, r["max_tt_rank"]) for l, r in results["verbs_4d"]["paradigms"].items()],
            key=lambda x: x[1]
        )
        results["notable"]["lowest_rank_verbs"] = verb_items_sorted[:10]
        results["notable"]["highest_rank_verbs"] = verb_items_sorted[-10:][::-1]

    # ── KNOWN IRREGULAR VERBS ──
    print("\n" + "=" * 60)
    print("KNOWN IRREGULAR TURKISH VERBS")
    print("=" * 60)

    irregular_verbs = ["ol", "et", "gel", "git", "ye", "de", "gör", "al",
                       "ver", "bil", "yap", "kal", "bul", "ist"]
    print(f"\n  Checking: {irregular_verbs}")
    for v in irregular_verbs:
        if v in results["verbs_4d"]["paradigms"]:
            r = results["verbs_4d"]["paradigms"][v]
            print(f"    {v:<12s} max_rank={r['max_tt_rank']}, "
                  f"fill={r['fill_rate']:.3f}, "
                  f"ranks={r['tt_ranks']}")
        else:
            print(f"    {v:<12s} (not in top paradigms)")

    # Write results
    with open(results_path, "w", encoding="utf-8") as f:
        json.dump(results, f, ensure_ascii=False, indent=2, cls=NumpyEncoder)
    print(f"\n\nResults written to {results_path}")
    print(f"  File size: {results_path.stat().st_size / 1024:.1f} KB")


if __name__ == "__main__":
    run_experiment()
