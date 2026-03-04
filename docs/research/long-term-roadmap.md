# MCE Long-Term Roadmap

> **Date**: 2026-03-04
> **Status**: Research document (does not modify implementation code)
> **Audience**: Project maintainer, contributors, academic reviewers
> **Scope**: Phase 2 through Phase 4 detailed planning, competitive analysis, risk assessment

---

## Table of Contents

1. [Current State Summary](#1-current-state-summary)
2. [Phase 2: Neural Enhancement](#2-phase-2-neural-enhancement)
3. [Phase 3: IDE and Browser Integration](#3-phase-3-ide-and-browser-integration)
4. [Phase 4: Academic Papers](#4-phase-4-academic-papers)
5. [Competitive Landscape Analysis](#5-competitive-landscape-analysis)
6. [Key Risks and Dependencies](#6-key-risks-and-dependencies)
7. [Suggested Prioritization](#7-suggested-prioritization)
8. [References](#8-references)

---

## 1. Current State Summary

### Phase 1 Achievements (DONE)

| Metric | Target | Achieved |
|--------|--------|----------|
| UPOS accuracy (CG + Suffix Tagger) | 94-96% | **94.58%** (CoNLL standard, PUNCT/SYM excluded) |
| UPOS accuracy (rule-only) | 88-92% | 82.71% (partial miss, compensated by suffix tagger) |
| Lemma accuracy | 90%+ | **88.44%** (48K production dict, 42K benchmark dict) |
| Coverage | 99%+ | **99.64%** |
| Speed | <5ms/sentence | **~1.35ms** (42,090 tok/s) |
| WASM binary | <500KB | **365KB** |
| Deploy size | <15MB | **~9.2MB** (gzip ~2-3MB) |
| CG rules | 50-100 | **62 active** (85 total) |
| Crates | -- | **11 crates**, ~41,800 LOC |
| Tests | -- | **1,496+** |
| npm | v0.2 | **@yongsk0066/mce@0.2.0** |
| CI/CD | -- | GitHub Actions pipeline |
| Papers | 0 | Paper-3 SCiL submission ready, Paper-2 ~85% |

### Architecture: MCE v3 (4-Machine Heterogeneous Model)

```
M1: Succinct Trie -----> Dictionary lookup, spell check
M2': Comonadic Engine --> Morphological analysis + coKleisli generation
M3: PDT ----------------> Compound word structure analysis
M4': Weighted Lattice --> CG-lite + Suffix Tagger + Viterbi disambiguation
```

### Key Constraints for All Future Phases

| Constraint | Budget | Rationale |
|------------|--------|-----------|
| WASM binary | <500KB (current 365KB) | Browser initial load time |
| Total deploy | <20MB | CDN + gzip feasibility |
| gzip transfer | <5MB | Mobile network tolerance |
| Latency per sentence | <10ms | Real-time editing UX |
| Offline capability | Required | Core differentiator vs server-based tools |
| Graceful degradation | Required | Each layer must work independently |

---

## 2. Phase 2: Neural Enhancement

### 2.1 Current Suffix Tagger State

The suffix tagger is a logistic regression model (lbfgs solver) already implemented and deployed:

- **Model format**: MCET v1 binary, 5.0MB (gzip: 1.03MB)
- **Features**: 156,896 suffix/prefix features, 15 UPOS classes
- **Integration**: Dynamic loading via `load_model()` API (Option B -- JS fetch + Rust load)
- **Pipeline**: CG-lite (prune) -> Suffix Tagger (emission scores) -> Viterbi (global optimization)
- **Result**: UPOS 82.71% (rule-only) -> **94.58%** (with suffix tagger)
- **Runtime overhead**: ~2-3us/token (negligible)
- **Memory**: ~8.7MB additional WASM heap (total ~16.7MB with model loaded)

The suffix tagger represents the ceiling achievable without contextual (sentence-level) models. Moving beyond 94.58% requires models that consider surrounding tokens.

### 2.2 UPOS 97%+ Strategy

Target: Match TurkuNLP TNPP (UPOS 97.80 on Finnish-TDT v2.8).

**Gap analysis** (94.58% -> 97%+ requires ~2.4pp improvement):

| Error category | Estimated share | Mitigation strategy |
|----------------|-----------------|----------------------|
| Context-dependent POS (kuusi, tuuli, etc.) | ~40% | Contextual neural model |
| OOV / rare words | ~20% | Dictionary expansion + character model |
| Compound word POS | ~15% | Improved compound analyzer |
| CG rule gaps | ~15% | Additional CG rules (62 -> 100+) |
| Systematic mapping errors | ~10% | pos_map.rs refinement |

**Proposed approach -- Micro Transformer POS Disambiguator**:

1. **Architecture**: 2-layer Transformer encoder with character-level embeddings (no BPE/subword)
   - Input: Window of 5-7 tokens, each represented by suffix features + FST analysis candidates
   - Output: UPOS distribution per token
   - Parameters: ~500K-2M (target model size: 2-5MB quantized)

2. **Training pipeline**:
   - Train on UD Finnish-TDT (train split, ~12K sentences)
   - Teacher: FinBERT fine-tuned for POS tagging (local, not deployed)
   - Student: Micro Transformer distilled from teacher
   - Knowledge distillation: soft labels from teacher, hard labels from gold

3. **Deployment**:
   - Export to ONNX, INT8 quantization
   - Load via ONNX Runtime Web (WASM backend) or custom Rust inference
   - Web Worker for async inference (non-blocking UI)
   - Fallback: suffix tagger when neural model unavailable

4. **Size/latency budget**:
   - Model: 2-5MB (INT8 ONNX), gzip ~1-2MB
   - Inference: 5-20ms per sentence (WASM), <5ms (WebGPU)
   - Total deploy with neural: ~14-17MB (within 20MB budget)

**Alternative approaches considered**:

| Approach | Size | Accuracy | Complexity | Verdict |
|----------|------|----------|------------|---------|
| Micro Transformer (proposed) | 2-5MB | 96-97%+ | Medium | **Selected** |
| BiLSTM CRF tagger | 3-8MB | 95-96% | Low | Backup option |
| FinBERT direct (INT4) | 30-55MB | 97-98% | High | Exceeds size budget |
| Additional CG rules only | 0MB | 95-96% | Medium | Diminishing returns |
| Weighted FST (k2-trained) | 0MB extra | 93-95% | Low | Insufficient for 97%+ |

### 2.3 Lemma 93%+ Strategy

Current: 88.44% (42K benchmark dict). Target: 93%+.

**Gap analysis** (88.44% -> 93% requires ~4.6pp improvement):

| Error category | Estimated share | Mitigation strategy |
|----------------|-----------------|----------------------|
| Missing dictionary entries | ~35% | Kotus 94K integration + Wiktionary |
| Compound lemmatization | ~25% | Improved compound splitter (currently 80.9%) |
| Irregular forms | ~20% | Seq2Seq / edit-tree model |
| Derivational morphology | ~15% | Derivation stem recognition |
| Systematic errors | ~5% | Rule fixes |

**Two-track approach**:

**Track A -- Dictionary expansion (v0.3.0-v0.4.0)**:
- Kotus XML: 94K lemmas, CC BY 4.0 (GO decision from Session 11)
- Expected lift: +2-3pp for known-word lemmatization
- Implementation: POS mapping (s->NOUN, a->ADJ, v->VERB), speller integration

**Track B -- Neural lemmatizer (Phase 2 proper)**:

1. **Edit-tree lemmatizer** (spaCy-style):
   - Learns transformation rules (edit trees) from token-lemma pairs
   - Very small model (~500KB-1MB)
   - Accuracy: 95%+ for many UD languages
   - Rust implementation feasible (no ONNX dependency)

2. **Seq2Seq character-level lemmatizer** (TurkuNLP Universal Lemmatizer style):
   - Encoder-decoder with attention, character-level
   - Input: surface form + morphological features (UPOS, case, number)
   - Model size: 2-5MB (quantized)
   - Accuracy: 96%+ (TurkuNLP achieved 96.13% on Finnish-TDT)
   - Deployment: ONNX Runtime Web or Rust native inference

3. **Hybrid**: Dictionary lookup first (fast path), neural fallback for OOV/ambiguous (slow path)
   - Expected combined accuracy: 93-95%

**Recommendation**: Start with edit-tree (Track B1) as it is small and can be implemented in pure Rust without ONNX dependency. Add Seq2Seq (Track B2) if edit-tree is insufficient.

### 2.4 Context-Aware Spell Correction

Currently MCE has word-level spell checking (FST-based). Context-aware correction requires:

1. **Phase 2a**: Confusion-set disambiguation (e.g., "sitten" vs "siithen")
   - Use n-gram or micro-LM to select correct word from spell candidates
   - Model size: 1-3MB (character bigram/trigram statistics)

2. **Phase 2b**: GEC (Grammatical Error Correction)
   - Finnish GEC benchmark construction (currently no standard exists)
   - ICLFI (International Corpus of Learner Finnish) data needed
   - Rule-based explanation + neural correction suggestion

### 2.5 Phase 2 Implementation Plan

| Step | What | Size impact | Accuracy target | Timeline |
|------|------|-------------|-----------------|----------|
| 2.1 | Kotus 94K dictionary integration | +~1MB dict | Lemma +2-3pp | v0.3.0-v0.4.0 |
| 2.2 | Compound analyzer improvement | 0 | Compound 80.9% -> 90%+ | v0.4.0 |
| 2.3 | Edit-tree lemmatizer (Rust native) | +0.5-1MB | Lemma 91-93% | v0.5.0 |
| 2.4 | Micro Transformer POS tagger | +2-5MB | UPOS 96-97%+ | v0.6.0 |
| 2.5 | Seq2Seq lemmatizer (if needed) | +2-5MB | Lemma 95%+ | v0.7.0 |
| 2.6 | Context spell correction | +1-3MB | F1 85%+ | v0.8.0 |
| 2.7 | Web Worker async pipeline | 0 | -- | v0.6.0 (with 2.4) |

**Total Phase 2 deploy budget**: ~15-20MB (current 9.2MB + 6-11MB neural models).

### 2.6 ML Runtime Decision

| Runtime | Pros | Cons | Verdict |
|---------|------|------|---------|
| **ONNX Runtime Web (WASM)** | Mature, wide model support, Microsoft-backed | 5x slower than native, large JS bundle (~2MB) | For Transformer models |
| **ONNX Runtime Web (WebGPU)** | GPU acceleration (10-15x vs WASM), FP16 | WebGPU not in Firefox/Safari stable yet | Future upgrade path |
| **Custom Rust inference** | No external dependency, small binary, fast | Must implement ops manually | For edit-tree, small models |
| **Candle (HuggingFace Rust ML)** | Rust-native, WASM target support | Less mature than ONNX RT | Backup option |
| **wonnx** | 100% Rust, WebGPU-accelerated ONNX | Early stage, limited op coverage | Watch |

**Recommendation**: Use custom Rust inference for small models (edit-tree, confusion sets). Use ONNX Runtime Web (WASM backend) for Transformer models, with WebGPU upgrade path when browser support matures.

---

## 3. Phase 3: IDE and Browser Integration

### 3.1 VS Code Extension (Language Server Protocol)

**Architecture**:

```
VS Code Editor
    |
    | Language Client (TypeScript, VS Code extension API)
    | Handles: activation, configuration, UI integration
    |
    | <-- LSP (JSON-RPC over stdio/pipe) -->
    |
Language Server (Rust binary OR Node.js + WASM)
    |
    | MCE Engine (Rust core)
    | Provides: diagnostics, quick fixes, completion
```

**Two deployment options**:

**Option A -- Rust binary language server (recommended)**:
- Ship `mce-lsp` binary (compiled for each platform: macOS ARM/x86, Linux, Windows)
- LSP communication via stdio
- Full native performance (~42K tok/s)
- Follows Harper's architecture (Rust LS + VS Code client)
- Size: ~5-10MB binary + 4MB dict + 5MB model = ~15-20MB total extension

**Option B -- Node.js + WASM**:
- Ship WASM module, run via Node.js in VS Code
- Same MCE engine, but WASM overhead (~2x slower)
- Single universal package (no platform-specific binaries)
- Size: ~9.2MB WASM+dict+model + Node.js wrapper

**LSP capabilities to implement**:

| LSP Feature | MCE Mapping | Priority |
|-------------|-------------|----------|
| `textDocument/publishDiagnostics` | Spelling errors, grammar rule violations | P0 |
| `textDocument/codeAction` | Spell suggestions, grammar fixes | P0 |
| `textDocument/completion` | Word completion from dictionary | P1 |
| `textDocument/hover` | Morphological analysis on hover | P1 |
| `textDocument/formatting` | Hyphenation suggestions | P2 |
| Custom: `mce/analyzeWord` | Full morphological breakdown | P2 |

**File type support**: Plain text, Markdown, LaTeX, HTML (with tag filtering).

**Key reference**: Harper (Automattic) ships a Rust-based language server with VS Code extension. It serves as the primary precedent for this architecture. Harper reached v1.0.0 and has active maintenance.

**Implementation effort**: ~2-4 weeks for core diagnostics + code actions. The main work is writing the LSP JSON-RPC handler in Rust and the VS Code extension client in TypeScript.

### 3.2 Chrome Extension

**Architecture**:

```
Chrome Extension (Manifest V3)
    |
    +-- Service Worker (background.js)
    |   - Loads MCE WASM engine
    |   - Manages dictionary + model cache (IndexedDB)
    |   - Receives text from content scripts, returns diagnostics
    |
    +-- Content Script (content.js)
    |   - Injected into web pages
    |   - Monitors textarea, contenteditable, input[type=text]
    |   - Highlights spelling/grammar errors with underlines
    |   - Shows suggestion popups on click/hover
    |
    +-- Popup (popup.html)
        - Extension settings (enable/disable, language options)
        - Statistics (words checked, errors found)
```

**Key technical considerations**:

1. **Manifest V3 requirements**:
   - Service workers replace persistent background pages
   - WASM must be loaded in service worker context
   - Storage: IndexedDB for dict/model caching (service worker compatible)

2. **Content script DOM interaction**:
   - MutationObserver for detecting text changes
   - Debounced checking (300-500ms after last keystroke)
   - Overlay-based error highlighting (CSS custom underlines)
   - Popup positioning relative to error spans

3. **Performance**:
   - Check visible text only (intersection observer)
   - Sentence-level incremental checking (not full document)
   - Web Worker for MCE inference (if service worker is too constrained)

4. **Size**: Chrome Web Store has a 50MB compressed extension limit. MCE at ~9.2MB (gzip ~2-3MB) fits easily.

**Key reference**: LanguageTool Chrome extension performs similar DOM manipulation for grammar checking. Write-better (GitHub: justiceo/write-better) is an open-source Chrome extension for grammar suggestions on Google Docs.

**Implementation effort**: ~3-5 weeks. The DOM manipulation and UX (underlines, popups) is the bulk of the work.

### 3.3 Google Docs Integration

**Three possible approaches**:

**Option A -- Google Workspace Add-on (Apps Script)**:
- Apps Script sidebar that calls MCE WASM
- Problem: Apps Script runs server-side (Google's servers), cannot run WASM
- Workaround: Deploy MCE as a serverless function (Cloud Functions), call from Apps Script
- This defeats the "offline / privacy-first" value proposition

**Option B -- Chrome Extension overlay on Google Docs**:
- Same Chrome extension from 3.2, but with Google Docs-specific DOM handling
- Google Docs uses a custom canvas-based renderer, not standard contenteditable
- Must intercept the Google Docs text model via its internal API or use accessibility tree
- Technically complex but preserves offline capability
- LanguageTool uses this approach for Google Docs

**Option C -- Google Docs API + Server**:
- Use Google Docs API to read/write document content
- MCE runs server-side
- Loses offline/privacy advantages

**Recommendation**: Option B (Chrome extension with Google Docs DOM adaptation). This is the most technically challenging but preserves the core value proposition. Start with textarea/contenteditable support in 3.2, then add Google Docs support as a follow-up.

**Implementation effort**: ~4-6 weeks additional on top of Chrome extension (3.2). Google Docs DOM reverse-engineering is the main challenge.

### 3.4 Obsidian Plugin

**Architecture**:
- Obsidian uses Electron (Node.js + Chromium)
- Can load MCE WASM directly via Node.js or browser WASM API
- Obsidian plugin API provides `MarkdownView` for text access
- Existing precedent: `voikko-obsidian` plugin (can be evolved)

**Implementation**: Simpler than Chrome extension (Obsidian API is more structured). ~1-2 weeks. Reuses the same WASM engine.

### 3.5 Phase 3 Priority Order

| Priority | Platform | Effort | User reach | Rationale |
|----------|----------|--------|------------|-----------|
| 1 | VS Code extension | 2-4 weeks | Developers, writers | Fastest path to users, LSP is standard |
| 2 | Chrome extension | 3-5 weeks | All web users | Largest potential audience |
| 3 | Obsidian plugin | 1-2 weeks | Note-takers | Low effort, existing precedent |
| 4 | Google Docs | 4-6 weeks | Document writers | High complexity, high value |

---

## 4. Phase 4: Academic Papers

### 4.1 Paper Program Overview (D020)

The 3-paper research program, ordered for progressive credibility building:

| Order | Paper | Venue | Status | Deadline |
|-------|-------|-------|--------|----------|
| 1 | Paper-3: Comonadic Morphophonology | SCiL 2026 (backup: SIGMORPHON) | **Submission-ready** (body 7.5p, 12p total, anonymized) | **2026-03-12** |
| 2 | Paper-2: Morphological Fingerprint via TT Decomposition | SIGMORPHON/EMNLP Workshop 2026 | 12-language experiment done, narrative rewrite needed (D026) | ~2026-08-09 |
| 3 | Paper-5: Comonadic Classification | ACL/EMNLP 2027 | Research stage | ~2027-01-05 |

### 4.2 Paper-3: Comonadic Morphophonology (SCiL 2026)

**Core contribution**: Writer Comonad formalization of Finnish morphophonological rules. Consonant gradation (11 patterns) as pure coKleisli arrows. First application of comonads to morphophonology.

**Current state**:
- LaTeX body: 7.5 pages, 12 pages total (with references, appendix)
- Anonymized, proofread
- Phase 3 review (R3+R4) complete
- UPOS 94.58% metrics reflected
- Writer Comonad implementation: 980 LOC Rust, 44 tests, associativity law verified

**Remaining TODO**:
- SCiL 2026 submission by **2026-03-12** (8 days from now)
- LaTeX compilation test with acl.sty
- SIGMORPHON CFP monitoring (backup venue)

**Rejection contingency**: SCiL 2026 -> SIGMORPHON 2026 -> SCiL 2027 -> EACL 2027

### 4.3 Paper-2: Morphological Fingerprint via Bond Rank Profiles

**Core contribution**: Tensor Train decomposition of morphological paradigm tensors reveals "bond rank profiles" as quantitative typological measures. Perfect k=2 clustering of agglutinative vs fusional languages.

**Current state**:
- 12 languages, 5 families, 3 typological groups
- Kruskal-Wallis p=8.03e-61 (statistically significant)
- Perfect k=2 clustering (agglutinative vs fusional+introflexive)
- Spearman rho=-0.743, p=0.006 (Bond1 vs syncretism correlation)
- Suffix-only encoding strengthens typological signal (eta-squared 0.270 -> 0.360)
- Framing: D026 confirmed "Morphological Fingerprint" (not syncretism detection)

**Remaining TODO**:
- Narrative full rewrite (fingerprint framing)
- Expand to 12-15 languages (currently 12; possibly add 2-3 more)
- Phase 2 R1+R2 re-run after rewrite
- Non-author proofreading
- Target submission: EMNLP Workshop ~2026-08-09

**Rejection contingency**: SIGMORPHON/EMNLP Workshop 2026 -> EACL 2027 -> ACL 2027

### 4.4 Paper-5: Comonadic Classification of Morphophonological Operations

**Core contribution**: Complete classification table of which morphophonological operations (deletion, epenthesis, metathesis) are coKleisli arrows. Connection to subregular correspondence (ISL-k, OSL-k).

**Current state**: Research stage. Depends on Paper-3 publication.

**Key research questions**:
- Epenthesis as coKleisli (context-dependent insertion) -- formalization needed
- Metathesis representation in comonadic framework
- ISL-k conjecture verification
- Complete classification table design

**Target**: ACL/EMNLP 2027 (submission ~2027-01-05)

### 4.5 Paper Timeline (Gantt-style)

```
2026 Mar  Apr  May  Jun  Jul  Aug  Sep  Oct  Nov  Dec  2027 Jan  Feb
  |----|----|----|----|----|----|----|----|----|----|----|----|
P3 [SUB].............[NOTIF]
P2      [REWRITE].........[EXPAND]......[SUB].........[NOTIF]
P5                                            [RESEARCH........[SUB]
```

### 4.6 Inactive Papers

| Paper | Status | Notes |
|-------|--------|-------|
| Paper-1 (Finnish GEC) | Inactive | Requires ICLFI data. May resume if data becomes available. |
| Paper-4 (Neural -> FST Distillation) | Not started | Long-term. Based on arXiv:2601.10918. |
| Paper-6 (Heterogeneous Framework) | Partially absorbed | Into Paper-5 (Classification). |

---

## 5. Competitive Landscape Analysis

### 5.1 Direct Competitors in Browser-Native NLP

| Tool | Language | Technology | Size | Offline | Status |
|------|----------|------------|------|---------|--------|
| **MCE (this project)** | Finnish | Rust + WASM | 365KB + 9.2MB deploy | Yes | Active |
| **Harper** (Automattic) | English only | Rust + WASM | Sub-20ms per document | Yes | Active, v1.0.0 |
| **Hunspell-asm** | Multi-language | C++ -> Emscripten WASM | ~2MB | Yes | Maintained |
| **LanguageTool (WASM)** | Multi-language | Java -> server (WASM partial) | Server-dependent | Partial | Active |

**Key observation**: Harper (Automattic acquisition, Nov 2024) validates the Rust+WASM grammar checker market. However, Harper supports only English. MCE is the only browser-native tool for Finnish with morphological analysis capabilities beyond spell checking.

### 5.2 Server-Based Finnish NLP Competitors

| Tool | UPOS | Lemma | Deploy | Maintained | Notes |
|------|------|-------|--------|------------|-------|
| **TurkuNLP TNPP** | 97.80% | 96.13% | Server (GPU) | Deprecated (2024-05) | Benchmark ceiling |
| **Trankit** | ~98.48% | ~97%+ | Server (GPU) | Active (server issue Jul 2025) | SOTA accuracy, heavy |
| **Stanza** | ~96% | ~95% | Server | Active | Stanford NLP |
| **UralicNLP** | N/A (FST) | N/A (FST) | Python (pip) | Active (MCP support added) | Wrapper, not direct competitor |
| **Omorfi** | 83.88% | 82.63% | CLI (HFST) | Active | Morphological resource, not parser |

**MCE positioning**: MCE does not compete with Trankit/Stanza on raw UPOS accuracy (94.58% vs 98.48%). MCE competes on deployment model: browser-native, offline, privacy-first, <10MB. No server-based tool can match this.

### 5.3 Recent Developments (2025-2026)

1. **Harper v1.0.0**: Rust+WASM grammar checker. Proves market viability. English only. Automattic backing.

2. **UralicNLP MCP support**: Added Model Context Protocol integration for LLM connectivity. Shows the field moving toward AI/LLM integration.

3. **Trankit server issues** (Jul 2025): Model download server problems reported. Highlights fragility of server-dependent tools -- strengthens MCE's offline value proposition.

4. **TNPP deprecated** (May 2024): TurkuNLP's neural parser pipeline officially ended. Recommends Trankit. Reduces Finnish NLP tool count.

5. **ONNX Runtime Web + WebGPU**: Official WebGPU support since ORT 1.17. Chrome/Edge stable, Firefox/Safari in progress. 10-15x speedup over WASM for GPU-compatible models. Key enabler for Phase 2 neural models.

6. **WebAssembly ecosystem growth**: WASM 3.0, WASI maturation, growing adoption in edge computing. Reinforces MCE's technology bet.

7. **LLM morphological analysis failure** (Moisio et al., CMCL 2024): GPT-4 struggles with Finnish morphology; FST-based systems maintain clear advantage for structured morphological analysis. Validates MCE's FST-first architecture.

### 5.4 Competitive Moat Assessment

| Moat | Strength | Threat level |
|------|----------|-------------|
| Only browser-native Finnish NLP | Strong | Low (no competing projects observed) |
| Offline/privacy-first | Strong | Low (trend is toward server/cloud) |
| Comonadic formalization (academic) | Strong | Low (novel approach) |
| UPOS accuracy (94.58%) | Moderate | Medium (Trankit at 98.48%) |
| Community adoption | Weak | High (no significant user base yet) |

---

## 6. Key Risks and Dependencies

### 6.1 Technical Risks

| Risk | Probability | Impact | Mitigation |
|------|-------------|--------|------------|
| ONNX Runtime Web too large for deployment | Medium | High | Use custom Rust inference for small models; ONNX only for Transformer |
| Micro Transformer insufficient for 97%+ UPOS | Medium | High | Fall back to larger model with lazy loading; or accept 96% as sufficient |
| WebGPU browser support fragmented | Low | Medium | WASM fallback always available; WebGPU is progressive enhancement |
| Edit-tree lemmatizer insufficient for Finnish | Low | Medium | Seq2Seq backup; dictionary expansion provides baseline improvement |
| Google Docs DOM changes break extension | Medium | Medium | Abstract DOM layer; monitor Google Docs updates |
| WASM memory pressure on mobile | Medium | Low | Model-free mode (82.71% UPOS) as fallback |

### 6.2 Academic Risks

| Risk | Probability | Impact | Mitigation |
|------|-------------|--------|------------|
| Paper-3 SCiL rejection | Medium | Medium | SIGMORPHON backup, then SCiL 2027 |
| Paper-2 "so what?" challenge persists | Low | High | Fingerprint framing with 12-language evidence + clustering |
| Independent researcher credibility | High | Medium | Workshop-first strategy (SIGMORPHON before ACL) |
| Competing publication on comonadic morphology | Low | High | Submit Paper-3 promptly; comonadic morphophonology is niche |

### 6.3 Resource Risks

| Risk | Probability | Impact | Mitigation |
|------|-------------|--------|------------|
| Single developer bottleneck | High | High | Prioritize ruthlessly; open-source contributions welcome |
| Finnish training data insufficient | Low | Low | UD Finnish-TDT + UniMorph + Omorfi + Kotus well-positioned |
| ICLFI data unavailable for GEC paper | High | Medium | Defer Paper-1; focus on Paper-3 and Paper-2 |

### 6.4 External Dependencies

| Dependency | Status | Risk |
|------------|--------|------|
| ONNX Runtime Web | Stable (v1.17+) | Low |
| wasm-pack / wasm-bindgen | Stable | Low |
| WebGPU (Chrome) | Stable since Chrome 113 | Low |
| WebGPU (Firefox) | In development | Medium (not blocking) |
| WebGPU (Safari) | In development | Medium (not blocking) |
| VS Code LSP | Stable | Low |
| Chrome Manifest V3 | Required (V2 deprecated) | Low |
| ICLFI corpus access | Unknown | High (blocks Paper-1/GEC work) |
| SIGMORPHON 2026 CFP | Not yet published | Medium |
| Kotus XML dictionary | Available, CC BY 4.0 | Low |

---

## 7. Suggested Prioritization

### 7.1 Near-Term (v0.3.0, March-April 2026)

Already confirmed in MEMORY.md:
1. **suggest() single backend** -- mce-speller already linked, ~0KB increase
2. **Plural generation** (11 -> 22 noun forms) -- <2KB, API contract completion
3. **suffix_tagger.bin.bak cleanup** -- 6.4MB repo size reduction
4. **Paper-3 SCiL submission** (2026-03-12)

### 7.2 Short-Term (v0.4.0-v0.5.0, April-July 2026)

1. **Kotus 94K dictionary integration** (speller + lemma, +2-3pp lemma)
2. **Compound analyzer improvement** (80.9% -> 90%+)
3. **Edit-tree lemmatizer** (Rust native, +2-3pp lemma, target: 91-93%)
4. **Paper-2 narrative rewrite** (fingerprint framing)
5. **SIGMORPHON CFP monitoring** for Paper-3 backup/Paper-2 submission

### 7.3 Medium-Term (v0.6.0-v0.8.0, July-December 2026)

1. **Micro Transformer POS tagger** (ONNX Runtime Web, UPOS 96-97%+)
2. **Web Worker async inference pipeline**
3. **VS Code extension** (LSP, Rust binary server)
4. **Paper-2 EMNLP Workshop submission** (~August-September)
5. **Seq2Seq lemmatizer** (if edit-tree insufficient)

### 7.4 Long-Term (2027)

1. **Chrome extension** (Manifest V3, textarea/contenteditable)
2. **Google Docs integration** (Chrome extension overlay approach)
3. **Paper-5 ACL/EMNLP submission**
4. **Context-aware spell correction / GEC**
5. **v1.0 release** with full neural enhancement + IDE integration
6. **Multi-language exploration** (Estonian, North Sami -- same architecture)
7. **WebGPU upgrade path** for neural models

### 7.5 Prioritization Rationale

The ordering follows these principles:

1. **Academic deadlines drive paper work**: Paper-3 SCiL (March 12) is immovable. Paper-2 EMNLP Workshop (~August) shapes mid-year work.

2. **Low-hanging fruit first**: Dictionary expansion and compound improvement yield significant accuracy gains with minimal implementation risk.

3. **Pure Rust before ONNX**: Edit-tree lemmatizer (Rust native) before Transformer POS tagger (ONNX dependency) reduces integration complexity.

4. **VS Code before Chrome extension**: VS Code has simpler integration (LSP is standardized) and reaches the developer audience first. Chrome extension's DOM manipulation is more fragile.

5. **Accuracy before distribution**: Achieving 96%+ UPOS and 93%+ lemma makes the IDE/browser extensions more compelling. Shipping a 94.58% UPOS VS Code extension is viable but less differentiated.

---

## 8. References

### Finnish NLP Tools
- TurkuNLP Finnish NLP: https://turkunlp.org/finnish_nlp.html
- FinBERT: https://github.com/TurkuNLP/FinBERT
- Trankit: https://github.com/nlp-uoregon/trankit
- UralicNLP: https://github.com/mikahama/uralicNLP
- Omorfi: https://github.com/flammie/omorfi

### Browser NLP Precedents
- Harper (Automattic): https://github.com/Automattic/harper
- Hunspell WASM: https://kwonoj.github.io/en/post/hunspell-webassembly/
- Sherpa-ONNX (k2-fsa): https://github.com/k2-fsa/sherpa-onnx

### ML Runtime
- ONNX Runtime Web: https://onnxruntime.ai/docs/tutorials/web/
- ONNX Runtime WebGPU: https://onnxruntime.ai/docs/tutorials/web/ep-webgpu.html
- wonnx (Rust WebGPU ONNX): https://github.com/webonnx/wonnx
- Candle (HuggingFace Rust ML): https://github.com/huggingface/candle

### VS Code / LSP
- LSP Extension Guide: https://code.visualstudio.com/api/language-extensions/language-server-extension-guide
- LTeX (LanguageTool + LSP): https://valentjn.github.io/ltex/

### Chrome Extension
- Manifest V3 Content Scripts: https://developer.chrome.com/docs/extensions/reference/manifest/content-scripts
- LanguageTool Chrome Extension: https://languagetool.org/

### Lemmatization Approaches
- spaCy Edit-Tree Lemmatizer: https://explosion.ai/blog/edit-tree-lemmatizer
- Universal Lemmatizer (TurkuNLP): https://www.cambridge.org/core/journals/natural-language-engineering/article/universal-lemmatizer
- ohnomore_seq2seq_rs (Rust Seq2Seq lemma): https://github.com/twuebi/ohnomore_seq2seq_rs

### Academic References
- Moisio et al. (CMCL 2024): LLMs' morphological analyses of Finnish words
- Pirinen (2019): Neural vs rule-based Finnish NLP
- Kanerva et al. (2018): Turku Neural Parser Pipeline
- arXiv:2601.10918 (2026): Neural Induction of Finite-State Transducers

### WebAssembly Trends
- State of WebAssembly 2025-2026: https://platform.uno/blog/the-state-of-webassembly-2025-2026/
- Chrome WebGPU + WASM enhancements: https://developer.chrome.com/blog/io24-webassembly-webgpu-1
