//! Do not disturb: when a user's notifications are held back.
//!
//! A user can snooze notifications until a set time, and can keep a weekly
//! schedule of quiet hours in their own time zone. Either one being in effect
//! makes them quiet.

use chrono::{DateTime, Datelike, Duration, NaiveDate, NaiveTime, TimeZone, Utc, Weekday};
use chrono_tz::Tz;

/// Days of the week, in the order the API lists them.
pub const DAY_NAMES: [&str; 7] = ["mon", "tue", "wed", "thu", "fri", "sat", "sun"];

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum DndError {
    #[error("times must be HH:MM")]
    InvalidTime,
    #[error("quiet hours must start and end at different times")]
    EmptyWindow,
    #[error("unknown day {0:?}; use mon, tue, wed, thu, fri, sat, sun")]
    UnknownDay(String),
    #[error("quiet hours need at least one day")]
    NoDays,
    #[error("unknown time zone {0:?}")]
    UnknownTimeZone(String),
    #[error("snooze must end in the future")]
    SnoozeInPast,
}

/// Weekly quiet hours. A window whose end is not after its start runs past
/// midnight, and belongs to the day it starts on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Schedule {
    pub start: NaiveTime,
    pub end: NaiveTime,
    /// Bit `n` set: windows start on the `n`th day from Monday.
    pub days: u8,
    pub time_zone: Tz,
}

impl Schedule {
    pub fn parse(
        start: &str,
        end: &str,
        days: &[String],
        time_zone: &str,
    ) -> Result<Self, DndError> {
        let start = parse_time(start)?;
        let end = parse_time(end)?;
        if start == end {
            return Err(DndError::EmptyWindow);
        }
        let mut mask = 0u8;
        for day in days {
            let index = DAY_NAMES
                .iter()
                .position(|d| d.eq_ignore_ascii_case(day))
                .ok_or_else(|| DndError::UnknownDay(day.clone()))?;
            mask |= 1 << index;
        }
        if mask == 0 {
            return Err(DndError::NoDays);
        }
        let time_zone = time_zone
            .parse::<Tz>()
            .map_err(|_| DndError::UnknownTimeZone(time_zone.to_string()))?;
        Ok(Self {
            start,
            end,
            days: mask,
            time_zone,
        })
    }

    pub fn day_names(&self) -> Vec<&'static str> {
        DAY_NAMES
            .iter()
            .enumerate()
            .filter(|(i, _)| self.days & (1 << i) != 0)
            .map(|(_, d)| *d)
            .collect()
    }

    fn starts_on(&self, day: Weekday) -> bool {
        self.days & (1 << day.num_days_from_monday()) != 0
    }

    /// If `now` is inside a quiet window, when that window ends.
    pub fn quiet_until(&self, now: DateTime<Utc>) -> Option<DateTime<Utc>> {
        let today = now.with_timezone(&self.time_zone).date_naive();
        // A window that began yesterday may still be running.
        [today.pred_opt()?, today].into_iter().find_map(|day| {
            if !self.starts_on(day.weekday()) {
                return None;
            }
            let from = self.instant(day, self.start);
            let end_day = if self.end > self.start {
                day
            } else {
                day.succ_opt()?
            };
            let to = self.instant(end_day, self.end);
            (from <= now && now < to).then_some(to)
        })
    }

    /// A local wall-clock time as an instant. A time skipped by a clock change
    /// is read an hour later; a repeated one is its first occurrence.
    fn instant(&self, day: NaiveDate, time: NaiveTime) -> DateTime<Utc> {
        let local = day.and_time(time);
        self.time_zone
            .from_local_datetime(&local)
            .earliest()
            .or_else(|| {
                self.time_zone
                    .from_local_datetime(&(local + Duration::hours(1)))
                    .earliest()
            })
            .map(|t| t.with_timezone(&Utc))
            .unwrap_or_else(|| local.and_utc())
    }
}

fn parse_time(value: &str) -> Result<NaiveTime, DndError> {
    NaiveTime::parse_from_str(value, "%H:%M").map_err(|_| DndError::InvalidTime)
}

/// A user's do-not-disturb settings.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct DoNotDisturb {
    pub snooze_until: Option<DateTime<Utc>>,
    pub schedule: Option<Schedule>,
}

impl DoNotDisturb {
    /// If the user is quiet at `now`, when that ends. When a snooze and a
    /// scheduled window overlap, the later end.
    pub fn quiet_until(&self, now: DateTime<Utc>) -> Option<DateTime<Utc>> {
        let snoozed = self.snooze_until.filter(|until| *until > now);
        let scheduled = self.schedule.and_then(|s| s.quiet_until(now));
        snoozed.max(scheduled)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(s: &str) -> DateTime<Utc> {
        s.parse().unwrap()
    }

    fn days(names: &[&str]) -> Vec<String> {
        names.iter().map(|d| d.to_string()).collect()
    }

    fn weeknights_paris() -> Schedule {
        Schedule::parse(
            "22:00",
            "07:00",
            &days(&["mon", "tue", "wed", "thu", "fri"]),
            "Europe/Paris",
        )
        .unwrap()
    }

    #[test]
    fn parses_and_names_days_in_order() {
        let s = Schedule::parse("09:00", "17:30", &days(&["SUN", "mon"]), "UTC").unwrap();
        assert_eq!(s.day_names(), vec!["mon", "sun"]);
        assert_eq!(s.end, NaiveTime::from_hms_opt(17, 30, 0).unwrap());
    }

    #[test]
    fn refuses_malformed_schedules() {
        let all = days(&["mon"]);
        assert_eq!(
            Schedule::parse("25:00", "07:00", &all, "UTC"),
            Err(DndError::InvalidTime)
        );
        assert_eq!(
            Schedule::parse("7am", "07:00", &all, "UTC"),
            Err(DndError::InvalidTime)
        );
        assert_eq!(
            Schedule::parse("07:00", "07:00", &all, "UTC"),
            Err(DndError::EmptyWindow)
        );
        assert_eq!(
            Schedule::parse("22:00", "07:00", &[], "UTC"),
            Err(DndError::NoDays)
        );
        assert_eq!(
            Schedule::parse("22:00", "07:00", &days(&["someday"]), "UTC"),
            Err(DndError::UnknownDay("someday".into()))
        );
        assert_eq!(
            Schedule::parse("22:00", "07:00", &all, "Mars/Olympus"),
            Err(DndError::UnknownTimeZone("Mars/Olympus".into()))
        );
    }

    #[test]
    fn a_daytime_window_is_quiet_between_its_bounds_only() {
        let s = Schedule::parse("12:00", "14:00", &days(&["thu"]), "UTC").unwrap();
        // Thursday 24 September 2026.
        assert_eq!(s.quiet_until(at("2026-09-24T11:59:59Z")), None);
        assert_eq!(
            s.quiet_until(at("2026-09-24T12:00:00Z")),
            Some(at("2026-09-24T14:00:00Z"))
        );
        assert_eq!(s.quiet_until(at("2026-09-24T14:00:00Z")), None);
        assert_eq!(
            s.quiet_until(at("2026-09-25T13:00:00Z")),
            None,
            "friday is not listed"
        );
    }

    #[test]
    fn an_overnight_window_runs_into_the_next_day() {
        let s = weeknights_paris();
        // Friday 25 September, 23:00 in Paris (UTC+2): quiet until Saturday 07:00.
        assert_eq!(
            s.quiet_until(at("2026-09-25T21:00:00Z")),
            Some(at("2026-09-26T05:00:00Z"))
        );
        // Saturday 06:00 in Paris: still Friday night's window.
        assert_eq!(
            s.quiet_until(at("2026-09-26T04:00:00Z")),
            Some(at("2026-09-26T05:00:00Z"))
        );
        // Saturday 23:00: no window starts on Saturday.
        assert_eq!(s.quiet_until(at("2026-09-26T21:00:00Z")), None);
        // Monday 06:00: Sunday night has no window.
        assert_eq!(s.quiet_until(at("2026-09-28T04:00:00Z")), None);
    }

    #[test]
    fn the_time_zone_decides_the_hours() {
        let paris = weeknights_paris();
        let tokyo = Schedule {
            time_zone: "Asia/Tokyo".parse().unwrap(),
            ..paris
        };
        // 21:00 UTC on a Wednesday: 23:00 in Paris, 06:00 Thursday in Tokyo.
        let now = at("2026-09-23T21:00:00Z");
        assert_eq!(paris.quiet_until(now), Some(at("2026-09-24T05:00:00Z")));
        assert_eq!(tokyo.quiet_until(now), Some(at("2026-09-23T22:00:00Z")));
    }

    #[test]
    fn clock_changes_keep_local_hours() {
        let s = Schedule::parse("22:00", "07:00", &days(&["sat"]), "Europe/Paris").unwrap();
        // Clocks go back on Sunday 25 October 2026: 07:00 local is 06:00 UTC.
        assert_eq!(
            s.quiet_until(at("2026-10-24T21:00:00Z")),
            Some(at("2026-10-25T06:00:00Z"))
        );
        // Clocks go forward on Sunday 29 March 2026, skipping 02:00 to 03:00.
        // A window starting at the skipped 02:30 begins at 03:30 local.
        let gap = Schedule::parse("02:30", "05:00", &days(&["sun"]), "Europe/Paris").unwrap();
        assert_eq!(gap.quiet_until(at("2026-03-29T00:45:00Z")), None);
        assert_eq!(
            gap.quiet_until(at("2026-03-29T01:30:00Z")),
            Some(at("2026-03-29T03:00:00Z"))
        );
    }

    #[test]
    fn a_snooze_is_quiet_until_it_ends() {
        let dnd = DoNotDisturb {
            snooze_until: Some(at("2026-09-24T13:00:00Z")),
            schedule: None,
        };
        assert_eq!(
            dnd.quiet_until(at("2026-09-24T12:00:00Z")),
            Some(at("2026-09-24T13:00:00Z"))
        );
        assert_eq!(dnd.quiet_until(at("2026-09-24T13:00:00Z")), None);
        assert_eq!(
            DoNotDisturb::default().quiet_until(at("2026-09-24T12:00:00Z")),
            None
        );
    }

    #[test]
    fn overlapping_snooze_and_schedule_end_at_the_later() {
        let schedule = Some(Schedule::parse("12:00", "14:00", &days(&["thu"]), "UTC").unwrap());
        let now = at("2026-09-24T12:30:00Z");
        let short = DoNotDisturb {
            snooze_until: Some(at("2026-09-24T13:00:00Z")),
            schedule,
        };
        assert_eq!(short.quiet_until(now), Some(at("2026-09-24T14:00:00Z")));
        let long = DoNotDisturb {
            snooze_until: Some(at("2026-09-24T18:00:00Z")),
            schedule,
        };
        assert_eq!(long.quiet_until(now), Some(at("2026-09-24T18:00:00Z")));
        // An expired snooze does not count.
        let expired = DoNotDisturb {
            snooze_until: Some(at("2026-09-24T01:00:00Z")),
            schedule,
        };
        assert_eq!(expired.quiet_until(now), Some(at("2026-09-24T14:00:00Z")));
    }
}
