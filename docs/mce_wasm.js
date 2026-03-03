/* @ts-self-types="./mce_wasm.d.ts" */

/**
 * MCE engine instance for browser use.
 *
 * Holds a loaded VFST transducer, a Viterbi disambiguator, a grammar checker,
 * and a hyphenator. Provides morphological analysis, spell checking, grammar
 * checking, hyphenation, sentence-level disambiguation, suggestions, and
 * compound splitting through a wasm-bindgen compatible API.
 */
export class MceEngine {
    static __wrap(ptr) {
        ptr = ptr >>> 0;
        const obj = Object.create(MceEngine.prototype);
        obj.__wbg_ptr = ptr;
        MceEngineFinalization.register(obj, obj.__wbg_ptr, obj);
        return obj;
    }
    __destroy_into_raw() {
        const ptr = this.__wbg_ptr;
        this.__wbg_ptr = 0;
        MceEngineFinalization.unregister(this);
        return ptr;
    }
    free() {
        const ptr = this.__destroy_into_raw();
        wasm.__wbg_mceengine_free(ptr, 0);
    }
    /**
     * Analyze a word and return JSON with all analyses.
     *
     * Returns a JSON array of objects, each containing the morphological
     * attributes (CLASS, BASEFORM, STRUCTURE, etc.) for one analysis.
     *
     * Example output:
     * ```json
     * [{"CLASS":"nimisana","BASEFORM":"koira","STRUCTURE":"=ppppp",...}]
     * ```
     * @param {string} word
     * @returns {string}
     */
    analyze(word) {
        let deferred2_0;
        let deferred2_1;
        try {
            const retptr = wasm.__wbindgen_add_to_stack_pointer(-16);
            const ptr0 = passStringToWasm0(word, wasm.__wbindgen_export, wasm.__wbindgen_export2);
            const len0 = WASM_VECTOR_LEN;
            wasm.mceengine_analyze(retptr, this.__wbg_ptr, ptr0, len0);
            var r0 = getDataViewMemory0().getInt32(retptr + 4 * 0, true);
            var r1 = getDataViewMemory0().getInt32(retptr + 4 * 1, true);
            deferred2_0 = r0;
            deferred2_1 = r1;
            return getStringFromWasm0(r0, r1);
        } finally {
            wasm.__wbindgen_add_to_stack_pointer(16);
            wasm.__wbindgen_export3(deferred2_0, deferred2_1, 1);
        }
    }
    /**
     * Analyze a sentence with tokenization and disambiguation.
     *
     * Pipeline:
     * 1. Tokenize the text into all tokens (words, punctuation, etc.)
     * 2. Analyze each word with FinnishAnalyzer
     * 3. Disambiguate word tokens using ViterbiDisambiguator (POS bigram model)
     * 4. Return JSON array including all non-whitespace tokens
     *
     * Word tokens get full analysis; punctuation tokens get `"type":"punctuation"`
     * with `null` analysis, matching CoNLL-U conventions.
     *
     * Example output:
     * ```json
     * [
     *   {"word":"Koira","type":"word","analysis":{"CLASS":"nimisana","BASEFORM":"koira"}},
     *   {"word":"juoksee","type":"word","analysis":{"CLASS":"teonsana","BASEFORM":"juosta"}},
     *   {"word":".","type":"punctuation","analysis":null}
     * ]
     * ```
     * @param {string} text
     * @returns {string}
     */
    analyze_sentence(text) {
        let deferred2_0;
        let deferred2_1;
        try {
            const retptr = wasm.__wbindgen_add_to_stack_pointer(-16);
            const ptr0 = passStringToWasm0(text, wasm.__wbindgen_export, wasm.__wbindgen_export2);
            const len0 = WASM_VECTOR_LEN;
            wasm.mceengine_analyze_sentence(retptr, this.__wbg_ptr, ptr0, len0);
            var r0 = getDataViewMemory0().getInt32(retptr + 4 * 0, true);
            var r1 = getDataViewMemory0().getInt32(retptr + 4 * 1, true);
            deferred2_0 = r0;
            deferred2_1 = r1;
            return getStringFromWasm0(r0, r1);
        } finally {
            wasm.__wbindgen_add_to_stack_pointer(16);
            wasm.__wbindgen_export3(deferred2_0, deferred2_1, 1);
        }
    }
    /**
     * Split a compound word into its constituent parts.
     *
     * Uses the CompoundAnalyzer (M3 pushdown transducer) with dictionary
     * lookups backed by FinnishAnalyzer. Returns the best compound splits
     * sorted by penalty (lowest first).
     *
     * Only words that decompose into 2+ dictionary parts are returned.
     * Single dictionary words return an empty array.
     *
     * Example output:
     * ```json
     * [
     *   {
     *     "parts": [
     *       {"surface":"rauta","start":0,"end":5,"is_linking":false},
     *       {"surface":"tie","start":5,"end":8,"is_linking":false},
     *       {"surface":"asema","start":8,"end":13,"is_linking":false}
     *     ],
     *     "penalty": 30
     *   }
     * ]
     * ```
     * @param {string} word
     * @returns {string}
     */
    compound_split(word) {
        let deferred2_0;
        let deferred2_1;
        try {
            const retptr = wasm.__wbindgen_add_to_stack_pointer(-16);
            const ptr0 = passStringToWasm0(word, wasm.__wbindgen_export, wasm.__wbindgen_export2);
            const len0 = WASM_VECTOR_LEN;
            wasm.mceengine_compound_split(retptr, this.__wbg_ptr, ptr0, len0);
            var r0 = getDataViewMemory0().getInt32(retptr + 4 * 0, true);
            var r1 = getDataViewMemory0().getInt32(retptr + 4 * 1, true);
            deferred2_0 = r0;
            deferred2_1 = r1;
            return getStringFromWasm0(r0, r1);
        } finally {
            wasm.__wbindgen_add_to_stack_pointer(16);
            wasm.__wbindgen_export3(deferred2_0, deferred2_1, 1);
        }
    }
    /**
     * Disambiguate a sentence and return full pipeline results as JSON.
     *
     * Full pipeline:
     * 1. Tokenize the text into all tokens (words, punctuation, etc.)
     * 2. Analyze each word with FinnishAnalyzer
     * 3. Disambiguate word tokens using ViterbiDisambiguator with emission scoring
     * 4. Return JSON with POS tags and baseforms, including punctuation tokens
     *
     * Example output:
     * ```json
     * [
     *   {"word":"Koira","type":"word","pos":"nimisana","baseform":"koira","attributes":{...}},
     *   {"word":".","type":"punctuation","pos":null,"baseform":null,"attributes":null}
     * ]
     * ```
     * @param {string} text
     * @returns {string}
     */
    disambiguate_sentence(text) {
        let deferred2_0;
        let deferred2_1;
        try {
            const retptr = wasm.__wbindgen_add_to_stack_pointer(-16);
            const ptr0 = passStringToWasm0(text, wasm.__wbindgen_export, wasm.__wbindgen_export2);
            const len0 = WASM_VECTOR_LEN;
            wasm.mceengine_disambiguate_sentence(retptr, this.__wbg_ptr, ptr0, len0);
            var r0 = getDataViewMemory0().getInt32(retptr + 4 * 0, true);
            var r1 = getDataViewMemory0().getInt32(retptr + 4 * 1, true);
            deferred2_0 = r0;
            deferred2_1 = r1;
            return getStringFromWasm0(r0, r1);
        } finally {
            wasm.__wbindgen_add_to_stack_pointer(16);
            wasm.__wbindgen_export3(deferred2_0, deferred2_1, 1);
        }
    }
    /**
     * Generate an inflected form from a baseform, case, and number.
     *
     * Uses the coKleisli morphophonological pipeline (consonant gradation,
     * vowel harmony, possessive suffix) to produce the surface form.
     *
     * The `case` parameter accepts both Finnish names (e.g., "omanto") and
     * English names (e.g., "genitive"). The `number` parameter is reserved
     * for future plural support (currently only singular is supported).
     *
     * Returns the generated form, or the baseform unchanged if the case
     * is not recognized.
     *
     * Example: `generate_form("kaappi", "genitive", "singular")` -> `"kaapin"`
     * @param {string} baseform
     * @param {string} _case
     * @param {string} _number
     * @returns {string}
     */
    generate_form(baseform, _case, _number) {
        let deferred4_0;
        let deferred4_1;
        try {
            const retptr = wasm.__wbindgen_add_to_stack_pointer(-16);
            const ptr0 = passStringToWasm0(baseform, wasm.__wbindgen_export, wasm.__wbindgen_export2);
            const len0 = WASM_VECTOR_LEN;
            const ptr1 = passStringToWasm0(_case, wasm.__wbindgen_export, wasm.__wbindgen_export2);
            const len1 = WASM_VECTOR_LEN;
            const ptr2 = passStringToWasm0(_number, wasm.__wbindgen_export, wasm.__wbindgen_export2);
            const len2 = WASM_VECTOR_LEN;
            wasm.mceengine_generate_form(retptr, this.__wbg_ptr, ptr0, len0, ptr1, len1, ptr2, len2);
            var r0 = getDataViewMemory0().getInt32(retptr + 4 * 0, true);
            var r1 = getDataViewMemory0().getInt32(retptr + 4 * 1, true);
            deferred4_0 = r0;
            deferred4_1 = r1;
            return getStringFromWasm0(r0, r1);
        } finally {
            wasm.__wbindgen_add_to_stack_pointer(16);
            wasm.__wbindgen_export3(deferred4_0, deferred4_1, 1);
        }
    }
    /**
     * Generate all singular case forms for a noun.
     *
     * Returns a JSON array of `{"case": "<name>", "form": "<inflected>"}` objects.
     *
     * Example output:
     * ```json
     * [
     *   {"case":"nominative","form":"talo"},
     *   {"case":"genitive","form":"talon"},
     *   {"case":"partitive","form":"taloa"},
     *   ...
     * ]
     * ```
     * @param {string} baseform
     * @returns {string}
     */
    generate_paradigm(baseform) {
        let deferred2_0;
        let deferred2_1;
        try {
            const retptr = wasm.__wbindgen_add_to_stack_pointer(-16);
            const ptr0 = passStringToWasm0(baseform, wasm.__wbindgen_export, wasm.__wbindgen_export2);
            const len0 = WASM_VECTOR_LEN;
            wasm.mceengine_generate_paradigm(retptr, this.__wbg_ptr, ptr0, len0);
            var r0 = getDataViewMemory0().getInt32(retptr + 4 * 0, true);
            var r1 = getDataViewMemory0().getInt32(retptr + 4 * 1, true);
            deferred2_0 = r0;
            deferred2_1 = r1;
            return getStringFromWasm0(r0, r1);
        } finally {
            wasm.__wbindgen_add_to_stack_pointer(16);
            wasm.__wbindgen_export3(deferred2_0, deferred2_1, 1);
        }
    }
    /**
     * Generate a conjugated verb form.
     *
     * Parameters:
     * - `baseform`: The verb infinitive (e.g., "puhua", "syödä").
     * - `tense`: "present", "past", or "conditional".
     * - `person`: "1sg", "2sg", "3sg", "1pl", "2pl", or "3pl".
     * - `polarity`: "affirmative" or "negative".
     *
     * Returns the conjugated form, or the baseform unchanged if parameters
     * are not recognized.
     *
     * Example: `generate_verb_form("puhua", "present", "1sg", "affirmative")` -> `"puhun"`
     * @param {string} baseform
     * @param {string} tense
     * @param {string} person
     * @param {string} polarity
     * @returns {string}
     */
    generate_verb_form(baseform, tense, person, polarity) {
        let deferred5_0;
        let deferred5_1;
        try {
            const retptr = wasm.__wbindgen_add_to_stack_pointer(-16);
            const ptr0 = passStringToWasm0(baseform, wasm.__wbindgen_export, wasm.__wbindgen_export2);
            const len0 = WASM_VECTOR_LEN;
            const ptr1 = passStringToWasm0(tense, wasm.__wbindgen_export, wasm.__wbindgen_export2);
            const len1 = WASM_VECTOR_LEN;
            const ptr2 = passStringToWasm0(person, wasm.__wbindgen_export, wasm.__wbindgen_export2);
            const len2 = WASM_VECTOR_LEN;
            const ptr3 = passStringToWasm0(polarity, wasm.__wbindgen_export, wasm.__wbindgen_export2);
            const len3 = WASM_VECTOR_LEN;
            wasm.mceengine_generate_verb_form(retptr, this.__wbg_ptr, ptr0, len0, ptr1, len1, ptr2, len2, ptr3, len3);
            var r0 = getDataViewMemory0().getInt32(retptr + 4 * 0, true);
            var r1 = getDataViewMemory0().getInt32(retptr + 4 * 1, true);
            deferred5_0 = r0;
            deferred5_1 = r1;
            return getStringFromWasm0(r0, r1);
        } finally {
            wasm.__wbindgen_add_to_stack_pointer(16);
            wasm.__wbindgen_export3(deferred5_0, deferred5_1, 1);
        }
    }
    /**
     * Generate all conjugated forms for a verb.
     *
     * Returns a JSON array of `{"label": "<tense person>", "form": "<conjugated>"}` objects.
     * Includes present, past, conditional, and negative present tenses.
     *
     * Returns `"[]"` if the verb infinitive is not recognized.
     *
     * Example: `generate_verb_paradigm("puhua")` returns a JSON array with
     * entries like `{"label":"present 1sg","form":"puhun"}`.
     * @param {string} baseform
     * @returns {string}
     */
    generate_verb_paradigm(baseform) {
        let deferred2_0;
        let deferred2_1;
        try {
            const retptr = wasm.__wbindgen_add_to_stack_pointer(-16);
            const ptr0 = passStringToWasm0(baseform, wasm.__wbindgen_export, wasm.__wbindgen_export2);
            const len0 = WASM_VECTOR_LEN;
            wasm.mceengine_generate_verb_paradigm(retptr, this.__wbg_ptr, ptr0, len0);
            var r0 = getDataViewMemory0().getInt32(retptr + 4 * 0, true);
            var r1 = getDataViewMemory0().getInt32(retptr + 4 * 1, true);
            deferred2_0 = r0;
            deferred2_1 = r1;
            return getStringFromWasm0(r0, r1);
        } finally {
            wasm.__wbindgen_add_to_stack_pointer(16);
            wasm.__wbindgen_export3(deferred2_0, deferred2_1, 1);
        }
    }
    /**
     * Quick baseform lookup for a word.
     *
     * Analyzes the word, disambiguates with a single-word context, and
     * returns the most likely baseform. Returns the word itself if no
     * analysis is found.
     *
     * Example: `get_baseform("koirien")` -> `"koira"`
     * @param {string} word
     * @returns {string}
     */
    get_baseform(word) {
        let deferred2_0;
        let deferred2_1;
        try {
            const retptr = wasm.__wbindgen_add_to_stack_pointer(-16);
            const ptr0 = passStringToWasm0(word, wasm.__wbindgen_export, wasm.__wbindgen_export2);
            const len0 = WASM_VECTOR_LEN;
            wasm.mceengine_get_baseform(retptr, this.__wbg_ptr, ptr0, len0);
            var r0 = getDataViewMemory0().getInt32(retptr + 4 * 0, true);
            var r1 = getDataViewMemory0().getInt32(retptr + 4 * 1, true);
            deferred2_0 = r0;
            deferred2_1 = r1;
            return getStringFromWasm0(r0, r1);
        } finally {
            wasm.__wbindgen_add_to_stack_pointer(16);
            wasm.__wbindgen_export3(deferred2_0, deferred2_1, 1);
        }
    }
    /**
     * Check grammar of Finnish text. Returns JSON array of errors.
     *
     * Each error object contains:
     * - `start`: Byte offset of the error start in the original text
     * - `end`: Byte offset of the error end
     * - `code`: Error code (e.g., "REPEATED_WORD", "CAPITALIZATION_ERROR", "AGREEMENT_ERROR")
     * - `message`: Human-readable error description
     * - `suggestions`: Array of suggested corrections (may be empty)
     *
     * Example output:
     * ```json
     * [
     *   {"start":6,"end":11,"code":"REPEATED_WORD","message":"Repeated word: koira","suggestions":["koira"]}
     * ]
     * ```
     * @param {string} text
     * @returns {string}
     */
    grammar_check(text) {
        let deferred2_0;
        let deferred2_1;
        try {
            const retptr = wasm.__wbindgen_add_to_stack_pointer(-16);
            const ptr0 = passStringToWasm0(text, wasm.__wbindgen_export, wasm.__wbindgen_export2);
            const len0 = WASM_VECTOR_LEN;
            wasm.mceengine_grammar_check(retptr, this.__wbg_ptr, ptr0, len0);
            var r0 = getDataViewMemory0().getInt32(retptr + 4 * 0, true);
            var r1 = getDataViewMemory0().getInt32(retptr + 4 * 1, true);
            deferred2_0 = r0;
            deferred2_1 = r1;
            return getStringFromWasm0(r0, r1);
        } finally {
            wasm.__wbindgen_add_to_stack_pointer(16);
            wasm.__wbindgen_export3(deferred2_0, deferred2_1, 1);
        }
    }
    /**
     * Check whether a suffix tagger model has been loaded.
     * @returns {boolean}
     */
    has_model() {
        const ret = wasm.mceengine_has_model(this.__wbg_ptr);
        return ret !== 0;
    }
    /**
     * Check whether a wordlist (M1 Succinct Trie) has been loaded.
     * @returns {boolean}
     */
    has_wordlist() {
        const ret = wasm.mceengine_has_wordlist(this.__wbg_ptr);
        return ret !== 0;
    }
    /**
     * Hyphenate a Finnish word. Returns the word with hyphens inserted.
     *
     * Uses rule-based Finnish syllabification to find valid break points.
     * No dictionary lookup is required.
     *
     * Example: `hyphenate("suomalainen")` -> `"suo-ma-lai-nen"`
     * @param {string} word
     * @returns {string}
     */
    hyphenate(word) {
        let deferred2_0;
        let deferred2_1;
        try {
            const retptr = wasm.__wbindgen_add_to_stack_pointer(-16);
            const ptr0 = passStringToWasm0(word, wasm.__wbindgen_export, wasm.__wbindgen_export2);
            const len0 = WASM_VECTOR_LEN;
            wasm.mceengine_hyphenate(retptr, this.__wbg_ptr, ptr0, len0);
            var r0 = getDataViewMemory0().getInt32(retptr + 4 * 0, true);
            var r1 = getDataViewMemory0().getInt32(retptr + 4 * 1, true);
            deferred2_0 = r0;
            deferred2_1 = r1;
            return getStringFromWasm0(r0, r1);
        } finally {
            wasm.__wbindgen_add_to_stack_pointer(16);
            wasm.__wbindgen_export3(deferred2_0, deferred2_1, 1);
        }
    }
    /**
     * Hyphenate all words in text. Returns text with hyphens at valid break points.
     *
     * Tokenizes the text, hyphenates each word token, and reassembles the text
     * preserving all non-word tokens (whitespace, punctuation) as-is.
     *
     * Example: `hyphenate_text("Koira juoksee.")` -> `"Koi-ra juok-see."`
     * @param {string} text
     * @returns {string}
     */
    hyphenate_text(text) {
        let deferred2_0;
        let deferred2_1;
        try {
            const retptr = wasm.__wbindgen_add_to_stack_pointer(-16);
            const ptr0 = passStringToWasm0(text, wasm.__wbindgen_export, wasm.__wbindgen_export2);
            const len0 = WASM_VECTOR_LEN;
            wasm.mceengine_hyphenate_text(retptr, this.__wbg_ptr, ptr0, len0);
            var r0 = getDataViewMemory0().getInt32(retptr + 4 * 0, true);
            var r1 = getDataViewMemory0().getInt32(retptr + 4 * 1, true);
            deferred2_0 = r0;
            deferred2_1 = r1;
            return getStringFromWasm0(r0, r1);
        } finally {
            wasm.__wbindgen_add_to_stack_pointer(16);
            wasm.__wbindgen_export3(deferred2_0, deferred2_1, 1);
        }
    }
    /**
     * Check if a word has a valid morphological analysis in the VFST dictionary.
     *
     * Pure linguistic check: returns `true` only if the FST-based morphological
     * analyzer produces at least one analysis. Unlike [`spell_check`](Self::spell_check),
     * this does **not** attempt compound splitting or other recovery strategies.
     * @param {string} word
     * @returns {boolean}
     */
    is_valid_word(word) {
        const ptr0 = passStringToWasm0(word, wasm.__wbindgen_export, wasm.__wbindgen_export2);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.mceengine_is_valid_word(this.__wbg_ptr, ptr0, len0);
        return ret !== 0;
    }
    /**
     * Load the engine from VFST dictionary bytes (mor.vfst).
     *
     * # Errors
     *
     * Returns a `JsValue` error if the VFST data is malformed.
     * @param {Uint8Array} mor_vfst
     * @returns {MceEngine}
     */
    static load(mor_vfst) {
        try {
            const retptr = wasm.__wbindgen_add_to_stack_pointer(-16);
            const ptr0 = passArray8ToWasm0(mor_vfst, wasm.__wbindgen_export);
            const len0 = WASM_VECTOR_LEN;
            wasm.mceengine_load(retptr, ptr0, len0);
            var r0 = getDataViewMemory0().getInt32(retptr + 4 * 0, true);
            var r1 = getDataViewMemory0().getInt32(retptr + 4 * 1, true);
            var r2 = getDataViewMemory0().getInt32(retptr + 4 * 2, true);
            if (r2) {
                throw takeObject(r1);
            }
            return MceEngine.__wrap(r0);
        } finally {
            wasm.__wbindgen_add_to_stack_pointer(16);
        }
    }
    /**
     * Load a suffix tagger model for improved POS disambiguation.
     *
     * The model is a binary MCET file (~5MB) trained offline. When loaded,
     * `analyze_sentence()` and `disambiguate_sentence()` use it for
     * emission scoring, boosting UPOS accuracy from ~83% to ~95%.
     *
     * # Errors
     *
     * Returns a `JsValue` error if the model data is malformed.
     * @param {Uint8Array} data
     */
    load_model(data) {
        try {
            const retptr = wasm.__wbindgen_add_to_stack_pointer(-16);
            const ptr0 = passArray8ToWasm0(data, wasm.__wbindgen_export);
            const len0 = WASM_VECTOR_LEN;
            wasm.mceengine_load_model(retptr, this.__wbg_ptr, ptr0, len0);
            var r0 = getDataViewMemory0().getInt32(retptr + 4 * 0, true);
            var r1 = getDataViewMemory0().getInt32(retptr + 4 * 1, true);
            if (r1) {
                throw takeObject(r0);
            }
        } finally {
            wasm.__wbindgen_add_to_stack_pointer(16);
        }
    }
    /**
     * Load a wordlist for dictionary-based spell checking and suggestion generation.
     *
     * Accepts one of two formats:
     * - **Text wordlist**: one word per line, UTF-8. Words are sorted and
     *   deduplicated internally. Lines starting with `#` are skipped.
     * - **TSV (lemma_dict.tsv)**: tab-separated `word\tUPOS\tlemma`. Extracts
     *   column 1 (word forms) and column 3 (lemmas), deduplicates, and builds
     *   a trie from the combined set.
     *
     * The format is auto-detected: if any line contains a tab character, TSV
     * parsing is used; otherwise plain one-word-per-line.
     *
     * When loaded, `suggest()` uses the trie's fuzzy search (Levenshtein
     * automaton) for fast candidate generation instead of brute-force
     * character-level edits.
     *
     * # Errors
     *
     * Returns a `JsValue` error if the data is not valid UTF-8.
     * @param {Uint8Array} data
     */
    load_wordlist(data) {
        try {
            const retptr = wasm.__wbindgen_add_to_stack_pointer(-16);
            const ptr0 = passArray8ToWasm0(data, wasm.__wbindgen_export);
            const len0 = WASM_VECTOR_LEN;
            wasm.mceengine_load_wordlist(retptr, this.__wbg_ptr, ptr0, len0);
            var r0 = getDataViewMemory0().getInt32(retptr + 4 * 0, true);
            var r1 = getDataViewMemory0().getInt32(retptr + 4 * 1, true);
            if (r1) {
                throw takeObject(r0);
            }
        } finally {
            wasm.__wbindgen_add_to_stack_pointer(16);
        }
    }
    /**
     * Check spelling of a word.
     *
     * Returns `true` if the word is correctly spelled Finnish. Uses a
     * multi-stage pipeline:
     * 1. Morphological analysis via VFST (handles inflections, derivations)
     * 2. Compound-aware check (splits the word and validates each part)
     *
     * This is more permissive than [`is_valid_word`](Self::is_valid_word),
     * which only checks stage 1. For example, novel compound words that
     * the FST doesn't recognize as a single entry may still pass the
     * compound check.
     * @param {string} word
     * @returns {boolean}
     */
    spell_check(word) {
        const ptr0 = passStringToWasm0(word, wasm.__wbindgen_export, wasm.__wbindgen_export2);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.mceengine_spell_check(this.__wbg_ptr, ptr0, len0);
        return ret !== 0;
    }
    /**
     * Generate spelling suggestions for a word.
     *
     * Uses FinnishAnalyzer to check if the word is valid. If valid, returns
     * an empty array. If invalid, generates candidates via:
     * - **With wordlist**: M1 Succinct Trie fuzzy search (Levenshtein automaton)
     *   for fast candidate generation, filtered through the morphological analyzer.
     * - **Without wordlist**: Falls back to `suggest_with_context()` using
     *   brute-force character-level edit generation.
     *
     * # Arguments
     *
     * * `word` - The word to check / suggest for.
     * * `max_edits` - Maximum edit distance for fuzzy search.
     *
     * Example output:
     * ```json
     * ["koira","koiru"]
     * ```
     * @param {string} word
     * @param {number} max_edits
     * @returns {string}
     */
    suggest(word, max_edits) {
        let deferred2_0;
        let deferred2_1;
        try {
            const retptr = wasm.__wbindgen_add_to_stack_pointer(-16);
            const ptr0 = passStringToWasm0(word, wasm.__wbindgen_export, wasm.__wbindgen_export2);
            const len0 = WASM_VECTOR_LEN;
            wasm.mceengine_suggest(retptr, this.__wbg_ptr, ptr0, len0, max_edits);
            var r0 = getDataViewMemory0().getInt32(retptr + 4 * 0, true);
            var r1 = getDataViewMemory0().getInt32(retptr + 4 * 1, true);
            deferred2_0 = r0;
            deferred2_1 = r1;
            return getStringFromWasm0(r0, r1);
        } finally {
            wasm.__wbindgen_add_to_stack_pointer(16);
            wasm.__wbindgen_export3(deferred2_0, deferred2_1, 1);
        }
    }
    /**
     * Generate context-aware spelling suggestions.
     *
     * Uses the previous word (if available) to rank suggestions by POS
     * bigram probability. Returns a JSON array of up to 5 suggestions.
     *
     * # Arguments
     *
     * * `word` - The misspelled word.
     * * `prev_word` - The previous word in context (or empty string for none).
     * * `max_edits` - Maximum edit distance for fuzzy search.
     *
     * Example output:
     * ```json
     * ["koira","koiru","kaira"]
     * ```
     * @param {string} word
     * @param {string} prev_word
     * @param {number} max_edits
     * @returns {string}
     */
    suggest_with_context(word, prev_word, max_edits) {
        let deferred3_0;
        let deferred3_1;
        try {
            const retptr = wasm.__wbindgen_add_to_stack_pointer(-16);
            const ptr0 = passStringToWasm0(word, wasm.__wbindgen_export, wasm.__wbindgen_export2);
            const len0 = WASM_VECTOR_LEN;
            const ptr1 = passStringToWasm0(prev_word, wasm.__wbindgen_export, wasm.__wbindgen_export2);
            const len1 = WASM_VECTOR_LEN;
            wasm.mceengine_suggest_with_context(retptr, this.__wbg_ptr, ptr0, len0, ptr1, len1, max_edits);
            var r0 = getDataViewMemory0().getInt32(retptr + 4 * 0, true);
            var r1 = getDataViewMemory0().getInt32(retptr + 4 * 1, true);
            deferred3_0 = r0;
            deferred3_1 = r1;
            return getStringFromWasm0(r0, r1);
        } finally {
            wasm.__wbindgen_add_to_stack_pointer(16);
            wasm.__wbindgen_export3(deferred3_0, deferred3_1, 1);
        }
    }
    /**
     * Return the MCE engine version string.
     * @returns {string}
     */
    static version() {
        let deferred1_0;
        let deferred1_1;
        try {
            const retptr = wasm.__wbindgen_add_to_stack_pointer(-16);
            wasm.mceengine_version(retptr);
            var r0 = getDataViewMemory0().getInt32(retptr + 4 * 0, true);
            var r1 = getDataViewMemory0().getInt32(retptr + 4 * 1, true);
            deferred1_0 = r0;
            deferred1_1 = r1;
            return getStringFromWasm0(r0, r1);
        } finally {
            wasm.__wbindgen_add_to_stack_pointer(16);
            wasm.__wbindgen_export3(deferred1_0, deferred1_1, 1);
        }
    }
}
if (Symbol.dispose) MceEngine.prototype[Symbol.dispose] = MceEngine.prototype.free;

function __wbg_get_imports() {
    const import0 = {
        __proto__: null,
        __wbg___wbindgen_throw_6ddd609b62940d55: function(arg0, arg1) {
            throw new Error(getStringFromWasm0(arg0, arg1));
        },
        __wbindgen_cast_0000000000000001: function(arg0, arg1) {
            // Cast intrinsic for `Ref(String) -> Externref`.
            const ret = getStringFromWasm0(arg0, arg1);
            return addHeapObject(ret);
        },
    };
    return {
        __proto__: null,
        "./mce_wasm_bg.js": import0,
    };
}

const MceEngineFinalization = (typeof FinalizationRegistry === 'undefined')
    ? { register: () => {}, unregister: () => {} }
    : new FinalizationRegistry(ptr => wasm.__wbg_mceengine_free(ptr >>> 0, 1));

function addHeapObject(obj) {
    if (heap_next === heap.length) heap.push(heap.length + 1);
    const idx = heap_next;
    heap_next = heap[idx];

    heap[idx] = obj;
    return idx;
}

function dropObject(idx) {
    if (idx < 1028) return;
    heap[idx] = heap_next;
    heap_next = idx;
}

let cachedDataViewMemory0 = null;
function getDataViewMemory0() {
    if (cachedDataViewMemory0 === null || cachedDataViewMemory0.buffer.detached === true || (cachedDataViewMemory0.buffer.detached === undefined && cachedDataViewMemory0.buffer !== wasm.memory.buffer)) {
        cachedDataViewMemory0 = new DataView(wasm.memory.buffer);
    }
    return cachedDataViewMemory0;
}

function getStringFromWasm0(ptr, len) {
    ptr = ptr >>> 0;
    return decodeText(ptr, len);
}

let cachedUint8ArrayMemory0 = null;
function getUint8ArrayMemory0() {
    if (cachedUint8ArrayMemory0 === null || cachedUint8ArrayMemory0.byteLength === 0) {
        cachedUint8ArrayMemory0 = new Uint8Array(wasm.memory.buffer);
    }
    return cachedUint8ArrayMemory0;
}

function getObject(idx) { return heap[idx]; }

let heap = new Array(1024).fill(undefined);
heap.push(undefined, null, true, false);

let heap_next = heap.length;

function passArray8ToWasm0(arg, malloc) {
    const ptr = malloc(arg.length * 1, 1) >>> 0;
    getUint8ArrayMemory0().set(arg, ptr / 1);
    WASM_VECTOR_LEN = arg.length;
    return ptr;
}

function passStringToWasm0(arg, malloc, realloc) {
    if (realloc === undefined) {
        const buf = cachedTextEncoder.encode(arg);
        const ptr = malloc(buf.length, 1) >>> 0;
        getUint8ArrayMemory0().subarray(ptr, ptr + buf.length).set(buf);
        WASM_VECTOR_LEN = buf.length;
        return ptr;
    }

    let len = arg.length;
    let ptr = malloc(len, 1) >>> 0;

    const mem = getUint8ArrayMemory0();

    let offset = 0;

    for (; offset < len; offset++) {
        const code = arg.charCodeAt(offset);
        if (code > 0x7F) break;
        mem[ptr + offset] = code;
    }
    if (offset !== len) {
        if (offset !== 0) {
            arg = arg.slice(offset);
        }
        ptr = realloc(ptr, len, len = offset + arg.length * 3, 1) >>> 0;
        const view = getUint8ArrayMemory0().subarray(ptr + offset, ptr + len);
        const ret = cachedTextEncoder.encodeInto(arg, view);

        offset += ret.written;
        ptr = realloc(ptr, len, offset, 1) >>> 0;
    }

    WASM_VECTOR_LEN = offset;
    return ptr;
}

function takeObject(idx) {
    const ret = getObject(idx);
    dropObject(idx);
    return ret;
}

let cachedTextDecoder = new TextDecoder('utf-8', { ignoreBOM: true, fatal: true });
cachedTextDecoder.decode();
const MAX_SAFARI_DECODE_BYTES = 2146435072;
let numBytesDecoded = 0;
function decodeText(ptr, len) {
    numBytesDecoded += len;
    if (numBytesDecoded >= MAX_SAFARI_DECODE_BYTES) {
        cachedTextDecoder = new TextDecoder('utf-8', { ignoreBOM: true, fatal: true });
        cachedTextDecoder.decode();
        numBytesDecoded = len;
    }
    return cachedTextDecoder.decode(getUint8ArrayMemory0().subarray(ptr, ptr + len));
}

const cachedTextEncoder = new TextEncoder();

if (!('encodeInto' in cachedTextEncoder)) {
    cachedTextEncoder.encodeInto = function (arg, view) {
        const buf = cachedTextEncoder.encode(arg);
        view.set(buf);
        return {
            read: arg.length,
            written: buf.length
        };
    };
}

let WASM_VECTOR_LEN = 0;

let wasmModule, wasm;
function __wbg_finalize_init(instance, module) {
    wasm = instance.exports;
    wasmModule = module;
    cachedDataViewMemory0 = null;
    cachedUint8ArrayMemory0 = null;
    return wasm;
}

async function __wbg_load(module, imports) {
    if (typeof Response === 'function' && module instanceof Response) {
        if (typeof WebAssembly.instantiateStreaming === 'function') {
            try {
                return await WebAssembly.instantiateStreaming(module, imports);
            } catch (e) {
                const validResponse = module.ok && expectedResponseType(module.type);

                if (validResponse && module.headers.get('Content-Type') !== 'application/wasm') {
                    console.warn("`WebAssembly.instantiateStreaming` failed because your server does not serve Wasm with `application/wasm` MIME type. Falling back to `WebAssembly.instantiate` which is slower. Original error:\n", e);

                } else { throw e; }
            }
        }

        const bytes = await module.arrayBuffer();
        return await WebAssembly.instantiate(bytes, imports);
    } else {
        const instance = await WebAssembly.instantiate(module, imports);

        if (instance instanceof WebAssembly.Instance) {
            return { instance, module };
        } else {
            return instance;
        }
    }

    function expectedResponseType(type) {
        switch (type) {
            case 'basic': case 'cors': case 'default': return true;
        }
        return false;
    }
}

function initSync(module) {
    if (wasm !== undefined) return wasm;


    if (module !== undefined) {
        if (Object.getPrototypeOf(module) === Object.prototype) {
            ({module} = module)
        } else {
            console.warn('using deprecated parameters for `initSync()`; pass a single object instead')
        }
    }

    const imports = __wbg_get_imports();
    if (!(module instanceof WebAssembly.Module)) {
        module = new WebAssembly.Module(module);
    }
    const instance = new WebAssembly.Instance(module, imports);
    return __wbg_finalize_init(instance, module);
}

async function __wbg_init(module_or_path) {
    if (wasm !== undefined) return wasm;


    if (module_or_path !== undefined) {
        if (Object.getPrototypeOf(module_or_path) === Object.prototype) {
            ({module_or_path} = module_or_path)
        } else {
            console.warn('using deprecated parameters for the initialization function; pass a single object instead')
        }
    }

    if (module_or_path === undefined) {
        module_or_path = new URL('mce_wasm_bg.wasm', import.meta.url);
    }
    const imports = __wbg_get_imports();

    if (typeof module_or_path === 'string' || (typeof Request === 'function' && module_or_path instanceof Request) || (typeof URL === 'function' && module_or_path instanceof URL)) {
        module_or_path = fetch(module_or_path);
    }

    const { instance, module } = await __wbg_load(await module_or_path, imports);

    return __wbg_finalize_init(instance, module);
}

export { initSync, __wbg_init as default };
