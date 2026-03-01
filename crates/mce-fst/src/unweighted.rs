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
            return Err(VfstError::TypeMismatch { expected: false, actual: true });
        }
        Self::from_bytes_inner(data)
    }

    fn from_bytes_inner(data: &[u8]) -> Result<Self, VfstError> {
        let (symbols, sym_end) = symbols::parse_symbol_table(data, HEADER_SIZE)?;
        let partial = sym_end % 8;
        let transition_offset = if partial > 0 { sym_end + (8 - partial) } else { sym_end };

        if transition_offset > data.len() {
            return Err(VfstError::TooShort { expected: transition_offset, actual: data.len() });
        }

        let remaining = &data[transition_offset..];
        let transition_count = remaining.len() / size_of::<Transition>();
        if transition_count == 0 {
            return Err(VfstError::TooShort {
                expected: transition_offset + size_of::<Transition>(),
                actual: data.len(),
            });
        }

        let mut transitions = vec![Transition { sym_in: 0, sym_out: 0, trans_info: 0 }; transition_count];
        let dst_bytes = bytemuck::cast_slice_mut::<Transition, u8>(&mut transitions);
        dst_bytes.copy_from_slice(&remaining[..transition_count * size_of::<Transition>()]);

        Ok(Self {
            unknown_symbol_ordinal: symbols.symbol_strings.len() as u16,
            transitions,
            symbols,
        })
    }

    pub fn symbols(&self) -> &SymbolTable { &self.symbols }
    pub fn flag_feature_count(&self) -> u16 { self.symbols.flag_feature_count }

    pub fn new_config(&self, buffer_size: usize) -> UnweightedConfig {
        UnweightedConfig::new(self.symbols.flag_feature_count, buffer_size)
    }

    pub fn next_prefix(
        &self, config: &mut UnweightedConfig, output: &mut String, prefix_length: &mut usize,
    ) -> bool {
        self.next_inner(config, output, Some(prefix_length))
    }

    fn next_inner(
        &self, config: &mut UnweightedConfig, output: &mut String, mut prefix_length: Option<&mut usize>,
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
                if tc == 1 && max_tc >= 255 { tc += 1; trans_idx += 1; }
                let ct = &transitions[trans_idx as usize];

                if ct.sym_in == UNWEIGHTED_FINAL_SYM {
                    if config.input_depth == config.input_length || prefix_length.is_some() {
                        output.clear();
                        for i in 0..config.stack_depth {
                            let out_sym = config.output_symbol_stack[i] as usize;
                            output.push_str(&self.symbols.symbol_strings[out_sym]);
                        }
                        config.current_transition_stack[config.stack_depth] = trans_idx + 1;
                        if let Some(ref mut pl) = prefix_length { **pl = config.input_depth; }
                        return true;
                    }
                } else if (config.input_depth < config.input_length
                    && config.input_symbol_stack[config.input_depth] == ct.sym_in)
                    || (ct.sym_in < first_normal && self.flag_diacritic_check(config, ct.sym_in))
                {
                    if config.stack_depth + 2 == config.buffer_size { return false; }
                    config.output_symbol_stack[config.stack_depth] =
                        if ct.sym_out >= first_normal { ct.sym_out } else { 0 };
                    config.current_transition_stack[config.stack_depth] = trans_idx;
                    config.stack_depth += 1;
                    config.state_index_stack[config.stack_depth] = ct.target_state();
                    config.current_transition_stack[config.stack_depth] = ct.target_state();
                    if ct.sym_in >= first_normal { config.input_depth += 1; }
                    loop_counter += 1;
                    continue 'outer;
                }
                tc += 1;
                trans_idx += 1;
            }

            if config.stack_depth == 0 { return false; }
            config.stack_depth -= 1;
            let prev_idx = config.current_transition_stack[config.stack_depth];
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
        if ffc == 0 || symbol == 0 { return true; }
        let ofv = &self.symbols.symbol_to_diacritic[symbol as usize];
        let current_value = config.current_flag_values[ofv.feature as usize];
        match flags::check_flag(ofv, current_value) {
            FlagCheckResult::Reject => false,
            FlagCheckResult::AcceptAndUpdate { feature, value } => {
                config.flag_undo_feature[config.flag_depth] = feature;
                config.flag_undo_value[config.flag_depth] = config.current_flag_values[feature as usize];
                config.current_flag_values[feature as usize] = value;
                config.flag_depth += 1;
                true
            }
            FlagCheckResult::AcceptNoUpdate { feature } => {
                config.flag_undo_feature[config.flag_depth] = feature;
                config.flag_undo_value[config.flag_depth] = config.current_flag_values[feature as usize];
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
        for s in symbols { buf.extend_from_slice(s.as_bytes()); buf.push(0); }
        buf
    }

    fn make_transition(sym_in: u16, sym_out: u16, target: u32, more: u8) -> Transition {
        Transition { sym_in, sym_out, trans_info: (target & 0x00FF_FFFF) | ((more as u32) << 24) }
    }

    fn build_simple_vfst() -> Vec<u8> {
        let mut data = Vec::new();
        data.extend_from_slice(&build_header(false));
        data.extend_from_slice(&build_symbol_table(&["", "a", "b", "x", "y"]));
        let partial = data.len() % 8;
        if partial > 0 { data.extend(std::iter::repeat_n(0u8, 8 - partial)); }
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
        if partial > 0 { data.extend(std::iter::repeat_n(0u8, 8 - partial)); }
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
}
