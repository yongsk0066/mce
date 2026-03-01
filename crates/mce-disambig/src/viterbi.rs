//! Viterbi algorithm for finding the best reading sequence through a lattice.
//!
//! The Viterbi decoder implements the standard dynamic programming algorithm
//! over the disambiguation lattice. It maximizes the total score, which is
//! the sum of:
//! - **Emission scores**: the per-reading log-probabilities stored in the lattice.
//! - **Transition scores**: bigram weights between consecutive readings,
//!   provided by a [`TransitionFn`].
//!
//! Complexity: O(n * |S|^2) where n = sentence length, |S| = max readings per position.
//! This is efficient for morphological disambiguation where |S| is typically small (2-10).

use mce_core::analysis::Analysis;

use crate::lattice::Lattice;

/// A function that computes transition weight between two consecutive readings.
///
/// Given the analysis at position (i-1) and the analysis at position i,
/// returns a log-probability score (higher = more likely transition).
pub type TransitionFn = dyn Fn(&Analysis, &Analysis) -> f64;

/// Find the best reading sequence through the lattice using Viterbi decoding.
///
/// Returns a vector of indices, one per lattice position, indicating which
/// reading was selected at each position. If the lattice is empty, returns
/// an empty vector. If any position has zero readings, returns an empty vector
/// (invalid lattice).
///
/// # Algorithm
///
/// Standard Viterbi over a trellis:
/// - `dp[i][j]` = best total score reaching position `i`, reading `j`
/// - `bp[i][j]` = index of the best predecessor reading at position `i-1`
/// - Score = emission(i, j) + max_{k} (dp[i-1][k] + transition(k, j))
///
/// The emission score is the `Reading::score` field.
///
/// # Arguments
///
/// * `lattice` - The disambiguation lattice.
/// * `transition` - Transition weight function between consecutive analyses.
///
/// # Returns
///
/// A vector of reading indices, one per position, representing the best path.
pub fn viterbi(lattice: &Lattice, transition: &TransitionFn) -> Vec<usize> {
    if lattice.is_empty() {
        return Vec::new();
    }

    let n = lattice.len();

    // Validate: every position must have at least one reading.
    for node in &lattice.nodes {
        if node.readings.is_empty() {
            return Vec::new();
        }
    }

    // dp[i][j] = best score reaching position i, reading j
    let mut dp: Vec<Vec<f64>> = Vec::with_capacity(n);
    // bp[i][j] = best predecessor reading index at position i-1
    let mut bp: Vec<Vec<usize>> = Vec::with_capacity(n);

    // Initialize position 0: emission scores only, no transitions.
    let first_node = &lattice.nodes[0];
    dp.push(first_node.readings.iter().map(|r| r.score).collect());
    bp.push(vec![0; first_node.num_readings()]); // no predecessors for position 0

    // Forward pass: fill dp and bp tables.
    for i in 1..n {
        let curr_node = &lattice.nodes[i];
        let prev_node = &lattice.nodes[i - 1];
        let num_curr = curr_node.num_readings();
        let prev_dp = &dp[i - 1];

        let mut dp_row = vec![f64::NEG_INFINITY; num_curr];
        let mut bp_row = vec![0usize; num_curr];

        for j in 0..num_curr {
            let emission = curr_node.readings[j].score;
            let curr_analysis = &curr_node.readings[j].analysis;

            for (k, (prev_score, prev_reading)) in
                prev_dp.iter().zip(prev_node.readings.iter()).enumerate()
            {
                let trans = transition(&prev_reading.analysis, curr_analysis);
                let total = prev_score + trans + emission;

                if total > dp_row[j] {
                    dp_row[j] = total;
                    bp_row[j] = k;
                }
            }
        }

        dp.push(dp_row);
        bp.push(bp_row);
    }

    // Backward pass: trace the best path.
    let mut path = vec![0usize; n];

    // Find the best reading at the last position.
    let last_dp = &dp[n - 1];
    let mut best_idx = 0;
    let mut best_score = f64::NEG_INFINITY;
    for (j, &score) in last_dp.iter().enumerate() {
        if score > best_score {
            best_score = score;
            best_idx = j;
        }
    }
    path[n - 1] = best_idx;

    // Trace back through backpointers.
    for i in (1..n).rev() {
        path[i - 1] = bp[i][path[i]];
    }

    path
}

/// Find the best reading sequence and return the corresponding analyses.
///
/// Convenience wrapper around [`viterbi`] that returns the selected
/// [`Analysis`] objects rather than indices.
pub fn viterbi_best(lattice: &Lattice, transition: &TransitionFn) -> Vec<Analysis> {
    let path = viterbi(lattice, transition);
    path.iter()
        .enumerate()
        .map(|(i, &j)| lattice.nodes[i].readings[j].analysis.clone())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lattice::{Lattice, LatticeNode, Reading};

    fn make_analysis(class: &str, baseform: &str) -> Analysis {
        let mut a = Analysis::new();
        a.set("CLASS", class);
        a.set("BASEFORM", baseform);
        a
    }

    /// Trivial lattice: one reading per position. Viterbi must pick them all.
    #[test]
    fn single_reading_per_position() {
        let lattice = Lattice {
            nodes: vec![
                LatticeNode::new(
                    0,
                    vec![Reading::new(make_analysis("nimisana", "koira"), -1.0)],
                ),
                LatticeNode::new(
                    1,
                    vec![Reading::new(make_analysis("teonsana", "juosta"), -0.5)],
                ),
                LatticeNode::new(
                    2,
                    vec![Reading::new(make_analysis("nimisana", "piha"), -1.2)],
                ),
            ],
        };

        let transition: Box<TransitionFn> = Box::new(|_, _| 0.0);
        let path = viterbi(&lattice, &transition);
        assert_eq!(path, vec![0, 0, 0]);
    }

    /// Three-position lattice with known weights. The transition function
    /// favors NOUN->VERB->NOUN transitions.
    #[test]
    fn three_position_known_weights() {
        // Position 0: NOUN(-1.0) or NUM(-0.8)
        // Position 1: VERB(-0.5) or ADJ(-1.5)
        // Position 2: NOUN(-0.7) or ADV(-0.3)
        let lattice = Lattice {
            nodes: vec![
                LatticeNode::new(
                    0,
                    vec![
                        Reading::new(make_analysis("nimisana", "kuusi"), -1.0),
                        Reading::new(make_analysis("lukusana", "kuusi"), -0.8),
                    ],
                ),
                LatticeNode::new(
                    1,
                    vec![
                        Reading::new(make_analysis("teonsana", "kasvaa"), -0.5),
                        Reading::new(make_analysis("laatusana", "korkea"), -1.5),
                    ],
                ),
                LatticeNode::new(
                    2,
                    vec![
                        Reading::new(make_analysis("nimisana", "puutarha"), -0.7),
                        Reading::new(make_analysis("seikkasana", "hyvin"), -0.3),
                    ],
                ),
            ],
        };

        // Transition: NOUN->VERB = -0.2, VERB->NOUN = -0.3, everything else = -2.0
        let transition: Box<TransitionFn> = Box::new(|prev, curr| {
            let prev_class = prev.get("CLASS").unwrap_or("");
            let curr_class = curr.get("CLASS").unwrap_or("");
            match (prev_class, curr_class) {
                ("nimisana", "teonsana") => -0.2,
                ("teonsana", "nimisana") => -0.3,
                ("nimisana", "nimisana") => -0.5,
                _ => -2.0,
            }
        });

        let path = viterbi(&lattice, &transition);
        // Best path: NOUN(-1.0) -> VERB(-0.5) -> NOUN(-0.7)
        //   Score: -1.0 + (-0.2 + -0.5) + (-0.3 + -0.7) = -2.7
        //
        // Compare NUM(-0.8) -> VERB(-0.5) -> NOUN(-0.7):
        //   Score: -0.8 + (-2.0 + -0.5) + (-0.3 + -0.7) = -4.3
        //
        // NOUN->VERB->NOUN wins due to good transitions.
        assert_eq!(path, vec![0, 0, 0]); // NOUN, VERB, NOUN
    }

    /// Uniform transitions: should pick readings with highest emission scores.
    #[test]
    fn uniform_transitions_picks_best_emission() {
        let lattice = Lattice {
            nodes: vec![
                LatticeNode::new(
                    0,
                    vec![
                        Reading::new(make_analysis("nimisana", "a"), -2.0),
                        Reading::new(make_analysis("teonsana", "b"), -1.0), // best
                    ],
                ),
                LatticeNode::new(
                    1,
                    vec![
                        Reading::new(make_analysis("nimisana", "c"), -0.5), // best
                        Reading::new(make_analysis("teonsana", "d"), -1.5),
                    ],
                ),
            ],
        };

        let transition: Box<TransitionFn> = Box::new(|_, _| 0.0);
        let path = viterbi(&lattice, &transition);
        assert_eq!(path, vec![1, 0]); // highest emission at each position
    }

    /// Transition weights can override emission preferences.
    #[test]
    fn transition_overrides_emission() {
        // Position 0: A(-1.0) or B(-0.5)  -- B has better emission
        // Position 1: C(-1.0) or D(-0.5)  -- D has better emission
        //
        // But A->C has transition +5.0, everything else = 0.0
        // So A->C path: -1.0 + (5.0 + -1.0) = 3.0
        //    B->D path: -0.5 + (0.0 + -0.5) = -1.0
        let lattice = Lattice {
            nodes: vec![
                LatticeNode::new(
                    0,
                    vec![
                        Reading::new(make_analysis("A", "a"), -1.0),
                        Reading::new(make_analysis("B", "b"), -0.5),
                    ],
                ),
                LatticeNode::new(
                    1,
                    vec![
                        Reading::new(make_analysis("C", "c"), -1.0),
                        Reading::new(make_analysis("D", "d"), -0.5),
                    ],
                ),
            ],
        };

        let transition: Box<TransitionFn> = Box::new(|prev, curr| {
            let p = prev.get("CLASS").unwrap_or("");
            let c = curr.get("CLASS").unwrap_or("");
            if p == "A" && c == "C" {
                5.0
            } else {
                0.0
            }
        });

        let path = viterbi(&lattice, &transition);
        assert_eq!(path, vec![0, 0]); // A->C despite worse individual emissions
    }

    /// Empty lattice returns empty path.
    #[test]
    fn empty_lattice() {
        let lattice = Lattice::new();
        let transition: Box<TransitionFn> = Box::new(|_, _| 0.0);
        let path = viterbi(&lattice, &transition);
        assert!(path.is_empty());
    }

    /// Single-word sentence (no transitions needed).
    #[test]
    fn single_word() {
        let lattice = Lattice {
            nodes: vec![LatticeNode::new(
                0,
                vec![
                    Reading::new(make_analysis("nimisana", "x"), -2.0),
                    Reading::new(make_analysis("teonsana", "y"), -0.5),
                    Reading::new(make_analysis("lukusana", "z"), -1.0),
                ],
            )],
        };

        let transition: Box<TransitionFn> = Box::new(|_, _| 0.0);
        let path = viterbi(&lattice, &transition);
        assert_eq!(path, vec![1]); // best emission score
    }

    /// A position with zero readings is invalid; returns empty path.
    #[test]
    fn empty_readings_returns_empty() {
        let lattice = Lattice {
            nodes: vec![
                LatticeNode::new(0, vec![Reading::new(make_analysis("nimisana", "x"), -1.0)]),
                LatticeNode::new(1, vec![]), // no readings!
            ],
        };

        let transition: Box<TransitionFn> = Box::new(|_, _| 0.0);
        let path = viterbi(&lattice, &transition);
        assert!(path.is_empty());
    }

    /// viterbi_best returns Analysis objects.
    #[test]
    fn viterbi_best_returns_analyses() {
        let lattice = Lattice {
            nodes: vec![
                LatticeNode::new(
                    0,
                    vec![
                        Reading::new(make_analysis("nimisana", "koira"), -1.0),
                        Reading::new(make_analysis("teonsana", "koira"), -2.0),
                    ],
                ),
                LatticeNode::new(
                    1,
                    vec![Reading::new(make_analysis("teonsana", "juosta"), -0.5)],
                ),
            ],
        };

        let transition: Box<TransitionFn> = Box::new(|_, _| 0.0);
        let result = viterbi_best(&lattice, &transition);
        assert_eq!(result.len(), 2);
        assert_eq!(result[0].get("BASEFORM"), Some("koira"));
        assert_eq!(result[0].get("CLASS"), Some("nimisana"));
        assert_eq!(result[1].get("BASEFORM"), Some("juosta"));
    }

    /// Longer sentence (5 positions) to verify DP correctness.
    #[test]
    fn five_position_sentence() {
        // Simulate: DET NOUN VERB ADJ NOUN
        // Each position has 2 readings. Transitions strongly favor the above pattern.
        let lattice = Lattice {
            nodes: vec![
                LatticeNode::new(
                    0,
                    vec![
                        Reading::new(make_analysis("DET", "se"), -1.0),
                        Reading::new(make_analysis("PRON", "se"), -0.8),
                    ],
                ),
                LatticeNode::new(
                    1,
                    vec![
                        Reading::new(make_analysis("NOUN", "koira"), -0.5),
                        Reading::new(make_analysis("VERB", "koira"), -1.5),
                    ],
                ),
                LatticeNode::new(
                    2,
                    vec![
                        Reading::new(make_analysis("VERB", "juosta"), -0.3),
                        Reading::new(make_analysis("NOUN", "juosta"), -1.0),
                    ],
                ),
                LatticeNode::new(
                    3,
                    vec![
                        Reading::new(make_analysis("ADJ", "nopea"), -0.4),
                        Reading::new(make_analysis("ADV", "nopea"), -0.6),
                    ],
                ),
                LatticeNode::new(
                    4,
                    vec![
                        Reading::new(make_analysis("NOUN", "piha"), -0.7),
                        Reading::new(make_analysis("VERB", "piha"), -1.2),
                    ],
                ),
            ],
        };

        let transition: Box<TransitionFn> = Box::new(|prev, curr| {
            let p = prev.get("CLASS").unwrap_or("");
            let c = curr.get("CLASS").unwrap_or("");
            match (p, c) {
                ("DET", "NOUN") => -0.1,
                ("NOUN", "VERB") => -0.2,
                ("VERB", "ADJ") => -0.3,
                ("ADJ", "NOUN") => -0.2,
                _ => -3.0,
            }
        });

        let path = viterbi(&lattice, &transition);
        // Expected: DET -> NOUN -> VERB -> ADJ -> NOUN
        assert_eq!(path, vec![0, 0, 0, 0, 0]);
    }
}
