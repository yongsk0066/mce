// Adapted from corevoikko (voikko-fst/transition.rs)

use bytemuck::{Pod, Zeroable};

/// Unweighted transition (8 bytes).
#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
pub struct Transition {
    pub sym_in: u16,
    pub sym_out: u16,
    pub trans_info: u32,
}

pub const UNWEIGHTED_FINAL_SYM: u16 = 0xFFFF;

impl Transition {
    #[inline]
    pub fn target_state(&self) -> u32 {
        self.trans_info & 0x00FF_FFFF
    }

    #[inline]
    pub fn more_transitions(&self) -> u8 {
        (self.trans_info >> 24) as u8
    }
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
pub struct OverflowCell {
    pub more_transitions: u32,
    pub _padding: u32,
}

/// Weighted transition (16 bytes).
#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
pub struct WeightedTransition {
    pub sym_in: u32,
    pub sym_out: u32,
    pub target_state: u32,
    pub weight: i16,
    pub more_transitions: u8,
    pub _reserved: u8,
}

pub const WEIGHTED_FINAL_SYM: u32 = 0xFFFF_FFFF;

#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
pub struct WeightedOverflowCell {
    pub more_transitions: u32,
    pub _short_padding: u32,
    pub _padding: u64,
}

#[inline]
pub fn unweighted_max_tc(transitions: &[Transition], state_index: u32) -> u32 {
    let idx = state_index as usize;
    if idx >= transitions.len() {
        return 0;
    }
    let max_tc = transitions[idx].more_transitions() as u32;
    if max_tc == 255 {
        if idx + 1 >= transitions.len() {
            return 0;
        }
        let overflow_bytes = bytemuck::bytes_of(&transitions[idx + 1]);
        let oc: &OverflowCell = bytemuck::from_bytes(overflow_bytes);
        oc.more_transitions + 1
    } else {
        max_tc
    }
}

#[inline]
pub fn weighted_max_tc(transitions: &[WeightedTransition], state_index: u32) -> u32 {
    let idx = state_index as usize;
    if idx >= transitions.len() {
        return 0;
    }
    let max_tc = transitions[idx].more_transitions as u32;
    if max_tc == 255 {
        if idx + 1 >= transitions.len() {
            return 0;
        }
        let overflow_bytes = bytemuck::bytes_of(&transitions[idx + 1]);
        let oc: &WeightedOverflowCell = bytemuck::from_bytes(overflow_bytes);
        oc.more_transitions + 1
    } else {
        max_tc
    }
}

const _: () = assert!(size_of::<Transition>() == 8);
const _: () = assert!(size_of::<OverflowCell>() == 8);
const _: () = assert!(size_of::<WeightedTransition>() == 16);
const _: () = assert!(size_of::<WeightedOverflowCell>() == 16);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn transition_field_extraction() {
        let t = Transition {
            sym_in: 5,
            sym_out: 10,
            trans_info: 0xAB_123456,
        };
        assert_eq!(t.target_state(), 0x123456);
        assert_eq!(t.more_transitions(), 0xAB);
    }

    #[test]
    fn zero_copy_cast_unweighted() {
        let raw: [u8; 16] = [
            0x01, 0x00, 0x02, 0x00, 0x03, 0x00, 0x00, 0x00, 0x04, 0x00, 0x05, 0x00, 0x06, 0x00,
            0x00, 0x01,
        ];
        let transitions: &[Transition] = bytemuck::cast_slice(&raw);
        assert_eq!(transitions.len(), 2);
        assert_eq!(transitions[0].sym_in, 1);
        assert_eq!(transitions[0].target_state(), 3);
        assert_eq!(transitions[1].more_transitions(), 1);
    }

    #[test]
    fn unweighted_max_tc_simple() {
        let transitions = vec![Transition {
            sym_in: 0,
            sym_out: 0,
            trans_info: 0x02_000000,
        }];
        assert_eq!(unweighted_max_tc(&transitions, 0), 2);
    }

    #[test]
    fn unweighted_max_tc_overflow() {
        let mut transitions = vec![Transition {
            sym_in: 0,
            sym_out: 0,
            trans_info: 0xFF_000000,
        }];
        let oc = OverflowCell {
            more_transitions: 300,
            _padding: 0,
        };
        transitions.push(bytemuck::cast(oc));
        assert_eq!(unweighted_max_tc(&transitions, 0), 301);
    }

    #[test]
    fn weighted_max_tc_simple() {
        let transitions = vec![WeightedTransition {
            sym_in: 0,
            sym_out: 0,
            target_state: 0,
            weight: 0,
            more_transitions: 5,
            _reserved: 0,
        }];
        assert_eq!(weighted_max_tc(&transitions, 0), 5);
    }

    #[test]
    fn weighted_max_tc_overflow() {
        let mut transitions = vec![WeightedTransition {
            sym_in: 0,
            sym_out: 0,
            target_state: 0,
            weight: 0,
            more_transitions: 255,
            _reserved: 0,
        }];
        let oc = WeightedOverflowCell {
            more_transitions: 400,
            _short_padding: 0,
            _padding: 0,
        };
        transitions.push(bytemuck::cast(oc));
        assert_eq!(weighted_max_tc(&transitions, 0), 401);
    }
}
