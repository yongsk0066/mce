#!/usr/bin/env python3
"""
statistical_tests.py — Statistical significance tests for TT-rank experiment.

Paper-2 (SIGMORPHON): Bootstrap CI, Mann-Whitney U, Spearman correlation,
permutation tests, Kruskal-Wallis for Finnish morphological paradigm TT-ranks.

Reads results.json and paradigms.json produced by tt_decompose.py.
"""

import json
import sys
from pathlib import Path

import numpy as np
from scipy import stats


class NumpyEncoder(json.JSONEncoder):
    """JSON encoder that handles numpy types."""

    def default(self, obj):
        if isinstance(obj, (np.bool_,)):
            return bool(obj)
        elif isinstance(obj, (np.integer,)):
            return int(obj)
        elif isinstance(obj, (np.floating,)):
            return float(obj)
        elif isinstance(obj, np.ndarray):
            return obj.tolist()
        return super().default(obj)


# ──────────────────────────────────────────────────────────────
# Data loading
# ──────────────────────────────────────────────────────────────


def load_data():
    """Load results.json and extract per-paradigm TT-rank and fill-rate arrays."""
    results_path = Path(__file__).parent / "results.json"
    if not results_path.exists():
        print("ERROR: results.json not found. Run tt_decompose.py first.")
        sys.exit(1)

    with open(results_path, encoding="utf-8") as f:
        results = json.load(f)

    data = {}
    for pos_key in ("nouns", "verbs", "adjectives"):
        paradigms = results[pos_key]["paradigms"]
        max_ranks = []
        fill_rates = []
        all_ranks = []  # full TT-rank vectors (for bond analysis)
        for lemma, pdata in paradigms.items():
            max_ranks.append(pdata["max_tt_rank"])
            fill_rates.append(pdata["fill_rate"])
            all_ranks.append(pdata["tt_ranks"])
        data[pos_key] = {
            "max_ranks": np.array(max_ranks),
            "fill_rates": np.array(fill_rates),
            "all_ranks": all_ranks,
            "n": len(max_ranks),
        }

    return data, results


def load_paradigms():
    """Load raw paradigm data for shuffled baseline computation."""
    paradigm_path = Path(__file__).parent / "paradigms.json"
    if not paradigm_path.exists():
        print("ERROR: paradigms.json not found. Run paradigm_extract.py first.")
        sys.exit(1)

    with open(paradigm_path, encoding="utf-8") as f:
        return json.load(f)


# ──────────────────────────────────────────────────────────────
# TT-SVD (minimal re-implementation for shuffled baseline)
# ──────────────────────────────────────────────────────────────


def tt_svd(tensor, relative_epsilon=1e-6):
    """Minimal TT-SVD for generating shuffled/random baselines."""
    shape = tensor.shape
    d = len(shape)
    if d < 2:
        return [1, 1]

    frobenius_norm = np.linalg.norm(tensor)
    if frobenius_norm < 1e-15:
        return [1] * (d + 1)

    delta = relative_epsilon * frobenius_norm / np.sqrt(d - 1)
    ranks = [1]
    C = tensor.copy().astype(np.float64)

    for k in range(d - 1):
        nk = shape[k]
        rk_prev = ranks[-1]
        C = C.reshape(rk_prev * nk, -1)
        _, S, Vt = np.linalg.svd(C, full_matrices=False)
        rk = max(int(np.sum(S > delta)), 1)
        ranks.append(rk)
        C = np.diag(S[:rk]) @ Vt[:rk, :]

    ranks.append(1)
    return ranks


def compute_max_rank(tensor):
    """Compute max TT-rank (excluding boundary ranks) for a tensor."""
    ranks = tt_svd(tensor)
    return max(ranks[1:-1]) if len(ranks) > 2 else 1


# ──────────────────────────────────────────────────────────────
# 1. Bootstrap confidence intervals
# ──────────────────────────────────────────────────────────────


def bootstrap_ci(data_array, n_bootstrap=1000, ci=0.95, rng=None):
    """
    Compute bootstrap confidence interval for the mean.

    Returns (mean, ci_lower, ci_upper).
    """
    if rng is None:
        rng = np.random.default_rng(42)

    n = len(data_array)
    boot_means = np.empty(n_bootstrap)
    for i in range(n_bootstrap):
        sample = rng.choice(data_array, size=n, replace=True)
        boot_means[i] = np.mean(sample)

    alpha = (1 - ci) / 2
    ci_lower = np.percentile(boot_means, 100 * alpha)
    ci_upper = np.percentile(boot_means, 100 * (1 - alpha))
    return float(np.mean(data_array)), float(ci_lower), float(ci_upper)


def run_bootstrap_tests(data):
    """Bootstrap CI for mean max-rank of each POS."""
    print("=" * 70)
    print("1. BOOTSTRAP CONFIDENCE INTERVALS (1000 resamples, 95% CI)")
    print("=" * 70)

    rng = np.random.default_rng(42)
    results = {}

    for pos in ("nouns", "verbs", "adjectives"):
        ranks = data[pos]["max_ranks"]
        mean, ci_lo, ci_hi = bootstrap_ci(ranks, n_bootstrap=1000, rng=rng)
        results[pos] = {
            "mean": round(mean, 4),
            "ci_lower": round(ci_lo, 4),
            "ci_upper": round(ci_hi, 4),
            "n": int(len(ranks)),
        }
        print(f"  {pos.upper():12s}: mean = {mean:.2f}  "
              f"[95% CI: {ci_lo:.2f}, {ci_hi:.2f}]  (n={len(ranks)})")

    print()
    return results


# ──────────────────────────────────────────────────────────────
# 2. Mann-Whitney U test
# ──────────────────────────────────────────────────────────────


def rank_biserial_r(U, n1, n2):
    """Compute rank-biserial correlation as effect size for Mann-Whitney U."""
    return 1.0 - (2.0 * U) / (n1 * n2)


def run_mannwhitney_tests(data):
    """Mann-Whitney U tests between POS pairs."""
    print("=" * 70)
    print("2. MANN-WHITNEY U TESTS (two-sided)")
    print("=" * 70)

    pairs = [
        ("nouns", "verbs"),
        ("nouns", "adjectives"),
        ("verbs", "adjectives"),
    ]
    results = {}

    for pos_a, pos_b in pairs:
        ranks_a = data[pos_a]["max_ranks"]
        ranks_b = data[pos_b]["max_ranks"]
        U_stat, p_value = stats.mannwhitneyu(ranks_a, ranks_b, alternative="two-sided")

        n1, n2 = len(ranks_a), len(ranks_b)
        r_effect = rank_biserial_r(U_stat, n1, n2)

        sig = "***" if p_value < 0.001 else "**" if p_value < 0.01 else "*" if p_value < 0.05 else "n.s."

        key = f"{pos_a}_vs_{pos_b}"
        results[key] = {
            "U_statistic": float(U_stat),
            "p_value": float(p_value),
            "rank_biserial_r": round(float(r_effect), 4),
            "significant": p_value < 0.05,
            "n1": n1,
            "n2": n2,
            "mean_1": round(float(np.mean(ranks_a)), 2),
            "mean_2": round(float(np.mean(ranks_b)), 2),
        }

        print(f"  {pos_a.upper()} vs {pos_b.upper()}:")
        print(f"    U = {U_stat:.1f}, p = {p_value:.6f}  {sig}")
        print(f"    rank-biserial r = {r_effect:.4f}")
        print(f"    means: {np.mean(ranks_a):.2f} vs {np.mean(ranks_b):.2f}")
        print()

    return results


# ──────────────────────────────────────────────────────────────
# 3. Spearman correlation (TT-rank vs fill-rate)
# ──────────────────────────────────────────────────────────────


def run_spearman_tests(data):
    """Spearman rank correlation between TT-rank and fill rate."""
    print("=" * 70)
    print("3. SPEARMAN CORRELATION: TT-rank vs Fill Rate")
    print("=" * 70)

    results = {}

    for pos in ("nouns", "verbs", "adjectives"):
        ranks = data[pos]["max_ranks"]
        fills = data[pos]["fill_rates"]
        rho, p_value = stats.spearmanr(ranks, fills)

        sig = "***" if p_value < 0.001 else "**" if p_value < 0.01 else "*" if p_value < 0.05 else "n.s."

        results[pos] = {
            "spearman_rho": round(float(rho), 4),
            "p_value": float(p_value),
            "significant": p_value < 0.05,
            "n": int(len(ranks)),
        }

        print(f"  {pos.upper():12s}: rho = {rho:.4f}, p = {p_value:.2e}  {sig}  (n={len(ranks)})")

    # Also compute pooled across all POS
    all_ranks = np.concatenate([data[p]["max_ranks"] for p in ("nouns", "verbs", "adjectives")])
    all_fills = np.concatenate([data[p]["fill_rates"] for p in ("nouns", "verbs", "adjectives")])
    rho_all, p_all = stats.spearmanr(all_ranks, all_fills)
    results["pooled"] = {
        "spearman_rho": round(float(rho_all), 4),
        "p_value": float(p_all),
        "significant": p_all < 0.05,
        "n": int(len(all_ranks)),
    }
    print(f"  {'POOLED':12s}: rho = {rho_all:.4f}, p = {p_all:.2e}  (n={len(all_ranks)})")

    print()
    return results


# ──────────────────────────────────────────────────────────────
# 4. Permutation test: Real vs Shuffled baseline
# ──────────────────────────────────────────────────────────────


def build_noun_tensor(pdata, max_len):
    """Build noun tensor from paradigm char_encoded data."""
    from paradigm_extract import CASES, NUMBERS
    tensor = np.zeros((len(CASES), len(NUMBERS), max_len), dtype=np.float64)
    for slot_key, char_vec in pdata["char_encoded"].items():
        parts = slot_key.split("|")
        ci = CASES.index(parts[0])
        ni = NUMBERS.index(parts[1])
        tensor[ci, ni, :] = char_vec
    return tensor


def build_verb_tensor(pdata, max_len):
    """Build verb tensor from paradigm char_encoded data."""
    from paradigm_extract import MOODS, TENSES, PERSONS, NUMBERS
    tensor = np.zeros((len(MOODS), len(TENSES), len(PERSONS), len(NUMBERS), max_len),
                      dtype=np.float64)
    for slot_key, char_vec in pdata["char_encoded"].items():
        parts = slot_key.split("|")
        mi = MOODS.index(parts[0])
        ti = TENSES.index(parts[1])
        pi = PERSONS.index(parts[2])
        ni = NUMBERS.index(parts[3])
        tensor[mi, ti, pi, ni, :] = char_vec
    return tensor


def shuffle_paradigm_tensor(tensor, rng):
    """
    Shuffle the morphological slots of a paradigm tensor.

    Keeps the same set of surface forms but randomly reassigns them
    to different morphological slots. For a tensor of shape
    (feat1, feat2, ..., featN, charpos), shuffles across the
    feature dimensions while preserving the character vectors.
    """
    shape = tensor.shape
    feat_shape = shape[:-1]  # morphological dimensions
    char_dim = shape[-1]

    # Flatten feature dimensions
    n_slots = int(np.prod(feat_shape))
    flat = tensor.reshape(n_slots, char_dim)

    # Find non-zero rows (filled slots)
    filled_mask = np.any(flat != 0, axis=1)
    filled_vecs = flat[filled_mask].copy()

    # Shuffle the filled vectors
    rng.shuffle(filled_vecs)

    # Reassign to random filled positions
    shuffled_flat = np.zeros_like(flat)
    shuffled_flat[filled_mask] = filled_vecs

    return shuffled_flat.reshape(shape)


def random_paradigm_tensor(tensor, rng, vocab_size=24):
    """
    Random baseline: same sparsity pattern but random character indices.
    """
    shape = tensor.shape
    feat_shape = shape[:-1]
    char_dim = shape[-1]

    n_slots = int(np.prod(feat_shape))
    flat = tensor.reshape(n_slots, char_dim)
    filled_mask = np.any(flat != 0, axis=1)

    random_flat = np.zeros_like(flat)
    n_filled = int(filled_mask.sum())
    if n_filled > 0:
        # Generate random character indices (2 to vocab_size-1, avoiding 0=PAD)
        for i in range(n_slots):
            if filled_mask[i]:
                # Find length of original form (first zero position)
                orig = flat[i]
                form_len = int(np.sum(orig > 0))
                random_flat[i, :form_len] = rng.integers(2, vocab_size, size=form_len)

    return random_flat.reshape(shape)


def run_permutation_tests(paradigm_data, data, n_permutations=1000):
    """
    Permutation test: Is the difference between real and shuffled TT-rank
    statistically significant?
    """
    print("=" * 70)
    print("4. PERMUTATION TEST: Real vs Shuffled Baseline")
    print(f"   ({n_permutations} permutations)")
    print("=" * 70)

    rng = np.random.default_rng(42)
    max_len = paradigm_data["metadata"]["max_form_length"]
    results = {}

    for pos, builder_fn in [("nouns", build_noun_tensor), ("verbs", build_verb_tensor)]:
        paradigms = paradigm_data[pos]["paradigms"]
        real_ranks = data[pos]["max_ranks"]
        real_mean = float(np.mean(real_ranks))

        # Compute shuffled baselines
        print(f"\n  {pos.upper()}: Computing shuffled baselines...")
        shuffled_means = np.empty(n_permutations)
        random_means = np.empty(n_permutations)

        for perm_i in range(n_permutations):
            shuffled_ranks_this = []
            random_ranks_this = []
            for lemma, pdata in paradigms.items():
                tensor = builder_fn(pdata, max_len)

                # Shuffled: same forms, random slot assignment
                shuffled_tensor = shuffle_paradigm_tensor(tensor, rng)
                shuffled_ranks_this.append(compute_max_rank(shuffled_tensor))

                # Random: same sparsity, random characters
                random_tensor = random_paradigm_tensor(tensor, rng)
                random_ranks_this.append(compute_max_rank(random_tensor))

            shuffled_means[perm_i] = np.mean(shuffled_ranks_this)
            random_means[perm_i] = np.mean(random_ranks_this)

            if (perm_i + 1) % 100 == 0:
                print(f"    permutation {perm_i + 1}/{n_permutations}")

        # --- Shuffled vs Real ---
        # p-value: fraction of shuffled means <= real mean
        # (one-sided: is real rank significantly lower than shuffled?)
        p_shuffled = float(np.mean(shuffled_means <= real_mean))
        shuffled_mean_avg = float(np.mean(shuffled_means))
        shuffled_diff = shuffled_mean_avg - real_mean

        # Cohen's d effect size
        pooled_std = float(np.std(shuffled_means))
        cohens_d_shuffled = shuffled_diff / pooled_std if pooled_std > 0 else 0.0

        sig_s = "***" if p_shuffled < 0.001 else "**" if p_shuffled < 0.01 else "*" if p_shuffled < 0.05 else "n.s."

        print(f"\n  {pos.upper()} Real vs Shuffled:")
        print(f"    Real mean:     {real_mean:.4f}")
        print(f"    Shuffled mean: {shuffled_mean_avg:.4f} (avg over {n_permutations} permutations)")
        print(f"    Difference:    {shuffled_diff:.4f}")
        print(f"    p-value:       {p_shuffled:.6f}  {sig_s}")
        print(f"    Cohen's d:     {cohens_d_shuffled:.4f}")

        # --- Random vs Real ---
        p_random = float(np.mean(random_means <= real_mean))
        random_mean_avg = float(np.mean(random_means))
        random_diff = random_mean_avg - real_mean
        pooled_std_r = float(np.std(random_means))
        cohens_d_random = random_diff / pooled_std_r if pooled_std_r > 0 else 0.0

        sig_r = "***" if p_random < 0.001 else "**" if p_random < 0.01 else "*" if p_random < 0.05 else "n.s."

        print(f"\n  {pos.upper()} Real vs Random:")
        print(f"    Real mean:     {real_mean:.4f}")
        print(f"    Random mean:   {random_mean_avg:.4f} (avg over {n_permutations} permutations)")
        print(f"    Difference:    {random_diff:.4f}")
        print(f"    p-value:       {p_random:.6f}  {sig_r}")
        print(f"    Cohen's d:     {cohens_d_random:.4f}")

        results[pos] = {
            "real_mean": round(real_mean, 4),
            "shuffled": {
                "mean": round(shuffled_mean_avg, 4),
                "std": round(float(np.std(shuffled_means)), 4),
                "ci_lower": round(float(np.percentile(shuffled_means, 2.5)), 4),
                "ci_upper": round(float(np.percentile(shuffled_means, 97.5)), 4),
                "difference": round(shuffled_diff, 4),
                "p_value": round(p_shuffled, 6),
                "cohens_d": round(cohens_d_shuffled, 4),
                "significant": p_shuffled < 0.05,
            },
            "random": {
                "mean": round(random_mean_avg, 4),
                "std": round(float(np.std(random_means)), 4),
                "ci_lower": round(float(np.percentile(random_means, 2.5)), 4),
                "ci_upper": round(float(np.percentile(random_means, 97.5)), 4),
                "difference": round(random_diff, 4),
                "p_value": round(p_random, 6),
                "cohens_d": round(cohens_d_random, 4),
                "significant": p_random < 0.05,
            },
            "n_permutations": n_permutations,
        }

    print()
    return results


# ──────────────────────────────────────────────────────────────
# 5. Kruskal-Wallis test
# ──────────────────────────────────────────────────────────────


def epsilon_squared(H, n):
    """Compute epsilon-squared effect size for Kruskal-Wallis."""
    return float(H) / (n - 1) if n > 1 else 0.0


def run_kruskal_wallis(data):
    """Kruskal-Wallis test comparing TT-ranks across POS categories."""
    print("=" * 70)
    print("5. KRUSKAL-WALLIS TEST: TT-rank across POS categories")
    print("=" * 70)

    noun_ranks = data["nouns"]["max_ranks"]
    verb_ranks = data["verbs"]["max_ranks"]
    adj_ranks = data["adjectives"]["max_ranks"]

    H_stat, p_value = stats.kruskal(noun_ranks, verb_ranks, adj_ranks)
    n_total = len(noun_ranks) + len(verb_ranks) + len(adj_ranks)
    eps_sq = epsilon_squared(H_stat, n_total)

    sig = "***" if p_value < 0.001 else "**" if p_value < 0.01 else "*" if p_value < 0.05 else "n.s."

    result = {
        "H_statistic": round(float(H_stat), 4),
        "p_value": float(p_value),
        "epsilon_squared": round(eps_sq, 4),
        "significant": p_value < 0.05,
        "n_total": n_total,
        "groups": {
            "nouns": {"n": int(len(noun_ranks)), "mean": round(float(np.mean(noun_ranks)), 2),
                      "median": round(float(np.median(noun_ranks)), 2)},
            "verbs": {"n": int(len(verb_ranks)), "mean": round(float(np.mean(verb_ranks)), 2),
                      "median": round(float(np.median(verb_ranks)), 2)},
            "adjectives": {"n": int(len(adj_ranks)), "mean": round(float(np.mean(adj_ranks)), 2),
                           "median": round(float(np.median(adj_ranks)), 2)},
        },
    }

    print(f"  H = {H_stat:.4f}, p = {p_value:.6f}  {sig}")
    print(f"  epsilon-squared = {eps_sq:.4f}")
    print(f"  Groups: NOUN (n={len(noun_ranks)}, med={np.median(noun_ranks):.1f}), "
          f"VERB (n={len(verb_ranks)}, med={np.median(verb_ranks):.1f}), "
          f"ADJ (n={len(adj_ranks)}, med={np.median(adj_ranks):.1f})")

    # Post-hoc: Dunn's test (pairwise Mann-Whitney with Bonferroni correction)
    print("\n  Post-hoc pairwise comparisons (Bonferroni-corrected):")
    pairs = [("nouns", "verbs"), ("nouns", "adjectives"), ("verbs", "adjectives")]
    n_comparisons = len(pairs)
    posthoc_results = {}

    for pos_a, pos_b in pairs:
        U, p = stats.mannwhitneyu(data[pos_a]["max_ranks"], data[pos_b]["max_ranks"],
                                  alternative="two-sided")
        p_corrected = min(p * n_comparisons, 1.0)  # Bonferroni
        sig_ph = "***" if p_corrected < 0.001 else "**" if p_corrected < 0.01 else "*" if p_corrected < 0.05 else "n.s."

        posthoc_results[f"{pos_a}_vs_{pos_b}"] = {
            "U": float(U),
            "p_raw": float(p),
            "p_bonferroni": float(p_corrected),
            "significant": p_corrected < 0.05,
        }
        print(f"    {pos_a.upper()} vs {pos_b.upper()}: "
              f"U={U:.1f}, p_raw={p:.6f}, p_bonferroni={p_corrected:.6f}  {sig_ph}")

    result["posthoc_bonferroni"] = posthoc_results
    print()
    return result


# ──────────────────────────────────────────────────────────────
# Summary
# ──────────────────────────────────────────────────────────────


def print_summary(all_results):
    """Print a concise summary suitable for paper inclusion."""
    print("=" * 70)
    print("SUMMARY FOR PAPER-2")
    print("=" * 70)

    print("\n--- Bootstrap 95% CI for Mean Max TT-Rank ---")
    for pos in ("nouns", "verbs", "adjectives"):
        r = all_results["bootstrap_ci"][pos]
        print(f"  {pos.upper():12s}: {r['mean']:.2f} [{r['ci_lower']:.2f}, {r['ci_upper']:.2f}]")

    print("\n--- Kruskal-Wallis (POS comparison) ---")
    kw = all_results["kruskal_wallis"]
    print(f"  H({kw['n_total'] - 1}) = {kw['H_statistic']:.2f}, "
          f"p = {kw['p_value']:.2e}, "
          f"epsilon^2 = {kw['epsilon_squared']:.4f}")

    print("\n--- Mann-Whitney U (pairwise) ---")
    for key, r in all_results["mann_whitney"].items():
        sig = "sig." if r["significant"] else "n.s."
        print(f"  {key}: U={r['U_statistic']:.0f}, p={r['p_value']:.2e}, "
              f"r={r['rank_biserial_r']:.3f} ({sig})")

    print("\n--- Spearman rho (TT-rank vs fill rate) ---")
    for pos in ("nouns", "verbs", "adjectives", "pooled"):
        r = all_results["spearman_correlation"][pos]
        sig = "sig." if r["significant"] else "n.s."
        print(f"  {pos.upper():12s}: rho={r['spearman_rho']:.4f}, p={r['p_value']:.2e} ({sig})")

    print("\n--- Permutation test (Real vs baselines) ---")
    for pos in all_results["permutation_test"]:
        pt = all_results["permutation_test"][pos]
        sh = pt["shuffled"]
        rn = pt["random"]
        print(f"  {pos.upper()}:")
        print(f"    vs Shuffled: diff={sh['difference']:.4f}, p={sh['p_value']:.4f}, "
              f"Cohen's d={sh['cohens_d']:.4f}")
        print(f"    vs Random:   diff={rn['difference']:.4f}, p={rn['p_value']:.4f}, "
              f"Cohen's d={rn['cohens_d']:.4f}")

    print()


# ──────────────────────────────────────────────────────────────
# Main
# ──────────────────────────────────────────────────────────────


def main():
    print("Statistical Tests for TT-Rank Experiment (Paper-2)")
    print("=" * 70)

    # Load data
    data, results_json = load_data()
    paradigm_data = load_paradigms()

    print(f"\nLoaded paradigms: "
          f"NOUN={data['nouns']['n']}, "
          f"VERB={data['verbs']['n']}, "
          f"ADJ={data['adjectives']['n']}")
    print()

    all_results = {}

    # 1. Bootstrap CI
    all_results["bootstrap_ci"] = run_bootstrap_tests(data)

    # 2. Mann-Whitney U
    all_results["mann_whitney"] = run_mannwhitney_tests(data)

    # 3. Spearman correlation
    all_results["spearman_correlation"] = run_spearman_tests(data)

    # 4. Permutation test (computationally intensive)
    all_results["permutation_test"] = run_permutation_tests(
        paradigm_data, data, n_permutations=1000
    )

    # 5. Kruskal-Wallis
    all_results["kruskal_wallis"] = run_kruskal_wallis(data)

    # Summary
    print_summary(all_results)

    # Save results
    output_path = Path(__file__).parent / "statistical_results.json"
    with open(output_path, "w", encoding="utf-8") as f:
        json.dump(all_results, f, ensure_ascii=False, indent=2, cls=NumpyEncoder)
    print(f"Results saved to {output_path}")
    print(f"  File size: {output_path.stat().st_size / 1024:.1f} KB")


if __name__ == "__main__":
    main()
