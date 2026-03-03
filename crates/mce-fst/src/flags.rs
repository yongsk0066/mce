// Adapted from corevoikko (voikko-fst/flags.rs)

use crate::VfstError;
use hashbrown::HashMap;

/// Flag diacritic operations.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FlagOp {
    P, // Positive Set
    C, // Clear
    U, // Unification
    R, // Require
    D, // Disallow
}

pub const FLAG_VALUE_NEUTRAL: u16 = 0;
pub const FLAG_VALUE_ANY: u16 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OpFeatureValue {
    pub op: FlagOp,
    pub feature: u16,
    pub value: u16,
}

impl Default for OpFeatureValue {
    fn default() -> Self {
        Self {
            op: FlagOp::P,
            feature: 0,
            value: FLAG_VALUE_NEUTRAL,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FlagCheckResult {
    Reject,
    AcceptAndUpdate { feature: u16, value: u16 },
    AcceptNoUpdate { feature: u16 },
}

pub fn check_flag(ofv: &OpFeatureValue, current_value: u16) -> FlagCheckResult {
    match ofv.op {
        FlagOp::P => FlagCheckResult::AcceptAndUpdate {
            feature: ofv.feature,
            value: ofv.value,
        },
        FlagOp::C => FlagCheckResult::AcceptAndUpdate {
            feature: ofv.feature,
            value: FLAG_VALUE_NEUTRAL,
        },
        FlagOp::U => {
            if current_value != FLAG_VALUE_NEUTRAL {
                if current_value != ofv.value {
                    FlagCheckResult::Reject
                } else {
                    FlagCheckResult::AcceptNoUpdate {
                        feature: ofv.feature,
                    }
                }
            } else {
                FlagCheckResult::AcceptAndUpdate {
                    feature: ofv.feature,
                    value: ofv.value,
                }
            }
        }
        FlagOp::R => {
            if ofv.value == FLAG_VALUE_ANY {
                if current_value == FLAG_VALUE_NEUTRAL {
                    return FlagCheckResult::Reject;
                }
            } else if current_value != ofv.value {
                return FlagCheckResult::Reject;
            }
            FlagCheckResult::AcceptNoUpdate {
                feature: ofv.feature,
            }
        }
        FlagOp::D => {
            if (ofv.value == FLAG_VALUE_ANY && current_value != FLAG_VALUE_NEUTRAL)
                || current_value == ofv.value
            {
                return FlagCheckResult::Reject;
            }
            FlagCheckResult::AcceptNoUpdate {
                feature: ofv.feature,
            }
        }
    }
}

pub struct FlagDiacriticParser {
    features: HashMap<String, u16>,
    values: HashMap<String, u16>,
}

impl Default for FlagDiacriticParser {
    fn default() -> Self {
        Self::new()
    }
}

impl FlagDiacriticParser {
    pub fn new() -> Self {
        let mut values = HashMap::new();
        values.insert(String::new(), FLAG_VALUE_NEUTRAL);
        values.insert("@".to_string(), FLAG_VALUE_ANY);
        Self {
            features: HashMap::new(),
            values,
        }
    }

    pub fn feature_count(&self) -> u16 {
        self.features.len() as u16
    }

    pub fn parse(&mut self, symbol: &str) -> Result<OpFeatureValue, VfstError> {
        let bytes = symbol.as_bytes();
        if bytes.len() <= 4 {
            return Err(VfstError::InvalidFlagDiacritic(format!(
                "too short: {symbol:?}"
            )));
        }

        let op = match bytes[1] {
            b'P' => FlagOp::P,
            b'C' => FlagOp::C,
            b'U' => FlagOp::U,
            b'R' => FlagOp::R,
            b'D' => FlagOp::D,
            _ => {
                return Err(VfstError::InvalidFlagDiacritic(format!(
                    "unknown operation '{}' in {symbol:?}",
                    bytes[1] as char,
                )));
            }
        };

        let inner = &symbol[3..symbol.len() - 1];
        let (feature_str, value_str) = match inner.find('.') {
            Some(dot_pos) => (&inner[..dot_pos], &inner[dot_pos + 1..]),
            None => (inner, "@"),
        };

        let feature = {
            let next_idx = self.features.len() as u16;
            *self
                .features
                .entry(feature_str.to_string())
                .or_insert(next_idx)
        };

        let value = {
            let next_idx = self.values.len() as u16;
            *self.values.entry(value_str.to_string()).or_insert(next_idx)
        };

        Ok(OpFeatureValue { op, feature, value })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn positive_set_always_updates() {
        let ofv = OpFeatureValue {
            op: FlagOp::P,
            feature: 0,
            value: 5,
        };
        assert_eq!(
            check_flag(&ofv, FLAG_VALUE_NEUTRAL),
            FlagCheckResult::AcceptAndUpdate {
                feature: 0,
                value: 5
            }
        );
    }

    #[test]
    fn unification_different_rejects() {
        let ofv = OpFeatureValue {
            op: FlagOp::U,
            feature: 0,
            value: 3,
        };
        assert_eq!(check_flag(&ofv, 5), FlagCheckResult::Reject);
    }

    #[test]
    fn require_any_neutral_rejects() {
        let ofv = OpFeatureValue {
            op: FlagOp::R,
            feature: 0,
            value: FLAG_VALUE_ANY,
        };
        assert_eq!(
            check_flag(&ofv, FLAG_VALUE_NEUTRAL),
            FlagCheckResult::Reject
        );
    }

    #[test]
    fn disallow_matching_rejects() {
        let ofv = OpFeatureValue {
            op: FlagOp::D,
            feature: 0,
            value: 5,
        };
        assert_eq!(check_flag(&ofv, 5), FlagCheckResult::Reject);
    }

    #[test]
    fn parse_flag_diacritics() {
        let mut parser = FlagDiacriticParser::new();
        let ofv1 = parser.parse("@P.CASE.NOM@").unwrap();
        assert_eq!(ofv1.op, FlagOp::P);
        assert_eq!(ofv1.feature, 0);

        let ofv2 = parser.parse("@P.NUM.SG@").unwrap();
        assert_eq!(ofv2.feature, 1);

        let ofv3 = parser.parse("@R.CASE.GEN@").unwrap();
        assert_eq!(ofv3.feature, 0); // same feature
        assert_eq!(parser.feature_count(), 2);
    }

    #[test]
    fn reject_invalid_flag() {
        let mut parser = FlagDiacriticParser::new();
        assert!(parser.parse("@X.FOO@").is_err());
        assert!(parser.parse("@P@").is_err());
    }

    #[test]
    fn clear_resets_to_neutral() {
        let ofv = OpFeatureValue {
            op: FlagOp::C,
            feature: 1,
            value: 99, // value is ignored for Clear
        };
        // Regardless of the current value, Clear always resets to NEUTRAL.
        assert_eq!(
            check_flag(&ofv, 42),
            FlagCheckResult::AcceptAndUpdate {
                feature: 1,
                value: FLAG_VALUE_NEUTRAL,
            }
        );
        assert_eq!(
            check_flag(&ofv, FLAG_VALUE_NEUTRAL),
            FlagCheckResult::AcceptAndUpdate {
                feature: 1,
                value: FLAG_VALUE_NEUTRAL,
            }
        );
    }

    #[test]
    fn unification_matching_value_accepts() {
        let ofv = OpFeatureValue {
            op: FlagOp::U,
            feature: 2,
            value: 7,
        };
        // When current value equals ofv.value, accept without update.
        assert_eq!(
            check_flag(&ofv, 7),
            FlagCheckResult::AcceptNoUpdate { feature: 2 }
        );
    }

    #[test]
    fn unification_neutral_sets_value() {
        let ofv = OpFeatureValue {
            op: FlagOp::U,
            feature: 3,
            value: 5,
        };
        // When current is neutral, unification sets the value.
        assert_eq!(
            check_flag(&ofv, FLAG_VALUE_NEUTRAL),
            FlagCheckResult::AcceptAndUpdate {
                feature: 3,
                value: 5,
            }
        );
    }

    #[test]
    fn require_specific_value_match() {
        let ofv = OpFeatureValue {
            op: FlagOp::R,
            feature: 0,
            value: 5,
        };
        // Current matches required value -> accept.
        assert_eq!(
            check_flag(&ofv, 5),
            FlagCheckResult::AcceptNoUpdate { feature: 0 }
        );
        // Current does not match -> reject.
        assert_eq!(check_flag(&ofv, 3), FlagCheckResult::Reject);
        // Neutral does not match a specific value -> reject.
        assert_eq!(
            check_flag(&ofv, FLAG_VALUE_NEUTRAL),
            FlagCheckResult::Reject
        );
    }

    #[test]
    fn disallow_any_rejects_non_neutral() {
        let ofv = OpFeatureValue {
            op: FlagOp::D,
            feature: 0,
            value: FLAG_VALUE_ANY,
        };
        // Any non-neutral current value is disallowed.
        assert_eq!(check_flag(&ofv, 5), FlagCheckResult::Reject);
        assert_eq!(check_flag(&ofv, 1), FlagCheckResult::Reject);
        // Neutral is accepted.
        assert_eq!(
            check_flag(&ofv, FLAG_VALUE_NEUTRAL),
            FlagCheckResult::AcceptNoUpdate { feature: 0 }
        );
    }

    #[test]
    fn parse_clear_flag() {
        let mut parser = FlagDiacriticParser::new();
        let ofv = parser.parse("@C.NUM@").unwrap();
        assert_eq!(ofv.op, FlagOp::C);
        // "NUM" is the feature, "@" (no value after dot) maps to FLAG_VALUE_ANY.
        assert_eq!(ofv.value, FLAG_VALUE_ANY);
    }
}
