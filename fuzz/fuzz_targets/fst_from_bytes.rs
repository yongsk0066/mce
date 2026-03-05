#![no_main]

use libfuzzer_sys::fuzz_target;
use mce_fst::unweighted::UnweightedTransducer;
use mce_fst::weighted::WeightedTransducer;
use mce_fst::Transducer;

fuzz_target!(|data: &[u8]| {
    // Attempt to parse as unweighted FST — must not panic.
    if let Ok(t) = UnweightedTransducer::from_bytes(data) {
        let mut cfg = t.new_config(64);
        // Try traversing with a simple input.
        let input: Vec<char> = "talo".chars().collect();
        t.prepare(&mut cfg, &input);
        let mut output = String::new();
        let _ = t.next(&mut cfg, &mut output);
    }

    // Attempt to parse as weighted FST — must not panic.
    if let Ok(t) = WeightedTransducer::from_bytes(data) {
        let mut cfg = t.new_config(64);
        let input: Vec<char> = "talo".chars().collect();
        t.prepare(&mut cfg, &input);
        let mut output = String::new();
        let _ = t.next(&mut cfg, &mut output);
    }
});
