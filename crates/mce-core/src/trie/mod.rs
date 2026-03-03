//! M1: Succinct Trie — LOUDS-encoded dictionary.
//!
//! MCE 아키텍처의 첫 번째 기계(M1).
//! 사전 검색, 맞춤법 확인, 퍼지 매칭을 위한 공간 효율적 trie.
//!
//! - 메모리: ~2 bits/node + labels (정보이론적 하한 근접)
//! - 시간: O(|key|) exact lookup, O(|key| * k) fuzzy matching

mod bitvec;

pub use bitvec::BitVec;

/// LOUDS(Level-Order Unary Degree Sequence) 인코딩 기반 succinct trie.
///
/// 일반 trie 대비 메모리를 ~10배 절약하면서 동일한 시간 복잡도를 유지한다.
/// Rank/Select 연산으로 parent-child 관계를 O(1)에 탐색.
pub struct SuccinctTrie {
    /// LOUDS bit vector: 각 노드의 자식 수를 unary로 인코딩.
    /// 자식마다 '1', 자식 끝에 '0'.
    tree: BitVec,
    /// 각 edge의 label (level-order).
    labels: Vec<u8>,
    /// 각 노드가 유효한 키의 끝인지 여부.
    is_terminal: BitVec,
}

impl SuccinctTrie {
    /// 정확한 키 검색. 키가 사전에 존재하면 `true`.
    pub fn contains(&self, key: &[u8]) -> bool {
        let mut node = 0usize; // root

        for &byte in key {
            match self.find_child(node, byte) {
                Some(child) => node = child,
                None => return false,
            }
        }

        self.is_terminal.get(node)
    }

    /// 편집 거리 `max_edits` 이내의 모든 키를 검색.
    /// 결과는 (편집 거리 오름차순, 사전순) 정렬.
    pub fn fuzzy_search(&self, query: &[u8], max_edits: usize) -> Vec<Vec<u8>> {
        if self.tree.is_empty() {
            return Vec::new();
        }

        let query_len = query.len();

        // Initial DP row for the root node (no characters consumed yet).
        // row[j] = edit distance between empty string and query[0..j].
        let initial_row: Vec<usize> = (0..=query_len).collect();

        // Collect results as (edit_distance, key).
        let mut results: Vec<(usize, Vec<u8>)> = Vec::new();

        // Check the root itself (empty key) — it's terminal if the empty
        // string was inserted.
        if self.is_terminal.get(0) && initial_row[query_len] <= max_edits {
            results.push((initial_row[query_len], Vec::new()));
        }

        // DFS stack: (node, dp_row, key_so_far)
        let mut stack: Vec<(usize, Vec<usize>, Vec<u8>)> = Vec::new();

        // Seed the stack with children of the root.
        for (label, child_node) in self.children(0) {
            stack.push((child_node, initial_row.clone(), vec![label]));
        }

        while let Some((node, parent_row, key)) = stack.pop() {
            let ch = *key.last().unwrap();

            // Compute the new DP row for this node's edge label.
            let mut current_row = Vec::with_capacity(query_len + 1);
            current_row.push(parent_row[0] + 1); // deletion from key

            for j in 1..=query_len {
                let cost = if query[j - 1] == ch { 0 } else { 1 };
                let val = (parent_row[j] + 1) // deletion from key
                    .min(current_row[j - 1] + 1) // insertion into key
                    .min(parent_row[j - 1] + cost); // substitution
                current_row.push(val);
            }

            // Prune: if the minimum value in the row exceeds max_edits,
            // no descendant can produce a match.
            let min_in_row = *current_row.iter().min().unwrap();
            if min_in_row > max_edits {
                continue;
            }

            // If this node is terminal and the final column is within
            // max_edits, record the match.
            if self.is_terminal.get(node) && current_row[query_len] <= max_edits {
                results.push((current_row[query_len], key.clone()));
            }

            // Push children onto the stack for further exploration.
            for (child_label, child_node) in self.children(node) {
                let mut child_key = key.clone();
                child_key.push(child_label);
                stack.push((child_node, current_row.clone(), child_key));
            }
        }

        // Sort by (edit_distance, key) — ascending distance, then
        // lexicographic order.
        results.sort();
        results.into_iter().map(|(_, key)| key).collect()
    }

    /// LOUDS에서 node의 자식 중 label이 `byte`인 것을 찾는다.
    fn find_child(&self, node: usize, byte: u8) -> Option<usize> {
        // LOUDS에서 node의 첫 번째 자식 위치:
        // first_child_pos = select0(node) + 1 in tree bitvec
        let child_start = self.tree.select0(node)? + 1;

        // 자식들을 순회 (연속된 '1' 비트)
        let mut pos = child_start;
        // labels 배열은 super-root 제외한 '1' 비트에 대응.
        // super-root의 '1' (position 0)은 label이 없으므로 rank1 - 1.
        let mut label_idx = self.tree.rank1(child_start) - 1;

        while pos < self.tree.len() && self.tree.get(pos) {
            if label_idx < self.labels.len() && self.labels[label_idx] == byte {
                // 자식 노드 번호 = rank1(pos+1) - 1 (0-indexed)
                return Some(self.tree.rank1(pos + 1) - 1);
            }
            pos += 1;
            label_idx += 1;
        }

        None
    }

    /// LOUDS에서 node의 모든 자식을 (label, child_node) 목록으로 반환한다.
    fn children(&self, node: usize) -> Vec<(u8, usize)> {
        let mut result = Vec::new();
        let child_start = match self.tree.select0(node) {
            Some(s) => s + 1,
            None => return result,
        };

        let mut pos = child_start;
        let mut label_idx = self.tree.rank1(child_start) - 1;

        while pos < self.tree.len() && self.tree.get(pos) {
            if label_idx < self.labels.len() {
                let child_node = self.tree.rank1(pos + 1) - 1;
                result.push((self.labels[label_idx], child_node));
            }
            pos += 1;
            label_idx += 1;
        }

        result
    }

    /// 전체 키 수.
    pub fn len(&self) -> usize {
        self.is_terminal.count_ones()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Serialize this trie to a binary format.
    ///
    /// Format:
    /// - Magic: `b"MCT1"` (4 bytes)
    /// - `labels.len()` as u64 LE (8 bytes)
    /// - `labels` raw bytes
    /// - `tree` BitVec (serialized)
    /// - `is_terminal` BitVec (serialized)
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut out = Vec::new();
        out.extend_from_slice(b"MCT1");
        out.extend_from_slice(&(self.labels.len() as u64).to_le_bytes());
        out.extend_from_slice(&self.labels);
        self.tree.to_bytes_into(&mut out);
        self.is_terminal.to_bytes_into(&mut out);
        out
    }

    /// Deserialize a trie from bytes produced by [`to_bytes`](Self::to_bytes).
    ///
    /// Returns `None` if the data is malformed or too short.
    pub fn from_bytes(data: &[u8]) -> Option<Self> {
        if data.len() < 12 {
            return None;
        }
        if &data[0..4] != b"MCT1" {
            return None;
        }
        let labels_len = u64::from_le_bytes(data[4..12].try_into().ok()?) as usize;
        let mut offset = 12;
        if data.len() < offset + labels_len {
            return None;
        }
        let labels = data[offset..offset + labels_len].to_vec();
        offset += labels_len;

        let (tree, consumed) = BitVec::from_bytes(&data[offset..])?;
        offset += consumed;

        let (is_terminal, _consumed) = BitVec::from_bytes(&data[offset..])?;

        Some(SuccinctTrie {
            tree,
            labels,
            is_terminal,
        })
    }

    /// Approximate heap memory used by this trie in bytes.
    ///
    /// Includes the LOUDS bit vector, labels array, and terminal bit vector.
    /// Does not include the size of the struct itself (stack allocation).
    pub fn heap_size_in_bytes(&self) -> usize {
        self.tree.heap_size_in_bytes()
            + self.labels.capacity() * std::mem::size_of::<u8>()
            + self.is_terminal.heap_size_in_bytes()
    }
}

/// 정렬된 키 목록에서 SuccinctTrie를 구축하는 빌더.
pub struct TrieBuilder {
    keys: Vec<Vec<u8>>,
}

impl TrieBuilder {
    pub fn new() -> Self {
        Self { keys: Vec::new() }
    }

    /// 키 추가. 모든 키 추가 후 `build()` 호출 전에 정렬됨.
    pub fn insert(&mut self, key: impl Into<Vec<u8>>) {
        self.keys.push(key.into());
    }

    /// Succinct Trie를 구축한다. BFS 순서로 LOUDS 인코딩을 생성.
    pub fn build(mut self) -> SuccinctTrie {
        self.keys.sort();
        self.keys.dedup();

        if self.keys.is_empty() {
            return SuccinctTrie {
                tree: BitVec::new(),
                labels: Vec::new(),
                is_terminal: BitVec::new(),
            };
        }

        // Phase 1: Build a plain trie in memory
        let mut nodes: Vec<TrieNode> = vec![TrieNode::default()]; // root
        for key in &self.keys {
            let mut current = 0;
            for &byte in key.iter() {
                let next = nodes[current]
                    .children
                    .iter()
                    .find(|&&(label, _)| label == byte)
                    .map(|&(_, idx)| idx);
                current = match next {
                    Some(idx) => idx,
                    None => {
                        let idx = nodes.len();
                        nodes.push(TrieNode::default());
                        nodes[current].children.push((byte, idx));
                        idx
                    }
                };
            }
            nodes[current].is_terminal = true;
        }

        // Phase 2: BFS to build LOUDS encoding
        let mut tree_bits = Vec::new();
        let mut labels = Vec::new();
        let mut terminal_bits = Vec::new();

        // Super-root: one child (the real root)
        tree_bits.push(true); // 1 child
        tree_bits.push(false); // end

        let mut queue = std::collections::VecDeque::new();
        queue.push_back(0);

        while let Some(node_idx) = queue.pop_front() {
            let node = &nodes[node_idx];
            terminal_bits.push(node.is_terminal);

            for &(label, child_idx) in &node.children {
                tree_bits.push(true);
                labels.push(label);
                queue.push_back(child_idx);
            }
            tree_bits.push(false); // end of this node's children
        }

        SuccinctTrie {
            tree: BitVec::from_bits(&tree_bits),
            labels,
            is_terminal: BitVec::from_bits(&terminal_bits),
        }
    }
}

impl Default for TrieBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Default)]
struct TrieNode {
    children: Vec<(u8, usize)>,
    is_terminal: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn build_test_trie() -> SuccinctTrie {
        let mut builder = TrieBuilder::new();
        builder.insert(b"cat".to_vec());
        builder.insert(b"car".to_vec());
        builder.insert(b"card".to_vec());
        builder.insert(b"dog".to_vec());
        builder.build()
    }

    #[test]
    fn contains_existing_keys() {
        let trie = build_test_trie();
        assert!(trie.contains(b"cat"));
        assert!(trie.contains(b"car"));
        assert!(trie.contains(b"card"));
        assert!(trie.contains(b"dog"));
    }

    #[test]
    fn does_not_contain_missing_keys() {
        let trie = build_test_trie();
        assert!(!trie.contains(b"ca"));
        assert!(!trie.contains(b"cards"));
        assert!(!trie.contains(b"do"));
        assert!(!trie.contains(b"xyz"));
    }

    #[test]
    fn empty_trie() {
        let trie = TrieBuilder::new().build();
        assert!(trie.is_empty());
        assert!(!trie.contains(b"anything"));
    }

    #[test]
    fn trie_len() {
        let trie = build_test_trie();
        assert_eq!(trie.len(), 4);
    }

    #[test]
    fn single_key() {
        let mut builder = TrieBuilder::new();
        builder.insert(b"hello".to_vec());
        let trie = builder.build();
        assert!(trie.contains(b"hello"));
        assert!(!trie.contains(b"hell"));
        assert_eq!(trie.len(), 1);
    }

    #[test]
    fn prefix_keys() {
        let mut builder = TrieBuilder::new();
        builder.insert(b"a".to_vec());
        builder.insert(b"ab".to_vec());
        builder.insert(b"abc".to_vec());
        let trie = builder.build();
        assert!(trie.contains(b"a"));
        assert!(trie.contains(b"ab"));
        assert!(trie.contains(b"abc"));
        assert!(!trie.contains(b"abcd"));
        assert_eq!(trie.len(), 3);
    }

    // ── fuzzy_search tests ──────────────────────────────────────

    #[test]
    fn fuzzy_search_exact_match() {
        let trie = build_test_trie(); // cat, car, card, dog
        let results = trie.fuzzy_search(b"cat", 0);
        assert_eq!(results, vec![b"cat".to_vec()]);

        let results = trie.fuzzy_search(b"dog", 0);
        assert_eq!(results, vec![b"dog".to_vec()]);

        // Non-existent key yields nothing at distance 0.
        let results = trie.fuzzy_search(b"cab", 0);
        assert!(results.is_empty());
    }

    #[test]
    fn fuzzy_search_one_edit() {
        let trie = build_test_trie(); // cat, car, card, dog

        // Substitution: "cot" -> "cat" (1 edit)
        let results = trie.fuzzy_search(b"cot", 1);
        assert!(results.contains(&b"cat".to_vec()));

        // Deletion: "cars" has distance 1 from "car" (delete 's')
        let results = trie.fuzzy_search(b"cars", 1);
        assert!(results.contains(&b"car".to_vec()));
        assert!(results.contains(&b"card".to_vec()));

        // Insertion: "ca" -> "cat" (insert 't'), "car" (insert 'r')
        let results = trie.fuzzy_search(b"ca", 1);
        assert!(results.contains(&b"cat".to_vec()));
        assert!(results.contains(&b"car".to_vec()));
    }

    #[test]
    fn fuzzy_search_two_edits() {
        let trie = build_test_trie(); // cat, car, card, dog

        // "dag" -> "dog" (1 sub), "car" (2 edits), "cat" (2 edits)
        let results = trie.fuzzy_search(b"dag", 2);
        assert!(results.contains(&b"dog".to_vec()));
        assert!(results.contains(&b"car".to_vec()));
        assert!(results.contains(&b"cat".to_vec()));
    }

    #[test]
    fn fuzzy_search_no_match() {
        let trie = build_test_trie(); // cat, car, card, dog

        // "xyz" is distance >= 3 from all keys.
        let results = trie.fuzzy_search(b"xyz", 1);
        assert!(results.is_empty());
    }

    #[test]
    fn fuzzy_search_empty_query() {
        let trie = build_test_trie(); // cat, car, card, dog (all length 3-4)

        // Empty query with max_edits=0 matches only the empty key (not present).
        let results = trie.fuzzy_search(b"", 0);
        assert!(results.is_empty());

        // Empty query with max_edits=3 matches keys of length <= 3.
        let results = trie.fuzzy_search(b"", 3);
        assert!(results.contains(&b"cat".to_vec()));
        assert!(results.contains(&b"car".to_vec()));
        assert!(results.contains(&b"dog".to_vec()));
        // "card" has length 4 => distance 4 from empty > 3, so excluded.
        assert!(!results.contains(&b"card".to_vec()));
    }

    #[test]
    fn fuzzy_search_empty_trie() {
        let trie = TrieBuilder::new().build();
        let results = trie.fuzzy_search(b"abc", 2);
        assert!(results.is_empty());
    }

    // ── serialization tests ──────────────────────────────────────

    #[test]
    fn trie_to_bytes_from_bytes_roundtrip() {
        let trie = build_test_trie(); // cat, car, card, dog
        let bytes = trie.to_bytes();
        let recovered = SuccinctTrie::from_bytes(&bytes).expect("deserialization should succeed");

        assert!(recovered.contains(b"cat"));
        assert!(recovered.contains(b"car"));
        assert!(recovered.contains(b"card"));
        assert!(recovered.contains(b"dog"));
        assert!(!recovered.contains(b"ca"));
        assert!(!recovered.contains(b"xyz"));
        assert_eq!(recovered.len(), 4);
    }

    #[test]
    fn trie_roundtrip_empty() {
        let trie = TrieBuilder::new().build();
        let bytes = trie.to_bytes();
        let recovered = SuccinctTrie::from_bytes(&bytes).expect("empty trie roundtrip");
        assert!(recovered.is_empty());
        assert!(!recovered.contains(b"anything"));
    }

    #[test]
    fn trie_roundtrip_single_key() {
        let mut builder = TrieBuilder::new();
        builder.insert(b"hello".to_vec());
        let trie = builder.build();
        let bytes = trie.to_bytes();
        let recovered = SuccinctTrie::from_bytes(&bytes).unwrap();
        assert!(recovered.contains(b"hello"));
        assert!(!recovered.contains(b"hell"));
        assert_eq!(recovered.len(), 1);
    }

    #[test]
    fn trie_roundtrip_preserves_fuzzy_search() {
        let trie = build_test_trie();
        let bytes = trie.to_bytes();
        let recovered = SuccinctTrie::from_bytes(&bytes).unwrap();

        let original = trie.fuzzy_search(b"cot", 1);
        let restored = recovered.fuzzy_search(b"cot", 1);
        assert_eq!(original, restored);
    }

    #[test]
    fn trie_from_bytes_rejects_garbage() {
        assert!(SuccinctTrie::from_bytes(&[0, 0, 0, 0]).is_none());
        assert!(SuccinctTrie::from_bytes(b"MCT1").is_none()); // too short
        assert!(SuccinctTrie::from_bytes(&[]).is_none());
    }

    #[test]
    fn fuzzy_search_sorted_by_distance() {
        let mut builder = TrieBuilder::new();
        builder.insert(b"abc".to_vec());
        builder.insert(b"ab".to_vec());
        builder.insert(b"abcd".to_vec());
        builder.insert(b"axyz".to_vec());
        builder.insert(b"xyz".to_vec());
        let trie = builder.build();

        let results = trie.fuzzy_search(b"abc", 3);

        // Expected distances:
        //   "abc"  -> 0
        //   "ab"   -> 1 (deletion)
        //   "abcd" -> 1 (insertion)
        //   "axyz" -> 3 (3 substitutions)
        //   "xyz"  -> 3 (3 edits)
        // Sorted by distance, then lexicographic:
        assert_eq!(results[0], b"abc".to_vec()); // distance 0
        // distance 1 group: "ab" < "abcd" lexicographically
        assert_eq!(results[1], b"ab".to_vec());
        assert_eq!(results[2], b"abcd".to_vec());
        // distance 3 group: "axyz" < "xyz" lexicographically
        assert_eq!(results[3], b"axyz".to_vec());
        assert_eq!(results[4], b"xyz".to_vec());
    }
}
