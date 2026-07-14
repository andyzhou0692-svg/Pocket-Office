//! Small authored character beats selected by the local visual behavior pack.

use std::time::SystemTime;

const LIQUOR_ANALYST_NAME: &str = "Alex";
const LIQUOR_CYCLE_MS: u64 = 60_000;
const LOOK_LEFT_START_MS: u64 = 50_000;
const LOOK_RIGHT_START_MS: u64 = 51_200;
const SWIG_START_MS: u64 = 52_400;
const SWIG_END_MS: u64 = 55_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) enum CharacterHabit {
    #[default]
    None,
    LookLeft,
    LookRight,
    Swig,
}

pub(crate) fn phase_for_label(
    label: &str,
    two_hundred_west: bool,
    now: SystemTime,
) -> CharacterHabit {
    if !two_hundred_west || label != LIQUOR_ANALYST_NAME {
        return CharacterHabit::None;
    }
    let phase_ms = crate::anim::elapsed_ms(now, SystemTime::UNIX_EPOCH) % LIQUOR_CYCLE_MS;
    match phase_ms {
        LOOK_LEFT_START_MS..LOOK_RIGHT_START_MS => CharacterHabit::LookLeft,
        LOOK_RIGHT_START_MS..SWIG_START_MS => CharacterHabit::LookRight,
        SWIG_START_MS..SWIG_END_MS => CharacterHabit::Swig,
        _ => CharacterHabit::None,
    }
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
}
