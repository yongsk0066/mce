// Adapted from corevoikko (voikko-fst/config.rs)

/// Traversal configuration for unweighted transducers.
/// Uses destructive-update + undo for flag diacritics.
pub struct UnweightedConfig {
    pub buffer_size: usize,
    pub stack_depth: usize,
    pub flag_depth: usize,
    pub input_depth: usize,
    pub input_length: usize,
    pub state_index_stack: Vec<u32>,
    pub current_transition_stack: Vec<u32>,
    pub input_symbol_stack: Vec<u16>,
    pub output_symbol_stack: Vec<u16>,
    pub current_flag_values: Vec<u16>,
    pub flag_undo_value: Vec<u16>,
    pub flag_undo_feature: Vec<u16>,
}

impl UnweightedConfig {
    pub fn new(flag_feature_count: u16, buffer_size: usize) -> Self {
        let fc = flag_feature_count as usize;
        Self {
            buffer_size,
            stack_depth: 0,
            flag_depth: 0,
            input_depth: 0,
            input_length: 0,
            state_index_stack: vec![0; buffer_size],
            current_transition_stack: vec![0; buffer_size],
            input_symbol_stack: vec![0; buffer_size],
            output_symbol_stack: vec![0; buffer_size],
            current_flag_values: vec![0; fc],
            flag_undo_value: if fc > 0 { vec![0; buffer_size] } else { Vec::new() },
            flag_undo_feature: if fc > 0 { vec![0; buffer_size] } else { Vec::new() },
        }
    }

    #[inline]
    pub fn reset(&mut self) {
        self.stack_depth = 0;
        self.flag_depth = 0;
        self.input_depth = 0;
        self.input_length = 0;
        self.state_index_stack[0] = 0;
        self.current_transition_stack[0] = 0;
        for v in &mut self.current_flag_values {
            *v = 0;
        }
    }
}

/// Traversal configuration for weighted transducers.
/// Uses copy-on-push for flag diacritics.
pub struct WeightedConfig {
    pub buffer_size: usize,
    pub stack_depth: usize,
    pub flag_depth: usize,
    pub input_depth: usize,
    pub input_length: usize,
    pub state_index_stack: Vec<u32>,
    pub current_transition_stack: Vec<u32>,
    pub input_symbol_stack: Vec<u32>,
    pub output_symbol_stack: Vec<u32>,
    pub flag_value_stack: Vec<u32>,
    pub flag_feature_count: u32,
}

impl WeightedConfig {
    pub fn new(flag_feature_count: u16, buffer_size: usize) -> Self {
        let fc = flag_feature_count as u32;
        Self {
            buffer_size,
            stack_depth: 0,
            flag_depth: 0,
            input_depth: 0,
            input_length: 0,
            state_index_stack: vec![0; buffer_size],
            current_transition_stack: vec![0; buffer_size],
            input_symbol_stack: vec![0; buffer_size],
            output_symbol_stack: vec![0; buffer_size],
            flag_value_stack: if fc > 0 { vec![0; fc as usize * buffer_size] } else { Vec::new() },
            flag_feature_count: fc,
        }
    }

    #[inline]
    pub fn reset(&mut self) {
        self.stack_depth = 0;
        self.flag_depth = 0;
        self.input_depth = 0;
        self.input_length = 0;
        self.state_index_stack[0] = 0;
        self.current_transition_stack[0] = 0;
        let fc = self.flag_feature_count as usize;
        for i in 0..fc {
            self.flag_value_stack[i] = 0;
        }
    }

    #[inline]
    pub fn current_flags(&self) -> &[u32] {
        let fc = self.flag_feature_count as usize;
        if fc == 0 { return &[]; }
        let start = self.flag_depth * fc;
        &self.flag_value_stack[start..start + fc]
    }

    #[inline]
    pub fn current_flags_mut(&mut self) -> &mut [u32] {
        let fc = self.flag_feature_count as usize;
        if fc == 0 { return &mut []; }
        let start = self.flag_depth * fc;
        &mut self.flag_value_stack[start..start + fc]
    }

    #[inline]
    pub fn push_flags(&mut self) {
        let fc = self.flag_feature_count as usize;
        if fc == 0 { return; }
        let src_start = self.flag_depth * fc;
        let dst_start = src_start + fc;
        for i in 0..fc {
            self.flag_value_stack[dst_start + i] = self.flag_value_stack[src_start + i];
        }
        self.flag_depth += 1;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unweighted_config_reset() {
        let mut cfg = UnweightedConfig::new(2, 50);
        cfg.stack_depth = 10;
        cfg.current_flag_values[0] = 42;
        cfg.reset();
        assert_eq!(cfg.stack_depth, 0);
        assert_eq!(cfg.current_flag_values[0], 0);
    }

    #[test]
    fn weighted_push_flags() {
        let mut cfg = WeightedConfig::new(3, 10);
        cfg.current_flags_mut()[0] = 5;
        cfg.current_flags_mut()[1] = 10;
        cfg.push_flags();
        assert_eq!(cfg.flag_depth, 1);
        assert_eq!(cfg.current_flags()[0], 5);
        cfg.current_flags_mut()[0] = 99;
        cfg.flag_depth -= 1;
        assert_eq!(cfg.current_flags()[0], 5); // original intact
    }
}
