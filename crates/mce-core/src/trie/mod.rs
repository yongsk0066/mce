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
    pub fn fuzzy_search(&self, _query: &[u8], _max_edits: usize) -> Vec<Vec<u8>> {
        todo!("M1: fuzzy search with Levenshtein automaton")
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

    /// 전체 키 수.
    pub fn len(&self) -> usize {
        self.is_terminal.count_ones()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
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
}
