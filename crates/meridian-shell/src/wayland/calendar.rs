#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[allow(dead_code)]
pub(crate) enum WeekStart {
    Monday,
    Sunday,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) struct CalendarDisplayPolicy {
    pub(crate) week_start: WeekStart,
}

impl Default for CalendarDisplayPolicy {
    fn default() -> Self {
        Self {
            week_start: WeekStart::Monday,
        }
    }
}

pub(crate) fn weekday_labels(week_start: WeekStart) -> [&'static str; 7] {
    match week_start {
        WeekStart::Monday => ["Mo", "Tu", "We", "Th", "Fr", "Sa", "Su"],
        WeekStart::Sunday => ["Su", "Mo", "Tu", "We", "Th", "Fr", "Sa"],
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CalendarMonthModel {
    pub(crate) year: i32,
    pub(crate) month: u8,
    pub(crate) today_day: Option<u8>,
    pub(crate) cells: [Option<u8>; 42],
    pub(crate) first_weekday_col0: u8,
}

impl CalendarMonthModel {
    pub(crate) fn for_month(
        year: i32,
        month: u8,
        today_day: Option<u8>,
        week_start: WeekStart,
    ) -> Option<Self> {
        let day_count = days_in_month(year, month)?;
        let first_weekday_sunday0 = weekday_sunday0(year, month, 1)?;
        let first_weekday_col0 = weekday_col0_from_sunday0(first_weekday_sunday0, week_start);

        let mut cells = [None; 42];
        let first_cell = first_weekday_col0 as usize;
        for day in 1..=day_count {
            let idx = first_cell + day as usize - 1;
            if idx >= cells.len() {
                return None;
            }
            cells[idx] = Some(day);
        }

        let today_day = today_day.filter(|day| *day > 0 && *day <= day_count);

        Some(Self {
            year,
            month,
            today_day,
            cells,
            first_weekday_col0,
        })
    }
}

fn days_in_month(year: i32, month: u8) -> Option<u8> {
    match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => Some(31),
        4 | 6 | 9 | 11 => Some(30),
        2 => Some(if is_leap_year(year) { 29 } else { 28 }),
        _ => None,
    }
}

fn is_leap_year(year: i32) -> bool {
    (year % 4 == 0 && year % 100 != 0) || year % 400 == 0
}

fn weekday_col0_from_sunday0(sunday0: u8, week_start: WeekStart) -> u8 {
    match week_start {
        WeekStart::Monday => (sunday0 + 6) % 7,
        WeekStart::Sunday => sunday0,
    }
}

fn weekday_sunday0(year: i32, month: u8, day: u8) -> Option<u8> {
    if day == 0 {
        return None;
    }

    let day_count = days_in_month(year, month)?;
    if day > day_count {
        return None;
    }

    // Sakamoto weekday with Sunday=0.
    const MONTH_OFFSETS: [i32; 12] = [0, 3, 2, 5, 0, 3, 5, 1, 4, 6, 2, 4];
    let mut y = year;
    if month < 3 {
        y -= 1;
    }

    let month_idx = usize::from(month - 1);
    let sunday0 =
        (y + y / 4 - y / 100 + y / 400 + MONTH_OFFSETS[month_idx] + i32::from(day)).rem_euclid(7);
    Some(sunday0 as u8)
}

#[cfg(test)]
mod tests {
    use super::{days_in_month, weekday_labels, CalendarMonthModel, WeekStart};

    #[test]
    fn leap_year_february_has_29_days() {
        assert_eq!(days_in_month(2024, 2), Some(29));
    }

    #[test]
    fn normal_february_has_28_days() {
        assert_eq!(days_in_month(2025, 2), Some(28));
    }

    #[test]
    fn thirty_day_month_is_reported() {
        assert_eq!(days_in_month(2026, 4), Some(30));
    }

    #[test]
    fn thirty_one_day_month_is_reported() {
        assert_eq!(days_in_month(2026, 3), Some(31));
    }

    #[test]
    fn first_weekday_uses_monday_zero_mapping_when_monday_is_selected() {
        let feb_2024 = CalendarMonthModel::for_month(2024, 2, None, WeekStart::Monday)
            .expect("valid month model");
        let feb_2025 = CalendarMonthModel::for_month(2025, 2, None, WeekStart::Monday)
            .expect("valid month model");
        let mar_2026 = CalendarMonthModel::for_month(2026, 3, None, WeekStart::Monday)
            .expect("valid month model");

        assert_eq!(feb_2024.first_weekday_col0, 3);
        assert_eq!(feb_2025.first_weekday_col0, 5);
        assert_eq!(mar_2026.first_weekday_col0, 6);
    }

    #[test]
    fn first_weekday_uses_sunday_zero_mapping_when_sunday_is_selected() {
        let feb_2024 = CalendarMonthModel::for_month(2024, 2, None, WeekStart::Sunday)
            .expect("valid month model");
        let feb_2025 = CalendarMonthModel::for_month(2025, 2, None, WeekStart::Sunday)
            .expect("valid month model");
        let mar_2026 = CalendarMonthModel::for_month(2026, 3, None, WeekStart::Sunday)
            .expect("valid month model");

        assert_eq!(feb_2024.first_weekday_col0, 4);
        assert_eq!(feb_2025.first_weekday_col0, 6);
        assert_eq!(mar_2026.first_weekday_col0, 0);
    }

    #[test]
    fn cells_place_days_in_expected_positions_for_monday_start() {
        let model = CalendarMonthModel::for_month(2025, 2, None, WeekStart::Monday)
            .expect("valid month model");

        assert_eq!(model.cells[4], None);
        assert_eq!(model.cells[5], Some(1));
        assert_eq!(model.cells[32], Some(28));
        assert_eq!(model.cells[33], None);
        assert_eq!(model.cells.iter().flatten().count(), 28);
    }

    #[test]
    fn cells_place_days_in_expected_positions_for_sunday_start() {
        let model = CalendarMonthModel::for_month(2025, 2, None, WeekStart::Sunday)
            .expect("valid month model");

        assert_eq!(model.cells[5], None);
        assert_eq!(model.cells[6], Some(1));
        assert_eq!(model.cells[33], Some(28));
        assert_eq!(model.cells[34], None);
        assert_eq!(model.cells.iter().flatten().count(), 28);
    }

    #[test]
    fn today_day_and_day_count_are_unchanged_across_week_start_modes() {
        let monday = CalendarMonthModel::for_month(2025, 2, Some(14), WeekStart::Monday)
            .expect("valid monday model");
        let sunday = CalendarMonthModel::for_month(2025, 2, Some(14), WeekStart::Sunday)
            .expect("valid sunday model");

        assert_eq!(monday.today_day, Some(14));
        assert_eq!(sunday.today_day, Some(14));
        assert_eq!(monday.cells.iter().flatten().count(), 28);
        assert_eq!(sunday.cells.iter().flatten().count(), 28);
    }

    #[test]
    fn today_day_is_kept_only_when_in_month_range() {
        let valid = CalendarMonthModel::for_month(2025, 2, Some(14), WeekStart::Monday)
            .expect("valid month model");
        let out_of_range = CalendarMonthModel::for_month(2025, 2, Some(29), WeekStart::Monday)
            .expect("valid month model");
        let zero = CalendarMonthModel::for_month(2025, 2, Some(0), WeekStart::Monday)
            .expect("valid month model");

        assert_eq!(valid.today_day, Some(14));
        assert_eq!(out_of_range.today_day, None);
        assert_eq!(zero.today_day, None);
    }

    #[test]
    fn invalid_month_is_rejected() {
        assert!(CalendarMonthModel::for_month(2025, 0, None, WeekStart::Monday).is_none());
        assert!(CalendarMonthModel::for_month(2025, 13, None, WeekStart::Monday).is_none());
    }

    #[test]
    fn weekday_labels_follow_selected_week_start() {
        assert_eq!(
            weekday_labels(WeekStart::Monday),
            ["Mo", "Tu", "We", "Th", "Fr", "Sa", "Su"]
        );
        assert_eq!(
            weekday_labels(WeekStart::Sunday),
            ["Su", "Mo", "Tu", "We", "Th", "Fr", "Sa"]
        );
    }
}
