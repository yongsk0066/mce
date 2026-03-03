// Adapted from corevoikko (voikko-fi/morphology/)
//
// Finnish morphological analyzer using the VFST backend.

use std::cell::RefCell;

use mce_core::analysis::{
    ATTR_BASEFORM, ATTR_CLASS, ATTR_COMPARISON, ATTR_FOCUS, ATTR_FSTOUTPUT, ATTR_KYSYMYSLIITE,
    ATTR_MALAGA_VAPAA_JALKIOSA, ATTR_MOOD, ATTR_NEGATIVE, ATTR_NUMBER, ATTR_PARTICIPLE,
    ATTR_PERSON, ATTR_POSSESSIVE, ATTR_POSSIBLE_GEOGRAPHICAL_NAME, ATTR_REQUIRE_FOLLOWING_VERB,
    ATTR_SIJAMUOTO, ATTR_STRUCTURE, ATTR_TENSE, ATTR_WORDBASES, ATTR_WORDIDS, Analysis,
    MAX_WORD_CHARS,
};
use mce_core::case::CaseType;
use mce_fst::unweighted::UnweightedTransducer;
use mce_fst::{Transducer, VfstError};

use crate::tag_parser::{
    BUFFER_SIZE, BasicAttributes, MAX_ANALYSIS_COUNT, fix_structure, is_valid_analysis,
    parse_baseform, parse_basic_attributes, parse_debug_attributes, starts_with,
};

/// Trait for morphological analyzers.
pub trait Analyzer {
    fn analyze(&self, word: &[char], word_len: usize) -> Vec<Analysis>;
}

impl<T: Analyzer + ?Sized> Analyzer for &T {
    fn analyze(&self, word: &[char], word_len: usize) -> Vec<Analysis> {
        (**self).analyze(word, word_len)
    }
}

/// Finnish morphological analyzer using the VFST backend.
pub struct FinnishAnalyzer {
    transducer: UnweightedTransducer,
    config: RefCell<mce_fst::config::UnweightedConfig>,
}

impl FinnishAnalyzer {
    /// Create a new FinnishAnalyzer from raw VFST binary data (mor.vfst).
    pub fn from_bytes(data: &[u8]) -> Result<Self, VfstError> {
        let transducer = UnweightedTransducer::from_bytes(data)?;
        let config = RefCell::new(transducer.new_config(BUFFER_SIZE));
        Ok(Self { transducer, config })
    }

    /// Analyze a word with full or partial morphology.
    pub fn analyze_full(
        &self,
        word: &[char],
        word_len: usize,
        full_morphology: bool,
    ) -> Vec<Analysis> {
        if word_len > MAX_WORD_CHARS {
            return Vec::new();
        }

        let mut word_lower: Vec<char> = word[..word_len].to_vec();
        mce_core::case::set_case(&mut word_lower, CaseType::AllLower);

        let mut analyses = Vec::new();
        let mut config = self.config.borrow_mut();

        if !self.transducer.prepare(&mut config, &word_lower) {
            // Unknown character; traversal may still partially work
        }

        let mut output_buf = String::new();
        let mut analysis_count = 0;

        while analysis_count < MAX_ANALYSIS_COUNT
            && self.transducer.next(&mut config, &mut output_buf)
        {
            analysis_count += 1;
            let fst_output: Vec<char> = output_buf.chars().collect();

            if !is_valid_analysis(&fst_output) {
                continue;
            }

            let mut analysis = Analysis::new();
            let mut structure: Vec<char> = parse_structure(&fst_output, word_len).chars().collect();

            let basic = parse_basic_attributes(&fst_output);
            apply_basic_attributes(&mut analysis, &basic);

            fix_structure(&mut structure, &fst_output);
            let structure_str: String = structure.iter().collect();
            analysis.set(ATTR_STRUCTURE, &structure_str);

            post_process_attributes(&mut analysis);

            let analysis_idx = analyses.len();
            analyses.push(analysis);

            if let Some(dup) = duplicate_org_name(&analyses[analysis_idx], &fst_output) {
                analyses.push(dup);
            }

            if full_morphology {
                let fst_output_str: String = fst_output.iter().collect();
                analyses[analysis_idx].set(ATTR_FSTOUTPUT, &fst_output_str);

                if let Some(baseform) = parse_baseform(&fst_output, &structure) {
                    analyses[analysis_idx].set(ATTR_BASEFORM, &baseform);
                }

                let debug = parse_debug_attributes(&fst_output);
                if let Some(wordbases) = &debug.wordbases {
                    analyses[analysis_idx].set(ATTR_WORDBASES, wordbases);
                }
                if let Some(wordids) = &debug.wordids {
                    analyses[analysis_idx].set(ATTR_WORDIDS, wordids);
                }
            }
        }

        analyses
    }
}

impl Analyzer for FinnishAnalyzer {
    fn analyze(&self, word: &[char], word_len: usize) -> Vec<Analysis> {
        self.analyze_full(word, word_len, true)
    }
}

fn apply_basic_attributes(analysis: &mut Analysis, attrs: &BasicAttributes) {
    if let Some(class) = attrs.class {
        analysis.set(ATTR_CLASS, class);
    }
    if let Some(sijamuoto) = attrs.sijamuoto {
        analysis.set(ATTR_SIJAMUOTO, sijamuoto);
    }
    if let Some(number) = attrs.number {
        analysis.set(ATTR_NUMBER, number);
    }
    if let Some(person) = attrs.person {
        analysis.set(ATTR_PERSON, person);
    }
    if let Some(mood) = attrs.mood {
        analysis.set(ATTR_MOOD, mood);
    }
    if let Some(tense) = attrs.tense {
        analysis.set(ATTR_TENSE, tense);
    }
    if let Some(focus) = attrs.focus {
        analysis.set(ATTR_FOCUS, focus);
    }
    if let Some(possessive) = attrs.possessive {
        analysis.set(ATTR_POSSESSIVE, possessive);
    }
    if let Some(negative) = attrs.negative {
        analysis.set(ATTR_NEGATIVE, negative);
    }
    if let Some(comparison) = attrs.comparison {
        analysis.set(ATTR_COMPARISON, comparison);
    }
    if let Some(participle) = attrs.participle {
        analysis.set(ATTR_PARTICIPLE, participle);
    }
    if attrs.kysymysliite {
        analysis.set(ATTR_KYSYMYSLIITE, "true");
    }
    if let Some(rfv) = attrs.require_following_verb {
        analysis.set(ATTR_REQUIRE_FOLLOWING_VERB, rfv);
    }
    if attrs.malaga_vapaa_jalkiosa {
        analysis.set(ATTR_MALAGA_VAPAA_JALKIOSA, "true");
    }
    if attrs.possible_geographical_name {
        analysis.set(ATTR_POSSIBLE_GEOGRAPHICAL_NAME, "true");
    }
}

fn post_process_attributes(analysis: &mut Analysis) {
    let wclass = analysis.get(ATTR_CLASS).map(str::to_string);
    let sijamuoto = analysis.get(ATTR_SIJAMUOTO).map(str::to_string);
    let mood = analysis.get(ATTR_MOOD).map(str::to_string);
    let participle = analysis.get(ATTR_PARTICIPLE).map(str::to_string);

    // 1. Remove NEGATIVE from non-verbs or certain infinitive forms
    if analysis.contains_key(ATTR_NEGATIVE) {
        let is_non_verb = wclass.as_deref().is_some_and(|c| c != "teonsana");
        let is_nominal_infinitive = mood.as_deref().is_some_and(|m| {
            m == "MINEN-infinitive" || m == "E-infinitive" || m == "MA-infinitive"
        });
        if is_non_verb || is_nominal_infinitive {
            analysis.remove(ATTR_NEGATIVE);
        }
    }

    // 2. Past passive participle forces class to "laatusana"
    if participle.as_deref() == Some("past_passive") && wclass.as_deref() != Some("laatusana") {
        analysis.remove(ATTR_CLASS);
        analysis.set(ATTR_CLASS, "laatusana");
    }

    let wclass = analysis.get(ATTR_CLASS).map(str::to_string);

    // 3. Remove NUMBER for "kerrontosti" case
    if analysis.contains_key(ATTR_NUMBER) && sijamuoto.as_deref() == Some("kerrontosti") {
        analysis.remove(ATTR_NUMBER);
    }

    // 4. Default COMPARISON for adjectives
    if !analysis.contains_key(ATTR_COMPARISON) {
        if matches!(
            wclass.as_deref(),
            Some("laatusana") | Some("nimisana_laatusana")
        ) {
            analysis.set(ATTR_COMPARISON, "positive");
        }
    } else if wclass.as_deref() == Some("nimisana") {
        // 5. Remove COMPARISON from plain nouns
        analysis.remove(ATTR_COMPARISON);
    }
}

fn duplicate_org_name(analysis: &Analysis, fst_output: &[char]) -> Option<Analysis> {
    let old_class = analysis.get(ATTR_CLASS)?;
    if old_class != "nimisana" {
        return None;
    }

    let fst_len = fst_output.len();
    if fst_len < 13 {
        return None;
    }
    if fst_output[0] == '-' {
        return None;
    }
    if starts_with(fst_output, 0, "[La]") {
        return None;
    }

    let mut i = fst_len.saturating_sub(5);
    while i >= 8 {
        if starts_with(fst_output, i, "[Bc]") {
            return None;
        }
        if starts_with(fst_output, i, "[Ion]") {
            let mut j = i.saturating_sub(4);
            while j >= 4 {
                if starts_with(fst_output, j, "[Bc]") {
                    let mut new_analysis = analysis.clone();
                    new_analysis.remove(ATTR_CLASS);
                    new_analysis.set(ATTR_CLASS, "nimi");
                    new_analysis.remove(ATTR_POSSIBLE_GEOGRAPHICAL_NAME);

                    if let Some(old_structure) = analysis.get(ATTR_STRUCTURE) {
                        let mut new_structure: Vec<char> = old_structure.chars().collect();
                        if new_structure.len() >= 2 {
                            new_structure[1] = 'i';
                            let new_struct_str: String = new_structure.iter().collect();
                            new_analysis.set(ATTR_STRUCTURE, &new_struct_str);

                            if let Some(baseform) = parse_baseform(fst_output, &new_structure) {
                                new_analysis.set(ATTR_BASEFORM, &baseform);
                            }
                        }
                    }

                    return Some(new_analysis);
                }
                if j == 0 {
                    break;
                }
                j -= 1;
            }
        }
        if i == 0 {
            break;
        }
        i -= 1;
    }

    None
}

// Re-export parse_structure for internal use by analyze_full
use crate::tag_parser::parse_structure;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn post_process_removes_negative_from_noun() {
        let mut a = Analysis::new();
        a.set(ATTR_CLASS, "nimisana");
        a.set(ATTR_NEGATIVE, "true");
        post_process_attributes(&mut a);
        assert!(!a.contains_key(ATTR_NEGATIVE));
    }

    #[test]
    fn post_process_keeps_negative_on_verb() {
        let mut a = Analysis::new();
        a.set(ATTR_CLASS, "teonsana");
        a.set(ATTR_NEGATIVE, "true");
        a.set(ATTR_MOOD, "indicative");
        post_process_attributes(&mut a);
        assert!(a.contains_key(ATTR_NEGATIVE));
    }

    #[test]
    fn post_process_past_passive_forces_laatusana() {
        let mut a = Analysis::new();
        a.set(ATTR_CLASS, "teonsana");
        a.set(ATTR_PARTICIPLE, "past_passive");
        post_process_attributes(&mut a);
        assert_eq!(a.get(ATTR_CLASS), Some("laatusana"));
    }

    #[test]
    fn post_process_removes_number_for_kerrontosti() {
        let mut a = Analysis::new();
        a.set(ATTR_CLASS, "laatusana");
        a.set(ATTR_SIJAMUOTO, "kerrontosti");
        a.set(ATTR_NUMBER, "singular");
        post_process_attributes(&mut a);
        assert!(!a.contains_key(ATTR_NUMBER));
    }

    #[test]
    fn post_process_adds_positive_comparison_to_adjective() {
        let mut a = Analysis::new();
        a.set(ATTR_CLASS, "laatusana");
        post_process_attributes(&mut a);
        assert_eq!(a.get(ATTR_COMPARISON), Some("positive"));
    }

    #[test]
    fn post_process_removes_comparison_from_noun() {
        let mut a = Analysis::new();
        a.set(ATTR_CLASS, "nimisana");
        a.set(ATTR_COMPARISON, "comparative");
        post_process_attributes(&mut a);
        assert!(!a.contains_key(ATTR_COMPARISON));
    }

    #[test]
    fn duplicate_org_name_returns_none_for_non_noun() {
        let fst: Vec<char> = "[Lt][Xp]juosta[X]juoksen[Tt][Ap][P1][Ny]".chars().collect();
        let mut a = Analysis::new();
        a.set(ATTR_CLASS, "teonsana");
        assert!(duplicate_org_name(&a, &fst).is_none());
    }
}
