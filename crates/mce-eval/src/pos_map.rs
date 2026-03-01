//! POS tag mapping between MCE Finnish word classes and UD UPOS tags.
//!
//! MCE's Finnish analyzer produces CLASS values from the Voikko FST output
//! (e.g., `nimisana`, `teonsana`). This module maps them to Universal
//! Dependencies UPOS tags for evaluation.

use mce_core::analysis::{
    Analysis, ATTR_BASEFORM, ATTR_CLASS, ATTR_PARTICIPLE, ATTR_POSSIBLE_GEOGRAPHICAL_NAME,
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
        "nimisana" => "NOUN",
        "laatusana" => {
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

/// Finnish particles that MCE might tag as `seikkasana` but UD tags as PART.
pub const FINNISH_PARTICLES: &[&str] = &[
    "myös", "vain", "kin", "kaan", "jo", "vielä", "edes", "kyllä", "kai", "nyt", "niin", "ihan",
    "aivan", "-ko", "-kö", "-han", "-hän", "-pa", "-pä",
];

/// Finnish particle baseforms that UD Finnish-TDT tags as PART.
///
/// These are words that MCE typically tags as `seikkasana` (ADV) but that
/// the UD standard classifies as particles. They are function words that
/// do not fit the ADV category semantically.
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
            // Some adverbs are actually particles in UD.
            // Check baseform first, then surface form.
            if let Some(baseform) = analysis.get(ATTR_BASEFORM) {
                if FINNISH_PARTICLE_BASEFORMS.contains(&baseform) {
                    return "PART";
                }
            }
            let lower: String = surface
                .chars()
                .map(|c| c.to_lowercase().next().unwrap_or(c))
                .collect();
            if FINNISH_PARTICLE_BASEFORMS.contains(&lower.as_str()) {
                return "PART";
            }
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
    fn particle_mapping() {
        // "myös" (also) is PART in UD, not ADV.
        let mut a = make_analysis("seikkasana");
        a.set(ATTR_BASEFORM, "myös");
        assert_eq!(mce_to_upos(&a, "myös"), "PART");

        // "vain" (only) is PART.
        let mut a2 = make_analysis("seikkasana");
        a2.set(ATTR_BASEFORM, "vain");
        assert_eq!(mce_to_upos(&a2, "vain"), "PART");

        // "nopeasti" (quickly) stays ADV.
        let mut a3 = make_analysis("seikkasana");
        a3.set(ATTR_BASEFORM, "nopeasti");
        assert_eq!(mce_to_upos(&a3, "nopeasti"), "ADV");
    }

    #[test]
    fn particle_by_surface_form() {
        // When baseform is missing, surface form is used.
        let a = make_analysis("seikkasana");
        assert_eq!(mce_to_upos(&a, "myös"), "PART");
        assert_eq!(mce_to_upos(&a, "vain"), "PART");
        assert_eq!(mce_to_upos(&a, "jo"), "PART");
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
}
