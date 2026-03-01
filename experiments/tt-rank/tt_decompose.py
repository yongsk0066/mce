#!/usr/bin/env python3
"""
tt_decompose.py — TT decomposition and rank analysis of Finnish paradigm tensors.

Implements the TT-SVD algorithm (Oseledets 2011) from scratch using only numpy,
and applies it to morphological paradigm tables extracted by paradigm_extract.py.

Key question: Do Finnish paradigm tensors have low TT-rank?
If yes, this reveals latent compressed structure in Finnish morphology.

Part of TT-rank experiment for Paper-2 (SIGMORPHON).
"""

import json
import sys
from collections import defaultdict
from pathlib import Path

import numpy as np


class NumpyEncoder(json.JSONEncoder):
    """JSON encoder that handles numpy types."""
    def default(self, obj):
        if isinstance(obj, (np.integer,)):
            return int(obj)
        elif isinstance(obj, (np.floating,)):
            return float(obj)
        elif isinstance(obj, np.ndarray):
            return obj.tolist()
        return super().default(obj)


# ──────────────────────────────────────────────────────────────
# TT-SVD Algorithm (Oseledets 2011, Algorithm 1)
# ──────────────────────────────────────────────────────────────

def tt_svd(tensor, max_rank=None, relative_epsilon=1e-6):
    """
    TT-SVD: Compute Tensor-Train decomposition of an n-dimensional tensor.

    Parameters
    ----------
    tensor : np.ndarray
        Input tensor of shape (n_1, n_2, ..., n_d).
    max_rank : int or None
        Maximum allowed TT-rank. If None, no truncation beyond epsilon.
    relative_epsilon : float
        Relative truncation threshold. Singular values below
        epsilon * ||tensor||_F / sqrt(d-1) are discarded.

    Returns
    -------
    cores : list of np.ndarray
        TT cores. cores[k] has shape (r_k, n_{k+1}, r_{k+1}).
        r_0 = r_d = 1.
    ranks : list of int
        TT-ranks [r_0, r_1, ..., r_d] where r_0 = r_d = 1.

    Algorithm (from Oseledets 2011):
        1. C := tensor
        2. for k = 1 to d-1:
             a. Reshape C to (r_{k-1} * n_k, remainder)
             b. SVD: C = U * S * V^T
             c. Truncate to rank r_k (by epsilon or max_rank)
             d. Core G_k := reshape(U[:, :r_k], (r_{k-1}, n_k, r_k))
             e. C := diag(S[:r_k]) @ V[:r_k, :]
        3. Last core G_d := reshape(C, (r_{d-1}, n_d, 1))
    """
    shape = tensor.shape
    d = len(shape)

    if d < 2:
        # Trivially a 1D "tensor"
        return [tensor.reshape(1, shape[0], 1)], [1, 1]

    frobenius_norm = np.linalg.norm(tensor)
    if frobenius_norm < 1e-15:
        # Zero tensor
        cores = [np.zeros((1, n, 1)) for n in shape]
        ranks = [1] * (d + 1)
        return cores, ranks

    # Truncation threshold per SVD step
    delta = relative_epsilon * frobenius_norm / np.sqrt(d - 1)

    cores = []
    ranks = [1]  # r_0 = 1
    C = tensor.copy().astype(np.float64)

    for k in range(d - 1):
        nk = shape[k]
        rk_prev = ranks[-1]

        # Reshape to matrix: (r_{k-1} * n_k) × (remaining dimensions)
        C = C.reshape(rk_prev * nk, -1)

        # SVD
        U, S, Vt = np.linalg.svd(C, full_matrices=False)

        # Determine rank: truncate small singular values
        # Keep singular values above delta threshold
        rk = np.sum(S > delta)
        rk = max(rk, 1)  # at least rank 1

        if max_rank is not None:
            rk = min(rk, max_rank)

        # Truncate
        U = U[:, :rk]
        S = S[:rk]
        Vt = Vt[:rk, :]

        # Store core
        core = U.reshape(rk_prev, nk, rk)
        cores.append(core)
        ranks.append(rk)

        # Prepare C for next iteration
        C = np.diag(S) @ Vt

    # Last core
    nd = shape[-1]
    rd_prev = ranks[-1]
    last_core = C.reshape(rd_prev, nd, 1)
    cores.append(last_core)
    ranks.append(1)  # r_d = 1

    return cores, ranks


def tt_to_full(cores):
    """
    Reconstruct full tensor from TT cores.

    Used for verification: ||T - reconstruct(TT(T))|| should be small.
    """
    result = cores[0]  # shape (1, n_1, r_1)
    for core in cores[1:]:
        # result shape: (1, n_1*...*n_k, r_k)
        # core shape: (r_k, n_{k+1}, r_{k+1})
        r_prev = result.shape[-1]
        n_prev = result.shape[0] * result.shape[1]  # product of all dims so far
        r_k, n_next, r_next = core.shape
        assert r_prev == r_k, f"Rank mismatch: {r_prev} vs {r_k}"

        # Contract: sum over r_k
        result = result.reshape(-1, r_prev)  # (prod, r_k)
        core_mat = core.reshape(r_k, n_next * r_next)  # (r_k, n_{k+1}*r_{k+1})
        result = result @ core_mat  # (prod, n_{k+1}*r_{k+1})
        result = result.reshape(1, -1, r_next)  # (1, prod*n_{k+1}, r_{k+1})

    return result.reshape([c.shape[1] for c in cores])


def tt_storage(cores):
    """Total number of parameters in TT representation."""
    return sum(c.size for c in cores)


def compression_ratio(original_shape, cores):
    """Compression ratio: original_size / tt_size."""
    orig = int(np.prod(original_shape))
    tt = tt_storage(cores)
    return orig / tt if tt > 0 else float("inf")


# ──────────────────────────────────────────────────────────────
# Paradigm tensorization strategies
# ──────────────────────────────────────────────────────────────

def build_noun_tensor(paradigm_data, shape, char_to_idx, max_len):
    """
    Build a Case(15) × Number(2) × CharPos(max_len) tensor for a noun.

    Each element T[case, number, pos] is the character index at position `pos`
    in the surface form for that (case, number) slot.

    Missing slots are filled with zeros (PAD).
    """
    n_cases, n_numbers = shape[0], shape[1]
    tensor = np.zeros((n_cases, n_numbers, max_len), dtype=np.float64)

    for slot_key, char_vec in paradigm_data["char_encoded"].items():
        parts = slot_key.split("|")
        case, number = parts[0], parts[1]
        from paradigm_extract import CASES, NUMBERS
        ci = CASES.index(case)
        ni = NUMBERS.index(number)
        tensor[ci, ni, :] = char_vec

    return tensor


def build_verb_tensor(paradigm_data, shape, char_to_idx, max_len):
    """
    Build Mood(4) × Tense(2) × Person(4) × Number(2) × CharPos(max_len) tensor.
    """
    tensor = np.zeros((*shape, max_len), dtype=np.float64)

    for slot_key, char_vec in paradigm_data["char_encoded"].items():
        parts = slot_key.split("|")
        mood, tense, person, number = parts
        from paradigm_extract import MOODS, TENSES, PERSONS, NUMBERS
        mi = MOODS.index(mood)
        ti = TENSES.index(tense)
        pi = PERSONS.index(person)
        ni = NUMBERS.index(number)
        tensor[mi, ti, pi, ni, :] = char_vec

    return tensor


def build_adj_tensor(paradigm_data, shape, char_to_idx, max_len):
    """
    Build Case(15) × Number(2) × Degree(3) × CharPos(max_len) tensor.
    """
    tensor = np.zeros((*shape, max_len), dtype=np.float64)

    for slot_key, char_vec in paradigm_data["char_encoded"].items():
        parts = slot_key.split("|")
        case, number, degree = parts
        from paradigm_extract import CASES, NUMBERS, DEGREES
        ci = CASES.index(case)
        ni = NUMBERS.index(number)
        di = DEGREES.index(degree)
        tensor[ci, ni, di, :] = char_vec

    return tensor


# ──────────────────────────────────────────────────────────────
# Alternative: Suffix-difference encoding
# ──────────────────────────────────────────────────────────────

def suffix_diff_encoding(form, lemma):
    """
    Encode a surface form as its difference from the lemma.

    Returns (shared_prefix_len, suffix_chars) where suffix_chars
    is the part of the form that differs from the lemma.

    Example: lemma="koira", form="koiran" → (5, "n")
             lemma="koira", form="koiria" → (4, "ia")

    This encoding captures the morphological operation more directly
    than raw character sequences.
    """
    # Find longest common prefix
    shared = 0
    for i in range(min(len(form), len(lemma))):
        if form[i] == lemma[i]:
            shared += 1
        else:
            break
    suffix = form[shared:]
    stem_change = lemma[shared:]  # what was removed from lemma
    return shared, stem_change, suffix


def build_suffix_tensor_noun(paradigm_data, lemma, max_suffix_len=8):
    """
    Build a Case(15) × Number(2) × SuffixPos(max_suffix_len) tensor
    using suffix-difference encoding.

    Each element encodes the suffix character at that position.
    This is a more linguistically meaningful tensor than raw characters,
    because suffixes are the actual morphological markers.
    """
    from paradigm_extract import CASES, NUMBERS

    # Build char vocab from suffixes
    all_suffixes = []
    for slot_key, form in paradigm_data["forms"].items():
        _, _, suffix = suffix_diff_encoding(form, lemma)
        all_suffixes.append(suffix)

    chars = sorted(set("".join(all_suffixes)))
    char_map = {ch: i + 2 for i, ch in enumerate(chars)}  # 0=pad, 1=unk

    tensor = np.zeros((len(CASES), len(NUMBERS), max_suffix_len), dtype=np.float64)

    for slot_key, form in paradigm_data["forms"].items():
        parts = slot_key.split("|")
        case, number = parts[0], parts[1]
        ci = CASES.index(case)
        ni = NUMBERS.index(number)

        _, _, suffix = suffix_diff_encoding(form, lemma)
        for j, ch in enumerate(suffix[:max_suffix_len]):
            tensor[ci, ni, j] = char_map.get(ch, 1)

    return tensor, char_map


# ──────────────────────────────────────────────────────────────
# Analysis: Singular value spectrum
# ──────────────────────────────────────────────────────────────

def analyze_singular_values(tensor):
    """
    Compute the singular value spectrum at each unfolding of the tensor.

    For a d-dimensional tensor, there are d-1 unfoldings. The singular
    values at each unfolding determine the TT-rank at that bond.

    Returns list of (unfolding_idx, singular_values) tuples.
    """
    shape = tensor.shape
    d = len(shape)
    spectra = []

    for k in range(1, d):
        # k-th unfolding: reshape to (n_1*...*n_k, n_{k+1}*...*n_d)
        left_size = int(np.prod(shape[:k]))
        right_size = int(np.prod(shape[k:]))
        mat = tensor.reshape(left_size, right_size)
        svs = np.linalg.svd(mat, compute_uv=False)
        spectra.append((k, svs))

    return spectra


# ──────────────────────────────────────────────────────────────
# Main experiment
# ──────────────────────────────────────────────────────────────

def run_experiment():
    """Run the full TT-rank experiment."""
    paradigm_path = Path(__file__).parent / "paradigms.json"
    results_path = Path(__file__).parent / "results.json"

    if not paradigm_path.exists():
        print("ERROR: paradigms.json not found. Run paradigm_extract.py first.")
        sys.exit(1)

    print("Loading paradigm data...")
    with open(paradigm_path, encoding="utf-8") as f:
        data = json.load(f)

    char_to_idx = data["metadata"]["char_to_idx"]
    max_len = data["metadata"]["max_form_length"]
    print(f"  Char vocab: {data['metadata']['char_vocab_size']}, max_len: {max_len}")

    results = {
        "experiment": "TT-rank of Finnish morphological paradigms",
        "encoding": "character-level (PAD=0, UNK=1, chars=2+)",
        "algorithm": "TT-SVD (Oseledets 2011)",
        "relative_epsilon": 1e-6,
        "nouns": {"paradigms": {}, "summary": {}},
        "verbs": {"paradigms": {}, "summary": {}},
        "adjectives": {"paradigms": {}, "summary": {}},
        "suffix_nouns": {"paradigms": {}, "summary": {}},
    }

    # ── NOUNS ──
    print("\n" + "=" * 60)
    print("NOUN PARADIGMS: Case(15) × Number(2) × CharPos")
    print("=" * 60)

    noun_ranks_all = []
    noun_max_ranks = []
    noun_compressions = []

    for lemma, pdata in data["nouns"]["paradigms"].items():
        tensor = build_noun_tensor(pdata, data["nouns"]["shape"], char_to_idx, max_len)
        # tensor shape: (15, 2, max_len)

        cores, ranks = tt_svd(tensor, relative_epsilon=1e-6)
        cr = compression_ratio(tensor.shape, cores)

        # Verify reconstruction
        recon = tt_to_full(cores)
        recon_error = np.linalg.norm(tensor - recon) / max(np.linalg.norm(tensor), 1e-15)

        max_rank = max(ranks[1:-1])  # exclude boundary ranks (always 1)
        noun_ranks_all.append(ranks)
        noun_max_ranks.append(max_rank)
        noun_compressions.append(cr)

        # Singular value analysis
        spectra = analyze_singular_values(tensor)
        sv_info = {}
        for k, svs in spectra:
            # How many SVs are > 1% of max SV?
            if len(svs) > 0 and svs[0] > 1e-15:
                significant = int(np.sum(svs > 0.01 * svs[0]))
                sv_info[f"bond_{k}"] = {
                    "rank": ranks[k],
                    "top_5_svs": svs[:5].tolist(),
                    "significant_svs": significant,
                    "sv_ratio_1_2": float(svs[0] / svs[1]) if len(svs) > 1 and svs[1] > 0 else float("inf"),
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

    # Noun summary
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
    print(f"  Max TT-rank: mean={np.mean(noun_max_ranks):.2f} ± {np.std(noun_max_ranks):.2f}")
    print(f"  Max TT-rank range: [{np.min(noun_max_ranks)}, {np.max(noun_max_ranks)}]")
    print(f"  Mean compression ratio: {np.mean(noun_compressions):.2f}x")

    # ── SUFFIX NOUNS ──
    print("\n" + "=" * 60)
    print("NOUN PARADIGMS (SUFFIX ENCODING): Case(15) × Number(2) × SuffixPos")
    print("=" * 60)

    suffix_max_ranks = []
    suffix_compressions = []
    max_suffix_len = 8

    for lemma, pdata in data["nouns"]["paradigms"].items():
        tensor, _ = build_suffix_tensor_noun(pdata, lemma, max_suffix_len)
        # tensor shape: (15, 2, max_suffix_len)

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
    print(f"  Max TT-rank: mean={np.mean(suffix_max_ranks):.2f} ± {np.std(suffix_max_ranks):.2f}")
    print(f"  Max TT-rank range: [{np.min(suffix_max_ranks)}, {np.max(suffix_max_ranks)}]")
    print(f"  Mean compression ratio: {np.mean(suffix_compressions):.2f}x")

    # ── VERBS ──
    print("\n" + "=" * 60)
    print("VERB PARADIGMS: Mood(4) × Tense(2) × Person(4) × Number(2) × CharPos")
    print("=" * 60)

    verb_ranks_all = []
    verb_max_ranks = []
    verb_compressions = []

    for lemma, pdata in data["verbs"]["paradigms"].items():
        tensor = build_verb_tensor(pdata, data["verbs"]["shape"], char_to_idx, max_len)
        # tensor shape: (4, 2, 4, 2, max_len)

        cores, ranks = tt_svd(tensor, relative_epsilon=1e-6)
        cr = compression_ratio(tensor.shape, cores)
        recon = tt_to_full(cores)
        recon_error = np.linalg.norm(tensor - recon) / max(np.linalg.norm(tensor), 1e-15)

        max_rank = max(ranks[1:-1]) if len(ranks) > 2 else 1
        verb_ranks_all.append(ranks)
        verb_max_ranks.append(max_rank)
        verb_compressions.append(cr)

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

        results["verbs"]["paradigms"][lemma] = {
            "coverage": pdata["coverage"],
            "fill_rate": pdata["fill_rate"],
            "tensor_shape": list(tensor.shape),
            "tt_ranks": ranks,
            "max_tt_rank": max_rank,
            "compression_ratio": round(cr, 2),
            "recon_error": float(recon_error),
            "sv_analysis": sv_info,
        }

    results["verbs"]["summary"] = {
        "n_paradigms": len(verb_max_ranks),
        "mean_max_rank": round(float(np.mean(verb_max_ranks)), 2),
        "median_max_rank": round(float(np.median(verb_max_ranks)), 2),
        "std_max_rank": round(float(np.std(verb_max_ranks)), 2),
        "min_max_rank": int(np.min(verb_max_ranks)),
        "max_max_rank": int(np.max(verb_max_ranks)),
        "rank_histogram": {str(r): int(c) for r, c in
                          zip(*np.unique(verb_max_ranks, return_counts=True))},
        "mean_compression": round(float(np.mean(verb_compressions)), 2),
    }

    print(f"\n  Paradigms analyzed: {len(verb_max_ranks)}")
    print(f"  Max TT-rank: mean={np.mean(verb_max_ranks):.2f} ± {np.std(verb_max_ranks):.2f}")
    print(f"  Max TT-rank range: [{np.min(verb_max_ranks)}, {np.max(verb_max_ranks)}]")
    print(f"  Mean compression ratio: {np.mean(verb_compressions):.2f}x")

    # ── ADJECTIVES ──
    print("\n" + "=" * 60)
    print("ADJ PARADIGMS: Case(15) × Number(2) × Degree(3) × CharPos")
    print("=" * 60)

    adj_ranks_all = []
    adj_max_ranks = []
    adj_compressions = []

    for lemma, pdata in data["adjectives"]["paradigms"].items():
        tensor = build_adj_tensor(pdata, data["adjectives"]["shape"], char_to_idx, max_len)
        # tensor shape: (15, 2, 3, max_len)

        cores, ranks = tt_svd(tensor, relative_epsilon=1e-6)
        cr = compression_ratio(tensor.shape, cores)
        recon = tt_to_full(cores)
        recon_error = np.linalg.norm(tensor - recon) / max(np.linalg.norm(tensor), 1e-15)

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
                    "top_5_svs": svs[:min(5, len(svs))].tolist(),
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

    results["adjectives"]["summary"] = {
        "n_paradigms": len(adj_max_ranks),
        "mean_max_rank": round(float(np.mean(adj_max_ranks)), 2),
        "median_max_rank": round(float(np.median(adj_max_ranks)), 2),
        "std_max_rank": round(float(np.std(adj_max_ranks)), 2),
        "min_max_rank": int(np.min(adj_max_ranks)),
        "max_max_rank": int(np.max(adj_max_ranks)),
        "rank_histogram": {str(r): int(c) for r, c in
                          zip(*np.unique(adj_max_ranks, return_counts=True))},
        "mean_compression": round(float(np.mean(adj_compressions)), 2),
    }

    print(f"\n  Paradigms analyzed: {len(adj_max_ranks)}")
    print(f"  Max TT-rank: mean={np.mean(adj_max_ranks):.2f} ± {np.std(adj_max_ranks):.2f}")
    print(f"  Max TT-rank range: [{np.min(adj_max_ranks)}, {np.max(adj_max_ranks)}]")
    print(f"  Mean compression ratio: {np.mean(adj_compressions):.2f}x")

    # ── CROSS-POS COMPARISON ──
    print("\n" + "=" * 60)
    print("CROSS-POS COMPARISON")
    print("=" * 60)

    comparison = {
        "pos": ["NOUN (raw)", "NOUN (suffix)", "VERB", "ADJ"],
        "n_paradigms": [len(noun_max_ranks), len(suffix_max_ranks),
                       len(verb_max_ranks), len(adj_max_ranks)],
        "mean_max_rank": [
            round(float(np.mean(noun_max_ranks)), 2),
            round(float(np.mean(suffix_max_ranks)), 2),
            round(float(np.mean(verb_max_ranks)), 2),
            round(float(np.mean(adj_max_ranks)), 2),
        ],
        "median_max_rank": [
            round(float(np.median(noun_max_ranks)), 2),
            round(float(np.median(suffix_max_ranks)), 2),
            round(float(np.median(verb_max_ranks)), 2),
            round(float(np.median(adj_max_ranks)), 2),
        ],
        "mean_compression": [
            round(float(np.mean(noun_compressions)), 2),
            round(float(np.mean(suffix_compressions)), 2),
            round(float(np.mean(verb_compressions)), 2),
            round(float(np.mean(adj_compressions)), 2),
        ],
    }

    print(f"  {'POS':<20s} {'N':>5s} {'Mean MaxR':>10s} {'Med MaxR':>10s} {'Compr':>8s}")
    print("  " + "-" * 55)
    for i, pos in enumerate(comparison["pos"]):
        print(f"  {pos:<20s} {comparison['n_paradigms'][i]:>5d} "
              f"{comparison['mean_max_rank'][i]:>10.2f} "
              f"{comparison['median_max_rank'][i]:>10.2f} "
              f"{comparison['mean_compression'][i]:>8.2f}x")

    results["cross_pos_comparison"] = comparison

    # ── NOTABLE PARADIGMS ──
    print("\n" + "=" * 60)
    print("NOTABLE PARADIGMS")
    print("=" * 60)

    # Lowest-rank nouns (most regular)
    noun_items = [(l, r["max_tt_rank"], r["fill_rate"])
                  for l, r in results["nouns"]["paradigms"].items()]
    noun_items.sort(key=lambda x: (x[1], -x[2]))
    print("\n  Most regular nouns (lowest TT-rank):")
    for lemma, rank, fill in noun_items[:10]:
        print(f"    {lemma:<20s} max_rank={rank}, fill={fill:.3f}")

    # Highest-rank nouns (most irregular)
    noun_items.sort(key=lambda x: (-x[1], -x[2]))
    print("\n  Most irregular nouns (highest TT-rank):")
    for lemma, rank, fill in noun_items[:10]:
        print(f"    {lemma:<20s} max_rank={rank}, fill={fill:.3f}")

    # Lowest-rank verbs
    verb_items = [(l, r["max_tt_rank"], r["fill_rate"])
                  for l, r in results["verbs"]["paradigms"].items()]
    verb_items.sort(key=lambda x: (x[1], -x[2]))
    print("\n  Most regular verbs (lowest TT-rank):")
    for lemma, rank, fill in verb_items[:10]:
        print(f"    {lemma:<20s} max_rank={rank}, fill={fill:.3f}")

    # Highest-rank verbs
    verb_items.sort(key=lambda x: (-x[1], -x[2]))
    print("\n  Most irregular verbs (highest TT-rank):")
    for lemma, rank, fill in verb_items[:10]:
        print(f"    {lemma:<20s} max_rank={rank}, fill={fill:.3f}")

    results["notable"] = {
        "lowest_rank_nouns": [(l, r) for l, r, _ in
                              sorted(noun_items, key=lambda x: (x[1], -x[2]))[:10]],
        "highest_rank_nouns": [(l, r) for l, r, _ in
                               sorted(noun_items, key=lambda x: (-x[1], -x[2]))[:10]],
        "lowest_rank_verbs": [(l, r) for l, r, _ in
                              sorted(verb_items, key=lambda x: (x[1], -x[2]))[:10]],
        "highest_rank_verbs": [(l, r) for l, r, _ in
                               sorted(verb_items, key=lambda x: (-x[1], -x[2]))[:10]],
    }

    # ── FILL RATE vs TT-RANK CORRELATION ──
    print("\n" + "=" * 60)
    print("FILL RATE vs TT-RANK CORRELATION")
    print("=" * 60)

    for pos_name, pos_data in [("nouns", results["nouns"]),
                                ("verbs", results["verbs"]),
                                ("adjectives", results["adjectives"])]:
        fills = []
        ranks = []
        for lemma, pdata in pos_data["paradigms"].items():
            fills.append(pdata["fill_rate"])
            ranks.append(pdata["max_tt_rank"])
        if len(fills) > 2:
            corr = np.corrcoef(fills, ranks)[0, 1]
            print(f"  {pos_name}: Pearson r = {corr:.4f} (n={len(fills)})")
            results[pos_name]["summary"]["fill_rank_correlation"] = round(float(corr), 4)

    # Write results
    with open(results_path, "w", encoding="utf-8") as f:
        json.dump(results, f, ensure_ascii=False, indent=2, cls=NumpyEncoder)
    print(f"\n\nResults written to {results_path}")
    print(f"  File size: {results_path.stat().st_size / 1024:.1f} KB")


if __name__ == "__main__":
    run_experiment()
