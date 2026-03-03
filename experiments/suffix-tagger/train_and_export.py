#!/usr/bin/env python3
"""
Train a suffix-based logistic regression POS tagger and export to MCET binary format.

This script replicates the exact feature extraction from
mce/crates/mce-disambig/src/suffix_tagger.rs, trains a sparse logistic
regression classifier using sklearn, and exports the model in the MCET v1
binary format that the Rust code can load.

Usage:
    python train_and_export.py [--config CONFIG_NAME] [--evaluate-only]
"""

import argparse
import struct
import sys
import time
from pathlib import Path
from typing import Optional

import numpy as np
from sklearn.linear_model import LogisticRegression
from sklearn.feature_extraction import DictVectorizer

# ──────────────────────────────────────────────────────────────────────
# Paths
# ──────────────────────────────────────────────────────────────────────

UD_DIR = Path.home() / "oss/finnishNLP/ud-finnish-tdt"
TRAIN_FILE = UD_DIR / "fi_tdt-ud-train.conllu"
DEV_FILE = UD_DIR / "fi_tdt-ud-dev.conllu"
TEST_FILE = UD_DIR / "fi_tdt-ud-test.conllu"
MODEL_DIR = Path.home() / "oss/finnishNLP/mce/data"
MODEL_FILE = MODEL_DIR / "suffix_tagger.bin"

# ──────────────────────────────────────────────────────────────────────
# Punctuation chars (must match Rust)
# ──────────────────────────────────────────────────────────────────────

PUNCT_CHARS = '.,;:!?"\'()[]{}'
PUNCT_CHARS_EXTENDED = PUNCT_CHARS + "\u2013\u2014"  # en-dash, em-dash

# ──────────────────────────────────────────────────────────────────────
# Feature extraction (mirrors suffix_tagger.rs exactly)
# ──────────────────────────────────────────────────────────────────────

# Finnish verb endings (must match Rust)
FI_VERB_ENDINGS = ["an", "en", "isi", "aa", "\u00e4\u00e4", "ee", "uu", "yy", "oo", "\u00f6\u00f6"]

# Finnish negation forms
FI_NEG_FORMS = {"en", "et", "ei", "emme", "ette", "eiv\u00e4t"}


def compressed_shape(word: str) -> str:
    """Compute compressed word shape (matches Rust compressed_shape)."""
    result = []
    prev = None
    for c in word[:20]:
        if c.isupper():
            mapped = "X"
        elif c.islower():
            mapped = "x"
        elif c.isdigit():
            mapped = "d"
        else:
            mapped = c
        if mapped != prev:
            result.append(mapped)
            prev = mapped
    return "".join(result)


def extract_features(
    word: str,
    prev_word: Optional[str],
    next_word: Optional[str],
    prev2_word: Optional[str],
    next2_word: Optional[str],
    position: int,
    sent_len: int,
    max_suffix_len: int = 8,
    max_prefix_len: int = 5,
    max_word_form_len: int = 6,
    max_word_form_ext_len: int = 8,
) -> dict:
    """Extract features from a word in context.

    Returns a dict of feature_name -> 1 (binary features).
    Must exactly match the Rust extract_features_ext function.
    """
    features = {}
    lower = word.lower()
    lower_len = len(lower)  # character count

    # ── Suffix features ──
    for n in range(1, min(max_suffix_len, lower_len) + 1):
        features[f"suf{n}={lower[-n:]}"] = 1

    # ── Prefix features ──
    for n in range(1, min(max_prefix_len, lower_len) + 1):
        features[f"pre{n}={lower[:n]}"] = 1

    # ── Word properties ──
    features[f"len={min(len(word.encode('utf-8')), 20)}"] = 1  # Rust uses word.len() = byte len
    features[f"shape={compressed_shape(word)}"] = 1

    if word and word[0].isupper():
        features["is_upper=true"] = 1
    if all(c.isupper() or not c.isalpha() for c in word) and any(c.isupper() for c in word):
        features["all_upper=True"] = 1
    if all(c.islower() or not c.isalpha() for c in word) and any(c.islower() for c in word):
        features["all_lower=True"] = 1
    if any(c.isdigit() for c in word):
        features["has_digit=True"] = 1
    if "-" in word:
        features["has_hyphen=True"] = 1
    if word and all(c.isdigit() for c in word):
        features["is_digit=True"] = 1

    # ── Position features ──
    if position == 0:
        features["is_first=True"] = 1
    if sent_len > 0 and position == sent_len - 1:
        features["is_last=True"] = 1
    if sent_len > 1:
        rel_pos = round(position / (sent_len - 1) * 100) / 100
    else:
        rel_pos = 0.0
    features[f"rel_pos={rel_pos:.2f}"] = 1

    # ── Punctuation ──
    if len(word) == 1 and word in PUNCT_CHARS:
        features["is_punct=True"] = 1
        features[f"punct_type={word}"] = 1

    # ── Finnish-specific suffix patterns ──
    if lower.endswith("ssa") or lower.endswith("ss\u00e4"):
        features["fi_case_iness=True"] = 1
    if lower.endswith("sta") or lower.endswith("st\u00e4"):
        features["fi_case_elat=True"] = 1
    if lower.endswith("lla") or lower.endswith("ll\u00e4"):
        features["fi_case_adess=True"] = 1
    if lower.endswith("lta") or lower.endswith("lt\u00e4"):
        features["fi_case_ablat=True"] = 1
    if lower.endswith("lle"):
        features["fi_case_allat=True"] = 1
    if lower.endswith("sti"):
        features["fi_adv_sti=True"] = 1
    if lower.endswith("inen"):
        features["fi_adj_inen=True"] = 1
    if lower.endswith("inen") or lower.endswith("llinen"):
        features["fi_adj_pattern=True"] = 1

    # Verb endings
    for ending in FI_VERB_ENDINGS:
        if lower.endswith(ending):
            features[f"fi_vend_{ending}=True"] = 1

    # Additional Finnish morphological patterns
    if lower.endswith("ksi"):
        features["fi_case_transl=True"] = 1
    if lower.endswith("na") or lower.endswith("n\u00e4"):
        features["fi_case_ess=True"] = 1
    if lower_len >= 4 and (lower.endswith("ta") or lower.endswith("t\u00e4")):
        features["fi_case_part=True"] = 1
    if lower_len >= 3 and lower.endswith("n"):
        last_two = lower[-2:]
        if last_two not in ("en", "an", "in", "on", "un", "yn", "\u00e4n", "\u00f6n"):
            features["fi_case_gen_other=True"] = 1
    if lower.endswith("ttu") or lower.endswith("tty"):
        features["fi_ptcp_pass=True"] = 1
    if lower.endswith("nut") or lower.endswith("nyt") or lower.endswith("neet"):
        features["fi_ptcp_act=True"] = 1
    if lower.endswith("ma") or lower.endswith("m\u00e4"):
        features["fi_ptcp_agent=True"] = 1
    if lower_len >= 3 and (lower.endswith("da") or lower.endswith("d\u00e4")):
        features["fi_inf1=True"] = 1
    if lower.endswith("maan") or lower.endswith("m\u00e4\u00e4n"):
        features["fi_inf3=True"] = 1
    if lower.endswith("mpi"):
        features["fi_comp=True"] = 1
    if lower_len >= 4 and lower.endswith("in"):
        features["fi_super=True"] = 1
    if lower.endswith("ni") or lower.endswith("si") or lower.endswith("nsa") or lower.endswith("ns\u00e4"):
        features["fi_poss=True"] = 1
    if lower_len >= 4 and "isi" in lower:
        features["fi_cond=True"] = 1
    if lower in FI_NEG_FORMS:
        features["fi_neg=True"] = 1

    # ── Character bigrams ──
    if lower_len >= 2:
        chars = list(lower[:11])
        limit = min(len(chars), 11) - 1
        for i in range(limit):
            bi = chars[i] + chars[i + 1]
            features[f"bi={bi}"] = 1

    # ── Context features ──
    if prev_word is not None:
        prev_lower = prev_word.lower()
        prev_char_len = len(prev_lower)
        if prev_char_len >= 3:
            features[f"prev_suf3={prev_lower[-3:]}"] = 1
        features[f"prev_shape={compressed_shape(prev_word)}"] = 1
        if prev_word and prev_word[0].isupper():
            features["prev_is_upper=True"] = 1
        if len(prev_word) == 1 and prev_word in PUNCT_CHARS_EXTENDED:
            features["prev_is_punct=True"] = 1
        if prev_char_len <= 4:
            features[f"prev_form={prev_lower}"] = 1
    else:
        features["prev_BOS=True"] = 1

    if next_word is not None:
        next_lower = next_word.lower()
        next_char_len = len(next_lower)
        if next_char_len >= 3:
            features[f"next_suf3={next_lower[-3:]}"] = 1
        features[f"next_shape={compressed_shape(next_word)}"] = 1
        if next_word and next_word[0].isupper():
            features["next_is_upper=True"] = 1
        if len(next_word) == 1 and next_word in PUNCT_CHARS_EXTENDED:
            features["next_is_punct=True"] = 1
        if next_char_len <= 4:
            features[f"next_form={next_lower}"] = 1
    else:
        features["next_EOS=True"] = 1

    # ── Extended context (prev-2, next-2) ──
    if prev2_word is not None:
        p2_lower = prev2_word.lower()
        p2_char_len = len(p2_lower)
        if p2_char_len >= 3:
            features[f"prev2_suf3={p2_lower[-3:]}"] = 1
        if p2_char_len <= 4:
            features[f"prev2_form={p2_lower}"] = 1
    else:
        features["prev2_BOS=True"] = 1

    if next2_word is not None:
        n2_lower = next2_word.lower()
        n2_char_len = len(n2_lower)
        if n2_char_len >= 3:
            features[f"next2_suf3={n2_lower[-3:]}"] = 1
        if n2_char_len <= 4:
            features[f"next2_form={n2_lower}"] = 1
    else:
        features["next2_EOS=True"] = 1

    # ── Word form (for short/medium words) ──
    if lower_len <= max_word_form_len:
        features[f"word_form={lower}"] = 1
    elif lower_len <= max_word_form_ext_len:
        features[f"word_form_ext={lower}"] = 1

    return features


# ──────────────────────────────────────────────────────────────────────
# CoNLL-U parsing
# ──────────────────────────────────────────────────────────────────────

def parse_conllu(path: Path) -> list:
    """Parse a CoNLL-U file into a list of sentences.

    Each sentence is a list of (word, upos) tuples.
    Skips multi-word tokens (lines with '-' in ID field).
    """
    sentences = []
    current = []
    with open(path, "r", encoding="utf-8") as f:
        for line in f:
            line = line.rstrip("\n")
            if not line or line.startswith("#"):
                if current:
                    sentences.append(current)
                    current = []
                continue
            parts = line.split("\t")
            if len(parts) < 4:
                continue
            # Skip multi-word tokens (e.g., "1-2")
            if "-" in parts[0] or "." in parts[0]:
                continue
            word = parts[1]
            upos = parts[3]
            current.append((word, upos))
    if current:
        sentences.append(current)
    return sentences


def sentences_to_features(sentences, **kwargs):
    """Convert parsed sentences to feature dicts and labels."""
    X_dicts = []
    y_labels = []
    for sent in sentences:
        words = [w for w, _ in sent]
        tags = [t for _, t in sent]
        n = len(words)
        for i in range(n):
            prev_w = words[i - 1] if i > 0 else None
            next_w = words[i + 1] if i + 1 < n else None
            prev2_w = words[i - 2] if i > 1 else None
            next2_w = words[i + 2] if i + 2 < n else None
            feats = extract_features(
                words[i], prev_w, next_w, prev2_w, next2_w,
                i, n, **kwargs
            )
            X_dicts.append(feats)
            y_labels.append(tags[i])
    return X_dicts, y_labels


# ──────────────────────────────────────────────────────────────────────
# MCET binary export
# ──────────────────────────────────────────────────────────────────────

def export_mcet(
    model: LogisticRegression,
    vectorizer: DictVectorizer,
    output_path: Path,
):
    """Export a trained model to MCET v1 binary format.

    Binary format:
        [4 bytes: magic "MCET"]
        [4 bytes: version (u32 LE) = 1]
        [4 bytes: n_features (u32 LE)]
        [4 bytes: n_classes (u32 LE)]
        [class names: (u16 LE len, UTF-8 bytes) x n_classes]
        [feature names: (u16 LE len, UTF-8 bytes, u32 LE index) x n_features]
        [intercepts: f32 LE x n_classes]
        [scale: f32 LE]
        [weights: i8 x (n_classes * n_features), row-major]
    """
    classes = list(model.classes_)
    feature_names = vectorizer.get_feature_names_out()
    n_classes = len(classes)
    n_features = len(feature_names)

    # Get the dense weight matrix (n_classes x n_features)
    coef = model.coef_  # shape: (n_classes, n_features)
    intercept = model.intercept_  # shape: (n_classes,)

    # Quantize weights to INT8
    max_abs = np.max(np.abs(coef))
    if max_abs == 0:
        scale = 1.0
        weights_i8 = np.zeros_like(coef, dtype=np.int8)
    else:
        scale = max_abs / 127.0
        weights_i8 = np.clip(np.round(coef / scale), -127, 127).astype(np.int8)

    # Build binary
    buf = bytearray()

    # Header
    buf.extend(b"MCET")
    buf.extend(struct.pack("<I", 1))  # version
    buf.extend(struct.pack("<I", n_features))
    buf.extend(struct.pack("<I", n_classes))

    # Class names
    for cls in classes:
        cls_bytes = cls.encode("utf-8")
        buf.extend(struct.pack("<H", len(cls_bytes)))
        buf.extend(cls_bytes)

    # Feature names with index
    for idx, fname in enumerate(feature_names):
        fname_bytes = fname.encode("utf-8")
        buf.extend(struct.pack("<H", len(fname_bytes)))
        buf.extend(fname_bytes)
        buf.extend(struct.pack("<I", idx))

    # Intercepts
    for i in range(n_classes):
        buf.extend(struct.pack("<f", float(intercept[i])))

    # Scale
    buf.extend(struct.pack("<f", float(scale)))

    # Weights (row-major: class x feature)
    for c in range(n_classes):
        for f in range(n_features):
            buf.extend(struct.pack("b", int(weights_i8[c, f])))

    output_path.parent.mkdir(parents=True, exist_ok=True)
    with open(output_path, "wb") as fout:
        fout.write(buf)

    print(f"Exported MCET model: {output_path}")
    print(f"  Classes: {n_classes}")
    print(f"  Features: {n_features}")
    print(f"  Scale: {scale:.6f}")
    print(f"  File size: {len(buf):,} bytes ({len(buf)/1024/1024:.2f} MB)")

    return len(buf)


# ──────────────────────────────────────────────────────────────────────
# Training configurations
# ──────────────────────────────────────────────────────────────────────

CONFIGS = {
    "baseline": {
        "desc": "Current baseline (suf 1-8, pre 1-5, C=1.0, liblinear)",
        "max_suffix_len": 8,
        "max_prefix_len": 5,
        "max_word_form_len": 6,
        "max_word_form_ext_len": 8,
        "C": 1.0,
        "solver": "liblinear",
        "max_iter": 1000,
    },
    "c2": {
        "desc": "Higher regularization C=2.0",
        "max_suffix_len": 8,
        "max_prefix_len": 5,
        "max_word_form_len": 6,
        "max_word_form_ext_len": 8,
        "C": 2.0,
        "solver": "liblinear",
        "max_iter": 1000,
    },
    "c5": {
        "desc": "Much higher regularization C=5.0",
        "max_suffix_len": 8,
        "max_prefix_len": 5,
        "max_word_form_len": 6,
        "max_word_form_ext_len": 8,
        "C": 5.0,
        "solver": "liblinear",
        "max_iter": 2000,
    },
    "c10": {
        "desc": "Very high regularization C=10.0",
        "max_suffix_len": 8,
        "max_prefix_len": 5,
        "max_word_form_len": 6,
        "max_word_form_ext_len": 8,
        "C": 10.0,
        "solver": "liblinear",
        "max_iter": 2000,
    },
    "suf10_pre6": {
        "desc": "Extended suffix/prefix (suf 1-10, pre 1-6)",
        "max_suffix_len": 10,
        "max_prefix_len": 6,
        "max_word_form_len": 6,
        "max_word_form_ext_len": 8,
        "C": 1.0,
        "solver": "liblinear",
        "max_iter": 1000,
    },
    "suf10_pre6_c5": {
        "desc": "Extended suffix/prefix + C=5.0",
        "max_suffix_len": 10,
        "max_prefix_len": 6,
        "max_word_form_len": 6,
        "max_word_form_ext_len": 8,
        "C": 5.0,
        "solver": "liblinear",
        "max_iter": 2000,
    },
    "wider_form": {
        "desc": "Wider word form capture (form<=8, ext<=12)",
        "max_suffix_len": 8,
        "max_prefix_len": 5,
        "max_word_form_len": 8,
        "max_word_form_ext_len": 12,
        "C": 1.0,
        "solver": "liblinear",
        "max_iter": 1000,
    },
    "wider_form_c5": {
        "desc": "Wider word form + C=5.0",
        "max_suffix_len": 8,
        "max_prefix_len": 5,
        "max_word_form_len": 8,
        "max_word_form_ext_len": 12,
        "C": 5.0,
        "solver": "liblinear",
        "max_iter": 2000,
    },
    "suf10_pre6_c2": {
        "desc": "Extended suffix/prefix + C=2.0",
        "max_suffix_len": 10,
        "max_prefix_len": 6,
        "max_word_form_len": 6,
        "max_word_form_ext_len": 8,
        "C": 2.0,
        "solver": "lbfgs",
        "max_iter": 1000,
    },
    "suf10_pre6_c07": {
        "desc": "Extended suffix/prefix + C=0.7",
        "max_suffix_len": 10,
        "max_prefix_len": 6,
        "max_word_form_len": 6,
        "max_word_form_ext_len": 8,
        "C": 0.7,
        "solver": "lbfgs",
        "max_iter": 1000,
    },
    "suf9_pre6": {
        "desc": "suf 1-9, pre 1-6, C=1.0",
        "max_suffix_len": 9,
        "max_prefix_len": 6,
        "max_word_form_len": 6,
        "max_word_form_ext_len": 8,
        "C": 1.0,
        "solver": "lbfgs",
        "max_iter": 1000,
    },
    "suf10_pre6_wider_c1": {
        "desc": "suf10 + pre6 + wider form + C=1.0",
        "max_suffix_len": 10,
        "max_prefix_len": 6,
        "max_word_form_len": 8,
        "max_word_form_ext_len": 12,
        "C": 1.0,
        "solver": "lbfgs",
        "max_iter": 1000,
    },
    "saga_c1": {
        "desc": "SAGA solver with C=1.0, L2",
        "max_suffix_len": 8,
        "max_prefix_len": 5,
        "max_word_form_len": 6,
        "max_word_form_ext_len": 8,
        "C": 1.0,
        "solver": "saga",
        "max_iter": 300,
        "penalty": "l2",
    },
    "full_combo": {
        "desc": "suf10 + pre6 + wider form + C=5.0",
        "max_suffix_len": 10,
        "max_prefix_len": 6,
        "max_word_form_len": 8,
        "max_word_form_ext_len": 12,
        "C": 5.0,
        "solver": "liblinear",
        "max_iter": 2000,
    },
}


def evaluate_greedy(model, vectorizer, sentences, **feature_kwargs):
    """Evaluate greedy (per-word) UPOS accuracy on sentences."""
    correct = 0
    total = 0
    for sent in sentences:
        words = [w for w, _ in sent]
        tags = [t for _, t in sent]
        n = len(words)
        for i in range(n):
            prev_w = words[i - 1] if i > 0 else None
            next_w = words[i + 1] if i + 1 < n else None
            prev2_w = words[i - 2] if i > 1 else None
            next2_w = words[i + 2] if i + 2 < n else None
            feats = extract_features(
                words[i], prev_w, next_w, prev2_w, next2_w,
                i, n, **feature_kwargs
            )
            feat_vec = vectorizer.transform([feats])
            pred = model.predict(feat_vec)[0]
            if pred == tags[i]:
                correct += 1
            total += 1
    return correct / total if total > 0 else 0.0


def evaluate_per_class(model, vectorizer, sentences, **feature_kwargs):
    """Evaluate per-class precision/recall/F1."""
    from collections import defaultdict
    tp = defaultdict(int)
    fp = defaultdict(int)
    fn = defaultdict(int)

    for sent in sentences:
        words = [w for w, _ in sent]
        tags = [t for _, t in sent]
        n = len(words)
        for i in range(n):
            prev_w = words[i - 1] if i > 0 else None
            next_w = words[i + 1] if i + 1 < n else None
            prev2_w = words[i - 2] if i > 1 else None
            next2_w = words[i + 2] if i + 2 < n else None
            feats = extract_features(
                words[i], prev_w, next_w, prev2_w, next2_w,
                i, n, **feature_kwargs
            )
            feat_vec = vectorizer.transform([feats])
            pred = model.predict(feat_vec)[0]
            gold = tags[i]
            if pred == gold:
                tp[gold] += 1
            else:
                fp[pred] += 1
                fn[gold] += 1

    all_tags = sorted(set(list(tp.keys()) + list(fp.keys()) + list(fn.keys())))
    print(f"\n{'Tag':<8} {'Prec':>7} {'Rec':>7} {'F1':>7} {'Support':>8}")
    print("-" * 42)
    for tag in all_tags:
        p = tp[tag] / (tp[tag] + fp[tag]) if (tp[tag] + fp[tag]) > 0 else 0
        r = tp[tag] / (tp[tag] + fn[tag]) if (tp[tag] + fn[tag]) > 0 else 0
        f1 = 2 * p * r / (p + r) if (p + r) > 0 else 0
        support = tp[tag] + fn[tag]
        print(f"{tag:<8} {p:>7.4f} {r:>7.4f} {f1:>7.4f} {support:>8}")


def train_config(config_name, config, train_sents, dev_sents, verbose=True):
    """Train a single configuration and evaluate on dev."""
    feature_kwargs = {
        "max_suffix_len": config["max_suffix_len"],
        "max_prefix_len": config["max_prefix_len"],
        "max_word_form_len": config["max_word_form_len"],
        "max_word_form_ext_len": config["max_word_form_ext_len"],
    }

    if verbose:
        print(f"\n{'='*60}")
        print(f"Config: {config_name}")
        print(f"  {config['desc']}")
        print(f"  suffix_len={config['max_suffix_len']}, prefix_len={config['max_prefix_len']}")
        print(f"  word_form_len={config['max_word_form_len']}, word_form_ext_len={config['max_word_form_ext_len']}")
        print(f"  C={config['C']}, solver={config['solver']}, max_iter={config['max_iter']}")

    t0 = time.time()
    X_train_dicts, y_train = sentences_to_features(train_sents, **feature_kwargs)
    t_feat = time.time() - t0
    if verbose:
        print(f"  Feature extraction: {t_feat:.1f}s ({len(X_train_dicts)} tokens)")

    vectorizer = DictVectorizer(sparse=True)
    X_train = vectorizer.fit_transform(X_train_dicts)
    if verbose:
        print(f"  Vocabulary size: {X_train.shape[1]}")

    t0 = time.time()
    solver = config["solver"]
    penalty = config.get("penalty", "l2")
    # sklearn 1.8+: use lbfgs solver (supports multiclass natively),
    # l1_ratio=0 for L2 regularization
    model = LogisticRegression(
        C=config["C"],
        solver="lbfgs",
        max_iter=config["max_iter"],
        verbose=0,
    )
    model.fit(X_train, y_train)
    t_train = time.time() - t0
    if verbose:
        print(f"  Training: {t_train:.1f}s")

    # Evaluate on dev
    t0 = time.time()
    dev_acc = evaluate_greedy(model, vectorizer, dev_sents, **feature_kwargs)
    t_eval = time.time() - t0
    if verbose:
        print(f"  Dev accuracy: {dev_acc*100:.2f}% (eval: {t_eval:.1f}s)")

    return model, vectorizer, dev_acc, feature_kwargs


def main():
    parser = argparse.ArgumentParser(description="Train suffix tagger")
    parser.add_argument("--config", default="all", help="Config name or 'all'")
    parser.add_argument("--evaluate-only", action="store_true")
    parser.add_argument("--export", default=None, help="Config to export")
    parser.add_argument("--per-class", action="store_true", help="Show per-class metrics")
    args = parser.parse_args()

    print("Loading data...")
    train_sents = parse_conllu(TRAIN_FILE)
    dev_sents = parse_conllu(DEV_FILE)
    test_sents = parse_conllu(TEST_FILE)
    print(f"  Train: {len(train_sents)} sentences, {sum(len(s) for s in train_sents)} tokens")
    print(f"  Dev:   {len(dev_sents)} sentences, {sum(len(s) for s in dev_sents)} tokens")
    print(f"  Test:  {len(test_sents)} sentences, {sum(len(s) for s in test_sents)} tokens")

    if args.config == "all":
        configs_to_run = CONFIGS
    else:
        if args.config not in CONFIGS:
            print(f"Unknown config: {args.config}")
            print(f"Available: {', '.join(CONFIGS.keys())}")
            sys.exit(1)
        configs_to_run = {args.config: CONFIGS[args.config]}

    results = {}
    best_name = None
    best_acc = 0.0

    for name, cfg in configs_to_run.items():
        model, vec, dev_acc, feat_kwargs = train_config(name, cfg, train_sents, dev_sents)
        results[name] = (model, vec, dev_acc, feat_kwargs)
        if dev_acc > best_acc:
            best_acc = dev_acc
            best_name = name

    # Summary
    print(f"\n{'='*60}")
    print("SUMMARY (dev set)")
    print(f"{'='*60}")
    print(f"{'Config':<20} {'Dev Acc':>10}")
    print("-" * 32)
    for name, (_, _, acc, _) in sorted(results.items(), key=lambda x: -x[1][2]):
        marker = " <-- BEST" if name == best_name else ""
        print(f"{name:<20} {acc*100:>9.2f}%{marker}")

    # Export best or specified config
    export_name = args.export or best_name
    if export_name and export_name in results:
        model, vec, dev_acc, feat_kwargs = results[export_name]

        # Test set evaluation
        test_acc = evaluate_greedy(model, vec, test_sents, **feat_kwargs)
        print(f"\nTest accuracy ({export_name}): {test_acc*100:.2f}%")

        if args.per_class:
            evaluate_per_class(model, vec, dev_sents, **feat_kwargs)

        # Back up existing model
        if MODEL_FILE.exists():
            backup = MODEL_FILE.with_suffix(".bin.bak")
            import shutil
            shutil.copy2(MODEL_FILE, backup)
            print(f"\nBacked up existing model to: {backup}")

        # Export
        print(f"\nExporting config '{export_name}'...")
        file_size = export_mcet(model, vec, MODEL_FILE)

        if file_size > 8 * 1024 * 1024:
            print(f"WARNING: Model exceeds 8MB limit ({file_size/1024/1024:.2f} MB)")

        print(f"\nFinal results for '{export_name}':")
        print(f"  Dev:  {dev_acc*100:.2f}%")
        print(f"  Test: {test_acc*100:.2f}%")
        print(f"  Size: {file_size/1024/1024:.2f} MB")


if __name__ == "__main__":
    main()
