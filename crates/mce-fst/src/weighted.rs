// Adapted from corevoikko (voikko-fst/weighted.rs)

use crate::config::WeightedConfig;
use crate::flags::{self, FlagCheckResult};
use crate::format::{self, HEADER_SIZE};
use crate::symbols::{self, SymbolTable};
use crate::transition::{WEIGHTED_FINAL_SYM, WeightedTransition, weighted_max_tc};
use crate::{MAX_LOOP_COUNT, Transducer, VfstError};

/// Weighted VFST transducer.
pub struct WeightedTransducer {
    transitions: Vec<WeightedTransition>,
    symbols: SymbolTable,
}

impl std::fmt::Debug for WeightedTransducer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("WeightedTransducer")
            .field("transition_count", &self.transitions.len())
            .field("symbol_count", &self.symbols.symbol_strings.len())
            .finish()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WeightedResult {
    pub weight: i16,
    pub first_not_reached_position: usize,
}

impl WeightedTransducer {
    pub fn from_bytes(data: &[u8]) -> Result<Self, VfstError> {
        let header = format::parse_header(data)?;
        if !header.weighted {
            return Err(VfstError::TypeMismatch {
                expected: true,
                actual: false,
            });
        }
        Self::from_bytes_inner(data)
    }

    fn from_bytes_inner(data: &[u8]) -> Result<Self, VfstError> {
        let (symbols, sym_end) = symbols::parse_symbol_table(data, HEADER_SIZE)?;
        let partial = sym_end % 16;
        let transition_offset = if partial > 0 {
            sym_end + (16 - partial)
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
        let tc = remaining.len() / size_of::<WeightedTransition>();
        if tc == 0 {
            return Err(VfstError::TooShort {
                expected: transition_offset + size_of::<WeightedTransition>(),
                actual: data.len(),
            });
        }

        let mut transitions = vec![
            WeightedTransition {
                sym_in: 0,
                sym_out: 0,
                target_state: 0,
                weight: 0,
                more_transitions: 0,
                _reserved: 0,
            };
            tc
        ];
        let dst = bytemuck::cast_slice_mut::<WeightedTransition, u8>(&mut transitions);
        dst.copy_from_slice(&remaining[..tc * size_of::<WeightedTransition>()]);

        Ok(Self {
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

    pub fn new_config(&self, buffer_size: usize) -> WeightedConfig {
        WeightedConfig::new(self.symbols.flag_feature_count, buffer_size)
    }

    pub fn next_weighted(
        &self,
        config: &mut WeightedConfig,
        output: &mut String,
        result: &mut WeightedResult,
    ) -> bool {
        let transitions = &self.transitions;
        let first_normal = self.symbols.first_normal_char as u32;
        let flag_feature_count = self.symbols.flag_feature_count;
        let mut loop_counter: u32 = 0;
        result.first_not_reached_position = config.input_depth;

        'outer: while loop_counter < MAX_LOOP_COUNT {
            let state_idx = config.state_index_stack[config.stack_depth];
            let current_idx = config.current_transition_stack[config.stack_depth];
            let start_ti = current_idx - state_idx;
            let max_tc = weighted_max_tc(transitions, state_idx);
            let input_sym: u32 = if config.input_depth == config.input_length {
                0
            } else {
                config.input_symbol_stack[config.input_depth]
            };

            let mut tc = start_ti;
            let mut ti = current_idx;

            while tc <= max_tc {
                if tc == 1 && max_tc >= 255 {
                    tc += 1;
                    ti += 1;
                }
                let ct = &transitions[ti as usize];

                if ct.sym_in == WEIGHTED_FINAL_SYM {
                    if config.input_depth == config.input_length {
                        output.clear();
                        for i in 0..config.stack_depth {
                            let os = config.output_symbol_stack[i] as usize;
                            output.push_str(&self.symbols.symbol_strings[os]);
                        }
                        config.current_transition_stack[config.stack_depth] = ti + 1;
                        let mut tw = ct.weight;
                        for i in 0..config.stack_depth {
                            tw += transitions[config.current_transition_stack[i] as usize].weight;
                        }
                        result.weight = tw;
                        return true;
                    }
                } else if input_sym == 0 && ct.sym_in >= first_normal {
                    break;
                } else if (config.input_depth < config.input_length && input_sym == ct.sym_in)
                    || (ct.sym_in < first_normal
                        && self.flag_diacritic_check(config, ct.sym_in as u16))
                {
                    if config.stack_depth + 2 == config.buffer_size {
                        return false;
                    }
                    config.output_symbol_stack[config.stack_depth] = if ct.sym_out >= first_normal {
                        ct.sym_out
                    } else {
                        0
                    };
                    config.current_transition_stack[config.stack_depth] = ti;
                    config.stack_depth += 1;
                    config.state_index_stack[config.stack_depth] = ct.target_state;
                    config.current_transition_stack[config.stack_depth] = ct.target_state;
                    if ct.sym_in >= first_normal {
                        config.input_depth += 1;
                        if result.first_not_reached_position < config.input_depth {
                            result.first_not_reached_position = config.input_depth;
                        }
                    }
                    loop_counter += 1;
                    continue 'outer;
                } else if ct.sym_in > input_sym {
                    break;
                } else if tc >= 1 && ct.sym_in >= first_normal && ct.sym_in < input_sym {
                    let mut min: u32 = 0;
                    let mut max: u32 = max_tc - tc;
                    while min + 1 < max {
                        let mid = (min + max) / 2;
                        if transitions[(ti + mid) as usize].sym_in < input_sym {
                            min = mid;
                        } else {
                            max = mid;
                        }
                    }
                    tc += min;
                    ti += min;
                }
                tc += 1;
                ti += 1;
            }

            if config.stack_depth == 0 {
                return false;
            }
            config.stack_depth -= 1;
            let pi = config.current_transition_stack[config.stack_depth];
            let ps = transitions[pi as usize].sym_in;
            if ps >= first_normal {
                config.input_depth -= 1;
            } else if flag_feature_count > 0 && ps != 0 {
                config.flag_depth -= 1;
            }
            config.current_transition_stack[config.stack_depth] += 1;
            loop_counter += 1;
        }
        false
    }

    fn flag_diacritic_check(&self, config: &mut WeightedConfig, symbol: u16) -> bool {
        let ffc = self.symbols.flag_feature_count;
        if ffc == 0 || symbol == 0 {
            return true;
        }
        let ofv = &self.symbols.symbol_to_diacritic[symbol as usize];
        let cv = config.current_flags()[ofv.feature as usize] as u16;
        match flags::check_flag(ofv, cv) {
            FlagCheckResult::Reject => false,
            FlagCheckResult::AcceptAndUpdate { feature, value } => {
                config.push_flags();
                config.current_flags_mut()[feature as usize] = value as u32;
                true
            }
            FlagCheckResult::AcceptNoUpdate { .. } => {
                config.push_flags();
                true
            }
        }
    }
}

impl Transducer for WeightedTransducer {
    type Config = WeightedConfig;

    fn prepare(&self, config: &mut Self::Config, input: &[char]) -> bool {
        config.reset();
        for &ch in input {
            match self.symbols.char_to_symbol.get(&ch) {
                Some(&si) => config.input_symbol_stack[config.input_length] = si as u32,
                None => return false,
            }
            config.input_length += 1;
        }
        true
    }

    fn next(&self, config: &mut Self::Config, output: &mut String) -> bool {
        let mut r = WeightedResult {
            weight: 0,
            first_not_reached_position: 0,
        };
        self.next_weighted(config, output, &mut r)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::transition::WeightedTransition;

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

    fn make_wt(si: u32, so: u32, ts: u32, w: i16, m: u8) -> WeightedTransition {
        WeightedTransition {
            sym_in: si,
            sym_out: so,
            target_state: ts,
            weight: w,
            more_transitions: m,
            _reserved: 0,
        }
    }

    fn build_simple() -> Vec<u8> {
        let mut d = Vec::new();
        d.extend_from_slice(&build_header(true));
        d.extend_from_slice(&build_symbol_table(&["", "a", "b", "x", "y"]));
        let p = d.len() % 16;
        if p > 0 {
            d.extend(std::iter::repeat_n(0u8, 16 - p));
        }
        d.extend_from_slice(bytemuck::bytes_of(&make_wt(1, 3, 1, 10, 0)));
        d.extend_from_slice(bytemuck::bytes_of(&make_wt(2, 4, 2, 20, 0)));
        d.extend_from_slice(bytemuck::bytes_of(&make_wt(0xFFFFFFFF, 0, 0, 5, 0)));
        d
    }

    #[test]
    fn traverse_weighted() {
        let t = WeightedTransducer::from_bytes(&build_simple()).unwrap();
        let mut cfg = t.new_config(100);
        assert!(t.prepare(&mut cfg, &['a', 'b']));
        let mut out = String::new();
        let mut r = WeightedResult {
            weight: 0,
            first_not_reached_position: 0,
        };
        assert!(t.next_weighted(&mut cfg, &mut out, &mut r));
        assert_eq!(out, "xy");
        assert_eq!(r.weight, 35);
    }

    #[test]
    fn reject_unknown() {
        let t = WeightedTransducer::from_bytes(&build_simple()).unwrap();
        let mut cfg = t.new_config(100);
        assert!(!t.prepare(&mut cfg, &['z', 'z']));
    }

    #[test]
    fn via_trait() {
        let t = WeightedTransducer::from_bytes(&build_simple()).unwrap();
        let mut cfg = t.new_config(100);
        t.prepare(&mut cfg, &['a', 'b']);
        let mut out = String::new();
        assert!(t.next(&mut cfg, &mut out));
        assert_eq!(out, "xy");
        assert!(!t.next(&mut cfg, &mut out));
    }
}
