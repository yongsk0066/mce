//! POS tag mapping between MCE Finnish word classes and UD UPOS tags.
//!
//! MCE's Finnish analyzer produces CLASS values from the Voikko FST output
//! (e.g., `nimisana`, `teonsana`). This module maps them to Universal
//! Dependencies UPOS tags for evaluation.

use mce_core::analysis::{
    Analysis, ATTR_BASEFORM, ATTR_CLASS, ATTR_PARTICIPLE, ATTR_POSSIBLE_GEOGRAPHICAL_NAME,
    ATTR_SIJAMUOTO,
};

/// Map an MCE Analysis to a UD UPOS tag.
///
/// Uses the `CLASS` attribute from the analysis, with special handling for:
/// - `etunimi`, `sukunimi`, `paikannimi` -> PROPN
/// - `kieltosana` (negation verb `ei`) -> AUX
/// - `nimisana_laatusana` -> NOUN (default; context-dependent in practice)
/// - `sidesana` -> CCONJ (default; could also be SCONJ for subordinating)
/// - Words with `POSSIBLE_GEOGRAPHICAL_NAME` flag -> PROPN
///
/// Returns `"X"` for unknown or missing classes.
pub fn mce_class_to_upos(analysis: &Analysis) -> &'static str {
    let class = match analysis.get(ATTR_CLASS) {
        Some(c) => c,
        None => return "X",
    };

    // Check geographical name flag — overrides to PROPN.
    if analysis.get(ATTR_POSSIBLE_GEOGRAPHICAL_NAME) == Some("true") {
        return "PROPN";
    }

    match class {
        // Nominals
        "nimisana" => {
            // Some words classified as nimisana by the Voikko FST are
            // actually pronouns in UD Finnish-TDT (e.g., kaikki, itse, sama).
            if let Some(baseform) = analysis.get(ATTR_BASEFORM) {
                if PRONOUN_OVERRIDES.contains(&baseform) {
                    return "PRON";
                }
            }
            "NOUN"
        }
        "laatusana" => {
            // Finnish manner adverbs in -sti (instructive case of adjectives)
            // are classified as laatusana + SIJAMUOTO=kerrontosti by the FST.
            // In UD Finnish-TDT, these are always ADV, not ADJ.
            if analysis.get(ATTR_SIJAMUOTO) == Some("kerrontosti") {
                return "ADV";
            }
            // If this was originally a verb form (participle), map to VERB.
            // post_process_attributes changes past_passive participles to
            // "laatusana", but the PARTICIPLE attribute is preserved.
            // In UD Finnish-TDT, most participial forms are tagged VERB
            // (compound tenses) rather than ADJ (attributive).
            if analysis.contains_key(ATTR_PARTICIPLE) {
                return "VERB";
            }
            "ADJ"
        }
        "asemosana" => "PRON",
        "lukusana" => "NUM",

        // Proper nouns
        "etunimi" => "PROPN",
        "sukunimi" => "PROPN",
        "paikannimi" => "PROPN",

        // Verbs
        "teonsana" => "VERB",
        "kieltosana" => "AUX", // Finnish negation verb (ei, en, et, ...)

        // Indeclinables
        "seikkasana" => "ADV",
        "suhdesana" => "ADP",
        "sidesana" => "CCONJ", // Default; SCONJ requires syntax context
        "huudahdussana" => "INTJ",

        // Compound / ambiguous classes
        "nimisana_laatusana" => "NOUN", // Default: NOUN (ADJ also possible)

        // Abbreviations
        "lyhenne" => "NOUN", // Default for abbreviations

        // Unknown
        _ => "X",
    }
}

/// Refine CCONJ vs SCONJ mapping using the surface form.
///
/// In Finnish UD treebanks, subordinating conjunctions (`että`, `kun`, `jos`,
/// `koska`, `vaikka`, `jotta`, `kunnes`, `ennen kuin`, etc.) are tagged as
/// SCONJ, while coordinating conjunctions (`ja`, `tai`, `vai`, `mutta`, `eli`,
/// `sekä`, `eikä`, etc.) are tagged as CCONJ.
///
/// Since the MCE FST tags both as `sidesana`, we refine using the surface form.
pub fn refine_conjunction(surface: &str) -> &'static str {
    let lower: String = surface
        .chars()
        .map(|c| c.to_lowercase().next().unwrap_or(c))
        .collect();

    match lower.as_str() {
        // Coordinating conjunctions (CCONJ)
        // "vaan" is adversative CCONJ in UD Finnish-TDT ("ei X vaan Y").
        "ja" | "tai" | "vai" | "mutta" | "eli" | "sekä" | "eikä" | "saati" | "taikka" | "ynnä"
        | "vaan" => "CCONJ",

        // Subordinating conjunctions (SCONJ)
        "että" | "kun" | "jos" | "koska" | "vaikka" | "jotta" | "kunnes" | "mikäli" | "ellei"
        | "ettei" | "joskin" | "jollei" | "jollen" | "jollet" | "jolloin" | "kohan" | "sillä"
        | "johon" | "vaikkei" | "vaikkakaan" | "kuin" => "SCONJ",

        // Default: keep CCONJ
        _ => "CCONJ",
    }
}

/// Finnish auxiliary verbs (olla, voida, saattaa, täytyä, pitää, etc.).
///
/// In UD Finnish-TDT, these are tagged as AUX when used as auxiliaries.
/// The MCE FST tags them all as `teonsana`.
const FINNISH_AUX_LEMMAS: &[&str] = &[
    "olla",
    "voida",
    "saattaa",
    "täytyä",
    "pitää",
    "joutua",
    "mahtaa",
    "taitaa",
    "aikoa",
    "tarvita",
    "ehtiä",
    "kannattaa",
    "kelvata",
];

/// Finnish determiners that MCE tags as `asemosana` but UD tags as DET.
///
/// NOTE: The PRON/DET distinction in UD requires syntactic analysis
/// (DET when modifying a noun, PRON when standalone). Without dependency
/// parsing, we cannot reliably distinguish them. We keep this list
/// intentionally EMPTY and map all asemosana → PRON for now.
/// A future integration with M3/M4' could use head-noun detection.
const FINNISH_DET_FORMS: &[&str] = &[];

/// Words that the Voikko FST classifies as `nimisana` (or `lukusana`) but
/// that UD Finnish-TDT tags as PRON.
///
/// The Voikko FST was designed for spell-checking, not UD POS tagging.
/// In Voikko's classification, `kaikki` (all) is a nominal quantifier
/// (`nimisana`), not a pronoun (`asemosana`). We override to match UD.
const PRONOUN_OVERRIDES: &[&str] = &[
    "kaikki", "itse", "sama", "toinen", "moni", "harva", "kumpikin", "molemmat", "ainoa",
];

/// Finnish particles that MCE might tag as `seikkasana` but UD tags as PART.
pub const FINNISH_PARTICLES: &[&str] = &[
    "myös", "vain", "kin", "kaan", "jo", "vielä", "edes", "kyllä", "kai", "nyt", "niin", "ihan",
    "aivan", "-ko", "-kö", "-han", "-hän", "-pa", "-pä",
];

/// Finnish particle baseforms — DISABLED for UD Finnish-TDT evaluation.
///
/// UD Finnish-TDT does NOT use the PART tag. All words that were previously
/// mapped to PART (myös, vain, jo, nyt, etc.) are tagged ADV in the gold
/// standard. This list is retained for reference but is no longer used in
/// the mapping logic.
///
/// If evaluating against a treebank that uses PART, re-enable this list in
/// the `mce_to_upos` function's ADV arm.
#[allow(dead_code)]
const FINNISH_PARTICLE_BASEFORMS: &[&str] = &[
    "myös", "vain", "jo", "edes", "kyllä", "nyt", "ihan", "aivan", "vasta", "asti", "saakka",
    "mukaan",
];

/// Map an MCE Analysis to a refined UD UPOS tag, using surface form for disambiguation.
///
/// This combines [`mce_class_to_upos`] with surface-form refinements for cases
/// where the MCE class is ambiguous (e.g., `sidesana` -> CCONJ or SCONJ,
/// `teonsana` -> VERB or AUX, `asemosana` -> PRON or DET,
/// `seikkasana` -> ADV or PART).
pub fn mce_to_upos(analysis: &Analysis, surface: &str) -> &'static str {
    let base = mce_class_to_upos(analysis);

    match base {
        "CCONJ" => refine_conjunction(surface),
        "VERB" => {
            // Check if this verb is an auxiliary based on its baseform.
            if let Some(baseform) = analysis.get(ATTR_BASEFORM) {
                if FINNISH_AUX_LEMMAS.contains(&baseform) {
                    return "AUX";
                }
            }
            "VERB"
        }
        "PRON" => {
            // Some pronouns are actually determiners in UD.
            if let Some(baseform) = analysis.get(ATTR_BASEFORM) {
                let lower: String = baseform
                    .chars()
                    .map(|c| c.to_lowercase().next().unwrap_or(c))
                    .collect();
                if FINNISH_DET_FORMS.contains(&lower.as_str()) {
                    return "DET";
                }
            }
            // Also check surface form
            let lower: String = surface
                .chars()
                .map(|c| c.to_lowercase().next().unwrap_or(c))
                .collect();
            if FINNISH_DET_FORMS.contains(&lower.as_str()) {
                return "DET";
            }
            "PRON"
        }
        "ADV" => {
            // NOTE: UD Finnish-TDT does not use the PART tag.
            // All words previously mapped to PART (myös, vain, jo, nyt, etc.)
            // are tagged ADV in the gold standard. We keep them as ADV.
            "ADV"
        }
        other => other,
    }
}

/// All valid UD UPOS tags.
pub const ALL_UPOS: &[&str] = &[
    "ADJ", "ADP", "ADV", "AUX", "CCONJ", "DET", "INTJ", "NOUN", "NUM", "PART", "PRON", "PROPN",
    "PUNCT", "SCONJ", "SYM", "VERB", "X",
];

#[cfg(test)]
mod tests {
    use super::*;

    fn make_analysis(class: &str) -> Analysis {
        let mut a = Analysis::new();
        a.set(ATTR_CLASS, class);
        a
    }

    #[test]
    fn basic_mappings() {
        assert_eq!(mce_class_to_upos(&make_analysis("nimisana")), "NOUN");
        assert_eq!(mce_class_to_upos(&make_analysis("teonsana")), "VERB");
        assert_eq!(mce_class_to_upos(&make_analysis("laatusana")), "ADJ");
        assert_eq!(mce_class_to_upos(&make_analysis("seikkasana")), "ADV");
        assert_eq!(mce_class_to_upos(&make_analysis("lukusana")), "NUM");
        assert_eq!(mce_class_to_upos(&make_analysis("asemosana")), "PRON");
        assert_eq!(mce_class_to_upos(&make_analysis("suhdesana")), "ADP");
        assert_eq!(mce_class_to_upos(&make_analysis("sidesana")), "CCONJ");
        assert_eq!(mce_class_to_upos(&make_analysis("huudahdussana")), "INTJ");
        assert_eq!(mce_class_to_upos(&make_analysis("kieltosana")), "AUX");
    }

    #[test]
    fn proper_nouns() {
        assert_eq!(mce_class_to_upos(&make_analysis("etunimi")), "PROPN");
        assert_eq!(mce_class_to_upos(&make_analysis("sukunimi")), "PROPN");
        assert_eq!(mce_class_to_upos(&make_analysis("paikannimi")), "PROPN");
    }

    #[test]
    fn geographical_name_override() {
        let mut a = make_analysis("nimisana");
        a.set(ATTR_POSSIBLE_GEOGRAPHICAL_NAME, "true");
        assert_eq!(mce_class_to_upos(&a), "PROPN");
    }

    #[test]
    fn unknown_class() {
        assert_eq!(mce_class_to_upos(&make_analysis("something_else")), "X");
        assert_eq!(mce_class_to_upos(&Analysis::new()), "X");
    }

    #[test]
    fn conjunction_refinement() {
        assert_eq!(refine_conjunction("ja"), "CCONJ");
        assert_eq!(refine_conjunction("tai"), "CCONJ");
        assert_eq!(refine_conjunction("mutta"), "CCONJ");
        assert_eq!(refine_conjunction("että"), "SCONJ");
        assert_eq!(refine_conjunction("kun"), "SCONJ");
        assert_eq!(refine_conjunction("koska"), "SCONJ");
        assert_eq!(refine_conjunction("jos"), "SCONJ");
    }

    #[test]
    fn mce_to_upos_conjunction() {
        let a = make_analysis("sidesana");
        assert_eq!(mce_to_upos(&a, "ja"), "CCONJ");
        assert_eq!(mce_to_upos(&a, "että"), "SCONJ");
        assert_eq!(mce_to_upos(&a, "koska"), "SCONJ");
    }

    #[test]
    fn mce_to_upos_non_conjunction() {
        let a = make_analysis("nimisana");
        assert_eq!(mce_to_upos(&a, "koira"), "NOUN");
    }

    #[test]
    fn vaan_is_cconj() {
        // "vaan" is adversative CCONJ in UD Finnish-TDT, not SCONJ.
        assert_eq!(refine_conjunction("vaan"), "CCONJ");
        let a = make_analysis("sidesana");
        assert_eq!(mce_to_upos(&a, "vaan"), "CCONJ");
    }

    #[test]
    fn silla_is_sconj() {
        assert_eq!(refine_conjunction("sillä"), "SCONJ");
    }

    #[test]
    fn kuin_is_sconj() {
        assert_eq!(refine_conjunction("kuin"), "SCONJ");
    }

    #[test]
    fn particle_words_stay_adv_for_finnish_tdt() {
        // UD Finnish-TDT does NOT use the PART tag.
        // Words like "myös", "vain", "jo" are ADV, not PART.
        let mut a = make_analysis("seikkasana");
        a.set(ATTR_BASEFORM, "myös");
        assert_eq!(mce_to_upos(&a, "myös"), "ADV");

        let mut a2 = make_analysis("seikkasana");
        a2.set(ATTR_BASEFORM, "vain");
        assert_eq!(mce_to_upos(&a2, "vain"), "ADV");

        // "nopeasti" (quickly) stays ADV.
        let mut a3 = make_analysis("seikkasana");
        a3.set(ATTR_BASEFORM, "nopeasti");
        assert_eq!(mce_to_upos(&a3, "nopeasti"), "ADV");
    }

    #[test]
    fn particle_words_stay_adv_by_surface_form() {
        // When baseform is missing, these still stay ADV (not PART).
        let a = make_analysis("seikkasana");
        assert_eq!(mce_to_upos(&a, "myös"), "ADV");
        assert_eq!(mce_to_upos(&a, "vain"), "ADV");
        assert_eq!(mce_to_upos(&a, "jo"), "ADV");
        assert_eq!(mce_to_upos(&a, "nyt"), "ADV");
        assert_eq!(mce_to_upos(&a, "ihan"), "ADV");
        assert_eq!(mce_to_upos(&a, "kyllä"), "ADV");
    }

    #[test]
    fn auxiliary_verb_detection() {
        // "olla" -> AUX
        let mut a = make_analysis("teonsana");
        a.set(ATTR_BASEFORM, "olla");
        assert_eq!(mce_to_upos(&a, "on"), "AUX");

        // "voida" -> AUX
        let mut a2 = make_analysis("teonsana");
        a2.set(ATTR_BASEFORM, "voida");
        assert_eq!(mce_to_upos(&a2, "voi"), "AUX");

        // "juosta" -> VERB (not an auxiliary)
        let mut a3 = make_analysis("teonsana");
        a3.set(ATTR_BASEFORM, "juosta");
        assert_eq!(mce_to_upos(&a3, "juoksee"), "VERB");
    }

    #[test]
    fn participle_maps_to_verb() {
        // Participles should map to VERB, not ADJ.
        let mut a = make_analysis("laatusana");
        a.set(ATTR_PARTICIPLE, "past_passive");
        assert_eq!(mce_class_to_upos(&a), "VERB");
    }

    #[test]
    fn kerrontosti_adverbs_map_to_adv() {
        // Finnish manner adverbs in -sti have SIJAMUOTO=kerrontosti.
        // They should map to ADV, not ADJ.
        let mut a = make_analysis("laatusana");
        a.set(ATTR_SIJAMUOTO, "kerrontosti");
        a.set(ATTR_BASEFORM, "erityinen");
        assert_eq!(mce_class_to_upos(&a), "ADV");
        assert_eq!(mce_to_upos(&a, "erityisesti"), "ADV");

        // "nopeasti" -> ADV
        let mut a2 = make_analysis("laatusana");
        a2.set(ATTR_SIJAMUOTO, "kerrontosti");
        a2.set(ATTR_BASEFORM, "nopea");
        assert_eq!(mce_class_to_upos(&a2), "ADV");

        // "todennäköisesti" -> ADV
        let mut a3 = make_analysis("laatusana");
        a3.set(ATTR_SIJAMUOTO, "kerrontosti");
        a3.set(ATTR_BASEFORM, "todennäköinen");
        assert_eq!(mce_class_to_upos(&a3), "ADV");
    }

    #[test]
    fn kerrontosti_takes_priority_over_participle() {
        // If a word has both kerrontosti and participle attributes,
        // kerrontosti should win (checked first).
        let mut a = make_analysis("laatusana");
        a.set(ATTR_SIJAMUOTO, "kerrontosti");
        a.set(ATTR_PARTICIPLE, "present_active");
        assert_eq!(mce_class_to_upos(&a), "ADV");
    }

    #[test]
    fn laatusana_without_kerrontosti_stays_adj() {
        // Normal adjectives (not -sti, no participle) remain ADJ.
        let mut a = make_analysis("laatusana");
        a.set(ATTR_SIJAMUOTO, "nimento");
        a.set(ATTR_BASEFORM, "suuri");
        assert_eq!(mce_class_to_upos(&a), "ADJ");
    }

    #[test]
    fn pronoun_overrides() {
        // "kaikki" is nimisana in FST but PRON in UD Finnish-TDT.
        let mut a = make_analysis("nimisana");
        a.set(ATTR_BASEFORM, "kaikki");
        assert_eq!(mce_class_to_upos(&a), "PRON");
        assert_eq!(mce_to_upos(&a, "kaikki"), "PRON");

        // "itse" -> PRON
        let mut a2 = make_analysis("nimisana");
        a2.set(ATTR_BASEFORM, "itse");
        assert_eq!(mce_class_to_upos(&a2), "PRON");

        // "sama" -> PRON
        let mut a3 = make_analysis("nimisana");
        a3.set(ATTR_BASEFORM, "sama");
        assert_eq!(mce_class_to_upos(&a3), "PRON");

        // "toinen" -> PRON
        let mut a4 = make_analysis("nimisana");
        a4.set(ATTR_BASEFORM, "toinen");
        assert_eq!(mce_class_to_upos(&a4), "PRON");

        // "moni" -> PRON
        let mut a5 = make_analysis("nimisana");
        a5.set(ATTR_BASEFORM, "moni");
        assert_eq!(mce_class_to_upos(&a5), "PRON");
    }

    #[test]
    fn regular_nouns_not_affected_by_pronoun_overrides() {
        // Regular nouns should not be affected.
        let mut a = make_analysis("nimisana");
        a.set(ATTR_BASEFORM, "koira");
        assert_eq!(mce_class_to_upos(&a), "NOUN");

        let mut a2 = make_analysis("nimisana");
        a2.set(ATTR_BASEFORM, "talo");
        assert_eq!(mce_class_to_upos(&a2), "NOUN");
    }

    #[test]
    fn nimisana_without_baseform_stays_noun() {
        // If no baseform is available, nimisana defaults to NOUN.
        let a = make_analysis("nimisana");
        assert_eq!(mce_class_to_upos(&a), "NOUN");
    }
}
