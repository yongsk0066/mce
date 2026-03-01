//! Benchmarks for M1 Succinct Trie.
//!
//! Measures build time, exact lookup (hit/miss), fuzzy search (d=1, d=2),
//! and memory usage across different dictionary sizes.
//!
//! Run with: cargo bench -p mce-core --bench trie_bench

use criterion::{
    black_box, criterion_group, criterion_main, BatchSize, BenchmarkId, Criterion, Throughput,
};
use mce_core::trie::{SuccinctTrie, TrieBuilder};

// ── Key generation ──────────────────────────────────────────────────────

/// Second bytes for two-byte UTF-8 Finnish characters:
///   a-umlaut = 0xC3 0xA4
///   o-umlaut = 0xC3 0xB6
const FINNISH_EXTRA: &[u8] = &[0xA4, 0xB6];

/// Simple deterministic PRNG (xorshift64) to avoid external dependencies.
struct Rng {
    state: u64,
}

impl Rng {
    fn new(seed: u64) -> Self {
        Self {
            state: seed.wrapping_add(1),
        }
    }

    fn next_u64(&mut self) -> u64 {
        let mut x = self.state;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.state = x;
        x
    }

    fn next_usize(&mut self, max: usize) -> usize {
        (self.next_u64() % max as u64) as usize
    }
}

/// Generate `n` unique sorted synthetic Finnish-like byte keys.
/// Key lengths range from 5 to 15 bytes. Approximately 10% of characters
/// are Finnish diacritics (two-byte sequences).
fn generate_keys(n: usize, seed: u64) -> Vec<Vec<u8>> {
    let mut rng = Rng::new(seed);
    let mut keys = Vec::with_capacity(n * 2);

    for _ in 0..n * 2 {
        let len = 5 + rng.next_usize(11); // 5..=15 character positions
        let mut key = Vec::with_capacity(len * 2);

        for _ in 0..len {
            // ~10% chance of a Finnish diacritic (2-byte UTF-8)
            if rng.next_usize(10) == 0 {
                key.push(0xC3);
                key.push(FINNISH_EXTRA[rng.next_usize(FINNISH_EXTRA.len())]);
            } else {
                key.push(b'a' + rng.next_usize(26) as u8);
            }
        }

        keys.push(key);
    }

    keys.sort();
    keys.dedup();
    keys.truncate(n);
    keys
}

/// Generate keys that are guaranteed NOT to exist in the trie
/// by appending a distinctive suffix to random prefixes.
fn generate_miss_keys(n: usize, seed: u64) -> Vec<Vec<u8>> {
    let mut rng = Rng::new(seed);
    let mut keys = Vec::with_capacity(n);

    for _ in 0..n {
        let len = 5 + rng.next_usize(11);
        let mut key = Vec::with_capacity(len + 3);
        for _ in 0..len {
            key.push(b'a' + rng.next_usize(26) as u8);
        }
        // Append bytes that make collision extremely unlikely
        key.extend_from_slice(b"ZZZ");
        keys.push(key);
    }

    keys
}

/// Build a trie from the given keys.
fn build_trie(keys: &[Vec<u8>]) -> SuccinctTrie {
    let mut builder = TrieBuilder::new();
    for key in keys {
        builder.insert(key.clone());
    }
    builder.build()
}

// ── Benchmarks ──────────────────────────────────────────────────────────

fn bench_build(c: &mut Criterion) {
    let mut group = c.benchmark_group("trie_build");

    for &n in &[1_000usize, 10_000, 100_000] {
        let keys = generate_keys(n, 42);
        group.throughput(Throughput::Elements(keys.len() as u64));

        group.bench_with_input(BenchmarkId::from_parameter(n), &keys, |b, keys| {
            b.iter(|| {
                let mut builder = TrieBuilder::new();
                for key in keys {
                    builder.insert(key.clone());
                }
                black_box(builder.build())
            });
        });
    }

    group.finish();
}

fn bench_contains_hit(c: &mut Criterion) {
    let mut group = c.benchmark_group("trie_contains_hit");

    for &n in &[1_000usize, 10_000, 100_000] {
        let keys = generate_keys(n, 42);
        let trie = build_trie(&keys);

        // Sample 1000 keys to look up repeatedly
        let sample_size = 1000.min(keys.len());
        let lookup_keys: Vec<Vec<u8>> = keys[..sample_size].to_vec();

        group.throughput(Throughput::Elements(sample_size as u64));

        group.bench_function(BenchmarkId::from_parameter(n), |b| {
            b.iter(|| {
                for key in &lookup_keys {
                    black_box(trie.contains(key));
                }
            });
        });
    }

    group.finish();
}

fn bench_contains_miss(c: &mut Criterion) {
    let mut group = c.benchmark_group("trie_contains_miss");

    for &n in &[1_000usize, 10_000, 100_000] {
        let keys = generate_keys(n, 42);
        let trie = build_trie(&keys);

        // Generate keys that are NOT in the trie
        let miss_keys = generate_miss_keys(1000, 99);

        group.throughput(Throughput::Elements(miss_keys.len() as u64));

        group.bench_function(BenchmarkId::from_parameter(n), |b| {
            b.iter(|| {
                for key in &miss_keys {
                    black_box(trie.contains(key));
                }
            });
        });
    }

    group.finish();
}

fn bench_fuzzy_search_d1(c: &mut Criterion) {
    let mut group = c.benchmark_group("trie_fuzzy_d1");
    // Fuzzy search is expensive; use a smaller sample per iteration.
    group.sample_size(20);

    for &n in &[1_000usize, 10_000, 100_000] {
        let keys = generate_keys(n, 42);
        let trie = build_trie(&keys);

        // Pick 10 existing keys as queries
        let queries: Vec<Vec<u8>> = keys.iter().take(10).cloned().collect();

        group.bench_function(BenchmarkId::from_parameter(n), |b| {
            b.iter(|| {
                for query in &queries {
                    black_box(trie.fuzzy_search(query, 1));
                }
            });
        });
    }

    group.finish();
}

fn bench_fuzzy_search_d2(c: &mut Criterion) {
    let mut group = c.benchmark_group("trie_fuzzy_d2");
    // Even more expensive; reduce sample size further.
    group.sample_size(10);

    for &n in &[1_000usize, 10_000] {
        let keys = generate_keys(n, 42);
        let trie = build_trie(&keys);

        // Pick 5 existing keys as queries
        let queries: Vec<Vec<u8>> = keys.iter().take(5).cloned().collect();

        group.bench_function(BenchmarkId::from_parameter(n), |b| {
            b.iter(|| {
                for query in &queries {
                    black_box(trie.fuzzy_search(query, 2));
                }
            });
        });
    }

    group.finish();
}

fn bench_memory(c: &mut Criterion) {
    let mut group = c.benchmark_group("trie_memory");

    for &n in &[1_000usize, 10_000, 100_000] {
        let keys = generate_keys(n, 42);

        group.bench_with_input(BenchmarkId::from_parameter(n), &keys, |b, keys| {
            b.iter_batched(
                || {
                    let mut builder = TrieBuilder::new();
                    for key in keys {
                        builder.insert(key.clone());
                    }
                    builder.build()
                },
                |trie| {
                    let heap_bytes = trie.heap_size_in_bytes();
                    let key_count = trie.len();
                    let bytes_per_key = if key_count > 0 {
                        heap_bytes as f64 / key_count as f64
                    } else {
                        0.0
                    };

                    // Print memory stats (visible in benchmark output).
                    // The actual measurement is just a read to prevent optimization.
                    eprintln!(
                        "  [n={key_count}] heap={heap_bytes} bytes, \
                         {bytes_per_key:.1} bytes/key"
                    );

                    black_box(heap_bytes);
                },
                BatchSize::SmallInput,
            );
        });
    }

    group.finish();
}

criterion_group!(
    benches,
    bench_build,
    bench_contains_hit,
    bench_contains_miss,
    bench_fuzzy_search_d1,
    bench_fuzzy_search_d2,
    bench_memory,
);
criterion_main!(benches);
