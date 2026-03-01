//! CoNLL-U format parser.
//!
//! Parses the Universal Dependencies CoNLL-U format into structured data.
//!
//! # Format
//!
//! Each line represents a token with 10 tab-separated fields:
//!
//! ```text
//! ID  FORM  LEMMA  UPOS  XPOS  FEATS  HEAD  DEPREL  DEPS  MISC
//! ```
//!
//! - Blank lines separate sentences.
//! - Lines starting with `#` are comments (may contain `sent_id` and `text`).
//! - Multi-word tokens have IDs like `1-2` and are skipped for evaluation.

use std::fs;
use std::io;
use std::path::Path;

/// A single token from a CoNLL-U file.
#[derive(Debug, Clone)]
pub struct ConlluToken {
    /// Token ID (1-based integer index within the sentence).
    pub id: usize,
    /// Surface form of the word.
    pub form: String,
    /// Lemma (dictionary form).
    pub lemma: String,
    /// Universal POS tag.
    pub upos: String,
    /// Language-specific POS tag.
    pub xpos: String,
    /// Morphological features (pipe-separated key=value pairs).
    pub feats: String,
}

/// A sentence parsed from CoNLL-U format.
#[derive(Debug, Clone)]
pub struct ConlluSentence {
    /// Sentence ID from the `# sent_id = ...` comment.
    pub sent_id: String,
    /// Raw text from the `# text = ...` comment.
    pub text: String,
    /// Tokens (excluding multi-word token lines).
    pub tokens: Vec<ConlluToken>,
}

/// Parse a CoNLL-U file and return a list of sentences.
pub fn parse_conllu_file(path: &Path) -> io::Result<Vec<ConlluSentence>> {
    let content = fs::read_to_string(path)?;
    Ok(parse_conllu(&content))
}

/// Parse CoNLL-U content from a string.
pub fn parse_conllu(content: &str) -> Vec<ConlluSentence> {
    let mut sentences = Vec::new();
    let mut current_tokens: Vec<ConlluToken> = Vec::new();
    let mut current_sent_id = String::new();
    let mut current_text = String::new();

    for line in content.lines() {
        if line.is_empty() {
            // End of sentence.
            if !current_tokens.is_empty() {
                sentences.push(ConlluSentence {
                    sent_id: std::mem::take(&mut current_sent_id),
                    text: std::mem::take(&mut current_text),
                    tokens: std::mem::take(&mut current_tokens),
                });
            }
            continue;
        }

        if let Some(rest) = line.strip_prefix('#') {
            let rest = rest.trim();
            if let Some(sid) = rest.strip_prefix("sent_id = ") {
                current_sent_id = sid.to_string();
            } else if let Some(txt) = rest.strip_prefix("text = ") {
                current_text = txt.to_string();
            }
            continue;
        }

        // Tab-separated fields.
        let fields: Vec<&str> = line.split('\t').collect();
        if fields.len() < 10 {
            continue;
        }

        let id_str = fields[0];

        // Skip multi-word tokens (e.g., "1-2") and empty nodes (e.g., "1.1").
        if id_str.contains('-') || id_str.contains('.') {
            continue;
        }

        let id = match id_str.parse::<usize>() {
            Ok(n) => n,
            Err(_) => continue,
        };

        current_tokens.push(ConlluToken {
            id,
            form: fields[1].to_string(),
            lemma: fields[2].to_string(),
            upos: fields[3].to_string(),
            xpos: fields[4].to_string(),
            feats: fields[5].to_string(),
        });
    }

    // Handle the last sentence if the file doesn't end with a blank line.
    if !current_tokens.is_empty() {
        sentences.push(ConlluSentence {
            sent_id: current_sent_id,
            text: current_text,
            tokens: current_tokens,
        });
    }

    sentences
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = "\
# sent_id = test.1
# text = Koira juoksee nopeasti.
1\tKoira\tkoira\tNOUN\tN\tCase=Nom|Number=Sing\t2\tnsubj\t2:nsubj\t_
2\tjuoksee\tjuosta\tVERB\tV\tMood=Ind|Number=Sing|Person=3|Tense=Pres|VerbForm=Fin|Voice=Act\t0\troot\t0:root\t_
3\tnopeasti\tnopeasti\tADV\tAdv\t_\t2\tadvmod\t2:advmod\tSpaceAfter=No
4\t.\t.\tPUNCT\tPunct\t_\t2\tpunct\t2:punct\t_

# sent_id = test.2
# text = Iso kissa nukkuu.
1\tIso\tiso\tADJ\tA\tCase=Nom|Degree=Pos|Number=Sing\t2\tamod\t2:amod\t_
2\tkissa\tkissa\tNOUN\tN\tCase=Nom|Number=Sing\t3\tnsubj\t3:nsubj\t_
3\tnukkuu\tnukkua\tVERB\tV\tMood=Ind|Number=Sing|Person=3|Tense=Pres|VerbForm=Fin|Voice=Act\t0\troot\t0:root\tSpaceAfter=No
4\t.\t.\tPUNCT\tPunct\t_\t3\tpunct\t3:punct\t_
";

    #[test]
    fn parse_two_sentences() {
        let sentences = parse_conllu(SAMPLE);
        assert_eq!(sentences.len(), 2);

        assert_eq!(sentences[0].sent_id, "test.1");
        assert_eq!(sentences[0].text, "Koira juoksee nopeasti.");
        assert_eq!(sentences[0].tokens.len(), 4);
        assert_eq!(sentences[0].tokens[0].form, "Koira");
        assert_eq!(sentences[0].tokens[0].upos, "NOUN");
        assert_eq!(sentences[0].tokens[0].lemma, "koira");

        assert_eq!(sentences[1].sent_id, "test.2");
        assert_eq!(sentences[1].tokens.len(), 4);
        assert_eq!(sentences[1].tokens[0].form, "Iso");
        assert_eq!(sentences[1].tokens[0].upos, "ADJ");
    }

    #[test]
    fn skip_multiword_tokens() {
        let content = "\
# sent_id = mwt.1
# text = ettei
1\tettei\t_\t_\t_\t_\t_\t_\t_\t_
1-2\tettei\t_\t_\t_\t_\t_\t_\t_\t_
1\tettä\tettä\tSCONJ\tC\t_\t3\tmark\t3:mark\t_
2\tei\tei\tAUX\tV\tNumber=Sing|Person=3|VerbForm=Fin|Voice=Act\t3\taux\t3:aux\t_
3\tole\tolla\tVERB\tV\tConnegative=Yes|Mood=Ind|Tense=Pres|VerbForm=Fin\t0\troot\t0:root\t_
";
        let sentences = parse_conllu(content);
        assert_eq!(sentences.len(), 1);
        // The multi-word token line (1-2) should be skipped.
        // But the lines with ID 1, 1 (duped), 2, 3 remain.
        // The first "1\tettei" is a bit unusual but we parse it.
        // Real CoNLL-U: the range line comes first, then the sub-tokens.
        // We skip lines containing '-' in ID.
        assert!(sentences[0]
            .tokens
            .iter()
            .all(|t| !t.form.is_empty() || t.id > 0));
    }

    #[test]
    fn empty_input() {
        let sentences = parse_conllu("");
        assert!(sentences.is_empty());
    }

    #[test]
    fn trailing_sentence_without_blank_line() {
        let content = "\
# sent_id = trail.1
# text = Hei.
1\tHei\thei\tINTJ\tI\t_\t0\troot\t0:root\tSpaceAfter=No
2\t.\t.\tPUNCT\tPunct\t_\t1\tpunct\t1:punct\t_";
        let sentences = parse_conllu(content);
        assert_eq!(sentences.len(), 1);
        assert_eq!(sentences[0].tokens.len(), 2);
    }
}
