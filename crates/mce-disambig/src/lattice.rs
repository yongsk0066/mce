//! Weighted lattice data structure for disambiguation.
//!
//! A lattice represents a sentence where each word position has one or more
//! candidate readings (morphological analyses). Each reading carries a score
//! (log-probability) reflecting how likely that analysis is in isolation.
//!
//! The Viterbi algorithm operates on this lattice to find the globally optimal
//! reading sequence, considering both emission scores and transition weights.

use mce_core::analysis::Analysis;

/// A single candidate reading at a word position.
///
/// Each reading pairs a morphological [`Analysis`] with a score representing
/// its unigram log-probability (higher values = more likely).
#[derive(Debug, Clone)]
pub struct Reading {
    /// The morphological analysis for this reading.
    pub analysis: Analysis,
    /// Log-probability score (higher = better). Typically <= 0.0 for
    /// log-probabilities, but we allow arbitrary reals for flexibility.
    pub score: f64,
}

impl Reading {
    /// Create a new reading with the given analysis and score.
    pub fn new(analysis: Analysis, score: f64) -> Self {
        Self { analysis, score }
    }
}

/// A node in the disambiguation lattice, representing one word position.
///
/// Each node contains all candidate readings for the word at this position.
/// The `position` field is the zero-based index in the sentence.
#[derive(Debug, Clone)]
pub struct LatticeNode {
    /// Zero-based word position in the sentence.
    pub position: usize,
    /// Candidate readings for this position.
    pub readings: Vec<Reading>,
}

impl LatticeNode {
    /// Create a new lattice node at the given position with the given readings.
    pub fn new(position: usize, readings: Vec<Reading>) -> Self {
        Self { position, readings }
    }

    /// Number of candidate readings at this position.
    pub fn num_readings(&self) -> usize {
        self.readings.len()
    }
}

/// A sentence-level lattice for morphological disambiguation.
///
/// The lattice is a sequence of [`LatticeNode`]s, one per word in the sentence.
/// The Viterbi algorithm finds the path through the lattice that maximizes
/// the total score (emission + transition weights).
#[derive(Debug, Clone)]
pub struct Lattice {
    /// Nodes in sentence order, one per word position.
    pub nodes: Vec<LatticeNode>,
}

impl Lattice {
    /// Create a new empty lattice.
    pub fn new() -> Self {
        Self { nodes: Vec::new() }
    }

    /// Create a lattice from a sequence of analysis candidates.
    ///
    /// Each element of `sentence` is a list of `(Analysis, score)` pairs
    /// for one word position. Positions are assigned sequentially from 0.
    pub fn from_scored(sentence: Vec<Vec<(Analysis, f64)>>) -> Self {
        let nodes = sentence
            .into_iter()
            .enumerate()
            .map(|(pos, readings)| {
                let readings = readings
                    .into_iter()
                    .map(|(analysis, score)| Reading::new(analysis, score))
                    .collect();
                LatticeNode::new(pos, readings)
            })
            .collect();
        Self { nodes }
    }

    /// Number of word positions in the lattice.
    pub fn len(&self) -> usize {
        self.nodes.len()
    }

    /// Whether the lattice is empty (no word positions).
    pub fn is_empty(&self) -> bool {
        self.nodes.is_empty()
    }
}

impl Default for Lattice {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_analysis(class: &str, baseform: &str) -> Analysis {
        let mut a = Analysis::new();
        a.set("CLASS", class);
        a.set("BASEFORM", baseform);
        a
    }

    #[test]
    fn empty_lattice() {
        let lat = Lattice::new();
        assert!(lat.is_empty());
        assert_eq!(lat.len(), 0);
    }

    #[test]
    fn lattice_from_scored() {
        let sentence = vec![
            vec![
                (make_analysis("nimisana", "kuusi"), -1.0),
                (make_analysis("lukusana", "kuusi"), -2.0),
            ],
            vec![(make_analysis("teonsana", "kasvaa"), -0.5)],
        ];
        let lat = Lattice::from_scored(sentence);
        assert_eq!(lat.len(), 2);
        assert_eq!(lat.nodes[0].num_readings(), 2);
        assert_eq!(lat.nodes[1].num_readings(), 1);
        assert_eq!(lat.nodes[0].position, 0);
        assert_eq!(lat.nodes[1].position, 1);
    }

    #[test]
    fn reading_stores_score() {
        let r = Reading::new(make_analysis("nimisana", "talo"), -0.3);
        assert!((r.score - (-0.3)).abs() < f64::EPSILON);
        assert_eq!(r.analysis.get("BASEFORM"), Some("talo"));
    }

    #[test]
    fn lattice_node_position() {
        let node = LatticeNode::new(5, vec![]);
        assert_eq!(node.position, 5);
        assert_eq!(node.num_readings(), 0);
    }
}
