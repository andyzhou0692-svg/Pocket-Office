//! Small authored character beats selected by the local visual behavior pack.

use std::time::SystemTime;

const LIQUOR_ANALYST_NAME: &str = "Alex";
const LIQUOR_CYCLE_MS: u64 = 60_000;
const LOOK_LEFT_START_MS: u64 = 50_000;
const LOOK_RIGHT_START_MS: u64 = 51_200;
const SWIG_START_MS: u64 = 52_400;
const SWIG_END_MS: u64 = 55_000;
const ALISON_NAME: &str = "Alison";
const ALISON_LOOK_LEFT_START_MS: u64 = 49_000;
const ALISON_LOOK_RIGHT_START_MS: u64 = 50_000;
const ALISON_VAPE_RAISE_START_MS: u64 = 51_000;
const ALISON_VAPE_EXHALE_START_MS: u64 = 52_000;
const ALISON_VAPE_EXHALE_END_MS: u64 = 54_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) enum CharacterHabit {
    #[default]
    None,
    LookLeft,
    LookRight,
    Swig,
    VapeRaise,
    VapeExhale,
}

pub(crate) fn phase_for_label(
    label: &str,
    two_hundred_west: bool,
    now: SystemTime,
) -> CharacterHabit {
    let phase_ms = crate::anim::elapsed_ms(now, SystemTime::UNIX_EPOCH) % LIQUOR_CYCLE_MS;
    if label == ALISON_NAME {
        return match phase_ms {
            ALISON_LOOK_LEFT_START_MS..ALISON_LOOK_RIGHT_START_MS => CharacterHabit::LookLeft,
            ALISON_LOOK_RIGHT_START_MS..ALISON_VAPE_RAISE_START_MS => CharacterHabit::LookRight,
            ALISON_VAPE_RAISE_START_MS..ALISON_VAPE_EXHALE_START_MS => CharacterHabit::VapeRaise,
            ALISON_VAPE_EXHALE_START_MS..ALISON_VAPE_EXHALE_END_MS => CharacterHabit::VapeExhale,
            _ => CharacterHabit::None,
        };
    }
    if !two_hundred_west || label != LIQUOR_ANALYST_NAME {
        return CharacterHabit::None;
    }
    match phase_ms {
        LOOK_LEFT_START_MS..LOOK_RIGHT_START_MS => CharacterHabit::LookLeft,
        LOOK_RIGHT_START_MS..SWIG_START_MS => CharacterHabit::LookRight,
        SWIG_START_MS..SWIG_END_MS => CharacterHabit::Swig,
        _ => CharacterHabit::None,
    }
}

pub(crate) fn vape_exhale_elapsed_ms(now: SystemTime) -> Option<u64> {
    let phase_ms = crate::anim::elapsed_ms(now, SystemTime::UNIX_EPOCH) % LIQUOR_CYCLE_MS;
    (ALISON_VAPE_EXHALE_START_MS..ALISON_VAPE_EXHALE_END_MS)
        .contains(&phase_ms)
        .then_some(phase_ms - ALISON_VAPE_EXHALE_START_MS)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{Duration, SystemTime};

    fn at(offset_ms: u64) -> SystemTime {
        SystemTime::UNIX_EPOCH + Duration::from_millis(offset_ms)
    }

    #[test]
    fn alex_liquor_habit_runs_normal_look_left_look_right_then_swig() {
        assert_eq!(
            phase_for_label("Alex", true, at(49_999)),
            CharacterHabit::None
        );
        assert_eq!(
            phase_for_label("Alex", true, at(50_000)),
            CharacterHabit::LookLeft
        );
        assert_eq!(
            phase_for_label("Alex", true, at(51_200)),
            CharacterHabit::LookRight
        );
        assert_eq!(
            phase_for_label("Alex", true, at(52_400)),
            CharacterHabit::Swig
        );
        assert_eq!(
            phase_for_label("Alex", true, at(55_000)),
            CharacterHabit::None
        );
    }

    #[test]
    fn liquor_habit_is_limited_to_alex_and_two_hundred_west() {
        assert_eq!(
            phase_for_label("Tristan Pembroke", true, at(52_400)),
            CharacterHabit::None
        );
        assert_eq!(
            phase_for_label("Alex", false, at(52_400)),
            CharacterHabit::None
        );
    }

    #[test]
    fn alison_vape_habit_runs_in_order_and_exhale_lasts_exactly_two_seconds() {
        assert_eq!(
            phase_for_label("Alison", false, at(48_999)),
            CharacterHabit::None
        );
        assert_eq!(
            phase_for_label("Alison", false, at(49_000)),
            CharacterHabit::LookLeft
        );
        assert_eq!(
            phase_for_label("Alison", false, at(50_000)),
            CharacterHabit::LookRight
        );
        assert_eq!(
            phase_for_label("Alison", false, at(51_000)),
            CharacterHabit::VapeRaise
        );
        assert_eq!(
            phase_for_label("Alison", false, at(52_000)),
            CharacterHabit::VapeExhale
        );
        assert_eq!(vape_exhale_elapsed_ms(at(52_000)), Some(0));
        assert_eq!(vape_exhale_elapsed_ms(at(53_999)), Some(1_999));
        assert_eq!(
            phase_for_label("Alison", false, at(54_000)),
            CharacterHabit::None
        );
        assert_eq!(vape_exhale_elapsed_ms(at(54_000)), None);
    }

    #[test]
    fn alison_vape_habit_is_label_specific_and_theme_independent() {
        assert_eq!(
            phase_for_label("Alison", true, at(52_000)),
            CharacterHabit::VapeExhale
        );
        assert_eq!(
            phase_for_label("Amy", true, at(52_000)),
            CharacterHabit::None
        );
        assert_eq!(
            phase_for_label("alison", false, at(52_000)),
            CharacterHabit::None
        );
    }
}
