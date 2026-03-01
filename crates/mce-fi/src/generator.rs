//! Morphological generation using coKleisli morphophonological rules.
//!
//! This module provides the **generation** path for Finnish morphology.
//! While [`crate::morphology::FinnishAnalyzer`] performs ANALYSIS (surface form
//! -> baseform + features) using the VFST transducer, [`MorphGenerator`]
//! performs GENERATION (baseform + features -> surface form) using the
//! coKleisli pipeline from [`mce_comonad::finnish`].
//!
//! # Pipeline
//!
//! 1. Determine gradation grade from the target case.
//! 2. Append the case suffix (with archiphonemic characters) to the stem.
//! 3. Apply consonant gradation via [`mce_comonad::finnish::apply_gradation`].
//! 4. Apply vowel harmony via [`mce_comonad::finnish::apply_vowel_harmony`].
//! 5. Apply possessive suffix vowel copying via [`mce_comonad::finnish::apply_possessive`].
//!
//! # Scope
//!
//! This is a **simplified** generator that handles regular Finnish noun
//! inflection. It does not cover:
//!
//! - Irregular stems (e.g., stems that change vowels beyond gradation)
//! - Verb conjugation
//! - Adjective comparison
//! - Numeral inflection
//!
//! For full morphological generation, the VFST transducer should be used
//! in reverse (generation) mode. This module demonstrates the coKleisli
//! pipeline's integration into the production path and handles the most
//! common regular patterns.

use mce_comonad::finnish::{apply_possessive_to_word, gradate, harmonize, Grade};

// ---------------------------------------------------------------------------
// Finnish case definitions
// ---------------------------------------------------------------------------

/// A Finnish grammatical case with its suffix and gradation grade.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CaseInfo {
    /// The Voikko-style case name (Finnish grammar term).
    pub voikko_name: &'static str,
    /// The English/international case name.
    pub name: &'static str,
    /// The suffix to append, using archiphonemic characters:
    /// - `A` resolves to `a` (back) or `ä` (front) via vowel harmony
    /// - `O` resolves to `o` (back) or `ö` (front)
    /// - `V` copies the preceding vowel (for illative)
    ///
    /// Special suffixes:
    /// - `None` means no suffix (nominative)
    /// - Some suffixes contain literal characters that do not change
    pub suffix: &'static str,
    /// The consonant gradation grade for this case.
    pub grade: Grade,
}

/// All singular Finnish noun cases.
///
/// Grade assignment follows Finnish grammar:
/// - **Strong grade**: nominative, partitive, essive, illative, comitative
/// - **Weak grade**: genitive, inessive, elative, adessive, ablative,
///   allative, translative, abessive, instructive
const SINGULAR_CASES: &[CaseInfo] = &[
    CaseInfo {
        voikko_name: "nimento",
        name: "nominative",
        suffix: "",
        grade: Grade::Strong,
    },
    CaseInfo {
        voikko_name: "omanto",
        name: "genitive",
        suffix: "n",
        grade: Grade::Weak,
    },
    CaseInfo {
        voikko_name: "osanto",
        name: "partitive",
        suffix: "A",
        grade: Grade::Strong,
    },
    CaseInfo {
        voikko_name: "sisaolento",
        name: "inessive",
        suffix: "ssA",
        grade: Grade::Weak,
    },
    CaseInfo {
        voikko_name: "sisaeronto",
        name: "elative",
        suffix: "stA",
        grade: Grade::Weak,
    },
    CaseInfo {
        voikko_name: "sisatulento",
        name: "illative",
        suffix: "Vn",
        grade: Grade::Strong,
    },
    CaseInfo {
        voikko_name: "ulkoolento",
        name: "adessive",
        suffix: "llA",
        grade: Grade::Weak,
    },
    CaseInfo {
        voikko_name: "ulkoeronto",
        name: "ablative",
        suffix: "ltA",
        grade: Grade::Weak,
    },
    CaseInfo {
        voikko_name: "ulkotulento",
        name: "allative",
        suffix: "lle",
        grade: Grade::Weak,
    },
    CaseInfo {
        voikko_name: "olento",
        name: "essive",
        suffix: "nA",
        grade: Grade::Strong,
    },
    CaseInfo {
        voikko_name: "tulento",
        name: "translative",
        suffix: "ksi",
        grade: Grade::Weak,
    },
];

// ---------------------------------------------------------------------------
// MorphGenerator
// ---------------------------------------------------------------------------

/// Morphological generator using coKleisli morphophonological rules.
///
/// Generates inflected surface forms from a baseform and target grammatical
/// features by applying the coKleisli pipeline (consonant gradation, vowel
/// harmony, possessive suffix vowel copying).
///
/// # Example
///
/// ```
/// use mce_fi::generator::MorphGenerator;
///
/// let gen = MorphGenerator::new();
///
/// // Generate genitive singular of "kaappi"
/// let form = gen.generate("kaappi", &[("SIJAMUOTO", "omanto")]);
/// assert_eq!(form, Some("kaapin".to_string()));
///
/// // Generate full paradigm
/// let paradigm = gen.generate_paradigm("talo");
/// assert!(paradigm.iter().any(|(case, form)| case == "genitive" && form == "talon"));
/// ```
pub struct MorphGenerator;

impl MorphGenerator {
    /// Create a new morphological generator.
    pub fn new() -> Self {
        MorphGenerator
    }

    /// Generate an inflected form from a baseform and grammatical features.
    ///
    /// The features are specified as key-value pairs using Voikko attribute
    /// names:
    ///
    /// - `("SIJAMUOTO", "<case>")` — the grammatical case, using either
    ///   Voikko names (e.g., "omanto", "osanto") or English names
    ///   (e.g., "genitive", "partitive")
    ///
    /// Returns `None` if the case is not recognized.
    ///
    /// # Example
    ///
    /// ```
    /// use mce_fi::generator::MorphGenerator;
    ///
    /// let gen = MorphGenerator::new();
    /// assert_eq!(
    ///     gen.generate("kaappi", &[("SIJAMUOTO", "omanto")]),
    ///     Some("kaapin".to_string()),
    /// );
    /// ```
    pub fn generate(&self, baseform: &str, features: &[(&str, &str)]) -> Option<String> {
        // Find the requested case.
        let case_name = features
            .iter()
            .find(|(k, _)| *k == "SIJAMUOTO")
            .map(|(_, v)| *v)?;

        let case_info = find_case(case_name)?;
        Some(apply_case(baseform, case_info))
    }

    /// Generate all singular case forms for a noun.
    ///
    /// Returns a vector of `(case_name, inflected_form)` pairs.
    ///
    /// # Example
    ///
    /// ```
    /// use mce_fi::generator::MorphGenerator;
    ///
    /// let gen = MorphGenerator::new();
    /// let paradigm = gen.generate_paradigm("talo");
    /// assert_eq!(paradigm[0], ("nominative".to_string(), "talo".to_string()));
    /// assert_eq!(paradigm[1], ("genitive".to_string(), "talon".to_string()));
    /// ```
    pub fn generate_paradigm(&self, baseform: &str) -> Vec<(String, String)> {
        SINGULAR_CASES
            .iter()
            .map(|case_info| {
                let form = apply_case(baseform, case_info);
                (case_info.name.to_string(), form)
            })
            .collect()
    }
}

impl Default for MorphGenerator {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Internal helpers
// ---------------------------------------------------------------------------

/// Look up a case by its Voikko name or English name.
fn find_case(name: &str) -> Option<&'static CaseInfo> {
    let lower = name.to_lowercase();
    SINGULAR_CASES
        .iter()
        .find(|c| c.voikko_name == lower || c.name == lower)
}

/// Apply a case to a baseform, producing the inflected surface form.
///
/// This function separates stem processing from suffix processing:
///
/// 1. **Consonant gradation** is applied to the stem ONLY. Suffix
///    consonants (e.g., the `k` in `-ksi`, the `lt` in `-ltA`) must
///    not be affected by gradation patterns.
/// 2. The suffix is appended to the graded stem.
/// 3. **Vowel harmony** is applied to the entire word (stem + suffix),
///    resolving archiphonemic `A`, `O`, `U` in the suffix.
/// 4. **Possessive vowel copying** resolves any `V` archiphonemes.
fn apply_case(baseform: &str, case_info: &CaseInfo) -> String {
    if case_info.suffix.is_empty() {
        // Nominative: no suffix, citation form unchanged.
        return baseform.to_string();
    }

    // Step 1: Apply consonant gradation to the stem only.
    let graded_stem = gradate(baseform, case_info.grade);

    // Step 2: Concatenate graded stem + archiphonemic suffix.
    let intermediate = format!("{}{}", graded_stem, case_info.suffix);

    // Step 3: Apply vowel harmony to the entire word.
    let after_harmony = harmonize(&intermediate);

    // Step 4: Apply possessive vowel copying.
    apply_possessive_to_word(&after_harmony)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn gen() -> MorphGenerator {
        MorphGenerator::new()
    }

    // =====================================================================
    // Case lookup
    // =====================================================================

    #[test]
    fn find_case_by_voikko_name() {
        assert_eq!(find_case("omanto").unwrap().name, "genitive");
        assert_eq!(find_case("osanto").unwrap().name, "partitive");
        assert_eq!(find_case("nimento").unwrap().name, "nominative");
    }

    #[test]
    fn find_case_by_english_name() {
        assert_eq!(find_case("genitive").unwrap().voikko_name, "omanto");
        assert_eq!(find_case("partitive").unwrap().voikko_name, "osanto");
        assert_eq!(find_case("nominative").unwrap().voikko_name, "nimento");
    }

    #[test]
    fn find_case_case_insensitive() {
        assert!(find_case("GENITIVE").is_some());
        assert!(find_case("Omanto").is_some());
    }

    #[test]
    fn find_case_unknown() {
        assert!(find_case("nonexistent").is_none());
    }

    // =====================================================================
    // kaappi (pp -> p gradation)
    // =====================================================================

    #[test]
    fn kaappi_nominative() {
        let g = gen();
        let form = g.generate("kaappi", &[("SIJAMUOTO", "nimento")]);
        assert_eq!(form, Some("kaappi".to_string()));
    }

    #[test]
    fn kaappi_genitive() {
        let g = gen();
        let form = g.generate("kaappi", &[("SIJAMUOTO", "omanto")]);
        assert_eq!(form, Some("kaapin".to_string()));
    }

    #[test]
    fn kaappi_partitive() {
        let g = gen();
        let form = g.generate("kaappi", &[("SIJAMUOTO", "osanto")]);
        assert_eq!(form, Some("kaappia".to_string()));
    }

    #[test]
    fn kaappi_inessive() {
        let g = gen();
        let form = g.generate("kaappi", &[("SIJAMUOTO", "sisaolento")]);
        assert_eq!(form, Some("kaapissa".to_string()));
    }

    #[test]
    fn kaappi_elative() {
        let g = gen();
        let form = g.generate("kaappi", &[("SIJAMUOTO", "sisaeronto")]);
        assert_eq!(form, Some("kaapista".to_string()));
    }

    #[test]
    fn kaappi_illative() {
        let g = gen();
        let form = g.generate("kaappi", &[("SIJAMUOTO", "sisatulento")]);
        assert_eq!(form, Some("kaappiin".to_string()));
    }

    #[test]
    fn kaappi_essive() {
        let g = gen();
        let form = g.generate("kaappi", &[("SIJAMUOTO", "olento")]);
        assert_eq!(form, Some("kaappina".to_string()));
    }

    #[test]
    fn kaappi_translative() {
        let g = gen();
        let form = g.generate("kaappi", &[("SIJAMUOTO", "tulento")]);
        assert_eq!(form, Some("kaapiksi".to_string()));
    }

    // =====================================================================
    // talo (no gradation, back vowels)
    // =====================================================================

    #[test]
    fn talo_nominative() {
        let g = gen();
        let form = g.generate("talo", &[("SIJAMUOTO", "nominative")]);
        assert_eq!(form, Some("talo".to_string()));
    }

    #[test]
    fn talo_genitive() {
        let g = gen();
        let form = g.generate("talo", &[("SIJAMUOTO", "genitive")]);
        assert_eq!(form, Some("talon".to_string()));
    }

    #[test]
    fn talo_partitive() {
        let g = gen();
        let form = g.generate("talo", &[("SIJAMUOTO", "partitive")]);
        assert_eq!(form, Some("taloa".to_string()));
    }

    #[test]
    fn talo_inessive() {
        let g = gen();
        let form = g.generate("talo", &[("SIJAMUOTO", "inessive")]);
        assert_eq!(form, Some("talossa".to_string()));
    }

    #[test]
    fn talo_illative() {
        let g = gen();
        let form = g.generate("talo", &[("SIJAMUOTO", "illative")]);
        assert_eq!(form, Some("taloon".to_string()));
    }

    #[test]
    fn talo_adessive() {
        let g = gen();
        let form = g.generate("talo", &[("SIJAMUOTO", "adessive")]);
        assert_eq!(form, Some("talolla".to_string()));
    }

    #[test]
    fn talo_ablative() {
        let g = gen();
        let form = g.generate("talo", &[("SIJAMUOTO", "ablative")]);
        assert_eq!(form, Some("talolta".to_string()));
    }

    #[test]
    fn talo_allative() {
        let g = gen();
        let form = g.generate("talo", &[("SIJAMUOTO", "allative")]);
        assert_eq!(form, Some("talolle".to_string()));
    }

    #[test]
    fn talo_essive() {
        let g = gen();
        let form = g.generate("talo", &[("SIJAMUOTO", "essive")]);
        assert_eq!(form, Some("talona".to_string()));
    }

    #[test]
    fn talo_translative() {
        let g = gen();
        let form = g.generate("talo", &[("SIJAMUOTO", "translative")]);
        assert_eq!(form, Some("taloksi".to_string()));
    }

    // =====================================================================
    // p\u{00f6}yt\u{00e4} (front vowels, t -> d gradation)
    // =====================================================================

    #[test]
    fn poyta_genitive() {
        let g = gen();
        let form = g.generate("p\u{00F6}yt\u{00E4}", &[("SIJAMUOTO", "genitive")]);
        assert_eq!(form, Some("p\u{00F6}yd\u{00E4}n".to_string()));
    }

    #[test]
    fn poyta_inessive() {
        let g = gen();
        let form = g.generate("p\u{00F6}yt\u{00E4}", &[("SIJAMUOTO", "inessive")]);
        assert_eq!(form, Some("p\u{00F6}yd\u{00E4}ss\u{00E4}".to_string()));
    }

    #[test]
    fn poyta_elative() {
        let g = gen();
        let form = g.generate("p\u{00F6}yt\u{00E4}", &[("SIJAMUOTO", "elative")]);
        assert_eq!(form, Some("p\u{00F6}yd\u{00E4}st\u{00E4}".to_string()));
    }

    #[test]
    fn poyta_partitive() {
        let g = gen();
        let form = g.generate("p\u{00F6}yt\u{00E4}", &[("SIJAMUOTO", "partitive")]);
        assert_eq!(form, Some("p\u{00F6}yt\u{00E4}\u{00E4}".to_string()));
    }

    #[test]
    fn poyta_essive() {
        let g = gen();
        let form = g.generate("p\u{00F6}yt\u{00E4}", &[("SIJAMUOTO", "essive")]);
        assert_eq!(form, Some("p\u{00F6}yt\u{00E4}n\u{00E4}".to_string()));
    }

    #[test]
    fn poyta_illative() {
        let g = gen();
        let form = g.generate("p\u{00F6}yt\u{00E4}", &[("SIJAMUOTO", "illative")]);
        assert_eq!(form, Some("p\u{00F6}yt\u{00E4}\u{00E4}n".to_string()));
    }

    // =====================================================================
    // kukka (kk -> k gradation)
    // =====================================================================

    #[test]
    fn kukka_genitive() {
        let g = gen();
        let form = g.generate("kukka", &[("SIJAMUOTO", "genitive")]);
        assert_eq!(form, Some("kukan".to_string()));
    }

    #[test]
    fn kukka_partitive() {
        let g = gen();
        let form = g.generate("kukka", &[("SIJAMUOTO", "partitive")]);
        assert_eq!(form, Some("kukkaa".to_string()));
    }

    #[test]
    fn kukka_inessive() {
        let g = gen();
        let form = g.generate("kukka", &[("SIJAMUOTO", "inessive")]);
        assert_eq!(form, Some("kukassa".to_string()));
    }

    #[test]
    fn kukka_illative() {
        let g = gen();
        let form = g.generate("kukka", &[("SIJAMUOTO", "illative")]);
        assert_eq!(form, Some("kukkaan".to_string()));
    }

    // =====================================================================
    // Vowel harmony: ensure A -> a (back) vs A -> \u{00e4} (front)
    // =====================================================================

    #[test]
    fn harmony_back_partitive() {
        let g = gen();
        // "koulu" has back vowels -> partitive suffix A -> a
        let form = g.generate("koulu", &[("SIJAMUOTO", "partitive")]);
        assert_eq!(form, Some("koulua".to_string()));
    }

    #[test]
    fn harmony_front_partitive() {
        let g = gen();
        // "työ" has front vowels -> partitive suffix A -> ä
        let form = g.generate("ty\u{00F6}", &[("SIJAMUOTO", "partitive")]);
        assert_eq!(form, Some("ty\u{00F6}\u{00E4}".to_string()));
    }

    #[test]
    fn harmony_back_adessive() {
        let g = gen();
        // "talo" back -> adessive "talolla"
        let form = g.generate("talo", &[("SIJAMUOTO", "adessive")]);
        assert_eq!(form, Some("talolla".to_string()));
    }

    #[test]
    fn harmony_front_adessive() {
        let g = gen();
        // "pöytä" front -> adessive "pöydällä"
        let form = g.generate("p\u{00F6}yt\u{00E4}", &[("SIJAMUOTO", "adessive")]);
        assert_eq!(form, Some("p\u{00F6}yd\u{00E4}ll\u{00E4}".to_string()));
    }

    // =====================================================================
    // Cluster gradation in generation
    // =====================================================================

    #[test]
    fn ranta_genitive() {
        let g = gen();
        // ranta: nt -> nn in weak grade
        let form = g.generate("ranta", &[("SIJAMUOTO", "genitive")]);
        assert_eq!(form, Some("rannan".to_string()));
    }

    #[test]
    fn ranta_inessive() {
        let g = gen();
        let form = g.generate("ranta", &[("SIJAMUOTO", "inessive")]);
        assert_eq!(form, Some("rannassa".to_string()));
    }

    #[test]
    fn ranta_partitive() {
        let g = gen();
        // Strong grade for partitive, nt stays nt
        let form = g.generate("ranta", &[("SIJAMUOTO", "partitive")]);
        assert_eq!(form, Some("rantaa".to_string()));
    }

    #[test]
    fn kampa_genitive() {
        let g = gen();
        // kampa: mp -> mm in weak grade
        let form = g.generate("kampa", &[("SIJAMUOTO", "genitive")]);
        assert_eq!(form, Some("kamman".to_string()));
    }

    // =====================================================================
    // generate_paradigm
    // =====================================================================

    #[test]
    fn paradigm_talo() {
        let g = gen();
        let paradigm = g.generate_paradigm("talo");

        assert_eq!(paradigm.len(), SINGULAR_CASES.len());
        assert_eq!(paradigm[0], ("nominative".to_string(), "talo".to_string()));
        assert_eq!(paradigm[1], ("genitive".to_string(), "talon".to_string()));
        assert_eq!(paradigm[2], ("partitive".to_string(), "taloa".to_string()));
        assert_eq!(paradigm[3], ("inessive".to_string(), "talossa".to_string()));
        assert_eq!(paradigm[4], ("elative".to_string(), "talosta".to_string()));
        assert_eq!(paradigm[5], ("illative".to_string(), "taloon".to_string()));
        assert_eq!(paradigm[6], ("adessive".to_string(), "talolla".to_string()));
        assert_eq!(paradigm[7], ("ablative".to_string(), "talolta".to_string()));
        assert_eq!(paradigm[8], ("allative".to_string(), "talolle".to_string()));
        assert_eq!(paradigm[9], ("essive".to_string(), "talona".to_string()));
        assert_eq!(
            paradigm[10],
            ("translative".to_string(), "taloksi".to_string())
        );
    }

    #[test]
    fn paradigm_poyta() {
        let g = gen();
        let paradigm = g.generate_paradigm("p\u{00F6}yt\u{00E4}");

        // Check a few key forms with front harmony + gradation
        assert_eq!(
            paradigm[0],
            ("nominative".to_string(), "p\u{00F6}yt\u{00E4}".to_string())
        );
        assert_eq!(
            paradigm[1],
            ("genitive".to_string(), "p\u{00F6}yd\u{00E4}n".to_string())
        );
        assert_eq!(
            paradigm[3],
            (
                "inessive".to_string(),
                "p\u{00F6}yd\u{00E4}ss\u{00E4}".to_string()
            )
        );
    }

    // =====================================================================
    // generate returns None for unknown case
    // =====================================================================

    #[test]
    fn generate_unknown_case_returns_none() {
        let g = gen();
        assert_eq!(g.generate("talo", &[("SIJAMUOTO", "bogus")]), None);
    }

    #[test]
    fn generate_missing_sijamuoto_returns_none() {
        let g = gen();
        assert_eq!(g.generate("talo", &[("CLASS", "nimisana")]), None);
    }

    // =====================================================================
    // English case name access
    // =====================================================================

    #[test]
    fn generate_with_english_names() {
        let g = gen();
        assert_eq!(
            g.generate("talo", &[("SIJAMUOTO", "genitive")]),
            Some("talon".to_string()),
        );
        assert_eq!(
            g.generate("talo", &[("SIJAMUOTO", "inessive")]),
            Some("talossa".to_string()),
        );
    }

    // =====================================================================
    // k-deletion (puku -> puu- in weak grade)
    // =====================================================================

    #[test]
    fn puku_genitive() {
        let g = gen();
        // puku: k deleted in weak grade -> puun
        let form = g.generate("puku", &[("SIJAMUOTO", "genitive")]);
        assert_eq!(form, Some("puun".to_string()));
    }

    #[test]
    fn puku_inessive() {
        let g = gen();
        let form = g.generate("puku", &[("SIJAMUOTO", "inessive")]);
        assert_eq!(form, Some("puussa".to_string()));
    }

    #[test]
    fn puku_partitive() {
        let g = gen();
        // Strong grade for partitive, k stays
        let form = g.generate("puku", &[("SIJAMUOTO", "partitive")]);
        assert_eq!(form, Some("pukua".to_string()));
    }
}
