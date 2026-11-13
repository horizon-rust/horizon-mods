/*
 * Copyright 2023 - 2026 dima_dencep.
 *
 * Licensed under the Open Software License, Version 3.0 (the "License");
 * you may not use this file except in compliance with the License.
 *
 * You may obtain a copy of the License at
 *     https://spdx.org/licenses/OSL-3.0.txt
 */

//! 1:1 translation of `AprilFool.java`. `Calendar` month/day arithmetic is
//! spelled out: Calendar months are 0-based, so April is 3.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AprilFool {
    OnApril,
    Always,
    Disabled,
}

impl AprilFool {
    /// Upstream caches the first `Calendar.getInstance()` for the process;
    /// here the moment is captured the same way and passed in, so a test can
    /// pin the date without touching global state.
    pub fn can_ues(&self, month0: i32, day_of_month: i32) -> bool {
        match self {
            AprilFool::OnApril => month0 == 3 && day_of_month == 1,
            AprilFool::Always => true,
            AprilFool::Disabled => false,
        }
    }
}

/// Civil-from-days, so `now` (seconds since epoch) maps to the same
/// (month0, day) Calendar would report.
pub fn calendar_month_day(epoch_secs: i64) -> (i32, i32) {
    let days = epoch_secs.div_euclid(86_400);
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let _y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let m0 = if mp < 10 { mp + 2 } else { mp - 10 };
    (m0 as i32, (doy - (153 * mp + 2) / 5 + 1) as i32)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn on_april_fires_only_april_first() {
        let f = AprilFool::OnApril;
        assert!(f.can_ues(3, 1), "Calendar.APRIL == 3");
        assert!(!f.can_ues(3, 2));
        assert!(!f.can_ues(2, 1));
        assert!(!f.can_ues(4, 1));
    }

    #[test]
    fn always_and_disabled() {
        assert!(AprilFool::Always.can_ues(0, 1));
        assert!(!AprilFool::Disabled.can_ues(3, 1));
    }

    #[test]
    fn calendar_math_matches_known_dates() {
        // 2026-04-01 12:00:00 UTC = 1775044800
        assert_eq!(calendar_month_day(1_775_044_800), (3, 1));
        // 2026-01-01 12:00:00 UTC = 1767268800
        assert_eq!(calendar_month_day(1_767_268_800), (0, 1));
        // 2026-12-31 12:00:00 UTC = 1798718400
        assert_eq!(calendar_month_day(1_798_718_400), (11, 31));
    }
}
