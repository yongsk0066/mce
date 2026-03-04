// String utility functions shared across MCE crates.

/// Compute the Levenshtein edit distance between two byte sequences.
///
/// Uses the standard dynamic-programming algorithm with O(min(n, m)) space.
pub fn levenshtein_distance(a: &[u8], b: &[u8]) -> usize {
    let n = a.len();
    let m = b.len();

    let mut prev: Vec<usize> = (0..=m).collect();
    let mut curr = vec![0usize; m + 1];

    for i in 1..=n {
        curr[0] = i;
        for j in 1..=m {
            let cost = if a[i - 1] == b[j - 1] { 0 } else { 1 };
            curr[j] = (prev[j] + 1).min(curr[j - 1] + 1).min(prev[j - 1] + cost);
        }
        std::mem::swap(&mut prev, &mut curr);
    }

    prev[m]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identical() {
        assert_eq!(levenshtein_distance(b"koira", b"koira"), 0);
    }

    #[test]
    fn one_substitution() {
        assert_eq!(levenshtein_distance(b"koira", b"koiru"), 1);
    }

    #[test]
    fn one_deletion() {
        assert_eq!(levenshtein_distance(b"koira", b"koir"), 1);
    }

    #[test]
    fn one_insertion() {
        assert_eq!(levenshtein_distance(b"koira", b"koiraa"), 1);
    }

    #[test]
    fn empty_strings() {
        assert_eq!(levenshtein_distance(b"", b"abc"), 3);
        assert_eq!(levenshtein_distance(b"abc", b""), 3);
        assert_eq!(levenshtein_distance(b"", b""), 0);
    }
}
