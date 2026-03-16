// Adapted from corevoikko (voikko-fst/unweighted.rs)

use crate::config::UnweightedConfig;
use crate::flags::{self, FlagCheckResult};
use crate::format::{self, HEADER_SIZE};
use crate::symbols::{self, SymbolTable};
use crate::transition::{Transition, UNWEIGHTED_FINAL_SYM, unweighted_max_tc};
use crate::{MAX_LOOP_COUNT, Transducer, VfstError};

/// Unweighted VFST transducer.
pub struct UnweightedTransducer {
    transitions: Vec<Transition>,
    symbols: SymbolTable,
    unknown_symbol_ordinal: u16,
}

impl std::fmt::Debug for UnweightedTransducer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("UnweightedTransducer")
            .field("transition_count", &self.transitions.len())
            .field("symbol_count", &self.symbols.symbol_strings.len())
            .finish()
    }
}

impl UnweightedTransducer {
    pub fn from_bytes(data: &[u8]) -> Result<Self, VfstError> {
        let header = format::parse_header(data)?;
        if header.weighted {
            return Err(VfstError::TypeMismatch {
                expected: false,
                actual: true,
            });
        }
        Self::from_bytes_inner(data)
    }

    fn from_bytes_inner(data: &[u8]) -> Result<Self, VfstError> {
        let (symbols, sym_end) = symbols::parse_symbol_table(data, HEADER_SIZE)?;
        let partial = sym_end % 8;
        let transition_offset = if partial > 0 {
            sym_end + (8 - partial)
        } else {
            sym_end
        };

        if transition_offset > data.len() {
            return Err(VfstError::TooShort {
                expected: transition_offset,
                actual: data.len(),
            });
        }

        let remaining = &data[transition_offset..];
        let transition_count = remaining.len() / size_of::<Transition>();
        if transition_count == 0 {
            return Err(VfstError::TooShort {
                expected: transition_offset + size_of::<Transition>(),
                actual: data.len(),
            });
        }

        let mut transitions = vec![
            Transition {
                sym_in: 0,
                sym_out: 0,
                trans_info: 0
            };
            transition_count
        ];
        let dst_bytes = bytemuck::cast_slice_mut::<Transition, u8>(&mut transitions);
        dst_bytes.copy_from_slice(&remaining[..transition_count * size_of::<Transition>()]);

        // Validate all transition targets are within bounds.
        let tc = transitions.len();
        for (i, t) in transitions.iter().enumerate() {
            if t.sym_in == UNWEIGHTED_FINAL_SYM {
                continue; // Final transitions don't have meaningful targets.
            }
            let target = t.target_state() as usize;
            if target >= tc {
                return Err(VfstError::InvalidSymbolTable(format!(
                    "transition {} target_state {} out of bounds (total {})",
                    i, target, tc,
                )));
            }
        }

        Ok(Self {
            unknown_symbol_ordinal: symbols.symbol_strings.len() as u16,
            transitions,
            symbols,
        })
    }

    pub fn symbols(&self) -> &SymbolTable {
        &self.symbols
    }
    pub fn flag_feature_count(&self) -> u16 {
        self.symbols.flag_feature_count
    }

    pub fn new_config(&self, buffer_size: usize) -> UnweightedConfig {
        UnweightedConfig::new(self.symbols.flag_feature_count, buffer_size)
    }

    pub fn next_prefix(
        &self,
        config: &mut UnweightedConfig,
        output: &mut String,
        prefix_length: &mut usize,
    ) -> bool {
        self.next_inner(config, output, Some(prefix_length))
    }

    fn next_inner(
        &self,
        config: &mut UnweightedConfig,
        output: &mut String,
        mut prefix_length: Option<&mut usize>,
    ) -> bool {
        let transitions = &self.transitions;
        let first_normal = self.symbols.first_normal_char;
        let flag_feature_count = self.symbols.flag_feature_count;
        let mut loop_counter: u32 = 0;

        'outer: while loop_counter < MAX_LOOP_COUNT {
            let state_idx = config.state_index_stack[config.stack_depth];
            let current_idx = config.current_transition_stack[config.stack_depth];
            let start_transition_index = current_idx - state_idx;
            let max_tc = unweighted_max_tc(transitions, state_idx);

            let mut tc = start_transition_index;
            let mut trans_idx = current_idx;

            while tc <= max_tc {
                if tc == 1 && max_tc >= 255 {
                    tc += 1;
                    trans_idx += 1;
                }
                if trans_idx as usize >= transitions.len() {
                    return false;
                }
                let ct = &transitions[trans_idx as usize];

                if ct.sym_in == UNWEIGHTED_FINAL_SYM {
                    if config.input_depth == config.input_length || prefix_length.is_some() {
                        output.clear();
                        for i in 0..config.stack_depth {
                            let out_sym = config.output_symbol_stack[i] as usize;
                            output.push_str(&self.symbols.symbol_strings[out_sym]);
                        }
                        config.current_transition_stack[config.stack_depth] = trans_idx + 1;
                        if let Some(ref mut pl) = prefix_length {
                            **pl = config.input_depth;
                        }
                        return true;
                    }
                } else if (config.input_depth < config.input_length
                    && config.input_symbol_stack[config.input_depth] == ct.sym_in)
                    || (ct.sym_in < first_normal && self.flag_diacritic_check(config, ct.sym_in))
                {
                    if config.stack_depth + 2 == config.buffer_size {
                        return false;
                    }
                    config.output_symbol_stack[config.stack_depth] = if ct.sym_out >= first_normal {
                        ct.sym_out
                    } else {
                        0
                    };
                    config.current_transition_stack[config.stack_depth] = trans_idx;
                    config.stack_depth += 1;
                    config.state_index_stack[config.stack_depth] = ct.target_state();
                    config.current_transition_stack[config.stack_depth] = ct.target_state();
                    if ct.sym_in >= first_normal {
                        config.input_depth += 1;
                    }
                    loop_counter += 1;
                    continue 'outer;
                }
                tc += 1;
                trans_idx += 1;
            }

            if config.stack_depth == 0 {
                return false;
            }
            config.stack_depth -= 1;
            let prev_idx = config.current_transition_stack[config.stack_depth];
            if prev_idx as usize >= transitions.len() {
                return false;
            }
            let prev_sym = transitions[prev_idx as usize].sym_in;
            if prev_sym >= first_normal {
                config.input_depth -= 1;
            } else if flag_feature_count > 0 && prev_sym != 0 {
                config.flag_depth -= 1;
                let f = config.flag_undo_feature[config.flag_depth] as usize;
                config.current_flag_values[f] = config.flag_undo_value[config.flag_depth];
            }
            config.current_transition_stack[config.stack_depth] += 1;
            loop_counter += 1;
        }
        false
    }

    fn flag_diacritic_check(&self, config: &mut UnweightedConfig, symbol: u16) -> bool {
        let ffc = self.symbols.flag_feature_count;
        if ffc == 0 || symbol == 0 {
            return true;
        }
        if symbol as usize >= self.symbols.symbol_to_diacritic.len() {
            return false;
        }
        let ofv = &self.symbols.symbol_to_diacritic[symbol as usize];
        let current_value = config.current_flag_values[ofv.feature as usize];
        match flags::check_flag(ofv, current_value) {
            FlagCheckResult::Reject => false,
            FlagCheckResult::AcceptAndUpdate { feature, value } => {
                config.flag_undo_feature[config.flag_depth] = feature;
                config.flag_undo_value[config.flag_depth] =
                    config.current_flag_values[feature as usize];
                config.current_flag_values[feature as usize] = value;
                config.flag_depth += 1;
                true
            }
            FlagCheckResult::AcceptNoUpdate { feature } => {
                config.flag_undo_feature[config.flag_depth] = feature;
                config.flag_undo_value[config.flag_depth] =
                    config.current_flag_values[feature as usize];
                config.flag_depth += 1;
                true
            }
        }
    }
}

impl Transducer for UnweightedTransducer {
    type Config = UnweightedConfig;

    fn prepare(&self, config: &mut Self::Config, input: &[char]) -> bool {
        config.reset();
        let mut all_known = true;
        for &ch in input {
            match self.symbols.char_to_symbol.get(&ch) {
                Some(&sym_idx) => config.input_symbol_stack[config.input_length] = sym_idx,
                None => {
                    config.input_symbol_stack[config.input_length] = self.unknown_symbol_ordinal;
                    all_known = false;
                }
            }
            config.input_length += 1;
        }
        all_known
    }

    fn next(&self, config: &mut Self::Config, output: &mut String) -> bool {
        self.next_inner(config, output, None)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::transition::Transition;

    fn build_header(weighted: bool) -> Vec<u8> {
        let mut buf = vec![0u8; 16];
        buf[..4].copy_from_slice(&0x0001_3A6Eu32.to_le_bytes());
        buf[4..8].copy_from_slice(&0x0003_51FAu32.to_le_bytes());
        buf[8] = if weighted { 1 } else { 0 };
        buf
    }

    fn build_symbol_table(symbols: &[&str]) -> Vec<u8> {
        let mut buf = Vec::new();
        buf.extend_from_slice(&(symbols.len() as u16).to_le_bytes());
        for s in symbols {
            buf.extend_from_slice(s.as_bytes());
            buf.push(0);
        }
        buf
    }

    fn make_transition(sym_in: u16, sym_out: u16, target: u32, more: u8) -> Transition {
        Transition {
            sym_in,
            sym_out,
            trans_info: (target & 0x00FF_FFFF) | ((more as u32) << 24),
        }
    }

    fn build_simple_vfst() -> Vec<u8> {
        let mut data = Vec::new();
        data.extend_from_slice(&build_header(false));
        data.extend_from_slice(&build_symbol_table(&["", "a", "b", "x", "y"]));
        let partial = data.len() % 8;
        if partial > 0 {
            data.extend(std::iter::repeat_n(0u8, 8 - partial));
        }
        data.extend_from_slice(bytemuck::bytes_of(&make_transition(1, 3, 1, 0)));
        data.extend_from_slice(bytemuck::bytes_of(&make_transition(2, 4, 2, 0)));
        data.extend_from_slice(bytemuck::bytes_of(&make_transition(0xFFFF, 0, 0, 0)));
        data
    }

    #[test]
    fn traverse_ab_to_xy() {
        let t = UnweightedTransducer::from_bytes(&build_simple_vfst()).unwrap();
        let mut cfg = t.new_config(100);
        assert!(t.prepare(&mut cfg, &['a', 'b']));
        let mut out = String::new();
        assert!(t.next(&mut cfg, &mut out));
        assert_eq!(out, "xy");
        assert!(!t.next(&mut cfg, &mut out));
    }

    #[test]
    fn unknown_input() {
        let t = UnweightedTransducer::from_bytes(&build_simple_vfst()).unwrap();
        let mut cfg = t.new_config(100);
        assert!(!t.prepare(&mut cfg, &['z', 'z']));
        let mut out = String::new();
        assert!(!t.next(&mut cfg, &mut out));
    }

    #[test]
    fn multiple_outputs() {
        let mut data = Vec::new();
        data.extend_from_slice(&build_header(false));
        data.extend_from_slice(&build_symbol_table(&["", "a", "x", "y"]));
        let partial = data.len() % 8;
        if partial > 0 {
            data.extend(std::iter::repeat_n(0u8, 8 - partial));
        }
        data.extend_from_slice(bytemuck::bytes_of(&make_transition(1, 2, 2, 1)));
        data.extend_from_slice(bytemuck::bytes_of(&make_transition(1, 3, 3, 0)));
        data.extend_from_slice(bytemuck::bytes_of(&make_transition(0xFFFF, 0, 0, 0)));
        data.extend_from_slice(bytemuck::bytes_of(&make_transition(0xFFFF, 0, 0, 0)));

        let t = UnweightedTransducer::from_bytes(&data).unwrap();
        let mut cfg = t.new_config(100);
        t.prepare(&mut cfg, &['a']);
        let mut out = String::new();
        assert!(t.next(&mut cfg, &mut out));
        assert_eq!(out, "x");
        assert!(t.next(&mut cfg, &mut out));
        assert_eq!(out, "y");
        assert!(!t.next(&mut cfg, &mut out));
    }

    #[test]
    fn empty_input_returns_too_short() {
        let result = UnweightedTransducer::from_bytes(&[]);
        assert!(matches!(result.unwrap_err(), VfstError::TooShort { .. }));
    }

    #[test]
    fn header_only_returns_too_short() {
        let data = build_header(false);
        let result = UnweightedTransducer::from_bytes(&data);
        assert!(result.is_err());
    }

    #[test]
    fn weighted_data_rejected_by_unweighted() {
        let data = build_header(true);
        let result = UnweightedTransducer::from_bytes(&data);
        assert!(matches!(
            result.unwrap_err(),
            VfstError::TypeMismatch {
                expected: false,
                actual: true,
            }
        ));
    }

    #[test]
    fn invalid_magic_rejected() {
        let data = vec![0xFFu8; 64];
        let result = UnweightedTransducer::from_bytes(&data);
        assert!(matches!(result.unwrap_err(), VfstError::InvalidMagic));
    }

    #[test]
    fn header_plus_symbols_but_no_transitions_rejected() {
        let mut data = build_header(false);
        data.extend_from_slice(&build_symbol_table(&["", "a"]));
        // Pad to 8-byte alignment but add no transition data
        let partial = data.len() % 8;
        if partial > 0 {
            data.extend(std::iter::repeat_n(0u8, 8 - partial));
        }
        let result = UnweightedTransducer::from_bytes(&data);
        assert!(matches!(result.unwrap_err(), VfstError::TooShort { .. }));
    }

    #[test]
    fn truncated_symbol_table_rejected() {
        let mut data = build_header(false);
        // Claim 10 symbols but only provide count bytes
        data.extend_from_slice(&10u16.to_le_bytes());
        let result = UnweightedTransducer::from_bytes(&data);
        assert!(result.is_err());
    }

    #[test]
    fn transition_target_out_of_bounds_rejected() {
        let mut data = Vec::new();
        data.extend_from_slice(&build_header(false));
        data.extend_from_slice(&build_symbol_table(&["", "a"]));
        let partial = data.len() % 8;
        if partial > 0 {
            data.extend(std::iter::repeat_n(0u8, 8 - partial));
        }
        // Single transition with target state pointing way past end
        data.extend_from_slice(bytemuck::bytes_of(&make_transition(1, 1, 999, 0)));
        let result = UnweightedTransducer::from_bytes(&data);
        assert!(result.is_err());
    }

    #[test]
    fn final_transition_target_not_validated() {
        // Final transitions (sym_in == 0xFFFF) should not have their target validated
        let mut data = Vec::new();
        data.extend_from_slice(&build_header(false));
        data.extend_from_slice(&build_symbol_table(&["", "a"]));
        let partial = data.len() % 8;
        if partial > 0 {
            data.extend(std::iter::repeat_n(0u8, 8 - partial));
        }
        // Final transition with out-of-bounds target (should be ignored during validation)
        data.extend_from_slice(bytemuck::bytes_of(&make_transition(0xFFFF, 0, 999, 0)));
        let result = UnweightedTransducer::from_bytes(&data);
        assert!(result.is_ok());
    }

    #[test]
    fn empty_input_word_with_accepting_start_state() {
        // Build a transducer where state 0 has a final transition -> accepting empty string
        let mut data = Vec::new();
        data.extend_from_slice(&build_header(false));
        data.extend_from_slice(&build_symbol_table(&["", "a"]));
        let partial = data.len() % 8;
        if partial > 0 {
            data.extend(std::iter::repeat_n(0u8, 8 - partial));
        }
        data.extend_from_slice(bytemuck::bytes_of(&make_transition(0xFFFF, 0, 0, 0)));

        let t = UnweightedTransducer::from_bytes(&data).unwrap();
        let mut cfg = t.new_config(100);
        t.prepare(&mut cfg, &[]);
        let mut out = String::new();
        // Empty input on a start-state that accepts -> should produce output
        assert!(t.next(&mut cfg, &mut out));
        assert_eq!(out, "");
    }

    #[test]
    fn prepare_returns_false_for_unknown_chars() {
        let t = UnweightedTransducer::from_bytes(&build_simple_vfst()).unwrap();
        let mut cfg = t.new_config(100);
        let all_known = t.prepare(&mut cfg, &['\u{1234}']);
        assert!(!all_known);
    }

    #[test]
    fn prepare_returns_true_for_known_chars() {
        let t = UnweightedTransducer::from_bytes(&build_simple_vfst()).unwrap();
        let mut cfg = t.new_config(100);
        let all_known = t.prepare(&mut cfg, &['a']);
        assert!(all_known);
    }

    #[test]
    fn next_without_prepare_returns_false() {
        let t = UnweightedTransducer::from_bytes(&build_simple_vfst()).unwrap();
        let mut cfg = t.new_config(100);
        // Don't call prepare — config is in initial state
        let mut out = String::new();
        assert!(!t.next(&mut cfg, &mut out));
    }

    #[test]
    fn symbols_accessor_returns_correct_data() {
        let t = UnweightedTransducer::from_bytes(&build_simple_vfst()).unwrap();
        assert_eq!(t.symbols().symbol_strings.len(), 5);
        assert_eq!(t.symbols().symbol_strings[1], "a");
    }

    #[test]
    fn flag_feature_count_with_no_flags() {
        let t = UnweightedTransducer::from_bytes(&build_simple_vfst()).unwrap();
        assert_eq!(t.flag_feature_count(), 0);
    }

    #[test]
    fn debug_impl_does_not_panic() {
        let t = UnweightedTransducer::from_bytes(&build_simple_vfst()).unwrap();
        let debug_str = format!("{:?}", t);
        assert!(debug_str.contains("UnweightedTransducer"));
    }

    #[test]
    fn next_prefix_accepts_partial_match() {
        // Build: state 0 --a--> state 1 (final), state 1 --b--> state 2 (final)
        let mut data = Vec::new();
        data.extend_from_slice(&build_header(false));
        data.extend_from_slice(&build_symbol_table(&["", "a", "b", "x", "y"]));
        let partial = data.len() % 8;
        if partial > 0 {
            data.extend(std::iter::repeat_n(0u8, 8 - partial));
        }
        // State 0: a->state 1 (transition 0, more=0)
        data.extend_from_slice(bytemuck::bytes_of(&make_transition(1, 3, 1, 0)));
        // State 1: final + b->state 2 (transition 1, more=1)
        data.extend_from_slice(bytemuck::bytes_of(&make_transition(0xFFFF, 0, 0, 1)));
        data.extend_from_slice(bytemuck::bytes_of(&make_transition(2, 4, 3, 0)));
        // State 2: final
        data.extend_from_slice(bytemuck::bytes_of(&make_transition(0xFFFF, 0, 0, 0)));

        let t = UnweightedTransducer::from_bytes(&data).unwrap();
        let mut cfg = t.new_config(100);
        t.prepare(&mut cfg, &['a', 'b']);
        let mut out = String::new();
        let mut prefix_len = 0usize;
        // Should find prefix "a" -> "x" (prefix_length=1)
        assert!(t.next_prefix(&mut cfg, &mut out, &mut prefix_len));
        assert_eq!(out, "x");
        assert_eq!(prefix_len, 1);
        // Then full "ab" -> "xy" (prefix_length=2)
        assert!(t.next_prefix(&mut cfg, &mut out, &mut prefix_len));
        assert_eq!(out, "xy");
        assert_eq!(prefix_len, 2);
    }
}
