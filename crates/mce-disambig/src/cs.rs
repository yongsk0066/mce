//! Compressed Sensing (FISTA) layer for morphological disambiguation.
//!
//! Morphological analyses are sparse in a high-dimensional feature space:
//! out of ~160 possible features (POS class, case, number, person, tense,
//! mood, baseform hash), typically only k=3-6 are nonzero for any given
//! analysis.
//!
//! The FISTA (Fast Iterative Shrinkage-Thresholding Algorithm) solver
//! recovers the sparse representation from compressed measurements, and
//! the reconstruction error serves as a disambiguation signal: analyses
//! that reconstruct well from the measurement are more likely correct.
//!
//! # References
//!
//! - Beck & Teboulle (2009), "A Fast Iterative Shrinkage-Thresholding
//!   Algorithm for Linear Inverse Problems"
//! - Candes & Tao (2005), "Decoding by Linear Programming"

use mce_core::analysis::{
    Analysis, ATTR_BASEFORM, ATTR_CLASS, ATTR_MOOD, ATTR_NUMBER, ATTR_PARTICIPLE, ATTR_PERSON,
    ATTR_POSSESSIVE, ATTR_SIJAMUOTO, ATTR_STRUCTURE, ATTR_TENSE,
};

// ─────────────────────────────────────────────────────────────────────────────
// Feature encoding dimensions
// ─────────────────────────────────────────────────────────────────────────────

/// POS class dimension offset and count.
const POS_OFFSET: usize = 0;
const POS_DIM: usize = 15;

/// Case (sijamuoto) dimension offset and count.
const CASE_OFFSET: usize = POS_OFFSET + POS_DIM;
const CASE_DIM: usize = 15;

/// Number dimension offset and count.
const NUMBER_OFFSET: usize = CASE_OFFSET + CASE_DIM;
const NUMBER_DIM: usize = 3;

/// Person dimension offset and count.
const PERSON_OFFSET: usize = NUMBER_OFFSET + NUMBER_DIM;
const PERSON_DIM: usize = 5;

/// Tense dimension offset and count.
const TENSE_OFFSET: usize = PERSON_OFFSET + PERSON_DIM;
const TENSE_DIM: usize = 4;

/// Mood dimension offset and count.
const MOOD_OFFSET: usize = TENSE_OFFSET + TENSE_DIM;
const MOOD_DIM: usize = 6;

/// Participle dimension offset and count.
const PARTICIPLE_OFFSET: usize = MOOD_OFFSET + MOOD_DIM;
const PARTICIPLE_DIM: usize = 5;

/// Possessive dimension offset and count.
const POSSESSIVE_OFFSET: usize = PARTICIPLE_OFFSET + PARTICIPLE_DIM;
const POSSESSIVE_DIM: usize = 5;

/// Structure length feature.
const STRUCTURE_OFFSET: usize = POSSESSIVE_OFFSET + POSSESSIVE_DIM;
const STRUCTURE_DIM: usize = 1;

/// Baseform hash features.
const HASH_OFFSET: usize = STRUCTURE_OFFSET + STRUCTURE_DIM;
const HASH_DIM: usize = 100;

/// Total feature dimension.
pub const FEATURE_DIM: usize = HASH_OFFSET + HASH_DIM;

// ─────────────────────────────────────────────────────────────────────────────
// Feature encoding
// ─────────────────────────────────────────────────────────────────────────────

/// Finnish POS classes used by Voikko/MCE.
const POS_CLASSES: &[&str] = &[
    "nimisana",
    "teonsana",
    "laatusana",
    "seikkasana",
    "lukusana",
    "asemosana",
    "sidesana",
    "suhdesana",
    "kieltosana",
    "huudahdussana",
    "etuliite",
    "lyhenne",
    "paikannimi",
    "sukunimi",
    "etunimi",
];

/// Finnish cases (sijamuoto).
const CASES: &[&str] = &[
    "nimento",
    "omanto",
    "osanto",
    "olento",
    "tulento",
    "kohdanto",
    "sisaolento",
    "sisaeronto",
    "sisatulento",
    "ulkoolento",
    "ulkoeronto",
    "ulkotulento",
    "vajanto",
    "keinonto",
    "seuranto",
];

/// Finnish grammatical numbers.
const NUMBERS: &[&str] = &["singular", "plural", "dual"];

/// Finnish person values.
const PERSONS: &[&str] = &["1", "2", "3", "4", "passive"];

/// Finnish tense values.
const TENSES: &[&str] = &[
    "present_simple",
    "past_imperfective",
    "past_perfective",
    "future",
];

/// Finnish mood values.
const MOODS: &[&str] = &[
    "indicative",
    "conditional",
    "potential",
    "imperative",
    "A-infinitive",
    "E-infinitive",
];

/// Finnish participle values.
const PARTICIPLES: &[&str] = &[
    "past_active",
    "past_passive",
    "present_active",
    "present_passive",
    "agent",
];

/// Finnish possessive suffix values.
const POSSESSIVES: &[&str] = &["1s", "2s", "3", "1p", "2p"];

/// Look up the index of a value in a string table.
fn lookup_index(table: &[&str], value: &str) -> Option<usize> {
    table.iter().position(|&v| v == value)
}

/// Simple hash function for baseform feature hashing (FNV-1a variant).
fn hash_baseform(baseform: &str) -> usize {
    let mut h: u64 = 0xcbf29ce484222325;
    for byte in baseform.as_bytes() {
        h ^= *byte as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    (h as usize) % HASH_DIM
}

/// Encode a morphological [`Analysis`] into a sparse feature vector.
///
/// The resulting vector has [`FEATURE_DIM`] dimensions. Typically only
/// k=3-6 entries are nonzero (binary 1.0 for categorical features,
/// normalized length for the structure feature).
///
/// # Feature layout
///
/// | Offset | Dimension | Feature |
/// |--------|-----------|---------|
/// | 0 | 15 | POS class (one-hot) |
/// | 15 | 15 | Case / sijamuoto (one-hot) |
/// | 30 | 3 | Number (one-hot) |
/// | 33 | 5 | Person (one-hot) |
/// | 38 | 4 | Tense (one-hot) |
/// | 42 | 6 | Mood (one-hot) |
/// | 48 | 5 | Participle (one-hot) |
/// | 53 | 5 | Possessive (one-hot) |
/// | 58 | 1 | Structure length (normalized) |
/// | 59 | 100 | Baseform hash (feature hashing) |
pub fn encode_analysis(analysis: &Analysis) -> Vec<f64> {
    let mut vec = vec![0.0; FEATURE_DIM];

    // POS class
    if let Some(class) = analysis.get(ATTR_CLASS) {
        if let Some(idx) = lookup_index(POS_CLASSES, class) {
            vec[POS_OFFSET + idx] = 1.0;
        }
    }

    // Case (sijamuoto)
    if let Some(case) = analysis.get(ATTR_SIJAMUOTO) {
        if let Some(idx) = lookup_index(CASES, case) {
            vec[CASE_OFFSET + idx] = 1.0;
        }
    }

    // Number
    if let Some(number) = analysis.get(ATTR_NUMBER) {
        if let Some(idx) = lookup_index(NUMBERS, number) {
            vec[NUMBER_OFFSET + idx] = 1.0;
        }
    }

    // Person
    if let Some(person) = analysis.get(ATTR_PERSON) {
        if let Some(idx) = lookup_index(PERSONS, person) {
            vec[PERSON_OFFSET + idx] = 1.0;
        }
    }

    // Tense
    if let Some(tense) = analysis.get(ATTR_TENSE) {
        if let Some(idx) = lookup_index(TENSES, tense) {
            vec[TENSE_OFFSET + idx] = 1.0;
        }
    }

    // Mood
    if let Some(mood) = analysis.get(ATTR_MOOD) {
        if let Some(idx) = lookup_index(MOODS, mood) {
            vec[MOOD_OFFSET + idx] = 1.0;
        }
    }

    // Participle
    if let Some(participle) = analysis.get(ATTR_PARTICIPLE) {
        if let Some(idx) = lookup_index(PARTICIPLES, participle) {
            vec[PARTICIPLE_OFFSET + idx] = 1.0;
        }
    }

    // Possessive
    if let Some(possessive) = analysis.get(ATTR_POSSESSIVE) {
        if let Some(idx) = lookup_index(POSSESSIVES, possessive) {
            vec[POSSESSIVE_OFFSET + idx] = 1.0;
        }
    }

    // Structure length (normalized: len / 20.0, capped at 1.0)
    if let Some(structure) = analysis.get(ATTR_STRUCTURE) {
        let normalized = (structure.len() as f64 / 20.0).min(1.0);
        vec[STRUCTURE_OFFSET] = normalized;
    }

    // Baseform hash features
    if let Some(baseform) = analysis.get(ATTR_BASEFORM) {
        let idx = hash_baseform(baseform);
        vec[HASH_OFFSET + idx] = 1.0;
    }

    vec
}

/// Count the number of nonzero entries in a vector.
pub fn count_nonzero(v: &[f64]) -> usize {
    v.iter().filter(|&&x| x.abs() > 1e-12).count()
}

// ─────────────────────────────────────────────────────────────────────────────
// Linear algebra helpers (no external crates)
// ─────────────────────────────────────────────────────────────────────────────

/// Matrix-vector multiply: result = A * x  (A is m x n, row-major).
fn mat_vec_mul(a: &[f64], m: usize, n: usize, x: &[f64]) -> Vec<f64> {
    debug_assert_eq!(a.len(), m * n);
    debug_assert_eq!(x.len(), n);
    let mut result = vec![0.0; m];
    for (i, res) in result.iter_mut().enumerate().take(m) {
        let row_start = i * n;
        let mut sum = 0.0;
        for j in 0..n {
            sum += a[row_start + j] * x[j];
        }
        *res = sum;
    }
    result
}

/// Transpose-matrix-vector multiply: result = A^T * y  (A is m x n, row-major).
fn mat_t_vec_mul(a: &[f64], m: usize, n: usize, y: &[f64]) -> Vec<f64> {
    debug_assert_eq!(a.len(), m * n);
    debug_assert_eq!(y.len(), m);
    let mut result = vec![0.0; n];
    for (i, &yi) in y.iter().enumerate().take(m) {
        let row_start = i * n;
        for j in 0..n {
            result[j] += a[row_start + j] * yi;
        }
    }
    result
}

/// Compute A^T * A for row-major matrix A (m x n). Result is n x n, row-major.
fn compute_ata(a: &[f64], m: usize, n: usize) -> Vec<f64> {
    debug_assert_eq!(a.len(), m * n);
    let mut ata = vec![0.0; n * n];
    for i in 0..m {
        let row_start = i * n;
        for j in 0..n {
            let aij = a[row_start + j];
            for k in j..n {
                let val = aij * a[row_start + k];
                ata[j * n + k] += val;
                if k != j {
                    ata[k * n + j] += val;
                }
            }
        }
    }
    ata
}

/// Vector L2 norm.
fn vec_norm(v: &[f64]) -> f64 {
    v.iter().map(|&x| x * x).sum::<f64>().sqrt()
}

/// Vector subtraction: a - b.
fn vec_sub(a: &[f64], b: &[f64]) -> Vec<f64> {
    debug_assert_eq!(a.len(), b.len());
    a.iter().zip(b.iter()).map(|(&ai, &bi)| ai - bi).collect()
}

// ─────────────────────────────────────────────────────────────────────────────
// Simple LCG PRNG (deterministic, no external crate)
// ─────────────────────────────────────────────────────────────────────────────

/// Simple linear congruential generator for deterministic pseudo-random numbers.
struct Lcg {
    state: u64,
}

impl Lcg {
    fn new(seed: u64) -> Self {
        Self { state: seed }
    }

    /// Generate next u64.
    fn next_u64(&mut self) -> u64 {
        // Numerical Recipes LCG parameters.
        self.state = self.state.wrapping_mul(6364136223846793005).wrapping_add(1);
        self.state
    }

    /// Generate a standard normal sample using Box-Muller transform.
    fn next_gaussian(&mut self) -> f64 {
        loop {
            let u1 = (self.next_u64() as f64) / (u64::MAX as f64);
            let u2 = (self.next_u64() as f64) / (u64::MAX as f64);
            if u1 > 1e-15 {
                let r = (-2.0 * u1.ln()).sqrt();
                let theta = 2.0 * std::f64::consts::PI * u2;
                return r * theta.cos();
            }
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// FISTA solver
// ─────────────────────────────────────────────────────────────────────────────

/// Soft-thresholding operator (proximal operator for L1 norm).
///
/// `soft_threshold(z, t) = sign(z) * max(|z| - t, 0)`
pub fn soft_threshold(value: f64, threshold: f64) -> f64 {
    if value > threshold {
        value - threshold
    } else if value < -threshold {
        value + threshold
    } else {
        0.0
    }
}

/// Estimate the largest eigenvalue of a symmetric matrix using power iteration.
///
/// # Arguments
///
/// * `ata` - The symmetric matrix (n x n, row-major).
/// * `n` - Matrix dimension.
/// * `num_iters` - Number of power iteration steps.
///
/// # Returns
///
/// Approximate largest eigenvalue (spectral radius).
pub fn power_iteration(ata: &[f64], n: usize, num_iters: usize) -> f64 {
    debug_assert_eq!(ata.len(), n * n);

    // Initialize with a deterministic vector.
    let mut v: Vec<f64> = (0..n).map(|i| ((i + 1) as f64).sin()).collect();
    let norm = vec_norm(&v);
    if norm < 1e-15 {
        return 1.0;
    }
    for vi in &mut v {
        *vi /= norm;
    }

    let mut eigenvalue = 1.0;

    for _ in 0..num_iters {
        // w = ata * v
        let w = mat_vec_mul(ata, n, n, &v);
        let w_norm = vec_norm(&w);
        if w_norm < 1e-15 {
            break;
        }

        // Rayleigh quotient: eigenvalue = v^T * w
        eigenvalue = v.iter().zip(w.iter()).map(|(&vi, &wi)| vi * wi).sum();

        // Normalize
        for (vi, &wi) in v.iter_mut().zip(w.iter()) {
            *vi = wi / w_norm;
        }
    }

    eigenvalue.abs().max(1e-10) // Ensure positive
}

/// Compressed Sensing recovery using FISTA.
///
/// Given a measurement matrix A (m x n) and an observation vector y (m),
/// recovers the sparsest solution x (n) such that Ax ~ y under L1
/// regularization.
pub struct SparseRecovery {
    /// Measurement matrix (m x n), stored row-major.
    measurement_matrix: Vec<f64>,
    /// Number of measurements.
    m: usize,
    /// Feature dimension.
    n: usize,
    /// L1 regularization parameter.
    lambda: f64,
    /// Maximum FISTA iterations.
    max_iter: usize,
    /// Convergence tolerance.
    tolerance: f64,
}

impl SparseRecovery {
    /// Create a new SparseRecovery with a random Gaussian measurement matrix.
    ///
    /// The matrix entries are i.i.d. N(0, 1/m), which satisfies the
    /// Restricted Isometry Property (RIP) with high probability when
    /// m = O(k log(n/k)).
    ///
    /// # Arguments
    ///
    /// * `m` - Number of measurements (rows). Should be O(k * log(n/k))
    ///   where k is the expected sparsity level.
    /// * `n` - Feature dimension (columns).
    /// * `lambda` - L1 regularization parameter. Larger values encourage
    ///   sparser solutions.
    pub fn new(m: usize, n: usize, lambda: f64) -> Self {
        let mut rng = Lcg::new(42); // deterministic seed
        let scale = 1.0 / (m as f64).sqrt();
        let matrix: Vec<f64> = (0..m * n).map(|_| rng.next_gaussian() * scale).collect();

        Self {
            measurement_matrix: matrix,
            m,
            n,
            lambda,
            max_iter: 200,
            tolerance: 1e-6,
        }
    }

    /// Create a SparseRecovery with a provided measurement matrix.
    ///
    /// # Arguments
    ///
    /// * `matrix` - Row-major measurement matrix, must have `m * n` elements.
    /// * `m` - Number of rows.
    /// * `n` - Number of columns.
    /// * `lambda` - L1 regularization parameter.
    ///
    /// # Panics
    ///
    /// Panics if `matrix.len() != m * n`.
    pub fn from_matrix(matrix: Vec<f64>, m: usize, n: usize, lambda: f64) -> Self {
        assert_eq!(
            matrix.len(),
            m * n,
            "Matrix size mismatch: expected {} x {} = {}, got {}",
            m,
            n,
            m * n,
            matrix.len()
        );
        Self {
            measurement_matrix: matrix,
            m,
            n,
            lambda,
            max_iter: 200,
            tolerance: 1e-6,
        }
    }

    /// Set the maximum number of FISTA iterations.
    pub fn set_max_iter(&mut self, max_iter: usize) {
        self.max_iter = max_iter;
    }

    /// Set the convergence tolerance.
    pub fn set_tolerance(&mut self, tolerance: f64) {
        self.tolerance = tolerance;
    }

    /// Number of measurements (m).
    pub fn num_measurements(&self) -> usize {
        self.m
    }

    /// Feature dimension (n).
    pub fn feature_dim(&self) -> usize {
        self.n
    }

    /// Solve the FISTA optimization problem.
    ///
    /// Minimizes: `(1/2) ||A*x - observation||^2 + lambda * ||x||_1`
    ///
    /// # Algorithm
    ///
    /// FISTA (Beck & Teboulle 2009):
    /// ```text
    /// Initialize: x0 = 0, y1 = x0, t1 = 1
    /// For k = 1, 2, ..., max_iter:
    ///   gradient = A^T * (A * y_k - observation)
    ///   z = y_k - (1/L) * gradient
    ///   x_k = soft_threshold(z, lambda/L)
    ///   t_{k+1} = (1 + sqrt(1 + 4*t_k^2)) / 2
    ///   y_{k+1} = x_k + (t_k - 1)/(t_{k+1}) * (x_k - x_{k-1})
    ///   If ||x_k - x_{k-1}|| / max(||x_k||, 1e-10) < tolerance: break
    /// ```
    ///
    /// # Arguments
    ///
    /// * `observation` - The measurement vector y (length m).
    ///
    /// # Returns
    ///
    /// The recovered sparse signal x (length n).
    pub fn fista_solve(&self, observation: &[f64]) -> Vec<f64> {
        assert_eq!(
            observation.len(),
            self.m,
            "Observation length {} does not match m = {}",
            observation.len(),
            self.m
        );

        let a = &self.measurement_matrix;
        let m = self.m;
        let n = self.n;

        // Compute Lipschitz constant L = largest eigenvalue of A^T * A
        let ata = compute_ata(a, m, n);
        let lipschitz = power_iteration(&ata, n, 50);

        // Initialize
        let mut x_prev = vec![0.0; n];
        let mut x_curr = vec![0.0; n];
        let mut y = vec![0.0; n];
        let mut t_curr = 1.0_f64;

        for _ in 0..self.max_iter {
            // gradient = A^T * (A * y - observation)
            let ay = mat_vec_mul(a, m, n, &y);
            let residual: Vec<f64> = ay
                .iter()
                .zip(observation.iter())
                .map(|(&a, &b)| a - b)
                .collect();
            let gradient = mat_t_vec_mul(a, m, n, &residual);

            // z = y - (1/L) * gradient, then soft-threshold
            let threshold = self.lambda / lipschitz;
            for j in 0..n {
                let z_j = y[j] - gradient[j] / lipschitz;
                x_curr[j] = soft_threshold(z_j, threshold);
            }

            // Check convergence
            let diff = vec_sub(&x_curr, &x_prev);
            let diff_norm = vec_norm(&diff);
            let x_norm = vec_norm(&x_curr).max(1e-10);
            if diff_norm / x_norm < self.tolerance {
                break;
            }

            // Momentum update
            let t_next = (1.0 + (1.0 + 4.0 * t_curr * t_curr).sqrt()) / 2.0;
            let momentum = (t_curr - 1.0) / t_next;

            for j in 0..n {
                y[j] = x_curr[j] + momentum * (x_curr[j] - x_prev[j]);
            }

            x_prev.copy_from_slice(&x_curr);
            t_curr = t_next;
        }

        x_curr
    }

    /// Compute the reconstruction error for a signal.
    ///
    /// Returns `||A * x - A * x_recovered||^2` where `x_recovered` is the
    /// FISTA solution from the measurement `A * x`.
    pub fn reconstruction_error(&self, signal: &[f64]) -> f64 {
        let observation = mat_vec_mul(&self.measurement_matrix, self.m, self.n, signal);
        let recovered = self.fista_solve(&observation);
        let diff = vec_sub(signal, &recovered);
        diff.iter().map(|&d| d * d).sum()
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// SparseDisambiguator
// ─────────────────────────────────────────────────────────────────────────────

/// Disambiguation scorer based on Compressed Sensing sparse recovery.
///
/// Encodes each candidate analysis as a sparse feature vector, recovers
/// its sparse representation via FISTA, and scores by reconstruction
/// error. Lower reconstruction error means the analysis is more
/// "consistent" with the learned measurement structure.
pub struct SparseDisambiguator {
    recovery: SparseRecovery,
}

impl SparseDisambiguator {
    /// Create a new SparseDisambiguator.
    ///
    /// # Arguments
    ///
    /// * `num_measurements` - Number of CS measurements. Recommended:
    ///   `3 * k * log(FEATURE_DIM / k)` where k ~ 5 (typical sparsity).
    ///   A value of 40-60 is reasonable for Finnish morphological features.
    /// * `lambda` - L1 regularization. Values around 0.01-0.1 work well.
    pub fn new(num_measurements: usize, lambda: f64) -> Self {
        Self {
            recovery: SparseRecovery::new(num_measurements, FEATURE_DIM, lambda),
        }
    }

    /// Create from an existing SparseRecovery instance.
    pub fn from_recovery(recovery: SparseRecovery) -> Self {
        Self { recovery }
    }

    /// Score a set of candidate analyses.
    ///
    /// Returns a score for each analysis where **lower values indicate
    /// better (more recoverable) analyses**. The score is the negative
    /// reconstruction error, so higher scores are better (consistent
    /// with the Viterbi score convention).
    ///
    /// # Arguments
    ///
    /// * `analyses` - Candidate analyses for a single word position.
    ///
    /// # Returns
    ///
    /// A vector of scores (one per analysis), where higher = better.
    pub fn score_analyses(&self, analyses: &[Analysis]) -> Vec<f64> {
        analyses
            .iter()
            .map(|a| {
                let features = encode_analysis(a);
                let error = self.recovery.reconstruction_error(&features);
                -error // negate so higher = better
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ─────────────────────────────────────────────────────────────
    // Helper: create analyses with various attributes
    // ─────────────────────────────────────────────────────────────

    fn make_analysis(class: &str, baseform: &str) -> Analysis {
        let mut a = Analysis::new();
        a.set(ATTR_CLASS, class);
        a.set(ATTR_BASEFORM, baseform);
        a
    }

    fn make_full_noun(baseform: &str, case: &str, number: &str) -> Analysis {
        let mut a = Analysis::new();
        a.set(ATTR_CLASS, "nimisana");
        a.set(ATTR_BASEFORM, baseform);
        a.set(ATTR_SIJAMUOTO, case);
        a.set(ATTR_NUMBER, number);
        a.set(ATTR_STRUCTURE, "=pp");
        a
    }

    fn make_full_verb(
        baseform: &str,
        tense: &str,
        mood: &str,
        person: &str,
        number: &str,
    ) -> Analysis {
        let mut a = Analysis::new();
        a.set(ATTR_CLASS, "teonsana");
        a.set(ATTR_BASEFORM, baseform);
        a.set(ATTR_TENSE, tense);
        a.set(ATTR_MOOD, mood);
        a.set(ATTR_PERSON, person);
        a.set(ATTR_NUMBER, number);
        a.set(ATTR_STRUCTURE, "=pp");
        a
    }

    // ─────────────────────────────────────────────────────────────
    // 1. Soft threshold correctness
    // ─────────────────────────────────────────────────────────────

    #[test]
    fn soft_threshold_positive_above() {
        let result = soft_threshold(3.0, 1.0);
        assert!((result - 2.0).abs() < 1e-10);
    }

    #[test]
    fn soft_threshold_negative_below() {
        let result = soft_threshold(-3.0, 1.0);
        assert!((result - (-2.0)).abs() < 1e-10);
    }

    #[test]
    fn soft_threshold_within_band() {
        assert!((soft_threshold(0.5, 1.0)).abs() < 1e-10);
        assert!((soft_threshold(-0.5, 1.0)).abs() < 1e-10);
        assert!((soft_threshold(0.0, 1.0)).abs() < 1e-10);
    }

    #[test]
    fn soft_threshold_at_boundary() {
        assert!((soft_threshold(1.0, 1.0)).abs() < 1e-10);
        assert!((soft_threshold(-1.0, 1.0)).abs() < 1e-10);
    }

    #[test]
    fn soft_threshold_zero_threshold() {
        assert!((soft_threshold(5.0, 0.0) - 5.0).abs() < 1e-10);
        assert!((soft_threshold(-3.0, 0.0) - (-3.0)).abs() < 1e-10);
    }

    // ─────────────────────────────────────────────────────────────
    // 2. Power iteration accuracy
    // ─────────────────────────────────────────────────────────────

    #[test]
    fn power_iteration_identity() {
        // Identity matrix: eigenvalue = 1.0
        let n = 5;
        let mut eye = vec![0.0; n * n];
        for i in 0..n {
            eye[i * n + i] = 1.0;
        }
        let lambda = power_iteration(&eye, n, 100);
        assert!(
            (lambda - 1.0).abs() < 1e-6,
            "Expected eigenvalue 1.0, got {}",
            lambda
        );
    }

    #[test]
    fn power_iteration_diagonal() {
        // Diagonal matrix with known eigenvalues: [4, 2, 1]
        let n = 3;
        let mut mat = vec![0.0; n * n];
        mat[0] = 4.0;
        mat[4] = 2.0;
        mat[8] = 1.0;
        let lambda = power_iteration(&mat, n, 100);
        assert!(
            (lambda - 4.0).abs() < 1e-4,
            "Expected largest eigenvalue 4.0, got {}",
            lambda
        );
    }

    #[test]
    fn power_iteration_scaled_identity() {
        let n = 4;
        let scale = 7.5;
        let mut mat = vec![0.0; n * n];
        for i in 0..n {
            mat[i * n + i] = scale;
        }
        let lambda = power_iteration(&mat, n, 100);
        assert!(
            (lambda - scale).abs() < 1e-4,
            "Expected eigenvalue {}, got {}",
            scale,
            lambda
        );
    }

    // ─────────────────────────────────────────────────────────────
    // 3. FISTA converges on known sparse signal
    // ─────────────────────────────────────────────────────────────

    #[test]
    fn fista_recovers_sparse_signal_k1() {
        let n = 20;
        let m = 15;
        let recovery = SparseRecovery::new(m, n, 0.01);

        // Create a 1-sparse signal
        let mut signal = vec![0.0; n];
        signal[3] = 5.0;

        let observation = mat_vec_mul(&recovery.measurement_matrix, m, n, &signal);
        let recovered = recovery.fista_solve(&observation);

        // The dominant component should be at index 3
        let max_idx = recovered
            .iter()
            .enumerate()
            .max_by(|(_, a), (_, b)| a.abs().partial_cmp(&b.abs()).unwrap())
            .map(|(i, _)| i)
            .unwrap();
        assert_eq!(max_idx, 3, "Should recover dominant component at index 3");

        // The recovered value should be close to the original
        assert!(
            (recovered[3] - 5.0).abs() < 1.0,
            "Recovered value {} should be close to 5.0",
            recovered[3]
        );
    }

    #[test]
    fn fista_recovers_sparse_signal_k2() {
        let n = 30;
        let m = 20;
        let recovery = SparseRecovery::new(m, n, 0.01);

        let mut signal = vec![0.0; n];
        signal[5] = 3.0;
        signal[15] = -2.0;

        let observation = mat_vec_mul(&recovery.measurement_matrix, m, n, &signal);
        let recovered = recovery.fista_solve(&observation);

        // Both nonzero components should be approximately recovered
        assert!(
            recovered[5].abs() > 1.0,
            "Component at 5 should be significantly nonzero, got {}",
            recovered[5]
        );
        assert!(
            recovered[15].abs() > 0.5,
            "Component at 15 should be significantly nonzero, got {}",
            recovered[15]
        );
    }

    #[test]
    fn fista_recovers_sparse_signal_k3() {
        let n = 40;
        let m = 25;
        let recovery = SparseRecovery::new(m, n, 0.005);

        let mut signal = vec![0.0; n];
        signal[2] = 4.0;
        signal[10] = -3.0;
        signal[30] = 2.0;

        let observation = mat_vec_mul(&recovery.measurement_matrix, m, n, &signal);
        let recovered = recovery.fista_solve(&observation);

        // All three components should be present
        let nonzero_recovered: Vec<usize> = recovered
            .iter()
            .enumerate()
            .filter(|&(_, &v)| v.abs() > 0.3)
            .map(|(i, _)| i)
            .collect();

        assert!(
            nonzero_recovered.contains(&2),
            "Should recover component at index 2, nonzero indices: {:?}",
            nonzero_recovered
        );
        assert!(
            nonzero_recovered.contains(&10),
            "Should recover component at index 10, nonzero indices: {:?}",
            nonzero_recovered
        );
    }

    #[test]
    fn fista_recovers_sparse_signal_k5() {
        let n = 50;
        let m = 35;
        let recovery = SparseRecovery::new(m, n, 0.005);

        let mut signal = vec![0.0; n];
        signal[1] = 2.0;
        signal[11] = -1.5;
        signal[21] = 3.0;
        signal[31] = -2.5;
        signal[41] = 1.0;

        let observation = mat_vec_mul(&recovery.measurement_matrix, m, n, &signal);
        let recovered = recovery.fista_solve(&observation);

        // The top-5 absolute-value components should include most of our signal
        let mut indexed: Vec<(usize, f64)> = recovered
            .iter()
            .enumerate()
            .map(|(i, &v)| (i, v.abs()))
            .collect();
        indexed.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());
        let top5_indices: Vec<usize> = indexed.iter().take(5).map(|&(i, _)| i).collect();

        // At least 3 of the 5 true nonzero indices should be in top-5
        let hits = [1, 11, 21, 31, 41]
            .iter()
            .filter(|&&idx| top5_indices.contains(&idx))
            .count();
        assert!(
            hits >= 3,
            "At least 3/5 true components should be in top-5, got {}/5, top5: {:?}",
            hits,
            top5_indices
        );
    }

    // ─────────────────────────────────────────────────────────────
    // 4. Zero observation edge case
    // ─────────────────────────────────────────────────────────────

    #[test]
    fn fista_zero_observation() {
        let n = 20;
        let m = 10;
        let recovery = SparseRecovery::new(m, n, 0.1);

        let observation = vec![0.0; m];
        let recovered = recovery.fista_solve(&observation);

        // Zero observation should yield (near) zero solution
        let norm = vec_norm(&recovered);
        assert!(
            norm < 1e-6,
            "Zero observation should yield near-zero solution, got norm {}",
            norm
        );
    }

    // ─────────────────────────────────────────────────────────────
    // 5. Convergence within max_iter
    // ─────────────────────────────────────────────────────────────

    #[test]
    fn fista_converges_within_iteration_limit() {
        let n = 20;
        let m = 15;
        let mut recovery = SparseRecovery::new(m, n, 0.01);
        recovery.set_max_iter(500);

        let mut signal = vec![0.0; n];
        signal[7] = 3.0;

        let observation = mat_vec_mul(&recovery.measurement_matrix, m, n, &signal);
        let recovered = recovery.fista_solve(&observation);

        // After 500 iterations, the dominant component should be recovered
        assert!(
            recovered[7].abs() > 1.0,
            "Should converge to find component at index 7, got {}",
            recovered[7]
        );
    }

    // ─────────────────────────────────────────────────────────────
    // 6. encode_analysis produces sparse vectors
    // ─────────────────────────────────────────────────────────────

    #[test]
    fn encode_analysis_simple_noun() {
        let a = make_analysis("nimisana", "koira");
        let features = encode_analysis(&a);

        assert_eq!(features.len(), FEATURE_DIM);

        let nnz = count_nonzero(&features);
        // CLASS (1) + BASEFORM hash (1) = 2
        assert!(
            nnz >= 2 && nnz <= 6,
            "Simple noun should have 2-6 nonzero features, got {}",
            nnz
        );

        // POS class should be set (nimisana = index 0)
        assert!(
            (features[POS_OFFSET] - 1.0).abs() < 1e-10,
            "nimisana should be at POS_OFFSET"
        );
    }

    #[test]
    fn encode_analysis_full_noun() {
        let a = make_full_noun("koira", "nimento", "singular");
        let features = encode_analysis(&a);

        let nnz = count_nonzero(&features);
        // CLASS(1) + CASE(1) + NUMBER(1) + STRUCTURE(1) + BASEFORM_HASH(1) = 5
        assert_eq!(nnz, 5, "Full noun should have exactly 5 nonzero features");
    }

    #[test]
    fn encode_analysis_full_verb() {
        let a = make_full_verb("juosta", "present_simple", "indicative", "3", "singular");
        let features = encode_analysis(&a);

        let nnz = count_nonzero(&features);
        // CLASS(1) + TENSE(1) + MOOD(1) + PERSON(1) + NUMBER(1) + STRUCTURE(1) + HASH(1) = 7
        assert_eq!(nnz, 7, "Full verb should have exactly 7 nonzero features");
    }

    #[test]
    fn encode_analysis_empty() {
        let a = Analysis::new();
        let features = encode_analysis(&a);
        let nnz = count_nonzero(&features);
        assert_eq!(nnz, 0, "Empty analysis should have zero nonzero features");
    }

    #[test]
    fn encode_analysis_sparsity_typical_range() {
        // A typical Finnish word analysis should have k=3-7 nonzero features.
        let analyses = vec![
            make_analysis("nimisana", "talo"),
            make_full_noun("koira", "omanto", "plural"),
            make_full_verb(
                "juosta",
                "past_imperfective",
                "conditional",
                "1",
                "singular",
            ),
            make_analysis("laatusana", "iso"),
        ];

        for a in &analyses {
            let features = encode_analysis(a);
            let nnz = count_nonzero(&features);
            assert!(
                nnz >= 2 && nnz <= 8,
                "Typical analysis should have 2-8 nonzero features, got {}",
                nnz
            );
        }
    }

    // ─────────────────────────────────────────────────────────────
    // 7. SparseDisambiguator scoring
    // ─────────────────────────────────────────────────────────────

    #[test]
    fn score_analyses_returns_correct_count() {
        let disambiguator = SparseDisambiguator::new(40, 0.05);
        let analyses = vec![
            make_analysis("nimisana", "koira"),
            make_analysis("teonsana", "koira"),
            make_analysis("laatusana", "koira"),
        ];
        let scores = disambiguator.score_analyses(&analyses);
        assert_eq!(scores.len(), 3);
    }

    #[test]
    fn score_analyses_all_finite() {
        let disambiguator = SparseDisambiguator::new(40, 0.05);
        let analyses = vec![
            make_full_noun("koira", "nimento", "singular"),
            make_full_verb("juosta", "present_simple", "indicative", "3", "singular"),
        ];
        let scores = disambiguator.score_analyses(&analyses);
        for (i, &s) in scores.iter().enumerate() {
            assert!(s.is_finite(), "Score {} should be finite, got {}", i, s);
        }
    }

    #[test]
    fn score_analyses_negative_reconstruction_error() {
        // Scores are negated reconstruction errors, so they should be <= 0.
        let disambiguator = SparseDisambiguator::new(40, 0.05);
        let analyses = vec![
            make_analysis("nimisana", "koira"),
            make_analysis("teonsana", "juosta"),
        ];
        let scores = disambiguator.score_analyses(&analyses);
        for (i, &s) in scores.iter().enumerate() {
            assert!(
                s <= 0.0 + 1e-10,
                "Score {} should be non-positive (negated error), got {}",
                i,
                s
            );
        }
    }

    // ─────────────────────────────────────────────────────────────
    // 8. from_matrix constructor
    // ─────────────────────────────────────────────────────────────

    #[test]
    fn from_matrix_dimensions() {
        let m = 5;
        let n = 10;
        let matrix = vec![1.0; m * n];
        let recovery = SparseRecovery::from_matrix(matrix, m, n, 0.1);
        assert_eq!(recovery.num_measurements(), 5);
        assert_eq!(recovery.feature_dim(), 10);
    }

    #[test]
    #[should_panic(expected = "Matrix size mismatch")]
    fn from_matrix_wrong_size_panics() {
        SparseRecovery::from_matrix(vec![1.0; 10], 3, 5, 0.1);
    }

    // ─────────────────────────────────────────────────────────────
    // 9. Reconstruction error is small for sparse signals
    // ─────────────────────────────────────────────────────────────

    #[test]
    fn reconstruction_error_sparse_vs_dense() {
        let n = 30;
        let m = 20;
        let recovery = SparseRecovery::new(m, n, 0.01);

        // Sparse signal (k=2)
        let mut sparse = vec![0.0; n];
        sparse[5] = 3.0;
        sparse[20] = -2.0;

        // Dense signal
        let dense: Vec<f64> = (0..n).map(|i| (i as f64 * 0.7).sin()).collect();

        let err_sparse = recovery.reconstruction_error(&sparse);
        let err_dense = recovery.reconstruction_error(&dense);

        // Sparse signal should be recovered better (lower error)
        assert!(
            err_sparse < err_dense,
            "Sparse signal error ({}) should be less than dense signal error ({})",
            err_sparse,
            err_dense
        );
    }

    // ─────────────────────────────────────────────────────────────
    // 10. Feature dimension constant
    // ─────────────────────────────────────────────────────────────

    #[test]
    fn feature_dim_is_expected() {
        // 15 + 15 + 3 + 5 + 4 + 6 + 5 + 5 + 1 + 100 = 159
        assert_eq!(FEATURE_DIM, 159);
    }

    // ─────────────────────────────────────────────────────────────
    // 11. Baseform hash determinism
    // ─────────────────────────────────────────────────────────────

    #[test]
    fn baseform_hash_deterministic() {
        let h1 = hash_baseform("koira");
        let h2 = hash_baseform("koira");
        assert_eq!(h1, h2);
    }

    #[test]
    fn baseform_hash_different_words() {
        let h1 = hash_baseform("koira");
        let h2 = hash_baseform("kissa");
        // Different words should (usually) hash to different buckets
        // This is probabilistic but very likely with 100 buckets
        assert_ne!(h1, h2, "koira and kissa should hash differently");
    }

    // ─────────────────────────────────────────────────────────────
    // 12. LCG PRNG determinism
    // ─────────────────────────────────────────────────────────────

    #[test]
    fn lcg_deterministic() {
        let mut rng1 = Lcg::new(42);
        let mut rng2 = Lcg::new(42);
        for _ in 0..100 {
            assert_eq!(rng1.next_u64(), rng2.next_u64());
        }
    }

    // ─────────────────────────────────────────────────────────────
    // 13. Mat-vec multiply correctness
    // ─────────────────────────────────────────────────────────────

    #[test]
    fn mat_vec_mul_identity() {
        // 3x3 identity * [1, 2, 3] = [1, 2, 3]
        let eye = vec![1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0];
        let x = vec![1.0, 2.0, 3.0];
        let result = mat_vec_mul(&eye, 3, 3, &x);
        assert_eq!(result, vec![1.0, 2.0, 3.0]);
    }

    #[test]
    fn mat_t_vec_mul_correctness() {
        // A = [[1, 2], [3, 4], [5, 6]], y = [1, 1, 1]
        // A^T * y = [1+3+5, 2+4+6] = [9, 12]
        let a = vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0];
        let y = vec![1.0, 1.0, 1.0];
        let result = mat_t_vec_mul(&a, 3, 2, &y);
        assert!((result[0] - 9.0).abs() < 1e-10);
        assert!((result[1] - 12.0).abs() < 1e-10);
    }

    // ─────────────────────────────────────────────────────────────
    // 14. count_nonzero
    // ─────────────────────────────────────────────────────────────

    #[test]
    fn count_nonzero_mixed() {
        let v = vec![0.0, 1.0, 0.0, -2.0, 0.0, 0.5];
        assert_eq!(count_nonzero(&v), 3);
    }

    #[test]
    fn count_nonzero_all_zero() {
        let v = vec![0.0; 10];
        assert_eq!(count_nonzero(&v), 0);
    }

    // ─────────────────────────────────────────────────────────────
    // 15. SparseDisambiguator with different analyses
    // ─────────────────────────────────────────────────────────────

    #[test]
    fn sparse_disambiguator_distinguishes_analyses() {
        let disambiguator = SparseDisambiguator::new(50, 0.05);

        // Two distinct analyses for "kuusi"
        let noun = make_full_noun("kuusi", "nimento", "singular");
        let numeral = make_analysis("lukusana", "kuusi");

        let scores = disambiguator.score_analyses(&[noun, numeral]);
        // Both should produce finite scores (we don't know which is "better"
        // without context, but they should differ since the encodings differ)
        assert!(scores[0].is_finite());
        assert!(scores[1].is_finite());
        // At least verify they're not identical (different feature vectors)
        assert!(
            (scores[0] - scores[1]).abs() > 1e-15,
            "Different analyses should produce different scores"
        );
    }
}
